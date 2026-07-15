//! HTML site routes (Sigma theme).

use serde::Deserialize;
use warp::Filter;
use warp::Rejection;
use warp::Reply;

use crate::dbc;
use crate::packages;
use crate::templates;
use crate::vss;

const PUBLISH_JS: &str = include_str!("../assets/publish.js");

#[derive(Debug, Deserialize)]
struct HomeQuery {
    page: Option<u32>,
    per_page: Option<u32>,
    q: Option<String>,
}

/// Build this module's routes.
pub fn routes() -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + Send + 'static
{
    home()
        .or(package_download())
        .or(dbc_download())
        .or(vss_download())
        .or(publish_js())
}

fn home() -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + Send + 'static {
    warp::path::end()
        .and(warp::get())
        .and(warp::query::<HomeQuery>())
        .and_then(|query: HomeQuery| async move {
            let page = packages::list_packages_page(
                query.page.unwrap_or(1),
                query.per_page.unwrap_or(packages::DEFAULT_PER_PAGE),
                query.q.as_deref().unwrap_or(""),
            );
            templates::render_home_html(&page, &dbc::list_dbc_files(), &vss::list_vss_files())
                .map(warp::reply::html)
                .map_err(|_| warp::reject::not_found())
        })
}

fn publish_js() -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + Send + 'static
{
    warp::path!("js" / "publish.js").and(warp::get()).map(|| {
        warp::reply::with_header(
            PUBLISH_JS,
            "content-type",
            "application/javascript; charset=utf-8",
        )
    })
}

fn dbc_download() -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + Send + 'static
{
    warp::path("dbc")
        .and(warp::path::param::<String>())
        .and(warp::path::end())
        .and(warp::get())
        .and_then(|filename: String| async move {
            let Some(path) = dbc::dbc_path(&filename) else {
                return Err(warp::reject::not_found());
            };
            let bytes = tokio::fs::read(&path)
                .await
                .map_err(|_| warp::reject::not_found())?;
            let mut resp = warp::reply::Response::new(bytes.into());
            resp.headers_mut().insert(
                warp::http::header::CONTENT_TYPE,
                "text/plain; charset=utf-8"
                    .parse()
                    .expect("valid content-type"),
            );
            resp.headers_mut().insert(
                warp::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\"")
                    .parse()
                    .expect("valid content-disposition"),
            );
            Ok::<_, Rejection>(resp)
        })
}

fn vss_download() -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + Send + 'static
{
    warp::path("vss")
        .and(warp::path::param::<String>())
        .and(warp::path::end())
        .and(warp::get())
        .and_then(|filename: String| async move {
            let Some(path) = vss::vss_path(&filename) else {
                return Err(warp::reject::not_found());
            };
            let bytes = tokio::fs::read(&path)
                .await
                .map_err(|_| warp::reject::not_found())?;
            let mut resp = warp::reply::Response::new(bytes.into());
            resp.headers_mut().insert(
                warp::http::header::CONTENT_TYPE,
                "text/plain; charset=utf-8"
                    .parse()
                    .expect("valid content-type"),
            );
            resp.headers_mut().insert(
                warp::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\"")
                    .parse()
                    .expect("valid content-disposition"),
            );
            Ok::<_, Rejection>(resp)
        })
}

fn package_download()
-> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + Send + 'static {
    warp::path("packages")
        .and(warp::path::param::<String>())
        .and(warp::path::end())
        .and(warp::get())
        .and_then(|filename: String| async move {
            let Some(path) = packages::package_path(&filename) else {
                return Err(warp::reject::not_found());
            };
            let bytes = tokio::fs::read(&path)
                .await
                .map_err(|_| warp::reject::not_found())?;
            let mut resp = warp::reply::Response::new(bytes.into());
            resp.headers_mut().insert(
                warp::http::header::CONTENT_TYPE,
                "application/vnd.debian.binary-package"
                    .parse()
                    .expect("valid content-type"),
            );
            resp.headers_mut().insert(
                warp::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\"")
                    .parse()
                    .expect("valid content-disposition"),
            );
            Ok::<_, Rejection>(resp)
        })
}
