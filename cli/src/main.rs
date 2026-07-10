//! sigma-updates-cli — list, check, and publish Debian packages.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use sigma_updates_client::{
    UpdatesClient, check_packages, collect_deb_paths, push_packages,
};
use sigma_updates_deb::VersionConstraint;

#[derive(Parser, Debug)]
#[command(name = "sigma-updates-cli")]
#[command(about = "Publish and inspect packages on sigma-updates")]
struct Cli {
    /// Base URL of the updates service (e.g. http://updates.sigma.localtest.me:30080)
    #[arg(long, env = "SIGMA_UPDATES_URL", global = true)]
    url: Option<String>,

    /// Shared secret for publish/delete (`SIGMA_INTERNAL_TOKEN`)
    #[arg(long, env = "SIGMA_INTERNAL_TOKEN", global = true)]
    token: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
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

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    let url = cli
        .url
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:8080".into());
    let mut client = UpdatesClient::new(url);
    if let Some(token) = cli.token.filter(|s| !s.trim().is_empty()) {
        client = client.with_token(token);
    }

    match cli.command {
        Command::List => {
            let packages = client.list_packages().map_err(|e| e.to_string())?;
            if packages.is_empty() {
                println!("(no packages)");
                return Ok(());
            }
            for pkg in packages {
                let deps = if pkg.depends.is_empty() && pkg.pre_depends.is_empty() {
                    String::new()
                } else {
                    let all: Vec<_> = pkg
                        .pre_depends
                        .iter()
                        .chain(pkg.depends.iter())
                        .map(format_dep)
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
        Command::Check { paths } => {
            let paths = collect_deb_paths(&paths).map_err(|e| e.to_string())?;
            let plan = check_packages(&client, &paths).map_err(|e| e.to_string())?;
            println!("order ({}):", plan.order.len());
            for pkg in &plan.order {
                println!(
                    "  {} {} ({})",
                    pkg.control.package, pkg.control.version, pkg.filename
                );
            }
            if plan.missing.is_empty() {
                println!("dependencies: ok");
                Ok(())
            } else {
                println!("dependencies: MISSING");
                for m in &plan.missing {
                    let deps = m.missing.iter().map(format_dep).collect::<Vec<_>>().join(", ");
                    println!("  {} needs: {deps}", m.package);
                }
                Err("one or more packages have unsatisfied dependencies".into())
            }
        }
        Command::Push {
            paths,
            allow_missing_deps,
        } => {
            let paths = collect_deb_paths(&paths).map_err(|e| e.to_string())?;
            let report = push_packages(&client, &paths, allow_missing_deps)
                .map_err(|e| e.to_string())?;
            for pkg in &report.published {
                println!("published {} ({} {})", pkg.filename, pkg.name, pkg.version);
            }
            println!("ok — {} package(s)", report.published.len());
            Ok(())
        }
        Command::Delete { filename } => {
            client
                .delete_package(&filename)
                .map_err(|e| e.to_string())?;
            println!("deleted {filename}");
            Ok(())
        }
    }
}

fn format_dep(expr: &sigma_updates_deb::DependencyExpr) -> String {
    expr.alternatives
        .iter()
        .map(|alt| {
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

fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let b = bytes as f64;
    if b >= MB {
        format!("{:.1} MiB", b / MB)
    } else if b >= KB {
        format!("{:.1} KiB", b / KB)
    } else {
        format!("{bytes} B")
    }
}
