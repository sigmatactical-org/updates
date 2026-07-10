//! Plan and execute package pushes with dependency checking and topo order.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use sigma_updates_deb::{
    DebControl, DependencyExpr, inspect_deb_file, satisfies,
};

use crate::{ClientError, DebPackage, UpdatesClient};

#[derive(Debug, Clone)]
pub struct LocalPackage {
    pub path: PathBuf,
    pub filename: String,
    pub control: DebControl,
}

#[derive(Debug, Clone)]
pub struct MissingDependency {
    pub package: String,
    pub missing: Vec<DependencyExpr>,
}

#[derive(Debug, Clone)]
pub struct PushPlan {
    /// Packages to publish, dependencies first.
    pub order: Vec<LocalPackage>,
    pub missing: Vec<MissingDependency>,
}

#[derive(Debug, Clone)]
pub struct PushReport {
    pub published: Vec<DebPackage>,
}

/// Inspect local `.deb` paths into structured packages.
pub fn load_local_packages(paths: &[PathBuf]) -> Result<Vec<LocalPackage>, ClientError> {
    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        let control = inspect_deb_file(path).map_err(|e| ClientError::Message(e.to_string()))?;
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| ClientError::Message(format!("invalid path {}", path.display())))?
            .to_owned();
        out.push(LocalPackage {
            path: path.clone(),
            filename,
            control,
        });
    }
    Ok(out)
}

/// Build a publish plan: topo-sort locals and report unsatisfied dependencies
/// against the remote index (+ other locals in the batch).
pub fn plan_push(
    remote: &[DebPackage],
    locals: &[LocalPackage],
) -> Result<PushPlan, ClientError> {
    let order = topo_sort(locals)?;
    let mut available: HashMap<String, String> = HashMap::new();
    for pkg in remote {
        available.insert(pkg.name.clone(), pkg.version.clone());
        for provided in &pkg.provides {
            available
                .entry(provided.name.clone())
                .or_insert_with(|| pkg.version.clone());
        }
    }

    let mut missing = Vec::new();
    for local in &order {
        let deps: Vec<_> = local.control.all_depends().cloned().collect();
        let avail_iter = available
            .iter()
            .map(|(n, v)| (n.as_str(), v.as_str()));
        if let Err(unsat) = satisfies(&deps, avail_iter) {
            missing.push(MissingDependency {
                package: local.control.package.clone(),
                missing: unsat,
            });
        }
        // After checking, treat this local as available for later packages.
        available.insert(
            local.control.package.clone(),
            local.control.version.clone(),
        );
        for provided in &local.control.provides {
            available
                .entry(provided.name.clone())
                .or_insert_with(|| local.control.version.clone());
        }
    }

    Ok(PushPlan { order, missing })
}

/// Publish packages in dependency order. Refuses if any dependency is missing
/// unless `allow_missing` is true.
pub fn push_packages(
    client: &UpdatesClient,
    paths: &[PathBuf],
    allow_missing: bool,
) -> Result<PushReport, ClientError> {
    let locals = load_local_packages(paths)?;
    let remote = client.list_packages()?;
    let plan = plan_push(&remote, &locals)?;

    if !plan.missing.is_empty() && !allow_missing {
        let detail = plan
            .missing
            .iter()
            .map(|m| {
                let deps = m
                    .missing
                    .iter()
                    .map(format_dep_expr)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{} needs: {deps}", m.package)
            })
            .collect::<Vec<_>>()
            .join("; ");
        return Err(ClientError::Message(format!(
            "refusing publish — missing dependencies: {detail}"
        )));
    }

    let mut published = Vec::new();
    for local in &plan.order {
        let pkg = client.publish_file(&local.path)?;
        published.push(pkg);
    }
    Ok(PushReport { published })
}

/// Check local packages against the remote index without publishing.
pub fn check_packages(
    client: &UpdatesClient,
    paths: &[PathBuf],
) -> Result<PushPlan, ClientError> {
    let locals = load_local_packages(paths)?;
    let remote = client.list_packages()?;
    plan_push(&remote, &locals)
}

fn topo_sort(locals: &[LocalPackage]) -> Result<Vec<LocalPackage>, ClientError> {
    let names: HashSet<String> = locals
        .iter()
        .map(|p| p.control.package.clone())
        .collect();

    // Edge: dep -> package (dep must come first) only for deps satisfied by other locals.
    let mut indegree: HashMap<String, usize> = locals
        .iter()
        .map(|p| (p.control.package.clone(), 0usize))
        .collect();
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();

    for pkg in locals {
        for expr in pkg.control.all_depends() {
            for alt in &expr.alternatives {
                let dep_name = &alt.package.name;
                if names.contains(dep_name) && dep_name != &pkg.control.package {
                    adjacency
                        .entry(dep_name.clone())
                        .or_default()
                        .push(pkg.control.package.clone());
                    *indegree.entry(pkg.control.package.clone()).or_default() += 1;
                    break; // one edge per AND-clause is enough for ordering
                }
            }
        }
    }

    let mut queue: VecDeque<String> = indegree
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(n, _)| n.clone())
        .collect();
    queue
        .make_contiguous()
        .sort(); // stable deterministic order

    let by_name: HashMap<_, _> = locals
        .iter()
        .map(|p| (p.control.package.clone(), p.clone()))
        .collect();

    let mut ordered = Vec::new();
    while let Some(name) = queue.pop_front() {
        let pkg = by_name
            .get(&name)
            .ok_or_else(|| ClientError::Message(format!("internal: missing {name}")))?;
        ordered.push(pkg.clone());
        if let Some(children) = adjacency.get(&name) {
            for child in children {
                if let Some(d) = indegree.get_mut(child) {
                    *d = d.saturating_sub(1);
                    if *d == 0 {
                        queue.push_back(child.clone());
                    }
                }
            }
        }
    }

    if ordered.len() != locals.len() {
        return Err(ClientError::Message(
            "dependency cycle detected among local packages".into(),
        ));
    }
    Ok(ordered)
}

fn format_dep_expr(expr: &DependencyExpr) -> String {
    expr.alternatives
        .iter()
        .map(|alt| {
            use sigma_updates_deb::VersionConstraint;
            let c = match &alt.constraint {
                VersionConstraint::Any => String::new(),
                VersionConstraint::Eq(v) => format!(" (= {v})"),
                VersionConstraint::Ne(v) => format!(" (!= {v})"),
                VersionConstraint::Gt(v) => format!(" (>> {v})"),
                VersionConstraint::Ge(v) => format!(" (>= {v})"),
                VersionConstraint::Lt(v) => format!(" (<< {v})"),
                VersionConstraint::Le(v) => format!(" (<= {v})"),
            };
            format!("{}{c}", alt.package.name)
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Collect `.deb` paths from files and directories (directories are scanned recursively).
pub fn collect_deb_paths(inputs: &[PathBuf]) -> Result<Vec<PathBuf>, ClientError> {
    let mut out = Vec::new();
    for input in inputs {
        if input.is_dir() {
            collect_deb_paths_recursive(input, &mut out)?;
        } else if input.is_file() {
            out.push(input.clone());
        } else {
            return Err(ClientError::Message(format!(
                "path not found: {}",
                input.display()
            )));
        }
    }
    out.sort();
    out.dedup();
    if out.is_empty() {
        return Err(ClientError::Message("no .deb files found".into()));
    }
    Ok(out)
}

fn collect_deb_paths_recursive(dir: &std::path::Path, out: &mut Vec<PathBuf>) -> Result<(), ClientError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_deb_paths_recursive(&path, out)?;
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("deb"))
        {
            out.push(path);
        }
    }
    Ok(())
}
