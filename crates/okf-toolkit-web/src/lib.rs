//! A local, fully offline graph viewer for an OKF bundle.
//!
//! The markup, styling, and graph script are Google's reference viewer,
//! vendored unchanged apart from one edit: the upstream template loads
//! Cytoscape and Marked from a CDN, which makes the viewer useless offline and
//! leaks a request to a third party every time someone inspects a private
//! knowledge base. Both libraries are embedded in the binary instead.

pub mod graph;

use axum::Router;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use okf_toolkit_core::bundle::Bundle;
use okf_toolkit_core::date::Date;
use std::sync::Arc;

const TEMPLATE: &str = include_str!("../assets/viz.html");
const VIZ_CSS: &str = include_str!("../assets/viz.css");
const VIZ_JS: &str = include_str!("../assets/viz.js");
const CYTOSCAPE_JS: &str = include_str!("../assets/cytoscape.min.js");
const MARKED_JS: &str = include_str!("../assets/marked.min.js");

/// Everything the viewer needs, rendered into a single self-contained page.
#[derive(Debug, Clone)]
pub struct Viewer {
    html: Arc<str>,
}

impl Viewer {
    /// Renders the viewer for a bundle.
    ///
    /// # Panics
    ///
    /// Panics if the graph fails to serialise, which would mean a bug in this
    /// crate: the graph is plain data with string keys and no input can make
    /// it unserialisable.
    pub fn render(bundle: &Bundle, name: &str, today: Date) -> Self {
        let graph = graph::build(bundle, today);
        // Both values are plain data with string keys, so serialisation is
        // infallible; a failure here would be a bug in this crate, not input.
        let data = serde_json::to_string(&graph).expect("graph serialises");
        let bundle_name = serde_json::to_string(name).expect("bundle name serialises");

        let html = TEMPLATE
            .replace("/*__CYTOSCAPE_JS__*/", CYTOSCAPE_JS)
            .replace("/*__MARKED_JS__*/", MARKED_JS)
            .replace("/*__VIZ_CSS__*/", VIZ_CSS)
            .replace("/*__VIZ_JS__*/", VIZ_JS)
            .replace("__BUNDLE_NAME__", &bundle_name)
            .replace("__BUNDLE_DATA__", &data);

        Self { html: html.into() }
    }

    pub fn html(&self) -> &str {
        &self.html
    }

    /// Builds the router serving this viewer.
    pub fn router(self) -> Router {
        Router::new()
            .route("/", get(page))
            .route("/healthz", get(|| async { "ok" }))
            .with_state(self)
    }
}

async fn page(axum::extract::State(viewer): axum::extract::State<Viewer>) -> Response {
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            ),
            // The page is generated per run and must not be cached between edits.
            (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
        ],
        viewer.html.to_string(),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;

    fn bundle() -> Bundle {
        Bundle::from_sources([
            ("index.md", "# Bundle\n\n* [orders](tables/orders.md)\n"),
            (
                "tables/orders.md",
                "---\ntype: BigQuery Table\ntitle: Orders\ndescription: One row per order.\n---\n\n\
                 Joins [customers](/tables/customers.md).\n",
            ),
            (
                "tables/customers.md",
                "---\ntype: BigQuery Table\ntitle: Customers\ndescription: One per customer.\n---\n",
            ),
        ])
    }

    fn viewer() -> Viewer {
        Viewer::render(
            &bundle(),
            "test-bundle",
            Date::parse("2026-06-01").expect("valid"),
        )
    }

    async fn get_body(path: &str) -> (StatusCode, String) {
        let response = viewer()
            .router()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        (status, String::from_utf8_lossy(&bytes).into_owned())
    }

    /// The whole point of vendoring: opening the page must not fetch anything.
    ///
    /// Concept `resource` URIs legitimately appear in the embedded data, so
    /// this asserts there is nothing the browser would *load*, rather than that
    /// no URL appears anywhere in the text.
    #[test]
    fn the_page_loads_no_external_subresources() {
        let html = viewer().html().to_owned();

        for host in ["cdn.jsdelivr.net", "unpkg.com", "cdnjs.cloudflare.com"] {
            assert!(
                !html.contains(host),
                "rendered page still references {host}"
            );
        }
        assert!(
            !html.contains("<script src"),
            "page loads an external script"
        );
        assert!(!html.contains("<link"), "page loads an external stylesheet");
        assert!(
            !html.contains("@import"),
            "stylesheet imports another sheet"
        );
    }

    /// The bundle's own `resource` URIs must survive into the page, since the
    /// detail pane links to them.
    #[test]
    fn concept_resource_uris_are_preserved() {
        let bundle = Bundle::from_sources([(
            "a.md",
            "---\ntype: X\ntitle: A\nresource: https://example.com/thing\n---\n",
        )]);
        let viewer = Viewer::render(&bundle, "b", Date::parse("2026-06-01").expect("valid"));
        assert!(viewer.html().contains("https://example.com/thing"));
    }

    #[test]
    fn every_placeholder_is_substituted() {
        let html = viewer().html().to_owned();
        for placeholder in [
            "/*__CYTOSCAPE_JS__*/",
            "/*__MARKED_JS__*/",
            "/*__VIZ_CSS__*/",
            "/*__VIZ_JS__*/",
            "__BUNDLE_NAME__",
            "__BUNDLE_DATA__",
        ] {
            assert!(
                !html.contains(placeholder),
                "{placeholder} was not substituted"
            );
        }
    }

    #[test]
    fn the_libraries_and_bundle_data_are_inlined() {
        let html = viewer().html().to_owned();
        assert!(html.contains("cytoscape"), "cytoscape should be inlined");
        assert!(html.contains("marked"), "marked should be inlined");
        assert!(html.contains("\"test-bundle\""));
        assert!(html.contains("tables/orders"));
        assert!(
            html.len() > 300_000,
            "page looks too small to contain the libraries"
        );
    }

    #[tokio::test]
    async fn the_root_route_serves_the_page() {
        let (status, body) = get_body("/").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.starts_with("<!DOCTYPE html>"));
        assert!(body.contains("OKF Bundle Viewer"));
    }

    #[tokio::test]
    async fn the_page_is_not_cached() {
        let response = viewer()
            .router()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/html; charset=utf-8"
        );
    }

    #[tokio::test]
    async fn health_check_responds() {
        let (status, body) = get_body("/healthz").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "ok");
    }

    #[tokio::test]
    async fn unknown_routes_are_not_found() {
        let (status, _) = get_body("/nope").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn an_empty_bundle_still_renders() {
        let viewer = Viewer::render(
            &Bundle::default(),
            "empty",
            Date::parse("2026-06-01").expect("valid"),
        );
        assert!(viewer.html().contains("OKF Bundle Viewer"));
    }
}
