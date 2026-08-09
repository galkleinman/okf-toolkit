//! End-to-end tests driving the compiled `okft` binary.
//!
//! These run the real process, so they also cover `main.rs` and the argument
//! plumbing that unit tests bypass.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;

const EXIT_FINDINGS: i32 = 1;
const EXIT_USAGE: i32 = 2;

fn okft() -> Command {
    Command::cargo_bin("okft").expect("binary builds")
}

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .canonicalize()
        .expect("fixtures exist")
}

fn upstream(name: &str) -> PathBuf {
    fixtures_root().join("upstream").join(name)
}

/// Writes a throwaway bundle and returns its directory.
fn bundle(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    for (relative, contents) in files {
        let path = dir.path().join(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, contents).expect("write");
    }
    dir
}

fn healthy() -> tempfile::TempDir {
    bundle(&[
        ("index.md", "# Bundle\n\n* [orders](tables/index.md)\n"),
        ("tables/index.md", "# Tables\n\n* [orders](orders.md)\n"),
        (
            "tables/orders.md",
            "---\ntype: BigQuery Table\ntitle: Orders\ndescription: One row per order.\n---\n\n\
             Joins [customers](/tables/customers.md).\n",
        ),
        (
            "tables/customers.md",
            "---\ntype: BigQuery Table\ntitle: Customers\ndescription: One row per customer.\n---\n\n\
             Joined from [orders](/tables/orders.md).\n",
        ),
    ])
}

#[test]
fn validate_accepts_every_upstream_bundle() {
    for name in ["acme_retail", "crypto_bitcoin", "ga4", "stackoverflow"] {
        okft()
            .arg("validate")
            .arg(upstream(name))
            .assert()
            .success();
    }
}

#[test]
fn validate_reports_nothing_for_a_clean_bundle() {
    let dir = healthy();
    okft()
        .arg("validate")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("no findings"));
}

#[test]
fn validate_fails_on_a_missing_type() {
    let dir = bundle(&[("a.md", "---\ntitle: No type here\n---\n")]);
    okft()
        .arg("validate")
        .arg(dir.path())
        .assert()
        .code(EXIT_FINDINGS)
        .stdout(predicate::str::contains("okf-type"));
}

#[test]
fn validate_fails_on_unparseable_frontmatter() {
    let dir = bundle(&[("a.md", "---\ntype: [unclosed\n---\n")]);
    okft()
        .arg("validate")
        .arg(dir.path())
        .assert()
        .code(EXIT_FINDINGS)
        .stdout(predicate::str::contains("okf-parse"));
}

/// The two-tier promise: §11 forbids failing a bundle for a broken link, so
/// `validate` passes while `lint --strict` does not.
#[test]
fn a_broken_link_fails_only_under_strict_lint() {
    let dir = bundle(&[
        ("index.md", "# Bundle\n\n* [a](a.md)\n"),
        (
            "a.md",
            "---\ntype: Metric\ntitle: A\ndescription: D\n---\n\nSee [gone](/nope.md).\n",
        ),
    ]);

    okft().arg("validate").arg(dir.path()).assert().success();
    okft().arg("lint").arg(dir.path()).assert().success();
    okft()
        .arg("lint")
        .arg(dir.path())
        .arg("--strict")
        .assert()
        .code(EXIT_FINDINGS)
        .stdout(predicate::str::contains("broken-link"));
    okft()
        .args(["lint", "-D", "broken-link"])
        .arg(dir.path())
        .assert()
        .code(EXIT_FINDINGS);
}

#[test]
fn allow_silences_a_rule() {
    let dir = bundle(&[(
        "a.md",
        "---\ntype: Metric\ntitle: A\ndescription: D\n---\n\nSee [gone](/nope.md).\n",
    )]);

    okft()
        .args([
            "lint",
            "--strict",
            "-A",
            "broken-link",
            "-A",
            "orphan-concept",
            "-A",
            "missing-index",
        ])
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("no findings"));
}

/// A silenced conformance rule must not be able to hide a broken bundle.
#[test]
fn allow_cannot_silence_a_conformance_rule() {
    let dir = bundle(&[("a.md", "---\ntitle: No type\n---\n")]);
    okft()
        .args(["validate", "-A", "okf-type"])
        .arg(dir.path())
        .assert()
        .code(EXIT_FINDINGS)
        .stdout(predicate::str::contains("okf-type"));
}

#[test]
fn unknown_rule_codes_are_a_usage_error() {
    let dir = healthy();
    okft()
        .args(["lint", "-D", "no-such-rule"])
        .arg(dir.path())
        .assert()
        .code(EXIT_USAGE)
        .stderr(predicate::str::contains("unknown rule"));
}

#[test]
fn a_missing_bundle_is_a_usage_error() {
    okft()
        .args(["validate", "/definitely/not/a/bundle"])
        .assert()
        .code(EXIT_USAGE)
        .stderr(predicate::str::contains("is not a directory"));
}

#[test]
fn a_malformed_today_is_a_usage_error() {
    let dir = healthy();
    okft()
        .args(["lint", "--today", "09/08/2026"])
        .arg(dir.path())
        .assert()
        .code(EXIT_USAGE)
        .stderr(predicate::str::contains("not a YYYY-MM-DD date"));
}

/// `--today` makes staleness reproducible rather than calendar-dependent.
#[test]
fn today_controls_staleness() {
    let dir = bundle(&[(
        "a.md",
        "---\ntype: Metric\ntitle: A\ndescription: D\nstale_after: 2026-06-01\n---\n",
    )]);

    okft()
        .args([
            "lint",
            "--today",
            "2026-05-31",
            "-A",
            "orphan-concept",
            "-A",
            "missing-index",
        ])
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("no findings"));

    okft()
        .args(["lint", "--today", "2026-06-01"])
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("stale-concept"));
}

#[test]
fn json_output_parses() {
    let dir = bundle(&[("a.md", "---\ntitle: No type\n---\n")]);
    let output = okft()
        .args(["validate", "--format", "json"])
        .arg(dir.path())
        .assert()
        .code(EXIT_FINDINGS)
        .get_output()
        .stdout
        .clone();

    let parsed: serde_json::Value = serde_json::from_slice(&output).expect("stdout is valid JSON");
    assert_eq!(parsed["summary"]["errors"], 1);
    assert_eq!(parsed["diagnostics"][0]["code"], "okf-type");
    assert_eq!(parsed["diagnostics"][0]["path"], "a.md");
}

#[test]
fn sarif_output_parses() {
    let dir = bundle(&[("a.md", "---\ntitle: No type\n---\n")]);
    let output = okft()
        .args(["validate", "--format", "sarif"])
        .arg(dir.path())
        .assert()
        .code(EXIT_FINDINGS)
        .get_output()
        .stdout
        .clone();

    let parsed: serde_json::Value = serde_json::from_slice(&output).expect("valid SARIF JSON");
    assert_eq!(parsed["version"], "2.1.0");
    assert_eq!(parsed["runs"][0]["results"][0]["ruleId"], "okf-type");
}

#[test]
fn github_output_is_an_annotation() {
    let dir = bundle(&[("a.md", "---\ntitle: No type\n---\n")]);
    okft()
        .args(["validate", "--format", "github"])
        .arg(dir.path())
        .assert()
        .code(EXIT_FINDINGS)
        .stdout(predicate::str::starts_with("::error file=a.md"));
}

#[test]
fn rules_lists_every_code() {
    okft()
        .arg("rules")
        .assert()
        .success()
        .stdout(predicate::str::contains("okf-type"))
        .stdout(predicate::str::contains("broken-link"))
        .stdout(predicate::str::contains("Conformance rules"));

    let output = okft()
        .args(["rules", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let parsed: serde_json::Value = serde_json::from_slice(&output).expect("valid JSON");
    assert_eq!(
        parsed.as_array().expect("array").len(),
        okf_toolkit_core::diagnostic::RULES.len()
    );
}

#[test]
fn help_and_version_exit_zero() {
    okft()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("okft"));
    okft().arg("--version").assert().success();
    okft().args(["validate", "--help"]).assert().success();
}

#[test]
fn an_unknown_command_is_a_usage_error() {
    okft().arg("frobnicate").assert().code(EXIT_USAGE);
}

#[test]
fn the_bundle_argument_defaults_to_the_current_directory() {
    let dir = healthy();
    okft()
        .arg("validate")
        .current_dir(dir.path())
        .assert()
        .success();
}

/// Without `--today` the binary reads the clock; this exercises that path.
#[test]
fn lint_without_today_uses_the_current_date() {
    let dir = healthy();
    okft().arg("lint").arg(dir.path()).assert().success();
}
