//! JSON API routes for the update catalog and package index.

use std::sync::Arc;

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use warp::http::StatusCode;
use warp::reply::Response;
use warp::{Filter, Rejection, Reply};

use crate::catalog::Catalog;
use crate::packages::{self, PublishError};

#[derive(Serialize)]
struct Health {
    status: &'static str,
    service: &'static str,
}

#[derive(Serialize)]
struct ChannelsResponse {
    channels: Vec<String>,
}

#[derive(Serialize)]
struct PackagesResponse {
    packages: Vec<packages::DebPackage>,
    total: usize,
    page: u32,
    per_page: u32,
    total_pages: u32,
    #[serde(skip_serializing_if = "String::is_empty")]
    query: String,
}

#[derive(Debug, Deserialize)]
struct PackageListQuery {
    page: Option<u32>,
    per_page: Option<u32>,
    q: Option<String>,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

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

fn publish_status(err: &PublishError) -> StatusCode {
    match err {
        PublishError::InvalidFilename
        | PublishError::EmptyBody
        | PublishError::InvalidDeb(_) => StatusCode::BAD_REQUEST,
        PublishError::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        PublishError::NotFound => StatusCode::NOT_FOUND,
        PublishError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub fn routes(
    catalog: Arc<Catalog>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let health = warp::path("health")
        .and(warp::get())
        .map(|| {
            warp::reply::json(&Health {
                status: "ok",
                service: "sigma-updates",
            })
        });

    let up = warp::path("up")
        .and(warp::get())
        .map(|| warp::reply::with_status("up", StatusCode::OK));

    let packages_v1 = warp::path!("v1" / "packages").and(warp::path::end());

    let pkg_list = warp::get()
        .and(warp::query::<PackageListQuery>())
        .map(|query: PackageListQuery| {
            let page = packages::list_packages_page(
                query.page.unwrap_or(1),
                query.per_page.unwrap_or(packages::DEFAULT_PER_PAGE),
                query.q.as_deref().unwrap_or(""),
            );
            warp::reply::json(&PackagesResponse {
                packages: page.packages,
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
        .map(move |channel: String| match catalog_latest.latest(&channel) {
            Some(rel) => warp::reply::with_status(warp::reply::json(rel), StatusCode::OK),
            None => warp::reply::with_status(
                warp::reply::json(&ErrorBody {
                    error: format!("unknown channel '{channel}'"),
                }),
                StatusCode::NOT_FOUND,
            ),
        });

    let bundle = warp::path!("v1" / "channel" / String / "bundle" / String)
        .and(warp::get())
        .map(|_channel: String, name: String| {
            warp::reply::with_status(
                warp::reply::json(&ErrorBody {
                    error: format!(
                        "bundle '{name}' not published yet — metadata-only catalog"
                    ),
                }),
                StatusCode::NOT_FOUND,
            )
        });

    health
        .or(up)
        .or(pkg_collection)
        .or(pkg_delete)
        .or(channels)
        .or(latest)
        .or(bundle)
}
