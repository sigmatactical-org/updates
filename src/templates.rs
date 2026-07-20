//! Askama rendering for the HTML index.

mod home_template;
mod package_row;
mod schema_row;

pub use home_template::HomeTemplate;
pub use package_row::PackageRow;
pub use schema_row::SchemaRow;

use askama::Template;
use sigma_theme::copyright_years;
use sigma_theme::nav::{SiteHeader, SiteMenuSection, site_menu};
use sigma_theme::site_nav::{AppSiteNav, render_app_site_nav};
use sigma_updates_deb::human_size;

use crate::config;
use crate::dbc::DbcFile;
use crate::listing::Page;
use crate::packages::DebPackage;
use crate::vss::VssFile;

fn page_header() -> SiteHeader {
    SiteHeader::new("Updates").with_menu(site_menu(Some(SiteMenuSection::Updates)))
}

fn site_nav() -> Result<String, askama::Error> {
    render_app_site_nav(&AppSiteNav {
        identity_base: &config::identity_public_base_url(),
        app_base: &config::public_base_url(),
        contact_base: &config::contact_public_base_url(),
        cart_url: &config::cart_public_base_url(),
        cart_count: 0,
        return_path: "/",
        show_cart: false,
        show_contact_us: false,
        leading_html: "",
    })
}

fn sign_in_url() -> String {
    let identity_root = config::identity_public_base_url()
        .trim_end_matches('/')
        .to_owned();
    let app_uri = config::public_base_url();
    let callback = format!("{identity_root}/auth/callback");
    format!(
        "{identity_root}/auth/login?app_uri={}&redirect_uri={}&scope=openid",
        urlencoding::encode(&app_uri),
        urlencoding::encode(&callback)
    )
}

fn page_href(page: u32, per_page: u32, query: &str) -> String {
    let mut href = format!("/?page={page}&per_page={per_page}");
    if !query.is_empty() {
        href.push_str("&q=");
        href.push_str(&urlencoding::encode(query));
    }
    href
}

fn schema_row(name: &str, filename: &str, size_bytes: u64, download_path: &str) -> SchemaRow {
    SchemaRow {
        name: name.to_owned(),
        filename: filename.to_owned(),
        size_label: human_size(size_bytes),
        download_path: download_path.to_owned(),
    }
}

/// Render the package-index home page.
pub fn render_home_html(
    page: &Page<DebPackage>,
    schemas: &[DbcFile],
    vss_files: &[VssFile],
) -> askama::Result<String> {
    let schema_rows: Vec<SchemaRow> = schemas
        .iter()
        .map(|s| schema_row(&s.name, &s.filename, s.size_bytes, &s.download_path))
        .collect();

    let vss_rows: Vec<SchemaRow> = vss_files
        .iter()
        .map(|s| schema_row(&s.name, &s.filename, s.size_bytes, &s.download_path))
        .collect();

    let rows: Vec<PackageRow> = page
        .items
        .iter()
        .map(|p| PackageRow {
            name: p.name.clone(),
            version: p.version.clone(),
            architecture: p.architecture.clone(),
            size_label: human_size(p.size_bytes),
            download_path: p.download_path.clone(),
            filename: p.filename.clone(),
        })
        .collect();

    let identity_root = config::identity_public_base_url()
        .trim_end_matches('/')
        .to_owned();

    let range_start = if page.total == 0 {
        0
    } else {
        ((page.page - 1) * page.per_page) as usize + 1
    };
    let range_end = range_start.saturating_add(rows.len().saturating_sub(1));

    HomeTemplate {
        title: "Sigma Updates".to_string(),
        package_count: page.total,
        packages: rows,
        schema_count: schema_rows.len(),
        schemas: schema_rows,
        vss_count: vss_rows.len(),
        vss_files: vss_rows,
        dbc_source: config::dbc_github_source(),
        packages_dir: config::packages_dir().display().to_string(),
        public_base: config::public_base_url_trimmed(),
        identity_base: format!("{identity_root}/"),
        sign_in_url: sign_in_url(),
        publish_api_url: format!("{identity_root}/api/v1/packages"),
        copyright_years: copyright_years(),
        site_header: page_header(),
        site_nav: site_nav()?,
        page: page.page,
        per_page: page.per_page,
        total_pages: page.total_pages,
        query: page.query.clone(),
        query_empty: page.query.is_empty(),
        has_prev: page.has_prev(),
        has_next: page.has_next(),
        prev_href: page_href(
            page.page.saturating_sub(1).max(1),
            page.per_page,
            &page.query,
        ),
        next_href: page_href(
            (page.page + 1).min(page.total_pages),
            page.per_page,
            &page.query,
        ),
        range_start,
        range_end,
    }
    .render()
}
