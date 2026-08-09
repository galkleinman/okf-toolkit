//! The three §11 conformance rules, and nothing else.
//!
//! §11 is deliberately short. A bundle is conformant if every non-reserved
//! `.md` file has a parseable frontmatter block, every block has a non-empty
//! `type`, and the reserved filenames follow §8 and §9. The section then
//! *forbids* consumers from rejecting a bundle for missing optional fields,
//! unknown types, unknown keys, broken links, or a missing `index.md`. Those
//! live in [`crate::lint`] and can never be errors.

use crate::bundle::{Bundle, Entry, EntryKind};
use crate::date::Date;
use crate::diagnostic::Diagnostic;
use crate::document::FrontmatterState;

/// Runs the §11 conformance rules over a bundle.
///
/// Every returned diagnostic has [`crate::diagnostic::Severity::Error`].
pub fn validate(bundle: &Bundle) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for entry in bundle.entries() {
        match entry.kind {
            EntryKind::Concept => {
                check_frontmatter_parses(entry, &mut diagnostics);
                check_type_present(entry, &mut diagnostics);
            }
            EntryKind::Index => check_index(entry, &mut diagnostics),
            EntryKind::Log => check_log(entry, &mut diagnostics),
        }
    }
    diagnostics
}

/// §11.1: every non-reserved `.md` file has a parseable frontmatter block.
fn check_frontmatter_parses(entry: &Entry, diagnostics: &mut Vec<Diagnostic>) {
    match &entry.document.frontmatter {
        FrontmatterState::Parsed(_) => {}
        FrontmatterState::Absent => diagnostics.push(
            Diagnostic::new("okf-parse", &entry.path, "concept document has no frontmatter block")
                .with_help("add a `---` delimited YAML block at the top of the file"),
        ),
        FrontmatterState::Malformed(error) => diagnostics.push(
            Diagnostic::new("okf-parse", &entry.path, error.message()).with_span(error.span()),
        ),
    }
}

/// §11.2: every frontmatter block has a non-empty `type`.
///
/// Only checked when the block parsed; a file with no frontmatter has already
/// produced `okf-parse` and would otherwise report the same problem twice.
fn check_type_present(entry: &Entry, diagnostics: &mut Vec<Diagnostic>) {
    let Some(frontmatter) = entry.document.frontmatter.parsed() else {
        return;
    };
    if frontmatter.concept_type().is_some() {
        return;
    }

    let (message, span) = match frontmatter.entries.entry("type") {
        Some((key, _)) => ("`type` is present but empty", key.span),
        None => ("frontmatter is missing the required `type` field", frontmatter.span),
    };
    diagnostics.push(
        Diagnostic::new("okf-type", &entry.path, message)
            .with_span(span)
            .with_help("every concept needs a `type`, for example `type: Metric`"),
    );
}

/// §8, via §11.3: `index.md` carries no frontmatter, except that a bundle-root
/// `index.md` may declare `okf_version` (§12).
///
/// The body shape (sections of links) is intentionally not enforced: §8 says
/// index files "MAY" appear and describes a convention, so demanding a
/// particular body would reject bundles the spec permits.
fn check_index(entry: &Entry, diagnostics: &mut Vec<Diagnostic>) {
    let Some(frontmatter) = entry.document.frontmatter.parsed() else {
        // Absent frontmatter is the expected case; a malformed block is
        // reported below as an unexpected block rather than a parse failure,
        // since §11.1 only requires *concept* documents to parse.
        if let Some(error) = entry.document.frontmatter.error() {
            diagnostics.push(
                Diagnostic::new("okf-reserved", &entry.path, error.message())
                    .with_span(error.span())
                    .with_help("`index.md` files carry no frontmatter (§8)"),
            );
        }
        return;
    };

    let is_bundle_root = entry.directory().is_empty();
    for key in frontmatter.entries.keys() {
        if is_bundle_root && key == "okf_version" {
            continue;
        }
        let span = frontmatter.entries.entry(key).map_or(frontmatter.span, |(k, _)| k.span);
        let help = if is_bundle_root {
            "a bundle-root `index.md` may only declare `okf_version` (§12)"
        } else {
            "only a bundle-root `index.md` may carry frontmatter, and only `okf_version` (§8, §12)"
        };
        diagnostics.push(
            Diagnostic::new(
                "okf-reserved",
                &entry.path,
                format!("`index.md` must not carry frontmatter key `{key}`"),
            )
            .with_span(span)
            .with_help(help),
        );
    }
}

/// §9, via §11.3: date headings in `log.md` use ISO 8601 `YYYY-MM-DD`.
///
/// §9's example groups entries under level-2 headings, so those are treated as
/// the date headings. Other levels are left alone: a `# Bundle history` title
/// is not a date heading, and the spec constrains only the latter. Frontmatter
/// is tolerated because §9 never forbids it and Google's own `acme_retail`
/// sample carries `type: Log`.
fn check_log(entry: &Entry, diagnostics: &mut Vec<Diagnostic>) {
    for heading in entry.body.headings_at(2) {
        if Date::parse(&heading.text).is_none() {
            diagnostics.push(
                Diagnostic::new(
                    "okf-reserved",
                    &entry.path,
                    format!("log date heading `{}` is not an ISO 8601 date", heading.text),
                )
                .with_span(heading.span)
                .with_help("use `## YYYY-MM-DD`, for example `## 2026-05-22`"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::Severity;

    fn codes(sources: &[(&str, &str)]) -> Vec<&'static str> {
        let bundle = Bundle::from_sources(sources.iter().copied());
        validate(&bundle).into_iter().map(|d| d.code).collect()
    }

    fn only(sources: &[(&str, &str)]) -> Diagnostic {
        let bundle = Bundle::from_sources(sources.iter().copied());
        let mut diagnostics = validate(&bundle);
        assert_eq!(diagnostics.len(), 1, "expected one diagnostic, got {diagnostics:?}");
        diagnostics.remove(0)
    }

    #[test]
    fn a_minimal_conformant_bundle_reports_nothing() {
        assert!(codes(&[("a.md", "---\ntype: Metric\n---\nbody\n")]).is_empty());
    }

    /// §4.1: a concept carrying just `type` is fully conformant.
    #[test]
    fn type_alone_is_conformant() {
        assert!(codes(&[("a.md", "---\ntype: X\n---\n")]).is_empty());
    }

    #[test]
    fn every_returned_diagnostic_is_an_error() {
        let bundle = Bundle::from_sources([
            ("no-frontmatter.md", "just text\n"),
            ("no-type.md", "---\ntitle: T\n---\n"),
            ("index.md", "---\nunexpected: 1\n---\n"),
            ("log.md", "## not-a-date\n"),
        ]);
        let diagnostics = validate(&bundle);
        assert_eq!(diagnostics.len(), 4);
        for diagnostic in &diagnostics {
            assert_eq!(diagnostic.severity, Severity::Error, "{diagnostic:?}");
            assert!(diagnostic.is_conformance());
        }
    }

    mod okf_parse {
        use super::*;

        #[test]
        fn missing_frontmatter_is_an_error() {
            let diagnostic = only(&[("a.md", "# Heading\n\nNo frontmatter here.\n")]);
            assert_eq!(diagnostic.code, "okf-parse");
            assert!(diagnostic.message.contains("no frontmatter"));
        }

        #[test]
        fn invalid_yaml_is_an_error_with_a_span() {
            let diagnostic = only(&[("a.md", "---\ntype: [unclosed\n---\nbody\n")]);
            assert_eq!(diagnostic.code, "okf-parse");
            assert!(diagnostic.span.is_some());
        }

        #[test]
        fn an_unterminated_block_is_an_error() {
            let diagnostic = only(&[("a.md", "---\ntype: Metric\n")]);
            assert_eq!(diagnostic.code, "okf-parse");
            assert!(diagnostic.message.contains("never closed"));
        }

        #[test]
        fn non_mapping_frontmatter_is_an_error() {
            let diagnostic = only(&[("a.md", "---\n- a\n- b\n---\n")]);
            assert_eq!(diagnostic.code, "okf-parse");
            assert!(diagnostic.message.contains("mapping"));
        }

        /// A file with no frontmatter cannot also be missing `type`; reporting
        /// both would be noise.
        #[test]
        fn a_parse_failure_does_not_also_report_a_missing_type() {
            assert_eq!(codes(&[("a.md", "no frontmatter\n")]), ["okf-parse"]);
        }

        #[test]
        fn reserved_files_may_omit_frontmatter() {
            assert!(codes(&[("index.md", "# Index\n"), ("log.md", "# Log\n")]).is_empty());
        }
    }

    mod okf_type {
        use super::*;

        #[test]
        fn a_missing_type_is_an_error() {
            let diagnostic = only(&[("a.md", "---\ntitle: Orders\n---\n")]);
            assert_eq!(diagnostic.code, "okf-type");
            assert!(diagnostic.message.contains("missing"));
            assert!(diagnostic.help.is_some());
        }

        #[test]
        fn an_empty_type_is_an_error_pointing_at_the_key() {
            let diagnostic = only(&[("a.md", "---\ntitle: T\ntype: '  '\n---\n")]);
            assert_eq!(diagnostic.code, "okf-type");
            assert!(diagnostic.message.contains("empty"));
            assert_eq!(diagnostic.span.expect("span").start.line, 3);
        }

        #[test]
        fn empty_frontmatter_is_missing_a_type() {
            let diagnostic = only(&[("a.md", "---\n---\n")]);
            assert_eq!(diagnostic.code, "okf-type");
        }

        /// §11: unknown types and unknown keys must not be rejected.
        #[test]
        fn unknown_types_and_extra_keys_are_accepted() {
            assert!(
                codes(&[(
                    "a.md",
                    "---\ntype: Something Nobody Registered\nvendor_field: 1\n---\n"
                )])
                .is_empty()
            );
        }
    }

    mod okf_reserved {
        use super::*;

        #[test]
        fn a_root_index_may_declare_okf_version() {
            assert!(codes(&[("index.md", "---\nokf_version: '0.2'\n---\n# Index\n")]).is_empty());
        }

        #[test]
        fn a_root_index_may_not_declare_other_keys() {
            let diagnostic = only(&[("index.md", "---\ntype: Index\n---\n")]);
            assert_eq!(diagnostic.code, "okf-reserved");
            assert!(diagnostic.message.contains("`type`"));
            assert!(diagnostic.help.expect("help").contains("okf_version"));
        }

        #[test]
        fn a_nested_index_may_not_declare_okf_version() {
            let diagnostic = only(&[("tables/index.md", "---\nokf_version: '0.2'\n---\n")]);
            assert_eq!(diagnostic.code, "okf-reserved");
            assert!(diagnostic.help.expect("help").contains("bundle-root"));
        }

        #[test]
        fn an_index_without_frontmatter_is_fine() {
            assert!(
                codes(&[
                    ("index.md", "# Subdirectories\n\n* [t](tables/index.md)\n"),
                    ("tables/index.md", "# Tables\n"),
                ])
                .is_empty()
            );
        }

        #[test]
        fn a_malformed_index_block_is_reported_as_a_reserved_violation() {
            let diagnostic = only(&[("index.md", "---\n[unclosed\n---\n")]);
            assert_eq!(diagnostic.code, "okf-reserved");
        }

        #[test]
        fn every_offending_index_key_is_reported() {
            assert_eq!(
                codes(&[("index.md", "---\ntype: Index\ntitle: T\n---\n")]),
                ["okf-reserved", "okf-reserved"]
            );
        }

        /// Google's own `acme_retail/log.md` carries `type: Log`, and §9 never
        /// forbids frontmatter, so this must not be an error.
        #[test]
        fn a_log_may_carry_frontmatter() {
            assert!(
                codes(&[(
                    "log.md",
                    "---\ntype: Log\ntitle: History\n---\n\n# History\n\n## 2026-07-01\n\n- **Verified**\n"
                )])
                .is_empty()
            );
        }

        #[test]
        fn iso_date_headings_are_accepted() {
            assert!(
                codes(&[("log.md", "# Log\n\n## 2026-05-22\n\n* **Update**\n\n## 2026-05-15\n")])
                    .is_empty()
            );
        }

        #[test]
        fn a_non_iso_date_heading_is_an_error() {
            let diagnostic =
                only(&[("log.md", "# Log\n\n## May 22, 2026\n\n* **Update**\n")]);
            assert_eq!(diagnostic.code, "okf-reserved");
            assert!(diagnostic.message.contains("May 22, 2026"));
            assert!(diagnostic.span.is_some());
        }

        #[test]
        fn an_impossible_date_heading_is_an_error() {
            assert_eq!(codes(&[("log.md", "## 2026-13-45\n")]), ["okf-reserved"]);
        }

        #[test]
        fn nested_logs_are_checked_too() {
            assert_eq!(codes(&[("decisions/log.md", "## nope\n")]), ["okf-reserved"]);
        }

        /// Only level-2 headings are date headings; a title is not.
        #[test]
        fn other_heading_levels_are_not_treated_as_dates() {
            assert!(codes(&[("log.md", "# Bundle history\n\n### Details\n")]).is_empty());
        }
    }
}
