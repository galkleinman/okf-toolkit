//! Diagnostics and the rule registry.
//!
//! The split between [`Severity::Error`] and the advisory severities is the
//! heart of this crate. OKF §11 lists exactly three conformance requirements
//! and then *forbids* consumers from rejecting a bundle for anything else --
//! broken links, unknown types, unknown keys, missing optional fields, or a
//! missing `index.md`. So only the three [`RuleKind::Conformance`] rules may
//! ever produce an error; everything else is advisory and opt-in via
//! `--strict` or `-D <code>`.

use std::path::{Path, PathBuf};

use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether a rule states a §11 conformance requirement or a hygiene opinion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleKind {
    /// One of the three §11 requirements. Always an error.
    Conformance,
    /// Advisory. Never an error unless the caller opts in.
    Lint,
}

/// A registered rule. Every code the crate can emit appears in [`RULES`].
#[derive(Debug, Clone, Copy)]
pub struct Rule {
    pub code: &'static str,
    pub kind: RuleKind,
    pub default_severity: Severity,
    pub description: &'static str,
    /// The spec section the rule derives from, for error messages.
    pub spec_section: &'static str,
}

/// Every rule this crate can emit.
///
/// Exactly three are [`RuleKind::Conformance`]; the rest are advisory. A test
/// asserts that invariant, and another asserts every code has both a fixture
/// that triggers it and one that does not.
pub const RULES: &[Rule] = &[
    Rule {
        code: "okf-parse",
        kind: RuleKind::Conformance,
        default_severity: Severity::Error,
        description: "concept documents must contain a parseable YAML frontmatter block",
        spec_section: "§11.1",
    },
    Rule {
        code: "okf-type",
        kind: RuleKind::Conformance,
        default_severity: Severity::Error,
        description: "every frontmatter block must contain a non-empty `type` field",
        spec_section: "§11.2",
    },
    Rule {
        code: "okf-reserved",
        kind: RuleKind::Conformance,
        default_severity: Severity::Error,
        description: "`index.md` and `log.md` must follow their specified structure",
        spec_section: "§11.3",
    },
    Rule {
        code: "broken-link",
        kind: RuleKind::Lint,
        default_severity: Severity::Warning,
        description: "a markdown link points at a concept that does not exist in the bundle",
        spec_section: "§6.1",
    },
    Rule {
        code: "missing-description",
        kind: RuleKind::Lint,
        default_severity: Severity::Warning,
        description: "concepts should carry a one-line `description`",
        spec_section: "§4.1",
    },
    Rule {
        code: "missing-title",
        kind: RuleKind::Lint,
        default_severity: Severity::Info,
        description: "concepts should carry a human-readable `title`",
        spec_section: "§4.1",
    },
    Rule {
        code: "stale-concept",
        kind: RuleKind::Lint,
        default_severity: Severity::Warning,
        description: "the concept is on or past its `stale_after` date",
        spec_section: "§5.5",
    },
    Rule {
        code: "legacy-timestamp",
        kind: RuleKind::Lint,
        default_severity: Severity::Warning,
        description: "`timestamp` is superseded by `generated.at`",
        spec_section: "§13.1",
    },
    Rule {
        code: "legacy-citations",
        kind: RuleKind::Lint,
        default_severity: Severity::Warning,
        description: "a body `# Citations` list is superseded by frontmatter `sources`",
        spec_section: "§13.1",
    },
    Rule {
        code: "unknown-actor",
        kind: RuleKind::Lint,
        default_severity: Severity::Warning,
        description: "actor fields should use `<producer>/<version>`, `human:<id>`, or \
                      `process:<id>`",
        spec_section: "§7",
    },
    Rule {
        code: "unresolved-footnote",
        kind: RuleKind::Lint,
        default_severity: Severity::Warning,
        description: "a footnote label does not match any `sources[].id`",
        spec_section: "§5.1",
    },
    Rule {
        code: "computation-missing-runtime",
        kind: RuleKind::Lint,
        default_severity: Severity::Warning,
        description: "an Attested Computation must declare a `runtime`",
        spec_section: "§10.2",
    },
    Rule {
        code: "source-missing-resource",
        kind: RuleKind::Lint,
        default_severity: Severity::Warning,
        description: "every `sources` entry requires a `resource`",
        spec_section: "§5.1",
    },
    Rule {
        code: "orphan-concept",
        kind: RuleKind::Lint,
        default_severity: Severity::Info,
        description: "no other concept links to this one",
        spec_section: "§6.1",
    },
    Rule {
        code: "missing-index",
        kind: RuleKind::Lint,
        default_severity: Severity::Info,
        description: "a directory of concepts has no `index.md`",
        spec_section: "§8",
    },
    Rule {
        code: "unreachable-path",
        kind: RuleKind::Lint,
        default_severity: Severity::Info,
        description: "a path-valued field points outside the bundle or at a missing file",
        spec_section: "§6.2",
    },
];

/// Looks up a rule by code.
pub fn rule(code: &str) -> Option<&'static Rule> {
    RULES.iter().find(|rule| rule.code == code)
}

/// A single finding against a bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    /// Bundle-relative path of the file the finding is in.
    pub path: PathBuf,
    pub span: Option<Span>,
    pub message: String,
    pub help: Option<String>,
}

impl Diagnostic {
    /// Builds a diagnostic at the rule's default severity.
    ///
    /// # Panics
    ///
    /// Panics if `code` is not in [`RULES`]; codes are `'static` literals, so
    /// an unregistered one is a bug in this crate rather than bad input.
    pub fn new(code: &'static str, path: impl AsRef<Path>, message: impl Into<String>) -> Self {
        let registered = rule(code).unwrap_or_else(|| panic!("unregistered rule code: {code}"));
        Self {
            code,
            severity: registered.default_severity,
            path: path.as_ref().to_path_buf(),
            span: None,
            message: message.into(),
            help: None,
        }
    }

    #[must_use]
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn is_conformance(&self) -> bool {
        rule(self.code).is_some_and(|rule| rule.kind == RuleKind::Conformance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Position;

    #[test]
    fn severities_order_and_render() {
        assert!(Severity::Error > Severity::Warning);
        assert!(Severity::Warning > Severity::Info);
        assert_eq!(Severity::Error.to_string(), "error");
        assert_eq!(Severity::Warning.as_str(), "warning");
        assert_eq!(Severity::Info.as_str(), "info");
    }

    #[test]
    fn exactly_three_conformance_rules_exist() {
        let conformance: Vec<_> =
            RULES.iter().filter(|r| r.kind == RuleKind::Conformance).map(|r| r.code).collect();
        assert_eq!(conformance, ["okf-parse", "okf-type", "okf-reserved"]);
    }

    /// §11 forbids rejecting a bundle for anything outside the three
    /// conformance rules, so no lint rule may default to `Error`.
    #[test]
    fn no_lint_rule_defaults_to_error() {
        for rule in RULES.iter().filter(|r| r.kind == RuleKind::Lint) {
            assert_ne!(rule.default_severity, Severity::Error, "{} defaults to error", rule.code);
        }
    }

    #[test]
    fn conformance_rules_always_default_to_error() {
        for rule in RULES.iter().filter(|r| r.kind == RuleKind::Conformance) {
            assert_eq!(rule.default_severity, Severity::Error, "{}", rule.code);
        }
    }

    #[test]
    fn rule_codes_are_unique_and_documented() {
        let mut codes: Vec<_> = RULES.iter().map(|r| r.code).collect();
        codes.sort_unstable();
        let count = codes.len();
        codes.dedup();
        assert_eq!(codes.len(), count, "duplicate rule codes");

        for rule in RULES {
            assert!(!rule.description.is_empty(), "{} has no description", rule.code);
            assert!(rule.spec_section.starts_with('§'), "{} has no spec section", rule.code);
        }
    }

    #[test]
    fn rule_lookup_finds_registered_codes_only() {
        assert_eq!(rule("okf-type").expect("registered").code, "okf-type");
        assert!(rule("no-such-rule").is_none());
    }

    #[test]
    fn diagnostics_default_to_the_rules_severity() {
        let diagnostic = Diagnostic::new("broken-link", "a/b.md", "dangling");
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert_eq!(diagnostic.path, PathBuf::from("a/b.md"));
        assert_eq!(diagnostic.message, "dangling");
        assert!(diagnostic.span.is_none());
        assert!(diagnostic.help.is_none());
        assert!(!diagnostic.is_conformance());
    }

    #[test]
    fn builders_attach_span_and_help() {
        let span = Span::at(Position::new(3, 1));
        let diagnostic = Diagnostic::new("okf-type", "c.md", "missing type")
            .with_span(span)
            .with_help("add `type:`");
        assert_eq!(diagnostic.span, Some(span));
        assert_eq!(diagnostic.help.as_deref(), Some("add `type:`"));
        assert!(diagnostic.is_conformance());
        assert_eq!(diagnostic.severity, Severity::Error);
    }

    #[test]
    #[should_panic(expected = "unregistered rule code: made-up")]
    fn unregistered_codes_panic() {
        Diagnostic::new("made-up", "x.md", "nope");
    }

    #[test]
    fn is_conformance_is_false_for_unregistered_codes() {
        let mut diagnostic = Diagnostic::new("broken-link", "x.md", "m");
        diagnostic.code = "not-registered";
        assert!(!diagnostic.is_conformance());
    }
}
