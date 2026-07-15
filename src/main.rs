#![forbid(unsafe_code)]

use sigma_updates::{Catalog, listen_addr, routes};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let catalog = Arc::new(Catalog::with_dev_defaults());
    sigma_updates::spawn_dbc_sync();
    let addr = listen_addr();
    let packages = sigma_updates::list_packages();
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| panic!("failed to bind {addr}: {e}"));
    eprintln!(
        "sigma-updates listening on http://{addr} ({} deb package(s), channels: {:?})",
        packages.len(),
        catalog.channels()
    );
    warp::serve(routes(catalog))
        .incoming(listener)
        .graceful(shutdown_signal())
        .run()
        .await;
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let (mut term, mut int) = match (
            signal(SignalKind::terminate()),
            signal(SignalKind::interrupt()),
        ) {
            (Ok(term), Ok(int)) => (term, int),
            _ => {
                eprintln!("warning: could not install signal handlers; graceful shutdown disabled");
                std::future::pending::<()>().await;
                return;
            }
        };
        tokio::select! {
            _ = term.recv() => {}
            _ = int.recv() => {}
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
