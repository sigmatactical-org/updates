use askama::Template;
use sigma_theme::copyright_years;
use sigma_theme::nav::SiteHeader;
use sigma_theme::site_nav::{AppSiteNav, render_app_site_nav};

use crate::config;
use crate::packages::PackagePage;

fn page_header(brand: &str) -> SiteHeader {
    SiteHeader::new(brand)
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

fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn sign_in_url() -> String {
    let identity_root = config::identity_public_base_url()
        .trim_end_matches('/')
        .to_owned();
    let app_uri = config::public_base_url();
    let callback = format!("{identity_root}/auth/callback");
    format!(
        "{identity_root}/auth/login?app_uri={}&redirect_uri={}&scope=openid",
        percent_encode(&app_uri),
        percent_encode(&callback)
    )
}

fn page_href(page: u32, per_page: u32, query: &str) -> String {
    let mut href = format!("/?page={page}&per_page={per_page}");
    if !query.is_empty() {
        href.push_str("&q=");
        href.push_str(&percent_encode(query));
    }
    href
}

#[derive(Debug)]
pub struct PackageRow {
    pub name: String,
    pub version: String,
    pub architecture: String,
    pub size_label: String,
    pub download_path: String,
    pub filename: String,
}

#[derive(Template)]
#[template(path = "home.html")]
pub struct HomeTemplate {
    pub title: String,
    pub package_count: usize,
    pub packages: Vec<PackageRow>,
    pub packages_dir: String,
    pub public_base: String,
    pub identity_base: String,
    pub sign_in_url: String,
    pub publish_api_url: String,
    pub copyright_years: String,
    pub site_header: SiteHeader,
    pub site_nav: String,
    pub page: u32,
    pub per_page: u32,
    pub total_pages: u32,
    pub query: String,
    pub query_empty: bool,
    pub has_prev: bool,
    pub has_next: bool,
    pub prev_href: String,
    pub next_href: String,
    pub range_start: usize,
    pub range_end: usize,
}

pub fn render_home_html(page: &PackagePage) -> askama::Result<String> {
    let rows: Vec<PackageRow> = page
        .packages
        .iter()
        .map(|p| PackageRow {
            name: p.name.clone(),
            version: p.version.clone(),
            architecture: p.architecture.clone(),
            size_label: format_size(p.size_bytes),
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
        packages_dir: config::packages_dir().display().to_string(),
        public_base: config::public_base_url_trimmed(),
        identity_base: format!("{identity_root}/"),
        sign_in_url: sign_in_url(),
        publish_api_url: format!("{identity_root}/api/v1/packages"),
        copyright_years: copyright_years(),
        site_header: page_header("Sigma Updates"),
        site_nav: site_nav()?,
        page: page.page,
        per_page: page.per_page,
        total_pages: page.total_pages,
        query: page.query.clone(),
        query_empty: page.query.is_empty(),
        has_prev: page.has_prev(),
        has_next: page.has_next(),
        prev_href: page_href(page.page.saturating_sub(1).max(1), page.per_page, &page.query),
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

fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let b = bytes as f64;
    if b >= MB {
        format!("{:.1} MiB", b / MB)
    } else if b >= KB {
        format!("{:.1} KiB", b / KB)
    } else {
        format!("{bytes} B")
    }
}