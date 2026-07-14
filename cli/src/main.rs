//! sigma-updates-cli — list, check, and publish Debian packages.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use sigma_updates_client::{
    UpdatesClient, check_packages, client_credentials_token, collect_deb_paths, push_packages,
    token_url_from_issuer,
};
use sigma_updates_deb::VersionConstraint;

#[derive(Parser, Debug)]
#[command(name = "sigma-updates-cli")]
#[command(about = "Publish and inspect packages on sigma-updates")]
struct Cli {
    /// Base URL of the updates service or Identity `/api` proxy
    /// (e.g. http://updates.sigma.localtest.me:30080 or https://identity…/api)
    #[arg(long, env = "SIGMA_UPDATES_URL", global = true)]
    url: Option<String>,

    /// Shared secret for direct updates publish/delete (`SIGMA_INTERNAL_TOKEN`)
    #[arg(long, env = "SIGMA_INTERNAL_TOKEN", global = true)]
    token: Option<String>,

    /// OIDC token endpoint (client-credentials). Overrides issuer derivation.
    #[arg(long, env = "SIGMA_OIDC_TOKEN_URL", global = true)]
    oidc_token_url: Option<String>,

    /// OIDC issuer URL used to derive the token endpoint when `--oidc-token-url` is unset
    #[arg(long, env = "SIGMA_OIDC_ISSUER", global = true)]
    oidc_issuer: Option<String>,

    /// OIDC confidential client id (service account)
    #[arg(long, env = "SIGMA_OIDC_CLIENT_ID", global = true)]
    oidc_client_id: Option<String>,

    /// OIDC confidential client secret
    #[arg(long, env = "SIGMA_OIDC_CLIENT_SECRET", global = true)]
    oidc_client_secret: Option<String>,

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

fn resolve_auth_token(cli: &Cli) -> Result<Option<String>, String> {
    let has_oidc = cli
        .oidc_client_id
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty())
        || cli
            .oidc_client_secret
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty())
        || cli
            .oidc_token_url
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty())
        || cli
            .oidc_issuer
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty());

    if has_oidc {
        let client_id = cli
            .oidc_client_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                "OIDC auth requires --oidc-client-id / SIGMA_OIDC_CLIENT_ID".to_string()
            })?;
        let client_secret = cli
            .oidc_client_secret
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                "OIDC auth requires --oidc-client-secret / SIGMA_OIDC_CLIENT_SECRET".to_string()
            })?;
        let token_url = if let Some(url) = cli
            .oidc_token_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            url.to_string()
        } else if let Some(issuer) = cli
            .oidc_issuer
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            token_url_from_issuer(issuer)
        } else {
            return Err(
                "OIDC auth requires --oidc-token-url / SIGMA_OIDC_TOKEN_URL or --oidc-issuer / SIGMA_OIDC_ISSUER"
                    .into(),
            );
        };
        let token = client_credentials_token(&token_url, client_id, client_secret)
            .map_err(|e| e.to_string())?;
        return Ok(Some(token));
    }

    Ok(cli
        .token
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty()))
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    let url = cli
        .url
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("http://127.0.0.1:8080")
        .to_string();
    let mut client = UpdatesClient::new(url);
    if let Some(token) = resolve_auth_token(&cli)? {
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
                    let deps = m
                        .missing
                        .iter()
                        .map(format_dep)
                        .collect::<Vec<_>>()
                        .join(", ");
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
            let report =
                push_packages(&client, &paths, allow_missing_deps).map_err(|e| e.to_string())?;
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
