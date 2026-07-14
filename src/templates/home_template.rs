//! [`HomeTemplate`].

#[allow(unused_imports)]
use super::*;
use askama::Template;
use sigma_theme::nav::SiteHeader;

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
