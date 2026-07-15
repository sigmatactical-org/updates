//! JSON API routes for the update catalog and package index.

mod channels_response;
mod dbc_response;
mod error_body;
mod health;
mod package_list_query;
mod packages_response;
pub(crate) use channels_response::ChannelsResponse;
pub(crate) use dbc_response::DbcResponse;
pub(crate) use error_body::ErrorBody;
pub(crate) use health::Health;
pub(crate) use package_list_query::PackageListQuery;
pub(crate) use packages_response::PackagesResponse;

use std::sync::Arc;

use bytes::Bytes;
use warp::http::StatusCode;
use warp::reply::Response;
use warp::{Filter, Rejection, Reply};

use crate::bundles;
use crate::catalog::Catalog;
use crate::dbc::{self};
use crate::packages::{self, PublishError};

fn json_error(status: StatusCode, message: impl Into<String>) -> Response {
    warp::reply::with_status(
        warp::reply::json(&ErrorBody {
            error: message.into(),
        }),
        status,
    )
    .into_response()
}

fn internal_auth()
-> impl Filter<Extract = (Option<String>, Option<String>), Error = Rejection> + Clone {
    warp::header::optional::<String>("authorization")
        .and(warp::header::optional::<String>("x-sigma-internal-token"))
}

fn ensure_internal(
    authorization: Option<String>,
    internal_token: Option<String>,
) -> Result<(), Rejection> {
    if sigma_pg::clients::internal::authorize_internal(
        authorization.as_deref(),
        internal_token.as_deref(),
    ) {
        Ok(())
    } else {
        Err(warp::reject::not_found())
    }
}

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

/// Build this module's routes.
pub fn routes(
    catalog: Arc<Catalog>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let health = warp::path("health").and(warp::get()).map(|| {
        warp::reply::json(&Health {
            status: "ok",
            service: "sigma-updates",
        })
    });

    let up = warp::path("up")
        .and(warp::get())
        .map(|| warp::reply::with_status("up", StatusCode::OK));

    let packages_v1 = warp::path!("v1" / "packages").and(warp::path::end());

    let pkg_list =
        warp::get()
            .and(warp::query::<PackageListQuery>())
            .map(|query: PackageListQuery| {
                let page = packages::list_packages_page(
                    query.page.unwrap_or(1),
                    query.per_page.unwrap_or(packages::DEFAULT_PER_PAGE),
                    query.q.as_deref().unwrap_or(""),
                );
                warp::reply::json(&PackagesResponse {
                    packages: page.items,
                    total: page.total,
                    page: page.page,
                    per_page: page.per_page,
                    total_pages: page.total_pages,
                    query: page.query,
                })
                .into_response()
            });

    let pkg_publish = warp::post()
        .and(internal_auth())
        .and(warp::header::optional::<String>("x-package-filename"))
        .and(warp::body::content_length_limit(packages::MAX_PACKAGE_BYTES))
        .and(warp::body::bytes())
        .and_then(
            |authorization,
             internal_token,
             filename_header: Option<String>,
             body: Bytes| async move {
                if ensure_internal(authorization, internal_token).is_err() {
                    return Ok::<_, Rejection>(json_error(StatusCode::NOT_FOUND, "not found"));
                }
                let Some(filename) = filename_header
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                else {
                    return Ok(json_error(
                        StatusCode::BAD_REQUEST,
                        "missing X-Package-Filename header",
                    ));
                };
                match packages::publish_package(&filename, &body) {
                    Ok(pkg) => Ok(warp::reply::with_status(
                        warp::reply::json(&pkg),
                        StatusCode::CREATED,
                    )
                    .into_response()),
                    Err(err) => Ok(json_error(publish_status(&err), err.to_string())),
                }
            },
        );

    let pkg_collection = packages_v1.and(pkg_list.or(pkg_publish));

    let pkg_delete = warp::path!("v1" / "packages" / String)
        .and(warp::delete())
        .and(internal_auth())
        .and_then(
            |filename: String, authorization, internal_token| async move {
                if ensure_internal(authorization, internal_token).is_err() {
                    return Ok::<_, Rejection>(json_error(StatusCode::NOT_FOUND, "not found"));
                }
                match packages::delete_package(&filename) {
                    Ok(()) => Ok(StatusCode::NO_CONTENT.into_response()),
                    Err(err) => Ok(json_error(publish_status(&err), err.to_string())),
                }
            },
        );

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
        .and_then(
            |channel: String, name: String, authorization, internal_token, body| async move {
                if ensure_internal(authorization, internal_token).is_err() {
                    return Ok::<_, Rejection>(json_error(StatusCode::NOT_FOUND, "not found"));
                }
                match bundles::store_bundle(&channel, &name, body).await {
                    Ok(bytes) => Ok(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "channel": channel,
                            "bundle": name,
                            "size_bytes": bytes,
                        })),
                        StatusCode::CREATED,
                    )
                    .into_response()),
                    Err(err) => Ok(json_error(publish_status(&err), err.to_string())),
                }
            },
        );

    let bundle_delete = warp::path!("v1" / "channel" / String / "bundle" / String)
        .and(warp::delete())
        .and(internal_auth())
        .and_then(
            |channel: String, name: String, authorization, internal_token| async move {
                if ensure_internal(authorization, internal_token).is_err() {
                    return Ok::<_, Rejection>(json_error(StatusCode::NOT_FOUND, "not found"));
                }
                match bundles::delete_bundle(&channel, &name) {
                    Ok(()) => Ok(StatusCode::NO_CONTENT.into_response()),
                    Err(err) => Ok(json_error(publish_status(&err), err.to_string())),
                }
            },
        );

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

    let dbc_v1 = warp::path!("v1" / "dbc").and(warp::path::end());

    let dbc_list =
        warp::get()
            .and(warp::query::<PackageListQuery>())
            .map(|query: PackageListQuery| {
                let page = dbc::list_dbc_files_page(
                    query.page.unwrap_or(1),
                    query.per_page.unwrap_or(dbc::DEFAULT_PER_PAGE),
                    query.q.as_deref().unwrap_or(""),
                );
                warp::reply::json(&DbcResponse {
                    files: page.items,
                    total: page.total,
                    page: page.page,
                    per_page: page.per_page,
                    total_pages: page.total_pages,
                    query: page.query,
                })
                .into_response()
            });

    let dbc_publish = warp::post()
        .and(internal_auth())
        .and(warp::header::optional::<String>("x-dbc-filename"))
        .and(warp::body::content_length_limit(dbc::MAX_DBC_BYTES))
        .and(warp::body::bytes())
        .and_then(
            |authorization,
             internal_token,
             filename_header: Option<String>,
             body: Bytes| async move {
                if ensure_internal(authorization, internal_token).is_err() {
                    return Ok::<_, Rejection>(json_error(StatusCode::NOT_FOUND, "not found"));
                }
                let Some(filename) = filename_header
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                else {
                    return Ok(json_error(
                        StatusCode::BAD_REQUEST,
                        "missing X-Dbc-Filename header",
                    ));
                };
                match dbc::publish_dbc(&filename, &body) {
                    Ok(file) => Ok(warp::reply::with_status(
                        warp::reply::json(&file),
                        StatusCode::CREATED,
                    )
                    .into_response()),
                    Err(err) => Ok(json_error(publish_status(&err), err.to_string())),
                }
            },
        );

    let dbc_collection = dbc_v1.and(dbc_list.or(dbc_publish));

    let dbc_latest = warp::path!("v1" / "dbc" / "latest")
        .and(warp::get())
        .map(|| match dbc::latest_dbc_file() {
            Some(file) => {
                warp::reply::with_status(warp::reply::json(&file), StatusCode::OK).into_response()
            }
            None => json_error(StatusCode::NOT_FOUND, "no DBC schemas published"),
        });

    let dbc_delete = warp::path!("v1" / "dbc" / String)
        .and(warp::delete())
        .and(internal_auth())
        .and_then(
            |filename: String, authorization, internal_token| async move {
                if ensure_internal(authorization, internal_token).is_err() {
                    return Ok::<_, Rejection>(json_error(StatusCode::NOT_FOUND, "not found"));
                }
                match dbc::delete_dbc(&filename) {
                    Ok(()) => Ok(StatusCode::NO_CONTENT.into_response()),
                    Err(err) => Ok(json_error(publish_status(&err), err.to_string())),
                }
            },
        );

    health
        .or(up)
        .or(pkg_collection)
        .or(pkg_delete)
        .or(dbc_collection)
        .or(dbc_latest)
        .or(dbc_delete)
        .or(channels)
        .or(latest)
        .or(bundle)
}
