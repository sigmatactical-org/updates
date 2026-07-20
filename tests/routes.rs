//! End-to-end checks of the assembled route tree (theme scaffold + JSON API +
//! streamed downloads).

use std::sync::Arc;

use sigma_updates::{Catalog, routes};

fn site()
-> impl warp::Filter<Extract = (impl warp::Reply,), Error = std::convert::Infallible> + Clone {
    routes(Arc::new(Catalog::with_dev_defaults()))
}

#[tokio::test]
async fn up_and_health_are_served() {
    let res = warp::test::request().path("/up").reply(&site()).await;
    assert_eq!(res.status(), 200);

    let res = warp::test::request().path("/health").reply(&site()).await;
    assert_eq!(res.status(), 200);
    let body = std::str::from_utf8(res.body()).unwrap();
    assert!(body.contains("sigma-updates"), "{body}");
    assert!(body.contains("\"status\""), "{body}");
}

#[tokio::test]
async fn index_renders_with_security_headers() {
    let res = warp::test::request().path("/").reply(&site()).await;
    assert_eq!(res.status(), 200);
    assert!(res.headers().contains_key("content-security-policy"));
    assert_eq!(res.headers().get("x-frame-options").unwrap(), "DENY");
}

#[tokio::test]
async fn package_listing_keeps_its_wire_shape() {
    let res = warp::test::request()
        .path("/v1/packages")
        .reply(&site())
        .await;
    assert_eq!(res.status(), 200);
    let body = std::str::from_utf8(res.body()).unwrap();
    assert!(body.contains("\"packages\":"), "{body}");
    assert!(body.contains("\"total_pages\":"), "{body}");
}

#[tokio::test]
async fn dbc_download_streams_as_an_attachment() {
    let res = warp::test::request()
        .path("/dbc/sigma-racer.dbc")
        .reply(&site())
        .await;
    assert_eq!(res.status(), 200);
    assert_eq!(
        res.headers().get("content-disposition").unwrap(),
        "attachment; filename=\"sigma-racer.dbc\""
    );
    assert!(!res.body().is_empty());

    let res = warp::test::request()
        .path("/dbc/../Cargo.toml")
        .reply(&site())
        .await;
    assert_eq!(res.status(), 404);
}

/// A streamed upload that is not a `.deb` is rejected, and nothing is left in
/// the packages directory (the temp file is removed before the rename).
#[tokio::test]
async fn publishing_invalid_content_is_rejected_and_leaves_no_file() {
    let dir = tempfile::tempdir().unwrap();
    temp_env::async_with_vars(
        [
            ("UPDATES_PACKAGES_DIR", Some(dir.path().to_str().unwrap())),
            (
                "SIGMA_INTERNAL_TOKEN",
                Some(sigma_pg::clients::internal::TEST_INTERNAL_TOKEN),
            ),
        ],
        async {
            let res = warp::test::request()
                .method("POST")
                .path("/v1/packages")
                .header("x-package-filename", "bogus_1_all.deb")
                .header(
                    "x-sigma-internal-token",
                    sigma_pg::clients::internal::TEST_INTERNAL_TOKEN,
                )
                .body("not a deb archive")
                .reply(&site())
                .await;
            assert_eq!(res.status(), 400);
            assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
        },
    )
    .await;
}

#[tokio::test]
async fn publishing_without_auth_is_not_found() {
    let res = warp::test::request()
        .method("POST")
        .path("/v1/packages")
        .header("x-package-filename", "x_1_all.deb")
        .body("not a deb")
        .reply(&site())
        .await;
    assert_eq!(res.status(), 404);
}
