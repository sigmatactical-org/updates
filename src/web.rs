//! HTML site routes (Sigma theme).

use warp::Filter;
use warp::Rejection;
use warp::Reply;
use warp::reply::Response;

use crate::api::PageQuery;
use crate::dbc::DbcCatalog;
use crate::listing::{self, CatalogSpec};
use crate::packages::PackageCatalog;
use crate::templates;
use crate::vss::VssCatalog;

const PUBLISH_JS: &str = include_str!("../assets/publish.js");
const HOME_CSS: &str = include_str!("../assets/home.css");

/// Build this module's routes.
pub fn routes() -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + Send + 'static
{
    home()
        .or(download::<PackageCatalog>(
            "packages",
            "application/vnd.debian.binary-package",
        ))
        .or(download::<DbcCatalog>("dbc", "text/plain; charset=utf-8"))
        .or(download::<VssCatalog>("vss", "text/plain; charset=utf-8"))
        .or(assets())
}

fn home() -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + Send + 'static {
    warp::path::end()
        .and(warp::get())
        .and(warp::query::<PageQuery>())
        .and_then(|query: PageQuery| async move {
            // Scanning the catalogs stats every file, so it runs on the
            // blocking pool rather than on a runtime worker.
            let html = tokio::task::spawn_blocking(move || {
                let page =
                    listing::page::<PackageCatalog>(query.page(), query.per_page(), query.query());
                templates::render_home_html(
                    &page,
                    &listing::list::<DbcCatalog>(),
                    &listing::list::<VssCatalog>(),
                )
            })
            .await
            .map_err(|_| warp::reject::not_found())?;
            html.map(warp::reply::html)
                .map_err(|_| warp::reject::not_found())
        })
}

/// Static page assets served from the binary (the theme owns `/static`).
fn assets() -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + Send + 'static {
    let js = warp::path!("js" / "publish.js")
        .and(warp::get())
        .map(|| inline_asset(PUBLISH_JS, "application/javascript; charset=utf-8"));
    let css = warp::path!("css" / "home.css")
        .and(warp::get())
        .map(|| inline_asset(HOME_CSS, "text/css; charset=utf-8"));
    js.or(css)
}

fn inline_asset(body: &'static str, content_type: &'static str) -> impl Reply {
    warp::reply::with_header(body, "content-type", content_type)
}

/// `GET /{prefix}/{filename}` — one catalog file as an attachment.
///
/// The file is served by `warp::fs::dir`, which streams it off disk (with
/// range support) instead of buffering up to [`CatalogSpec::MAX_BYTES`] in
/// memory; the catalog's own filename check runs first, so only files this
/// catalog publishes are reachable.
fn download<S: CatalogSpec>(
    prefix: &'static str,
    content_type: &'static str,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + Send + 'static {
    warp::path(prefix)
        .and(warp::path::peek())
        .and_then(move |peek: warp::path::Peek| async move {
            if S::is_safe_filename(peek.as_str()) {
                Ok(())
            } else {
                Err(warp::reject::not_found())
            }
        })
        .untuple_one()
        .and(warp::fs::dir(S::dir()))
        .map(move |file: warp::filters::fs::File| {
            let filename = file
                .path()
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_owned();
            attachment(file, &filename, content_type)
        })
}

/// Serve `reply` as a download with the catalog's content type.
fn attachment(reply: impl Reply, filename: &str, content_type: &'static str) -> Response {
    let mut resp = reply.into_response();
    resp.headers_mut().insert(
        warp::http::header::CONTENT_TYPE,
        content_type.parse().expect("valid content-type"),
    );
    resp.headers_mut().insert(
        warp::http::header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{filename}\"")
            .parse()
            .expect("valid content-disposition"),
    );
    resp
}
