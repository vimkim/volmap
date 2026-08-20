//! Compile-time embedded browser assets for the live inspection adapter.

use axum::response::{Html, IntoResponse};

const INDEX_HTML: &str = include_str!("assets/index.html");
const APP_CSS: &str = include_str!("assets/app.css");
const DISTRIBUTION_CSS: &str = include_str!("assets/distribution.css");
const ROUTES_JS: &str = include_str!("assets/routes.js");
const DISTRIBUTION_JS: &str = include_str!("assets/distribution.js");
const APP_JS: &str = include_str!("assets/app.js");

pub(super) async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

pub(super) async fn css() -> impl IntoResponse {
    css_response(APP_CSS)
}

pub(super) async fn distribution_css() -> impl IntoResponse {
    css_response(DISTRIBUTION_CSS)
}

fn css_response(source: &'static str) -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/css; charset=utf-8")],
        source,
    )
}

pub(super) async fn javascript() -> impl IntoResponse {
    javascript_response(APP_JS)
}

pub(super) async fn routes_javascript() -> impl IntoResponse {
    javascript_response(ROUTES_JS)
}

pub(super) async fn distribution_javascript() -> impl IntoResponse {
    javascript_response(DISTRIBUTION_JS)
}

fn javascript_response(source: &'static str) -> impl IntoResponse {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/javascript; charset=utf-8",
        )],
        source,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header::CONTENT_TYPE;

    fn compact(source: &str) -> String {
        source
            .chars()
            .filter(|value| !value.is_whitespace())
            .collect()
    }

    #[test]
    fn handlers_serve_the_embedded_assets_with_browser_media_types() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            for (response, media_type, marker) in [
                (
                    index().await.into_response(),
                    "text/html; charset=utf-8",
                    "<!doctype html>",
                ),
                (
                    css().await.into_response(),
                    "text/css; charset=utf-8",
                    ":root {",
                ),
                (
                    distribution_css().await.into_response(),
                    "text/css; charset=utf-8",
                    ".page-distribution {",
                ),
                (
                    routes_javascript().await.into_response(),
                    "text/javascript; charset=utf-8",
                    "window.volmapRoutes",
                ),
                (
                    distribution_javascript().await.into_response(),
                    "text/javascript; charset=utf-8",
                    "window.volmapDistribution",
                ),
                (
                    javascript().await.into_response(),
                    "text/javascript; charset=utf-8",
                    "\"use strict\";",
                ),
            ] {
                assert_eq!(response.headers()[CONTENT_TYPE], media_type);
                let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
                    .await
                    .unwrap();
                assert!(String::from_utf8(bytes.to_vec()).unwrap().contains(marker));
            }
        });
    }

    #[test]
    fn browser_starts_directly_without_a_credential_gate() {
        assert!(!APP_JS.contains("Authorization"));
        assert!(!APP_JS.contains("Bearer"));
        assert!(!INDEX_HTML.contains("unlockForm"));
        assert!(!INDEX_HTML.contains("Bearer"));
        assert!(!APP_CSS.contains("#unlock"));
        assert!(APP_JS.contains("async function start()"));
        assert!(APP_JS.contains("start();"));

        let routes = INDEX_HTML.find("/routes.js").unwrap();
        let distribution = INDEX_HTML.find("/distribution.js").unwrap();
        let application = INDEX_HTML.find("/app.js").unwrap();
        assert!(routes < distribution && distribution < application);
        assert!(
            INDEX_HTML.find("/app.css").unwrap() < INDEX_HTML.find("/distribution.css").unwrap()
        );
    }

    #[test]
    fn browser_renders_structured_errors() {
        assert!(APP_JS.contains("payload.error.message"));
        assert!(APP_JS.contains("error.status"));
        assert!(APP_JS.contains("error.code"));
        assert!(APP_JS.contains("Could not complete this view"));
        assert!(APP_CSS.contains(".error-note"));
    }

    #[test]
    fn browser_contract_exposes_full_volume_sector_mosaic() {
        let javascript = compact(APP_JS);
        let stylesheet = compact(APP_CSS);

        assert!(javascript.contains("map.id=\"volumeMap\""));
        assert!(APP_JS.contains("Unreserved"));
        assert!(APP_JS.contains("Reserved, unallocated"));
        assert!(APP_JS.contains("Occupied"));
        assert!(APP_JS.contains("Slotted free"));
        assert!(APP_JS.contains("System metadata"));
        assert!(APP_JS.contains("Finding outline"));
        assert!(stylesheet.contains("grid-template-columns:repeat(8,1fr)"));
        assert!(APP_CSS.contains(".page.unreserved"));
        assert!(APP_CSS.contains(".page.reserved-unallocated"));
        assert!(APP_CSS.contains(".page.allocated"));
        assert!(APP_CSS.contains(".page.allocated.occupancy-known"));
        assert!(APP_CSS.contains(".page.allocated.occupancy-unknown"));
        assert!(stylesheet.contains("var(--occupied)"));
        assert!(APP_CSS.contains(".page.system-metadata"));
        assert!(APP_CSS.contains(".page.finding"));
        assert!(javascript.contains("/sectors/${currentVolume.vol_id}"));
        assert!(javascript.contains("next_cursor"));
        assert!(javascript.contains("pages.length!==64"));
        assert!(javascript.contains("functionapplyPageFill("));
        assert!(javascript.contains("page.occupancy.occupied_percent"));
        assert!(javascript.contains("applyPageFill(node,page)"));
    }

    #[test]
    fn browser_contract_replaces_workspace_for_sector_and_page_drilldown() {
        let javascript = compact(APP_JS);
        let distribution_javascript = compact(DISTRIBUTION_JS);

        assert!(INDEX_HTML.contains("id=\"drillBreadcrumb\""));
        assert!(INDEX_HTML.contains("id=\"workspaceContent\""));
        assert!(APP_CSS.contains(".sector-focus-grid"));
        assert!(APP_CSS.contains(".page-workspace"));
        assert!(APP_CSS.contains(".slot-table"));
        assert!(javascript.contains("functionshowSector("));
        assert!(javascript.contains("functionshowVolume("));
        assert!(javascript.contains("asyncfunctionshowPage("));
        assert!(javascript.contains("renderPageWorkspace"));
        assert!(javascript.contains("deep.state===\"not-enriched\""));
        assert!(javascript.contains("slot.offset"));
        assert!(javascript.contains("slot.length"));
        assert!(javascript.contains("renderSlottedDistribution"));
        assert!(distribution_javascript.contains("functionrender("));
        assert!(APP_JS.contains("64 physical pages"));
    }

    #[test]
    fn browser_contract_uses_revision_pinned_canonical_history() {
        let javascript = compact(APP_JS);
        let routes = compact(ROUTES_JS);

        assert!(javascript.contains("parse:parseBrowserRoute"));
        assert!(javascript.contains("functionsyncBrowserRoute("));
        assert!(javascript.contains("history.pushState"));
        assert!(javascript.contains("history.replaceState"));
        assert!(javascript.contains("popstate"));
        assert!(routes.contains("constROUTE_KINDS="));
        assert!(javascript.contains("route.snapshot!==session.snapshot.id"));
        assert!(javascript.contains("session.snapshot.revision=route.revision"));
        assert!(javascript.contains("awaitrestoreBrowserRoute(route)"));
        assert!(javascript.contains("showPage(route.page,true,\"none\")"));
        assert!(!javascript.contains("token=${"));
    }

    #[test]
    fn browser_contract_renders_complete_slotted_page_distribution() {
        let javascript = compact(APP_JS);
        let distribution_javascript = compact(DISTRIBUTION_JS);

        assert!(DISTRIBUTION_CSS.contains(".page-distribution"));
        assert!(DISTRIBUTION_CSS.contains(".region-fragmented-free"));
        assert!(DISTRIBUTION_CSS.contains(".slot-entry.unallocated"));
        assert!(DISTRIBUTION_CSS.contains(".slot-entry.deleted"));
        assert!(distribution_javascript.contains("distribution.free_regions"));
        assert!(distribution_javascript.contains("distribution.slot_entries"));
        assert!(DISTRIBUTION_JS.contains("Full slotted-page distribution"));
        assert!(DISTRIBUTION_JS.contains("not allocated"));
        assert!(javascript.contains("Number(slot.offset)>0&&slot.record_type===\"home\""));
        assert!(!distribution_javascript.contains("width/16384"));
    }
}
