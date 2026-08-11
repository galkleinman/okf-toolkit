//! Lint behaviour against Google's published bundles.
//!
//! Unlike conformance, lint findings on upstream bundles are expected: they are
//! opinions, not spec violations. The snapshot exists so a rule that suddenly
//! starts firing hundreds of times is visible in review rather than discovered
//! by a user.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use okft_core::bundle::Bundle;
use okft_core::date::Date;
use okft_core::diagnostic::Severity;
use okft_core::lint::{LintOptions, lint};

const UPSTREAM_BUNDLES: [&str; 4] = ["acme_retail", "crypto_bitcoin", "ga4", "stackoverflow"];

/// Pinned so `stale_after` comparisons do not change with the calendar.
fn fixed_today() -> Date {
    Date::parse("2026-08-09").expect("valid date")
}

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .canonicalize()
        .expect("fixtures directory exists")
}

fn counts(bundle_name: &str) -> BTreeMap<&'static str, usize> {
    let path = fixtures_root().join("upstream").join(bundle_name);
    let bundle = Bundle::load(&path).expect("loads");
    let mut counts = BTreeMap::new();
    for diagnostic in lint(&bundle, &LintOptions::new(fixed_today())) {
        *counts.entry(diagnostic.code).or_default() += 1;
    }
    counts
}

#[test]
fn lint_findings_on_upstream_bundles_are_stable() {
    let summary: BTreeMap<&str, BTreeMap<&'static str, usize>> = UPSTREAM_BUNDLES
        .iter()
        .map(|name| (*name, counts(name)))
        .collect();
    insta::assert_json_snapshot!(summary);
}

/// The whole point of the two-tier split: lint may complain about Google's own
/// bundles, but must never claim they are non-conformant.
#[test]
fn no_upstream_lint_finding_is_an_error() {
    for name in UPSTREAM_BUNDLES {
        let path = fixtures_root().join("upstream").join(name);
        let bundle = Bundle::load(&path).expect("loads");
        for diagnostic in lint(&bundle, &LintOptions::new(fixed_today())) {
            assert_ne!(
                diagnostic.severity,
                Severity::Error,
                "{name}: {diagnostic:?}"
            );
        }
    }
}
