#![forbid(unsafe_code)]

use std::sync::Arc;

use sigma_theme::warp::{listen_addr_from_env, serve};
use sigma_updates::{Catalog, list_packages, routes};

#[tokio::main]
async fn main() {
    let catalog = Arc::new(Catalog::with_dev_defaults());
    sigma_updates::spawn_dbc_sync();
    eprintln!(
        "sigma-updates: {} deb package(s), channels: {:?}",
        list_packages().len(),
        catalog.channels()
    );
    let addr = listen_addr_from_env();
    if let Err(e) = serve("sigma-updates", addr, routes(catalog)).await {
        eprintln!("failed to bind {addr}: {e}");
        std::process::exit(1);
    }
}
