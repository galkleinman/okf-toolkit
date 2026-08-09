//! Rendering diagnostics for humans and for CI.

use std::fmt::Write as _;

use okf_toolkit_core::diagnostic::{Diagnostic, Severity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// Grouped by file, with spans and help text.
    Human,
    /// One JSON object with a `diagnostics` array.
    Json,
    /// GitHub Actions workflow commands, which render as inline annotations.
    Github,
    /// SARIF 2.1.0, for GitHub code scanning and other analysis tooling.
    Sarif,
}

/// What a run produced, independent of how it is rendered.
#[derive(Debug, Clone)]
pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
    /// Bundle path as the user typed it, for display only.
    pub bundle: String,
}

impl Report {
    pub fn errors(&self) -> usize {
        self.count(Severity::Error)
    }

    pub fn warnings(&self) -> usize {
        self.count(Severity::Warning)
    }

    pub fn infos(&self) -> usize {
        self.count(Severity::Info)
    }

    fn count(&self, severity: Severity) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == severity)
            .count()
    }

    pub fn render(&self, format: Format) -> String {
        match format {
            Format::Human => self.render_human(),
            Format::Json => self.render_json(),
            Format::Github => self.render_github(),
            Format::Sarif => self.render_sarif(),
        }
    }

    fn render_human(&self) -> String {
        if self.diagnostics.is_empty() {
            return format!("{}: no findings\n", self.bundle);
        }

        let mut out = String::new();
        let mut current_file = None;
        for diagnostic in &self.diagnostics {
            let path = diagnostic.path.display().to_string();
            if current_file.as_deref() != Some(path.as_str()) {
                if current_file.is_some() {
                    out.push('\n');
                }
                let _ = writeln!(out, "{path}");
                current_file = Some(path);
            }

            let location = diagnostic.span.map_or_else(
                || "   ".to_owned(),
                |span| format!("{:>3}", span.start.line),
            );
            let _ = writeln!(
                out,
                "  {location}  {:<7} {}  {}",
                diagnostic.severity, diagnostic.code, diagnostic.message
            );
            if let Some(help) = &diagnostic.help {
                let _ = writeln!(out, "         help: {help}");
            }
        }

        let _ = writeln!(
            out,
            "\n{} error(s), {} warning(s), {} info(s)",
            self.errors(),
            self.warnings(),
            self.infos()
        );
        out
    }

    fn render_json(&self) -> String {
        let diagnostics: Vec<_> = self
            .diagnostics
            .iter()
            .map(|d| {
                serde_json::json!({
                    "code": d.code,
                    "severity": d.severity.as_str(),
                    "path": d.path.display().to_string(),
                    "line": d.span.map(|s| s.start.line),
                    "column": d.span.map(|s| s.start.column),
                    "message": d.message,
                    "help": d.help,
                })
            })
            .collect();

        let document = serde_json::json!({
            "bundle": self.bundle,
            "summary": {
                "errors": self.errors(),
                "warnings": self.warnings(),
                "infos": self.infos(),
            },
            "diagnostics": diagnostics,
        });
        format!(
            "{}\n",
            serde_json::to_string_pretty(&document).unwrap_or_default()
        )
    }

    /// GitHub Actions workflow commands.
    ///
    /// The runner turns these into inline annotations on the PR diff, which is
    /// what makes a failing bundle visible where the change was made.
    fn render_github(&self) -> String {
        let mut out = String::new();
        for diagnostic in &self.diagnostics {
            let level = match diagnostic.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "notice",
            };
            let _ = write!(out, "::{level} file={}", diagnostic.path.display());
            if let Some(span) = diagnostic.span {
                let _ = write!(out, ",line={},col={}", span.start.line, span.start.column);
            }
            let _ = writeln!(
                out,
                ",title={}::{}",
                diagnostic.code,
                escape_workflow_data(&diagnostic.message)
            );
        }
        out
    }

    fn render_sarif(&self) -> String {
        let rules: Vec<_> = okf_toolkit_core::diagnostic::RULES
            .iter()
            .map(|rule| {
                serde_json::json!({
                    "id": rule.code,
                    "shortDescription": { "text": rule.description },
                    "helpUri": SPEC_URL,
                    "properties": { "specSection": rule.spec_section },
                })
            })
            .collect();

        let results: Vec<_> = self
            .diagnostics
            .iter()
            .map(|d| {
                let region = d.span.map(|span| {
                    serde_json::json!({
                        "startLine": span.start.line,
                        "startColumn": span.start.column,
                    })
                });
                serde_json::json!({
                    "ruleId": d.code,
                    "level": match d.severity {
                        Severity::Error => "error",
                        Severity::Warning => "warning",
                        Severity::Info => "note",
                    },
                    "message": { "text": d.message },
                    "locations": [{
                        "physicalLocation": {
                            "artifactLocation": { "uri": d.path.display().to_string() },
                            "region": region,
                        }
                    }],
                })
            })
            .collect();

        let document = serde_json::json!({
            "version": "2.1.0",
            "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
            "runs": [{
                "tool": {
                    "driver": {
                        "name": "okft",
                        "informationUri": "https://github.com/galkleinman/okf-toolkit",
                        "version": env!("CARGO_PKG_VERSION"),
                        "rules": rules,
                    }
                },
                "results": results,
            }],
        });
        format!(
            "{}\n",
            serde_json::to_string_pretty(&document).unwrap_or_default()
        )
    }
}

const SPEC_URL: &str =
    "https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md";

/// Escapes the characters GitHub treats as delimiters in workflow commands.
fn escape_workflow_data(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

#[cfg(test)]
mod tests {
    use okf_toolkit_core::span::{Position, Span};

    use super::*;

    fn report() -> Report {
        Report {
            bundle: "./knowledge".to_owned(),
            diagnostics: vec![
                Diagnostic::new(
                    "okf-type",
                    "a.md",
                    "frontmatter is missing the required `type` field",
                )
                .with_span(Span::at(Position::new(2, 1)))
                .with_help("every concept needs a `type`"),
                Diagnostic::new(
                    "broken-link",
                    "a.md",
                    "link target `/gone.md` does not exist",
                )
                .with_span(Span::at(Position::new(7, 5))),
                Diagnostic::new("orphan-concept", "b.md", "no concept links to `b`"),
            ],
        }
    }

    fn empty_report() -> Report {
        Report {
            bundle: "./knowledge".to_owned(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn counts_by_severity() {
        let report = report();
        assert_eq!(report.errors(), 1);
        assert_eq!(report.warnings(), 1);
        assert_eq!(report.infos(), 1);
    }

    #[test]
    fn human_output_groups_by_file() {
        insta::assert_snapshot!(report().render(Format::Human));
    }

    #[test]
    fn human_output_says_so_when_clean() {
        assert_eq!(
            empty_report().render(Format::Human),
            "./knowledge: no findings\n"
        );
    }

    #[test]
    fn json_output_is_stable() {
        insta::assert_snapshot!(report().render(Format::Json));
    }

    #[test]
    fn github_output_is_stable() {
        insta::assert_snapshot!(report().render(Format::Github));
    }

    #[test]
    fn sarif_output_is_valid_and_stable() {
        let rendered = report().render(Format::Sarif);
        let parsed: serde_json::Value = serde_json::from_str(&rendered).expect("valid JSON");
        assert_eq!(parsed["version"], "2.1.0");
        assert_eq!(
            parsed["runs"][0]["results"]
                .as_array()
                .expect("results")
                .len(),
            3
        );
        assert_eq!(
            parsed["runs"][0]["tool"]["driver"]["rules"]
                .as_array()
                .expect("rules")
                .len(),
            okf_toolkit_core::diagnostic::RULES.len()
        );
        assert_eq!(
            parsed["runs"][0]["results"][2]["locations"][0]["physicalLocation"]["region"],
            serde_json::Value::Null
        );
    }

    #[test]
    fn empty_reports_render_in_every_format() {
        let report = empty_report();
        assert!(report.render(Format::Github).is_empty());
        for format in [Format::Json, Format::Sarif] {
            let parsed: serde_json::Value =
                serde_json::from_str(&report.render(format)).expect("valid JSON");
            assert!(parsed.is_object());
        }
    }

    #[test]
    fn workflow_data_is_escaped() {
        assert_eq!(escape_workflow_data("100% done"), "100%25 done");
        assert_eq!(escape_workflow_data("a\nb"), "a%0Ab");
        assert_eq!(escape_workflow_data("a\r\nb"), "a%0D%0Ab");
        assert_eq!(escape_workflow_data("plain"), "plain");
    }

    #[test]
    fn github_annotations_omit_position_when_absent() {
        let rendered = Report {
            bundle: "b".to_owned(),
            diagnostics: vec![Diagnostic::new("orphan-concept", "b.md", "no links")],
        }
        .render(Format::Github);
        assert_eq!(
            rendered,
            "::notice file=b.md,title=orphan-concept::no links\n"
        );
    }
}
