//! `okft`: validate, lint, and serve Open Knowledge Format bundles.
//!
//! The command surface mirrors the two-tier design in
//! [`okf_toolkit_core::diagnostic`]: `validate` reports only the three §11
//! conformance rules and is the thing CI gates on, while `lint` adds advisory
//! findings that a caller opts into failing on.

pub mod format;
pub mod severity;

use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use okf_toolkit_core::bundle::Bundle;
use okf_toolkit_core::date::Date;
use okf_toolkit_core::diagnostic::Severity;
use okf_toolkit_core::lint::{LintOptions, lint};
use okf_toolkit_core::{conformance, diagnostic};

use crate::format::{Format, Report};
use crate::severity::SeverityOverrides;

/// Exit code returned when a run produces at least one error.
pub const EXIT_FINDINGS: u8 = 1;
/// Exit code returned when the bundle could not be read at all.
pub const EXIT_USAGE: u8 = 2;

#[derive(Debug, Parser)]
#[command(
    name = "okft",
    version,
    about = "Validate, lint, and serve Open Knowledge Format (OKF) v0.2 bundles",
    long_about = None,
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Check a bundle against the three OKF v0.2 conformance rules (§11).
    Validate(CheckArgs),
    /// Report bundle hygiene beyond conformance.
    Lint(CheckArgs),
    /// List every rule and its default severity.
    Rules {
        /// Emit JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, clap::Args)]
struct CheckArgs {
    /// Path to the bundle directory.
    #[arg(default_value = ".")]
    bundle: PathBuf,

    #[arg(long, value_enum, default_value_t = Format::Human)]
    format: Format,

    /// Treat warnings as errors.
    #[arg(long)]
    strict: bool,

    /// Promote a rule to an error, repeatable (`-D broken-link`).
    #[arg(short = 'D', long = "deny", value_name = "RULE")]
    deny: Vec<String>,

    /// Silence a rule, repeatable (`-A orphan-concept`).
    #[arg(short = 'A', long = "allow", value_name = "RULE")]
    allow: Vec<String>,

    /// Date used for staleness checks, as `YYYY-MM-DD`. Defaults to today (UTC).
    #[arg(long, value_name = "YYYY-MM-DD")]
    today: Option<String>,
}

/// Runs the CLI and returns the process exit code.
pub fn run<I, T>(args: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    match Cli::try_parse_from(args) {
        Ok(cli) => execute(&cli.command),
        Err(error) => {
            // clap renders help and `--version` as "errors"; those exit zero.
            let _ = error.print();
            if error.use_stderr() {
                ExitCode::from(EXIT_USAGE)
            } else {
                ExitCode::SUCCESS
            }
        }
    }
}

fn execute(command: &Command) -> ExitCode {
    match command {
        Command::Validate(args) => check(args, Mode::Validate),
        Command::Lint(args) => check(args, Mode::Lint),
        Command::Rules { json } => {
            print!("{}", render_rules(*json));
            ExitCode::SUCCESS
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Validate,
    Lint,
}

fn check(args: &CheckArgs, mode: Mode) -> ExitCode {
    let overrides = match SeverityOverrides::new(&args.deny, &args.allow, args.strict) {
        Ok(overrides) => overrides,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(EXIT_USAGE);
        }
    };

    let today = match resolve_today(args.today.as_deref()) {
        Ok(today) => today,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(EXIT_USAGE);
        }
    };

    let bundle = match Bundle::load(&args.bundle) {
        Ok(bundle) => bundle,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(EXIT_USAGE);
        }
    };

    let mut diagnostics = conformance::validate(&bundle);
    if mode == Mode::Lint {
        diagnostics.extend(lint(&bundle, &LintOptions { today }));
    }

    let diagnostics = overrides.apply(diagnostics);
    let report = Report {
        diagnostics,
        bundle: args.bundle.display().to_string(),
    };

    let rendered = report.render(args.format);
    print!("{rendered}");

    if report.errors() > 0 {
        ExitCode::from(EXIT_FINDINGS)
    } else {
        ExitCode::SUCCESS
    }
}

/// Parses `--today`, falling back to the current UTC date.
fn resolve_today(value: Option<&str>) -> Result<Date, String> {
    match value {
        Some(text) => {
            Date::parse(text).ok_or_else(|| format!("`--today {text}` is not a YYYY-MM-DD date"))
        }
        None => Ok(current_utc_date()),
    }
}

fn current_utc_date() -> Date {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    Date::from_unix_seconds(i64::try_from(seconds).unwrap_or(0))
}

fn render_rules(json: bool) -> String {
    if json {
        let rules: Vec<_> = diagnostic::RULES
            .iter()
            .map(|rule| {
                serde_json::json!({
                    "code": rule.code,
                    "kind": if rule.kind == diagnostic::RuleKind::Conformance {
                        "conformance"
                    } else {
                        "lint"
                    },
                    "defaultSeverity": rule.default_severity.as_str(),
                    "description": rule.description,
                    "specSection": rule.spec_section,
                })
            })
            .collect();
        return format!(
            "{}\n",
            serde_json::to_string_pretty(&rules).unwrap_or_default()
        );
    }

    let mut out = String::new();
    out.push_str("Conformance rules (§11) always fail `validate`:\n");
    for rule in diagnostic::RULES
        .iter()
        .filter(|r| r.kind == diagnostic::RuleKind::Conformance)
    {
        let _ = writeln!(
            out,
            "  {:<28} {:<8} {}",
            rule.code, rule.spec_section, rule.description
        );
    }
    out.push_str("\nLint rules are advisory unless denied or run with --strict:\n");
    for rule in diagnostic::RULES
        .iter()
        .filter(|r| r.kind == diagnostic::RuleKind::Lint)
    {
        let _ = writeln!(
            out,
            "  {:<28} {:<8} {:<8} {}",
            rule.code,
            rule.spec_section,
            rule.default_severity.as_str(),
            rule.description
        );
    }
    out
}

/// Whether a severity should count as failing.
pub fn is_failing(severity: Severity) -> bool {
    severity == Severity::Error
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unwraps the arguments of a check command, or `None` for `rules`.
    fn check_args(command: Command) -> Option<CheckArgs> {
        match command {
            Command::Validate(args) | Command::Lint(args) => Some(args),
            Command::Rules { .. } => None,
        }
    }

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).expect("parses")
    }

    #[test]
    fn rules_is_not_a_check_command() {
        assert!(check_args(parse(&["okft", "rules"]).command).is_none());
    }

    #[test]
    fn parses_the_validate_command_with_defaults() {
        let args = check_args(parse(&["okft", "validate"]).command).expect("check command");
        assert_eq!(args.bundle, PathBuf::from("."));
        assert_eq!(args.format, Format::Human);
        assert!(!args.strict);
        assert!(args.deny.is_empty());
        assert!(args.allow.is_empty());
        assert!(args.today.is_none());
    }

    #[test]
    fn parses_every_check_flag() {
        let args = check_args(
            parse(&[
                "okft",
                "lint",
                "./knowledge",
                "--format",
                "sarif",
                "--strict",
                "-D",
                "broken-link",
                "-A",
                "orphan-concept",
                "--today",
                "2026-08-09",
            ])
            .command,
        )
        .expect("check command");
        assert_eq!(args.bundle, PathBuf::from("./knowledge"));
        assert_eq!(args.format, Format::Sarif);
        assert!(args.strict);
        assert_eq!(args.deny, ["broken-link"]);
        assert_eq!(args.allow, ["orphan-concept"]);
        assert_eq!(args.today.as_deref(), Some("2026-08-09"));
    }

    #[test]
    fn rejects_unknown_commands() {
        assert!(Cli::try_parse_from(["okft", "frobnicate"]).is_err());
    }

    #[test]
    fn resolves_today_from_the_flag_or_the_clock() {
        assert_eq!(
            resolve_today(Some("2026-08-09")),
            Ok(Date::parse("2026-08-09").expect("d"))
        );
        assert!(resolve_today(Some("09/08/2026")).is_err());
        // The clock-derived date must at least be after the epoch.
        assert!(resolve_today(None).expect("current date") > Date::parse("2020-01-01").expect("d"));
    }

    #[test]
    fn current_date_is_sane() {
        let today = current_utc_date();
        assert!(today > Date::parse("2024-01-01").expect("d"));
        assert!(today < Date::parse("2100-01-01").expect("d"));
    }

    #[test]
    fn renders_the_rule_table_and_json() {
        let table = render_rules(false);
        for rule in diagnostic::RULES {
            assert!(
                table.contains(rule.code),
                "{} missing from table",
                rule.code
            );
        }
        assert!(table.contains("Conformance rules"));

        let json: serde_json::Value =
            serde_json::from_str(&render_rules(true)).expect("valid JSON");
        assert_eq!(
            json.as_array().expect("array").len(),
            diagnostic::RULES.len()
        );
        assert_eq!(json[0]["kind"], "conformance");
    }

    #[test]
    fn only_errors_are_failing() {
        assert!(is_failing(Severity::Error));
        assert!(!is_failing(Severity::Warning));
        assert!(!is_failing(Severity::Info));
    }
}
