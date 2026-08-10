//! Per-rule severity overrides: `--strict`, `-D`, and `-A`.
//!
//! `-A` wins over `-D` and `--strict`: silencing a rule is the most specific
//! instruction a caller can give, so it is applied last.

use std::collections::BTreeSet;

use okft_core::diagnostic::{Diagnostic, RuleKind, Severity, rule};

#[derive(Debug, Clone, Default)]
pub struct SeverityOverrides {
    denied: BTreeSet<String>,
    allowed: BTreeSet<String>,
    strict: bool,
}

impl SeverityOverrides {
    /// Validates rule codes up front so a typo fails loudly instead of
    /// silently doing nothing.
    ///
    /// # Errors
    ///
    /// Returns a message naming the first unknown rule code.
    pub fn new(deny: &[String], allow: &[String], strict: bool) -> Result<Self, String> {
        for code in deny.iter().chain(allow) {
            if rule(code).is_none() {
                return Err(format!(
                    "unknown rule `{code}`; run `okf rules` to list the available codes"
                ));
            }
        }
        Ok(Self {
            denied: deny.iter().cloned().collect(),
            allowed: allow.iter().cloned().collect(),
            strict,
        })
    }

    /// Applies the overrides, dropping silenced diagnostics.
    ///
    /// Conformance diagnostics are never downgraded or dropped: §11 makes them
    /// requirements, so `-A okf-type` must not be able to make a broken bundle
    /// look conformant.
    pub fn apply(&self, diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
        diagnostics
            .into_iter()
            .filter_map(|mut diagnostic| {
                let is_conformance =
                    rule(diagnostic.code).is_some_and(|rule| rule.kind == RuleKind::Conformance);
                if is_conformance {
                    return Some(diagnostic);
                }

                if self.allowed.contains(diagnostic.code) {
                    return None;
                }
                if self.denied.contains(diagnostic.code)
                    || (self.strict && diagnostic.severity == Severity::Warning)
                {
                    diagnostic.severity = Severity::Error;
                }
                Some(diagnostic)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagnostics() -> Vec<Diagnostic> {
        vec![
            Diagnostic::new("okf-type", "a.md", "missing type"),
            Diagnostic::new("broken-link", "a.md", "dangling"),
            Diagnostic::new("orphan-concept", "b.md", "orphan"),
        ]
    }

    fn overrides(deny: &[&str], allow: &[&str], strict: bool) -> SeverityOverrides {
        let deny: Vec<String> = deny.iter().map(|s| (*s).to_owned()).collect();
        let allow: Vec<String> = allow.iter().map(|s| (*s).to_owned()).collect();
        SeverityOverrides::new(&deny, &allow, strict).expect("valid rules")
    }

    #[test]
    fn without_overrides_nothing_changes() {
        let result = SeverityOverrides::default().apply(diagnostics());
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].severity, Severity::Error);
        assert_eq!(result[1].severity, Severity::Warning);
        assert_eq!(result[2].severity, Severity::Info);
    }

    #[test]
    fn deny_promotes_a_single_rule() {
        let result = overrides(&["orphan-concept"], &[], false).apply(diagnostics());
        assert_eq!(result[1].severity, Severity::Warning);
        assert_eq!(result[2].severity, Severity::Error);
    }

    /// `--strict` promotes warnings only; infos stay advisory so a bundle is
    /// not failed for style-level observations.
    #[test]
    fn strict_promotes_warnings_but_not_infos() {
        let result = overrides(&[], &[], true).apply(diagnostics());
        assert_eq!(result[1].severity, Severity::Error);
        assert_eq!(result[2].severity, Severity::Info);
    }

    #[test]
    fn allow_drops_a_rule_entirely() {
        let result = overrides(&[], &["broken-link"], false).apply(diagnostics());
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|d| d.code != "broken-link"));
    }

    #[test]
    fn allow_beats_deny_and_strict() {
        let result = overrides(&["broken-link"], &["broken-link"], true).apply(diagnostics());
        assert!(result.iter().all(|d| d.code != "broken-link"));
    }

    /// §11 conformance rules are requirements, not preferences.
    #[test]
    fn conformance_rules_cannot_be_silenced_or_downgraded() {
        let result = overrides(&[], &["okf-type"], false).apply(diagnostics());
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].code, "okf-type");
        assert_eq!(result[0].severity, Severity::Error);
    }

    #[test]
    fn unknown_rule_codes_are_rejected() {
        let error = SeverityOverrides::new(&["nope".to_owned()], &[], false).expect_err("rejects");
        assert!(error.contains("unknown rule `nope`"));
        assert!(error.contains("okf rules"));

        assert!(SeverityOverrides::new(&[], &["also-nope".to_owned()], false).is_err());
        assert!(SeverityOverrides::new(&["broken-link".to_owned()], &[], false).is_ok());
    }

    #[test]
    fn an_unregistered_code_is_left_alone() {
        let mut diagnostic = Diagnostic::new("broken-link", "a.md", "m");
        diagnostic.code = "not-registered";
        let result = overrides(&[], &[], true).apply(vec![diagnostic]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].severity, Severity::Error);
    }
}
