//! JSON API routes for the update catalog and package index.

mod channels_response;
mod page_query;
mod page_response;

pub(crate) use channels_response::ChannelsResponse;
pub(crate) use page_query::PageQuery;
pub(crate) use page_response::PageResponse;

use std::sync::Arc;

use serde::Serialize;
use sigma_pg::api::{ErrorBody, internal_auth, json_error};
use sigma_pg::health::warp::health_routes;
use warp::http::StatusCode;
use warp::reply::Response;
use warp::{Filter, Rejection, Reply};

use crate::bundles;
use crate::catalog::Catalog;
use crate::dbc::{self, DbcCatalog};
use crate::listing::{self, CatalogSpec, PublishError};
use crate::packages::PackageCatalog;
use crate::vss::{self, VssCatalog};

/// HTTP status for a catalog publish/delete failure.
fn publish_status(err: &PublishError) -> StatusCode {
    match err {
        PublishError::InvalidFilename
        | PublishError::EmptyBody
        | PublishError::InvalidContent(_) => StatusCode::BAD_REQUEST,
        PublishError::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        PublishError::NotFound => StatusCode::NOT_FOUND,
        PublishError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Scanning a catalog directory stats (and for `.deb`, opens) every file, so
/// it runs on the blocking pool rather than on a runtime worker.
async fn blocking<T: Send + 'static>(
    what: &str,
    job: impl FnOnce() -> T + Send + 'static,
) -> Result<T, Response> {
    tokio::task::spawn_blocking(job).await.map_err(|e| {
        json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("{what} failed: {e}"),
        )
    })
}

/// One page of a catalog as JSON, with the items under `items_field`.
async fn page_json<S: CatalogSpec>(items_field: &'static str, query: PageQuery) -> Response
where
    S::Item: Serialize,
{
    match blocking("listing", move || {
        listing::page::<S>(query.page(), query.per_page(), query.query())
    })
    .await
    {
        Ok(page) => warp::reply::json(&PageResponse::new(items_field, page)).into_response(),
        Err(err) => err,
    }
}

/// Build this module's routes.
pub fn routes(
    catalog: Arc<Catalog>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let packages_v1 = warp::path!("v1" / "packages").and(warp::path::end());

    let pkg_list = warp::get()
        .and(warp::query::<PageQuery>())
        .then(|query: PageQuery| page_json::<PackageCatalog>("packages", query));

    let pkg_publish = warp::post()
        .and(internal_auth())
        .and(warp::header::optional::<String>("x-package-filename"))
        .and(warp::body::content_length_limit(PackageCatalog::MAX_BYTES))
        .and(warp::body::stream())
        .and_then(|filename_header: Option<String>, body| async move {
            let Some(filename) = filename_header
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
            else {
                return Ok::<_, Rejection>(json_error(
                    StatusCode::BAD_REQUEST,
                    "missing X-Package-Filename header",
                ));
            };
            // The body streams straight to a temp file; only the control
            // metadata is ever held in memory.
            match listing::publish_stream::<PackageCatalog, _, _, _>(&filename, body).await {
                Ok(pkg) => Ok(warp::reply::with_status(
                    warp::reply::json(&pkg),
                    StatusCode::CREATED,
                )
                .into_response()),
                Err(err) => Ok(json_error(publish_status(&err), err.to_string())),
            }
        });

    let pkg_collection = packages_v1.and(pkg_list.or(pkg_publish));

    let pkg_delete = warp::path!("v1" / "packages" / String)
        .and(warp::delete())
        .and(internal_auth())
        .and_then(|filename: String| async move {
            let deleted = blocking("delete", move || {
                listing::delete::<PackageCatalog>(&filename)
            })
            .await;
            Ok::<_, Rejection>(match deleted {
                Ok(Ok(())) => StatusCode::NO_CONTENT.into_response(),
                Ok(Err(err)) => json_error(publish_status(&err), err.to_string()),
                Err(err) => err,
            })
        });

    let catalog_ch = catalog.clone();
    let channels = warp::path!("v1" / "channels")
        .and(warp::get())
        .map(move || {
            warp::reply::json(&ChannelsResponse {
                channels: catalog_ch.channels(),
            })
        });

    let catalog_latest = catalog.clone();
    let latest = warp::path!("v1" / "channel" / String / "latest")
        .and(warp::get())
        .map(
            move |channel: String| match catalog_latest.latest(&channel) {
                Some(rel) => warp::reply::with_status(warp::reply::json(rel), StatusCode::OK),
                None => warp::reply::with_status(
                    warp::reply::json(&ErrorBody {
                        error: format!("unknown channel '{channel}'"),
                    }),
                    StatusCode::NOT_FOUND,
                ),
            },
        );

    // GETs stream straight off disk: the on-disk layout under bundles_dir
    // mirrors the URL tail (<channel>/bundle/<name>), so warp's fs filter
    // handles streaming, ranges, and path sanitization.
    let bundle = warp::path!("v1" / "channel" / ..)
        .and(warp::get())
        .and(warp::fs::dir(crate::config::bundles_dir()));

    let bundle_publish = warp::path!("v1" / "channel" / String / "bundle" / String)
        .and(warp::post())
        .and(internal_auth())
        .and(warp::body::content_length_limit(bundles::MAX_BUNDLE_BYTES))
        .and(warp::body::stream())
        .and_then(|channel: String, name: String, body| async move {
            match bundles::store_bundle(&channel, &name, body).await {
                Ok(bytes) => Ok::<_, Rejection>(
                    warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "channel": channel,
                            "bundle": name,
                            "size_bytes": bytes,
                        })),
                        StatusCode::CREATED,
                    )
                    .into_response(),
                ),
                Err(err) => Ok(json_error(publish_status(&err), err.to_string())),
            }
        });

    let bundle_delete = warp::path!("v1" / "channel" / String / "bundle" / String)
        .and(warp::delete())
        .and(internal_auth())
        .and_then(|channel: String, name: String| async move {
            Ok::<_, Rejection>(match bundles::delete_bundle(&channel, &name) {
                Ok(()) => StatusCode::NO_CONTENT.into_response(),
                Err(err) => json_error(publish_status(&err), err.to_string()),
            })
        });

    // fs::dir rejects missing files; without an explicit GET fallback the
    // rejection combines with the POST/DELETE method mismatches into a 405.
    let bundle_missing = warp::path!("v1" / "channel" / String / "bundle" / String)
        .and(warp::get())
        .map(|channel: String, name: String| {
            json_error(
                StatusCode::NOT_FOUND,
                format!("bundle '{name}' not published on channel '{channel}'"),
            )
        });

    let bundle = bundle
        .or(bundle_missing)
        .or(bundle_publish)
        .or(bundle_delete);

    // The DBC and VSS catalogs are read-only mirrors of the canonical schemas
    // on GitHub (see `dbc::spawn_github_sync`); there is no publish/delete API.
    let dbc_list = warp::path!("v1" / "dbc")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::query::<PageQuery>())
        .then(|query: PageQuery| page_json::<DbcCatalog>("files", query));

    let dbc_latest = warp::path!("v1" / "dbc" / "latest")
        .and(warp::get())
        .then(|| async {
            match blocking("listing", dbc::latest_dbc_file).await {
                Ok(Some(file)) => {
                    warp::reply::with_status(warp::reply::json(&file), StatusCode::OK)
                        .into_response()
                }
                Ok(None) => json_error(StatusCode::NOT_FOUND, "no DBC schemas published"),
                Err(err) => err,
            }
        });

    let vss_list = warp::path!("v1" / "vss")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::query::<PageQuery>())
        .then(|query: PageQuery| page_json::<VssCatalog>("files", query));

    let vss_latest = warp::path!("v1" / "vss" / "latest")
        .and(warp::get())
        .then(|| async {
            match blocking("listing", vss::latest_vss_file).await {
                Ok(Some(file)) => {
                    warp::reply::with_status(warp::reply::json(&file), StatusCode::OK)
                        .into_response()
                }
                Ok(None) => json_error(StatusCode::NOT_FOUND, "no VSS schemas published"),
                Err(err) => err,
            }
        });

    // `internal_auth` rejects unauthenticated writes as not-found; without
    // these fallbacks the rejection would combine with the other methods on
    // the same path into a 405 instead of the documented JSON 404.
    let denied = warp::path!("v1" / "packages")
        .and(warp::post())
        .map(not_found_json)
        .or(warp::path!("v1" / "packages" / String)
            .and(warp::delete())
            .map(|_| not_found_json()))
        .or(warp::path!("v1" / "channel" / String / "bundle" / String)
            .and(warp::post())
            .map(|_, _| not_found_json()))
        .or(warp::path!("v1" / "channel" / String / "bundle" / String)
            .and(warp::delete())
            .map(|_, _| not_found_json()));

    health_routes("sigma-updates", None)
        .or(pkg_collection)
        .or(pkg_delete)
        .or(dbc_list)
        .or(dbc_latest)
        .or(vss_list)
        .or(vss_latest)
        .or(channels)
        .or(latest)
        .or(bundle)
        .or(denied)
}

/// The JSON body unauthorized and unknown requests share.
fn not_found_json() -> Response {
    json_error(StatusCode::NOT_FOUND, "not found")
}
