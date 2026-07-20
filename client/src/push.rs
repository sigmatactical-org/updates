//! Plan and execute package pushes with dependency checking and topo order.

mod local_package;
mod missing_dependency;
mod push_plan;
mod push_report;

pub use local_package::LocalPackage;
pub use missing_dependency::MissingDependency;
pub use push_plan::PushPlan;
pub use push_report::PushReport;

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use sigma_updates_deb::{DebPackage, inspect_deb_file, satisfies};

use crate::client_error::ClientError;
use crate::updates_client::UpdatesClient;

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
pub fn plan_push(remote: &[DebPackage], locals: &[LocalPackage]) -> Result<PushPlan, ClientError> {
    let order: Vec<LocalPackage> = topo_sort(locals)?
        .into_iter()
        .map(|i| locals[i].clone())
        .collect();

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
        let avail_iter = available.iter().map(|(n, v)| (n.as_str(), v.as_str()));
        if let Err(unsat) = satisfies(&deps, avail_iter) {
            missing.push(MissingDependency {
                package: local.control.package.clone(),
                missing: unsat,
            });
        }
        // After checking, treat this local as available for later packages.
        available.insert(local.control.package.clone(), local.control.version.clone());
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
                    .map(ToString::to_string)
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
pub fn check_packages(client: &UpdatesClient, paths: &[PathBuf]) -> Result<PushPlan, ClientError> {
    let locals = load_local_packages(paths)?;
    let remote = client.list_packages()?;
    plan_push(&remote, &locals)
}

/// Order `locals` so every package follows the batch-local packages it depends
/// on. Returns indices into `locals`; no package is cloned.
fn topo_sort(locals: &[LocalPackage]) -> Result<Vec<usize>, ClientError> {
    let index_of: HashMap<&str, usize> = locals
        .iter()
        .enumerate()
        .map(|(i, p)| (p.control.package.as_str(), i))
        .collect();

    // Edge: dep -> package (dep must come first) only for deps satisfied by other locals.
    let mut indegree = vec![0usize; locals.len()];
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); locals.len()];

    for (i, pkg) in locals.iter().enumerate() {
        for expr in pkg.control.all_depends() {
            for alt in &expr.alternatives {
                if let Some(&dep) = index_of.get(alt.package.name.as_str())
                    && dep != i
                {
                    adjacency[dep].push(i);
                    indegree[i] += 1;
                    break; // one edge per AND-clause is enough for ordering
                }
            }
        }
    }

    // Stable deterministic order: roots by package name.
    let mut roots: Vec<usize> = (0..locals.len()).filter(|&i| indegree[i] == 0).collect();
    roots.sort_by(|&a, &b| locals[a].control.package.cmp(&locals[b].control.package));
    let mut queue: VecDeque<usize> = roots.into();

    let mut ordered = Vec::with_capacity(locals.len());
    while let Some(i) = queue.pop_front() {
        ordered.push(i);
        for &child in &adjacency[i] {
            indegree[child] = indegree[child].saturating_sub(1);
            if indegree[child] == 0 {
                queue.push_back(child);
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

fn collect_deb_paths_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), ClientError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use sigma_updates_deb::DebControl;

    fn local(name: &str, depends: &str) -> LocalPackage {
        let control = DebControl::parse(&format!(
            "Package: {name}\nVersion: 1.0\nArchitecture: all\nDepends: {depends}\n"
        ))
        .expect("control parses");
        LocalPackage {
            path: PathBuf::from(format!("{name}.deb")),
            filename: format!("{name}.deb"),
            control,
        }
    }

    #[test]
    fn orders_dependencies_first() {
        let locals = vec![local("b", "a"), local("a", ""), local("c", "b")];
        let order = topo_sort(&locals).expect("acyclic");
        let names: Vec<&str> = order
            .iter()
            .map(|&i| locals[i].control.package.as_str())
            .collect();
        assert_eq!(names, ["a", "b", "c"]);
    }

    #[test]
    fn detects_cycles() {
        let locals = vec![local("a", "b"), local("b", "a")];
        assert!(topo_sort(&locals).is_err());
    }
}
