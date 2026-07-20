//! [`Command`].

use std::path::PathBuf;

use clap::Subcommand;
use sigma_updates_client::{UpdatesClient, check_packages, collect_deb_paths, push_packages};
use sigma_updates_deb::human_size;

/// What the CLI was asked to do.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// List packages published on the server
    List,
    /// Check that local .deb dependencies are satisfied by the remote index
    Check {
        /// .deb files and/or directories containing .deb files
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
    /// Publish local .deb packages (topo-sorted; refuses if deps are missing)
    Push {
        /// .deb files and/or directories containing .deb files
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Publish even when dependencies are missing from the index
        #[arg(long)]
        allow_missing_deps: bool,
    },
    /// Delete a published package by filename
    Delete {
        /// Exact filename, e.g. foo_1.0.0-1_all.deb
        filename: String,
    },
}

impl Command {
    /// Run the command against `client`, printing its report.
    pub fn run(self, client: &UpdatesClient) -> Result<(), String> {
        match self {
            Self::List => list(client),
            Self::Check { paths } => check(client, &paths),
            Self::Push {
                paths,
                allow_missing_deps,
            } => push(client, &paths, allow_missing_deps),
            Self::Delete { filename } => {
                client
                    .delete_package(&filename)
                    .map_err(|e| e.to_string())?;
                println!("deleted {filename}");
                Ok(())
            }
        }
    }
}

fn list(client: &UpdatesClient) -> Result<(), String> {
    let packages = client.list_packages().map_err(|e| e.to_string())?;
    if packages.is_empty() {
        println!("(no packages)");
        return Ok(());
    }
    for pkg in packages {
        let deps = if pkg.depends.is_empty() && pkg.pre_depends.is_empty() {
            String::new()
        } else {
            let all: Vec<String> = pkg
                .pre_depends
                .iter()
                .chain(pkg.depends.iter())
                .map(ToString::to_string)
                .collect();
            format!("  depends: {}", all.join(", "))
        };
        println!(
            "{}_{}_{}.deb\t{}\t{}{}",
            pkg.name,
            pkg.version,
            pkg.architecture,
            pkg.filename,
            human_size(pkg.size_bytes),
            deps
        );
    }
    Ok(())
}

fn check(client: &UpdatesClient, paths: &[PathBuf]) -> Result<(), String> {
    let paths = collect_deb_paths(paths).map_err(|e| e.to_string())?;
    let plan = check_packages(client, &paths).map_err(|e| e.to_string())?;
    println!("order ({}):", plan.order.len());
    for pkg in &plan.order {
        println!(
            "  {} {} ({})",
            pkg.control.package, pkg.control.version, pkg.filename
        );
    }
    if plan.missing.is_empty() {
        println!("dependencies: ok");
        return Ok(());
    }
    println!("dependencies: MISSING");
    for m in &plan.missing {
        let deps = m
            .missing
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        println!("  {} needs: {deps}", m.package);
    }
    Err("one or more packages have unsatisfied dependencies".into())
}

fn push(client: &UpdatesClient, paths: &[PathBuf], allow_missing: bool) -> Result<(), String> {
    let paths = collect_deb_paths(paths).map_err(|e| e.to_string())?;
    let report = push_packages(client, &paths, allow_missing).map_err(|e| e.to_string())?;
    for pkg in &report.published {
        println!("published {} ({} {})", pkg.filename, pkg.name, pkg.version);
    }
    println!("ok — {} package(s)", report.published.len());
    Ok(())
}
