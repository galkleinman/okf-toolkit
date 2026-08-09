//! The anti-false-positive net.
//!
//! Google's four published OKF sample bundles are vendored under
//! `tests/fixtures/upstream`. They are the reference for what a conformant
//! bundle looks like, so `validate` must report **zero** errors on every one of
//! them. A failure here means a rule is stricter than §11 allows, which is the
//! single worst defect this tool can ship.

use std::path::{Path, PathBuf};

use okf_toolkit_core::bundle::Bundle;
use okf_toolkit_core::conformance::validate;

const UPSTREAM_BUNDLES: [&str; 4] = ["acme_retail", "crypto_bitcoin", "ga4", "stackoverflow"];

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .canonicalize()
        .expect("fixtures directory exists")
}

fn load(bundle: &str) -> Bundle {
    let path = fixtures_root().join("upstream").join(bundle);
    Bundle::load(&path).unwrap_or_else(|error| panic!("loading {bundle}: {error}"))
}

#[test]
fn every_upstream_bundle_is_conformant() {
    for name in UPSTREAM_BUNDLES {
        let bundle = load(name);
        let diagnostics = validate(&bundle);

        assert!(
            diagnostics.is_empty(),
            "{name} must validate clean, but produced {} diagnostic(s):\n{}",
            diagnostics.len(),
            diagnostics
                .iter()
                .map(|d| format!(
                    "  [{}] {}:{} {}",
                    d.code,
                    d.path.display(),
                    d.span.map_or(0, |s| s.start.line),
                    d.message
                ))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

#[test]
fn every_upstream_bundle_actually_loaded_concepts() {
    for name in UPSTREAM_BUNDLES {
        let bundle = load(name);
        assert!(bundle.concepts().count() > 0, "{name} loaded no concepts");
        assert!(
            bundle.entries().count() > bundle.concepts().count(),
            "{name} should contain reserved files too"
        );
    }
}

/// Guards the vendoring itself: a silently empty fixture tree would make the
/// conformance test above pass for the wrong reason.
#[test]
fn the_vendored_fixtures_are_present_and_attributed() {
    let upstream = fixtures_root().join("upstream");
    assert!(upstream.join("NOTICE").is_file(), "upstream NOTICE is missing");

    for name in UPSTREAM_BUNDLES {
        assert!(upstream.join(name).is_dir(), "fixture bundle {name} is missing");
    }

    let notice = std::fs::read_to_string(upstream.join("NOTICE")).expect("readable NOTICE");
    assert!(notice.contains("Apache License 2.0"), "NOTICE must record the licence");
    assert!(notice.contains("commit "), "NOTICE must pin the upstream commit");
}

/// The upstream bundles exercise the v0.2 families, so the accessors should
/// find real data rather than silently reading everything as absent.
#[test]
fn upstream_bundles_exercise_the_v02_families() {
    let acme = load("acme_retail");

    let has_attested_computation = acme
        .concepts()
        .filter_map(|entry| entry.document.frontmatter.parsed())
        .any(|frontmatter| frontmatter.concept_type() == Some("Attested Computation"));
    assert!(has_attested_computation, "acme_retail should contain an Attested Computation");

    let has_human_review = acme
        .concepts()
        .filter_map(|entry| entry.document.frontmatter.parsed())
        .any(|frontmatter| {
            frontmatter.trust_tier() == okf_toolkit_core::trust::TrustTier::HumanReviewed
        });
    assert!(has_human_review, "acme_retail should contain a human-reviewed concept");

    let has_sources = acme
        .concepts()
        .filter_map(|entry| entry.document.frontmatter.parsed())
        .any(|frontmatter| !frontmatter.sources().is_empty());
    assert!(has_sources, "acme_retail should contain provenance sources");
}
