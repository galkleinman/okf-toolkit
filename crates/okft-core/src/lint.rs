//! Advisory rules: bundle hygiene beyond §11 conformance.
//!
//! Nothing here may ever be an error by default. §11 explicitly forbids
//! rejecting a bundle for broken links, missing optional fields, or a missing
//! `index.md`, so these findings are warnings and infos that a caller opts into
//! failing on. See [`crate::diagnostic`] for the split.

use std::collections::BTreeSet;

use crate::bundle::{Bundle, Entry, EntryKind};
use crate::date::Date;
use crate::diagnostic::{Diagnostic, rule};
use crate::document::Frontmatter;
use crate::trust::Actor;
use crate::version::{self, OkfVersion};

/// Inputs a lint run needs from the caller.
#[derive(Debug, Clone, Copy)]
pub struct LintOptions {
    /// The date staleness is judged against (§5.5).
    ///
    /// Supplied rather than read from the clock so a run is reproducible: the
    /// same bundle and the same `--today` always give the same result.
    pub today: Date,
    /// The OKF revision the bundle is judged against (§12).
    pub version: OkfVersion,
}

impl LintOptions {
    /// Options targeting the latest revision this toolkit supports.
    pub fn new(today: Date) -> Self {
        Self {
            today,
            version: version::LATEST,
        }
    }

    #[must_use]
    pub fn with_version(mut self, version: OkfVersion) -> Self {
        self.version = version;
        self
    }
}

/// Runs every advisory rule that applies to the targeted revision.
pub fn lint(bundle: &Bundle, options: &LintOptions) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    check_declared_version(bundle, options.version, &mut diagnostics);

    for entry in bundle.concepts() {
        check_links(bundle, entry, &mut diagnostics);
        check_orphan(bundle, entry, &mut diagnostics);

        let Some(frontmatter) = entry.document.frontmatter.parsed() else {
            continue;
        };
        check_recommended_fields(entry, frontmatter, &mut diagnostics);
        check_staleness(entry, frontmatter, options.today, &mut diagnostics);
        check_legacy_fields(entry, frontmatter, &mut diagnostics);
        check_actors(entry, frontmatter, &mut diagnostics);
        check_sources(entry, frontmatter, &mut diagnostics);
        check_footnotes(entry, frontmatter, &mut diagnostics);
        check_computation(entry, frontmatter, &mut diagnostics);
        check_paths(bundle, entry, frontmatter, &mut diagnostics);
    }

    check_indexes(bundle, &mut diagnostics);

    // Version gating happens once, here, rather than inside each check: the
    // rule registry is then the single place that records which revision
    // introduced a rule, and `okf rules --okf-version` cannot disagree with
    // what a run actually reports.
    diagnostics.retain(|diagnostic| {
        rule(diagnostic.code).is_some_and(|rule| rule.applies_to(options.version))
    });
    diagnostics
}

/// §12: the declared `okf_version` should be one this toolkit knows, and should
/// agree with the revision the run is checking against.
fn check_declared_version(bundle: &Bundle, version: OkfVersion, diagnostics: &mut Vec<Diagnostic>) {
    let Some((declared, span)) = bundle.declared_okf_version() else {
        return;
    };

    let (message, help) = match OkfVersion::parse(declared) {
        Some(target) if target == version => return,
        Some(_) => (
            format!("bundle declares `okf_version: {declared}` but is being checked as {version}"),
            "drop `--okf-version` to check the bundle against the revision it declares",
        ),
        None => (
            format!("`okf_version: {declared}` is not a revision this tool supports"),
            "checking as the newest supported revision; declare `0.1` or `0.2` (§12)",
        ),
    };

    // §12 permits the declaration in the bundle-root `index.md` only, so that
    // is where a declaration necessarily came from.
    diagnostics.push(
        Diagnostic::new("version-mismatch", "index.md", message)
            .with_span(span)
            .with_help(help),
    );
}

fn check_links(bundle: &Bundle, entry: &Entry, diagnostics: &mut Vec<Diagnostic>) {
    for link in &entry.body.links {
        let Some(target) = link.resolve(&entry.id) else {
            continue;
        };
        if !bundle.contains(&target) {
            diagnostics.push(
                Diagnostic::new(
                    "broken-link",
                    &entry.path,
                    format!("link target `{}` does not exist in the bundle", link.target),
                )
                .with_span(link.span)
                .with_help("§6.1 permits links to not-yet-written concepts, so this is advisory"),
            );
        }
    }
}

fn check_orphan(bundle: &Bundle, entry: &Entry, diagnostics: &mut Vec<Diagnostic>) {
    if bundle.backlinks(&entry.id).is_empty() {
        diagnostics.push(
            Diagnostic::new(
                "orphan-concept",
                &entry.path,
                format!("no concept links to `{}`", entry.id),
            )
            .with_help("link it from a neighbouring concept or the directory's `index.md`"),
        );
    }
}

fn check_recommended_fields(
    entry: &Entry,
    frontmatter: &Frontmatter,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if frontmatter.description().is_none() {
        diagnostics.push(
            Diagnostic::new(
                "missing-description",
                &entry.path,
                "concept has no `description`",
            )
            .with_span(frontmatter.span)
            .with_help("index generators and search previews use `description` (§4.1)"),
        );
    }
    if frontmatter.title().is_none() {
        diagnostics.push(
            Diagnostic::new("missing-title", &entry.path, "concept has no `title`")
                .with_span(frontmatter.span)
                .with_help("without `title`, consumers fall back to the filename (§4.1)"),
        );
    }
}

fn check_staleness(
    entry: &Entry,
    frontmatter: &Frontmatter,
    today: Date,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if frontmatter.is_stale_on(today) {
        let stale_after = frontmatter.stale_after().unwrap_or(today);
        diagnostics.push(
            Diagnostic::new(
                "stale-concept",
                &entry.path,
                format!("concept went stale on {stale_after}"),
            )
            .with_span(
                frontmatter
                    .stale_after_raw()
                    .map_or(frontmatter.span, |n| n.span),
            )
            .with_help("re-verify the content and move `stale_after` forward (§5.5)"),
        );
    }
}

fn check_legacy_fields(
    entry: &Entry,
    frontmatter: &Frontmatter,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(timestamp) = frontmatter.legacy_timestamp() {
        diagnostics.push(
            Diagnostic::new(
                "legacy-timestamp",
                &entry.path,
                "`timestamp` is a v0.1 field",
            )
            .with_span(timestamp.span)
            .with_help("replace it with `generated: { by, at }` (§13.1)"),
        );
    }

    for heading in entry.body.headings_at(1) {
        if heading.text.eq_ignore_ascii_case("citations") {
            diagnostics.push(
                Diagnostic::new(
                    "legacy-citations",
                    &entry.path,
                    "a body `# Citations` list is a v0.1 construct",
                )
                .with_span(heading.span)
                .with_help("move the citations into frontmatter `sources` (§13.1)"),
            );
        }
    }
}

fn check_actors(entry: &Entry, frontmatter: &Frontmatter, diagnostics: &mut Vec<Diagnostic>) {
    for (actor, span) in frontmatter.actors() {
        if !Actor::parse(actor).is_recognized() {
            diagnostics.push(
                Diagnostic::new(
                    "unknown-actor",
                    &entry.path,
                    format!("`{actor}` does not match the actor conventions"),
                )
                .with_span(span)
                .with_help(
                    "use `<producer>/<version>`, `human:<id>`, or `process:<id>` (§7); trust \
                     tiers key off the `human:` prefix",
                ),
            );
        }
    }
}

fn check_sources(entry: &Entry, frontmatter: &Frontmatter, diagnostics: &mut Vec<Diagnostic>) {
    for source in frontmatter.sources() {
        if source.resource.is_none() {
            diagnostics.push(
                Diagnostic::new(
                    "source-missing-resource",
                    &entry.path,
                    "`sources` entry has no `resource`",
                )
                .with_span(source.span)
                .with_help("`resource` is required within a `sources` entry (§5.1)"),
            );
        }
    }
}

/// §5.1: a footnote label is the join key into `sources[].id`.
fn check_footnotes(entry: &Entry, frontmatter: &Frontmatter, diagnostics: &mut Vec<Diagnostic>) {
    let ids: BTreeSet<&str> = frontmatter.sources().iter().filter_map(|s| s.id).collect();
    for footnote in &entry.body.footnotes {
        if !ids.contains(footnote.label.as_str()) {
            diagnostics.push(
                Diagnostic::new(
                    "unresolved-footnote",
                    &entry.path,
                    format!("footnote `[^{}]` matches no `sources[].id`", footnote.label),
                )
                .with_span(footnote.span)
                .with_help("give the source an `id` equal to the footnote label (§5.1)"),
            );
        }
    }
}

fn check_computation(entry: &Entry, frontmatter: &Frontmatter, diagnostics: &mut Vec<Diagnostic>) {
    if frontmatter.is_attested_computation() && frontmatter.runtime().is_none() {
        diagnostics.push(
            Diagnostic::new(
                "computation-missing-runtime",
                &entry.path,
                "Attested Computation has no `runtime`",
            )
            .with_span(frontmatter.span)
            .with_help(
                "`runtime` defines what `parameters` mean, e.g. `runtime: bigquery` (§10.2)",
            ),
        );
    }
}

fn check_paths(
    bundle: &Bundle,
    entry: &Entry,
    frontmatter: &Frontmatter,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (path, span) in frontmatter.path_fields() {
        if !is_bundle_path(path) || bundle.contains_path(path) {
            continue;
        }
        diagnostics.push(
            Diagnostic::new(
                "unreachable-path",
                &entry.path,
                format!("`{path}` does not name a file in the bundle"),
            )
            .with_span(span)
            .with_help(
                "path-valued fields accept a URL, a `/`-rooted path, or a relative path (§6.2)",
            ),
        );
    }
}

/// Whether a path-valued field looks like a path into the bundle.
///
/// URLs point outside it, and a value containing whitespace is prose rather
/// than a path, so neither can be resolved against the bundle.
fn is_bundle_path(value: &str) -> bool {
    !value.is_empty()
        && !value.contains(char::is_whitespace)
        && value
            .split_once("://")
            .is_none_or(|(scheme, _)| scheme.is_empty())
}

/// §8: a directory holding concepts should offer an `index.md`.
fn check_indexes(bundle: &Bundle, diagnostics: &mut Vec<Diagnostic>) {
    let indexed: BTreeSet<&str> = bundle
        .entries()
        .filter(|entry| entry.kind == EntryKind::Index)
        .map(Entry::directory)
        .collect();

    for directory in bundle.directories() {
        let holds_concepts = bundle
            .concepts()
            .any(|entry| entry.directory() == directory);
        if holds_concepts && !indexed.contains(directory) {
            let shown = if directory.is_empty() {
                "the bundle root"
            } else {
                directory
            };
            diagnostics.push(
                Diagnostic::new(
                    "missing-index",
                    std::path::Path::new(directory).join("index.md"),
                    format!("{shown} has no `index.md`"),
                )
                .with_help("an index lets agents see what exists before opening documents (§8)"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::Severity;

    fn today() -> Date {
        Date::parse("2026-08-09").expect("valid")
    }

    fn run(sources: &[(&str, &str)]) -> Vec<Diagnostic> {
        run_as(sources, version::LATEST)
    }

    fn run_as(sources: &[(&str, &str)], version: OkfVersion) -> Vec<Diagnostic> {
        let bundle = Bundle::from_sources(sources.iter().copied());
        lint(&bundle, &LintOptions::new(today()).with_version(version))
    }

    /// Filters to one rule so each test asserts about its own rule only.
    fn codes_for(sources: &[(&str, &str)], code: &str) -> Vec<Diagnostic> {
        run(sources)
            .into_iter()
            .filter(|d| d.code == code)
            .collect()
    }

    /// §11 forbids rejecting a bundle for any of this; nothing may be an error.
    #[test]
    fn no_lint_finding_is_ever_an_error() {
        let findings = run(&[
            (
                "a.md",
                "---\ntimestamp: '2026-01-01T00:00:00Z'\n---\n[x](/gone.md)\n\n# Citations\n",
            ),
            (
                "b.md",
                "---\ntype: Attested Computation\nstale_after: 2020-01-01\n---\n",
            ),
        ]);
        assert!(!findings.is_empty());
        for finding in findings {
            assert_ne!(finding.severity, Severity::Error, "{finding:?}");
            assert!(!finding.is_conformance());
        }
    }

    #[test]
    fn a_well_formed_bundle_is_quiet() {
        let findings = run(&[
            ("index.md", "# Bundle\n\n* [orders](tables/index.md)\n"),
            ("tables/index.md", "# Tables\n\n* [orders](orders.md)\n"),
            (
                "tables/orders.md",
                "---\ntype: BigQuery Table\ntitle: Orders\ndescription: One row per order.\n---\n\
                 \nSee [customers](/tables/customers.md).\n",
            ),
            (
                "tables/customers.md",
                "---\ntype: BigQuery Table\ntitle: Customers\ndescription: One row per customer.\n---\n",
            ),
        ]);
        assert!(
            findings.is_empty(),
            "expected no findings, got {findings:?}"
        );
    }

    #[test]
    fn flags_links_to_missing_concepts() {
        let findings = codes_for(
            &[(
                "a.md",
                "---\ntype: X\n---\n[gone](/nope.md) [also](./missing.md)\n",
            )],
            "broken-link",
        );
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(findings[0].span.is_some());
    }

    #[test]
    fn does_not_flag_external_or_resolvable_links() {
        assert!(
            codes_for(
                &[
                    (
                        "a.md",
                        "---\ntype: X\n---\n[ok](/b.md) [ext](https://example.com/x.md)\n"
                    ),
                    ("b.md", "---\ntype: X\n---\n[back](/a.md)\n"),
                ],
                "broken-link",
            )
            .is_empty()
        );
    }

    #[test]
    fn flags_missing_title_and_description() {
        let findings = run(&[("a.md", "---\ntype: X\n---\n")]);
        assert_eq!(
            findings
                .iter()
                .filter(|d| d.code == "missing-description")
                .count(),
            1
        );
        assert_eq!(
            findings
                .iter()
                .filter(|d| d.code == "missing-title")
                .count(),
            1
        );

        let quiet = run(&[("a.md", "---\ntype: X\ntitle: T\ndescription: D\n---\n")]);
        assert!(quiet.iter().all(|d| !d.code.starts_with("missing-t")));
    }

    #[test]
    fn flags_a_concept_past_its_stale_after() {
        let findings = codes_for(
            &[("a.md", "---\ntype: X\nstale_after: 2026-08-08\n---\n")],
            "stale-concept",
        );
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("2026-08-08"));
    }

    /// §5.5: stale means `today >= stale_after`, so the boundary day counts.
    #[test]
    fn staleness_includes_the_boundary_day() {
        assert_eq!(
            codes_for(
                &[("a.md", "---\ntype: X\nstale_after: 2026-08-09\n---\n")],
                "stale-concept"
            )
            .len(),
            1
        );
        assert!(
            codes_for(
                &[("a.md", "---\ntype: X\nstale_after: 2026-08-10\n---\n")],
                "stale-concept"
            )
            .is_empty()
        );
    }

    #[test]
    fn flags_the_two_superseded_v01_constructs() {
        let findings = run(&[(
            "a.md",
            "---\ntype: X\ntimestamp: '2026-05-28T22:53:05+00:00'\n---\n\n# Citations\n\n- https://x\n",
        )]);
        assert_eq!(
            findings
                .iter()
                .filter(|d| d.code == "legacy-timestamp")
                .count(),
            1
        );
        assert_eq!(
            findings
                .iter()
                .filter(|d| d.code == "legacy-citations")
                .count(),
            1
        );
    }

    #[test]
    fn a_citations_heading_is_matched_case_insensitively() {
        assert_eq!(
            codes_for(
                &[("a.md", "---\ntype: X\n---\n\n# CITATIONS\n")],
                "legacy-citations"
            )
            .len(),
            1
        );
        assert!(
            codes_for(
                &[("a.md", "---\ntype: X\n---\n\n## Citations\n")],
                "legacy-citations"
            )
            .is_empty()
        );
    }

    #[test]
    fn flags_actors_outside_the_conventions() {
        let findings = codes_for(
            &[(
                "a.md",
                "---\ntype: X\ngenerated: { by: nonsense, at: 2026-01-01T00:00:00Z }\n\
                 verified: { by: team:finance, at: 2026-01-02T00:00:00Z }\n---\n",
            )],
            "unknown-actor",
        );
        assert_eq!(findings.len(), 2);
    }

    #[test]
    fn accepts_all_three_actor_conventions() {
        assert!(
            codes_for(
                &[(
                    "a.md",
                    "---\ntype: X\ngenerated: { by: agent/v1, at: 2026-01-01T00:00:00Z }\n\
                     verified:\n  - { by: human:a, at: 2026-01-02T00:00:00Z }\n  \
                     - { by: process:nightly, at: 2026-01-03T00:00:00Z }\n---\n",
                )],
                "unknown-actor",
            )
            .is_empty()
        );
    }

    #[test]
    fn flags_sources_without_a_resource() {
        let findings = codes_for(
            &[(
                "a.md",
                "---\ntype: X\nsources:\n  - title: No resource\n---\n",
            )],
            "source-missing-resource",
        );
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn flags_footnotes_with_no_matching_source_id() {
        let findings = codes_for(
            &[(
                "a.md",
                "---\ntype: X\nsources:\n  - id: known\n    resource: https://x\n---\n\
                 \nClaim.[^known] Other.[^unknown]\n\n[^known]: K\n\n[^unknown]: U\n",
            )],
            "unresolved-footnote",
        );
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("unknown"));
    }

    #[test]
    fn flags_an_attested_computation_without_a_runtime() {
        let findings = codes_for(
            &[(
                "a.md",
                "---\ntype: Attested Computation\n---\n\n# Computation\n",
            )],
            "computation-missing-runtime",
        );
        assert_eq!(findings.len(), 1);

        assert!(
            codes_for(
                &[(
                    "a.md",
                    "---\ntype: Attested Computation\nruntime: bigquery\n---\n"
                )],
                "computation-missing-runtime",
            )
            .is_empty()
        );
    }

    #[test]
    fn flags_path_fields_that_name_nothing() {
        let findings = codes_for(
            &[(
                "a.md",
                "---\ntype: X\nattester:\n  resource: attesters/gone.py\n---\n",
            )],
            "unreachable-path",
        );
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn accepts_path_fields_that_resolve() {
        assert!(
            codes_for(
                &[
                    (
                        "a.md",
                        "---\ntype: X\nattester:\n  resource: attesters/check.py\n\
                         computation: computations/revenue.sql\n---\n",
                    ),
                    ("attesters/check.py", "code"),
                    ("computations/revenue.sql", "SELECT 1"),
                ],
                "unreachable-path",
            )
            .is_empty()
        );
    }

    /// §5.1 allows prose scope descriptors, and §6.2 allows URLs; neither is a
    /// bundle path, so neither may be reported as unreachable.
    #[test]
    fn ignores_urls_and_prose_in_path_fields() {
        assert!(
            codes_for(
                &[(
                    "a.md",
                    "---\ntype: X\ncomputation: https://example.com/revenue.sql\n\
                     executor:\n  resource: all queries in BigQuery project X\n---\n",
                )],
                "unreachable-path",
            )
            .is_empty()
        );
        assert!(!is_bundle_path(""));
        assert!(!is_bundle_path("https://x/y"));
        assert!(!is_bundle_path("has space"));
        assert!(is_bundle_path("a/b.md"));
        assert!(is_bundle_path("://leading"));
    }

    #[test]
    fn flags_concepts_nothing_links_to() {
        let findings = codes_for(
            &[
                ("a.md", "---\ntype: X\n---\n"),
                ("b.md", "---\ntype: X\n---\n"),
            ],
            "orphan-concept",
        );
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].severity, Severity::Info);
    }

    #[test]
    fn flags_directories_of_concepts_with_no_index() {
        let findings = codes_for(
            &[
                ("tables/orders.md", "---\ntype: X\n---\n"),
                ("root.md", "---\ntype: X\n---\n"),
            ],
            "missing-index",
        );
        let messages: Vec<_> = findings.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(messages.len(), 2);
        assert!(messages.iter().any(|m| m.contains("the bundle root")));
        assert!(messages.iter().any(|m| m.contains("tables")));
    }

    #[test]
    fn a_directory_with_an_index_is_not_flagged() {
        assert!(
            codes_for(
                &[
                    ("index.md", "# Root\n"),
                    ("tables/index.md", "# Tables\n"),
                    ("tables/orders.md", "---\ntype: X\n---\n"),
                ],
                "missing-index",
            )
            .is_empty()
        );
    }

    /// A directory holding only subdirectories has no concepts of its own.
    #[test]
    fn intermediate_directories_are_not_required_to_have_an_index() {
        let findings = codes_for(
            &[
                ("index.md", "# Root\n"),
                ("a/b/deep.md", "---\ntype: X\n---\n"),
            ],
            "missing-index",
        );
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("a/b"));
    }

    #[test]
    fn reserved_files_are_not_linted_as_concepts() {
        // `index.md` has no title or description and `log.md` links nowhere;
        // neither may be reported, because only concepts are linted.
        let findings = run(&[
            ("index.md", "# Index\n"),
            ("log.md", "# Log\n\n## 2026-01-01\n"),
            ("a.md", "---\ntype: X\ntitle: T\ndescription: D\n---\n"),
        ]);

        assert!(
            !findings.is_empty(),
            "expected the orphaned concept to be reported"
        );
        for finding in &findings {
            assert_eq!(
                finding.path,
                std::path::Path::new("a.md"),
                "reserved files should not be linted as concepts: {finding:?}"
            );
        }
    }

    #[test]
    fn concepts_whose_frontmatter_failed_to_parse_are_skipped_by_field_rules() {
        let findings = run(&[("a.md", "---\n[unclosed\n---\n")]);
        assert!(findings.iter().all(|d| d.code != "missing-title"));
        assert!(findings.iter().any(|d| d.code == "orphan-concept"));
    }

    #[test]
    fn lint_options_default_to_the_latest_revision() {
        let options = LintOptions::new(today());
        assert_eq!(options.version, version::LATEST);
        assert_eq!(
            options.with_version(OkfVersion::V0_1).version,
            OkfVersion::V0_1
        );
    }

    mod versions {
        use super::*;
        use crate::diagnostic::{RULES, RuleKind};

        /// The two constructs §13.1 supersedes are the correct ones to use in a
        /// v0.1 bundle, so reporting them there would be a false positive.
        #[test]
        fn v01_bundles_are_not_told_off_for_v01_constructs() {
            let sources = [(
                "a.md",
                "---\ntype: X\ntitle: T\ndescription: D\ntimestamp: '2026-05-28T22:53:05+00:00'\n\
                 ---\n\n# Citations\n\n- https://example.com\n",
            )];
            let codes: Vec<_> = run_as(&sources, OkfVersion::V0_1)
                .iter()
                .map(|d| d.code)
                .collect();
            assert!(!codes.contains(&"legacy-timestamp"), "{codes:?}");
            assert!(!codes.contains(&"legacy-citations"), "{codes:?}");

            let latest: Vec<_> = run(&sources).iter().map(|d| d.code).collect();
            assert!(latest.contains(&"legacy-timestamp"));
            assert!(latest.contains(&"legacy-citations"));
        }

        /// Every rule the registry marks as v0.2-only, exercised through a
        /// bundle that trips all of them at once.
        #[test]
        fn no_v02_only_rule_fires_against_a_v01_bundle() {
            let sources = [(
                "a.md",
                "---\ntype: Attested Computation\ntitle: T\ndescription: D\n\
                 stale_after: 2020-01-01\ntimestamp: '2026-05-28T22:53:05+00:00'\n\
                 generated: { by: nonsense, at: 2026-01-01T00:00:00Z }\n\
                 sources:\n  - id: known\n    title: No resource\n\
                 attester:\n  resource: attesters/gone.py\n---\n\n\
                 # Citations\n\nClaim.[^missing]\n\n[^missing]: M\n",
            )];

            let reported: BTreeSet<&str> = run_as(&sources, OkfVersion::V0_1)
                .iter()
                .map(|d| d.code)
                .collect();
            for rule in RULES
                .iter()
                .filter(|r| r.since == OkfVersion::V0_2 && r.kind == RuleKind::Lint)
            {
                assert!(
                    !reported.contains(rule.code),
                    "{} fired against a v0.1 bundle",
                    rule.code
                );
            }

            // The same bundle read as v0.2 must trip every one of them, or the
            // assertion above would pass for the wrong reason.
            let latest: BTreeSet<&str> = run(&sources).iter().map(|d| d.code).collect();
            for rule in RULES
                .iter()
                .filter(|r| r.since == OkfVersion::V0_2 && r.kind == RuleKind::Lint)
            {
                assert!(latest.contains(rule.code), "{} never fired", rule.code);
            }
        }

        #[test]
        fn revision_independent_rules_still_fire_for_v01() {
            let codes: Vec<_> = run_as(
                &[("a.md", "---\ntype: X\n---\n[gone](/nope.md)\n")],
                OkfVersion::V0_1,
            )
            .iter()
            .map(|d| d.code)
            .collect();
            assert!(codes.contains(&"broken-link"));
            assert!(codes.contains(&"missing-title"));
            assert!(codes.contains(&"missing-description"));
            assert!(codes.contains(&"orphan-concept"));
            assert!(codes.contains(&"missing-index"));
        }

        #[test]
        fn a_declaration_matching_the_checked_revision_is_quiet() {
            for (declared, version) in [("0.2", OkfVersion::V0_2), ("0.1", OkfVersion::V0_1)] {
                let source = format!("---\nokf_version: '{declared}'\n---\n# Bundle\n");
                let findings = run_as(&[("index.md", source.as_str())], version);
                assert!(findings.is_empty(), "{declared}: {findings:?}");
            }
        }

        #[test]
        fn an_explicit_override_of_the_declaration_is_reported() {
            let findings = run_as(
                &[("index.md", "---\nokf_version: '0.2'\n---\n# Bundle\n")],
                OkfVersion::V0_1,
            );
            assert_eq!(findings.len(), 1);
            assert_eq!(findings[0].code, "version-mismatch");
            assert_eq!(findings[0].path, std::path::Path::new("index.md"));
            assert!(findings[0].message.contains("checked as 0.1"));
            assert_eq!(findings[0].span.expect("span").start.line, 2);
            assert!(
                findings[0]
                    .help
                    .as_deref()
                    .expect("help")
                    .contains("--okf-version")
            );
        }

        #[test]
        fn an_unsupported_declaration_is_reported() {
            let findings = run(&[("index.md", "---\nokf_version: '0.9'\n---\n# Bundle\n")]);
            assert_eq!(findings.len(), 1);
            assert_eq!(findings[0].code, "version-mismatch");
            assert!(findings[0].message.contains("`okf_version: 0.9`"));
            assert!(findings[0].help.as_deref().expect("help").contains("§12"));
        }

        #[test]
        fn a_bundle_that_declares_nothing_is_not_asked_to() {
            assert!(run(&[("index.md", "# Bundle\n")]).is_empty());
        }
    }
}
