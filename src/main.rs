use sigma_updates::{Catalog, listen_addr, routes};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let catalog = Arc::new(Catalog::with_dev_defaults());
    let addr = listen_addr();
    let packages = sigma_updates::list_packages();
    eprintln!(
        "sigma-updates listening on http://{addr} ({} deb package(s), channels: {:?})",
        packages.len(),
        catalog.channels()
    );
    warp::serve(routes(catalog)).run(addr).await;
}
