//! `okf`: validate, lint, and serve Open Knowledge Format bundles.
//!
//! The command surface mirrors the two-tier design in
//! [`okft_core::diagnostic`]: `validate` reports only the three §11
//! conformance rules and is the thing CI gates on, while `lint` adds advisory
//! findings that a caller opts into failing on.

pub mod format;
pub mod severity;

use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use okft_core::bundle::Bundle;
use okft_core::date::Date;
use okft_core::diagnostic::Severity;
use okft_core::lint::{LintOptions, lint};
use okft_core::{conformance, diagnostic};

use crate::format::{Format, Report};
use crate::severity::SeverityOverrides;

/// Exit code returned when a run produces at least one error.
pub const EXIT_FINDINGS: u8 = 1;
/// Exit code returned when the bundle could not be read at all.
pub const EXIT_USAGE: u8 = 2;

#[derive(Debug, Parser)]
#[command(
    name = "okf",
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
    /// Serve a bundle to AI agents over MCP, or to a browser as a graph.
    Serve(ServeArgs),
}

#[derive(Debug, clap::Args)]
struct ServeArgs {
    /// Path to the bundle directory.
    #[arg(default_value = ".")]
    bundle: PathBuf,

    /// Serve the bundle over the Model Context Protocol on stdio.
    #[arg(long, conflicts_with = "web")]
    mcp: bool,

    /// Serve a local graph viewer over HTTP.
    #[arg(long, conflicts_with = "mcp")]
    web: bool,

    /// Address to bind the web viewer to.
    #[arg(long, default_value = "127.0.0.1:7878")]
    addr: String,

    /// Date used for staleness reporting, as `YYYY-MM-DD`. Defaults to today (UTC).
    #[arg(long, value_name = "YYYY-MM-DD")]
    today: Option<String>,
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
        Command::Serve(args) => serve::run(args),
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
            Command::Rules { .. } | Command::Serve(_) => None,
        }
    }

    /// Unwraps the arguments of a serve command, or `None` for anything else.
    fn serve_args(command: Command) -> Option<ServeArgs> {
        match command {
            Command::Serve(args) => Some(args),
            Command::Validate(_) | Command::Lint(_) | Command::Rules { .. } => None,
        }
    }

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).expect("parses")
    }

    #[test]
    fn commands_are_told_apart() {
        assert!(check_args(parse(&["okf", "rules"]).command).is_none());
        assert!(check_args(parse(&["okf", "serve", "--mcp"]).command).is_none());
        assert!(serve_args(parse(&["okf", "rules"]).command).is_none());
        assert!(serve_args(parse(&["okf", "validate"]).command).is_none());
        assert!(serve_args(parse(&["okf", "lint"]).command).is_none());
    }

    #[test]
    fn parses_the_serve_command() {
        let cli = parse(&[
            "okf",
            "serve",
            "./knowledge",
            "--web",
            "--addr",
            "0.0.0.0:9000",
            "--today",
            "2026-08-09",
        ]);
        let args = serve_args(cli.command).expect("serve command");
        assert_eq!(args.bundle, PathBuf::from("./knowledge"));
        assert!(args.web);
        assert!(!args.mcp);
        assert_eq!(args.addr, "0.0.0.0:9000");
        assert_eq!(args.today.as_deref(), Some("2026-08-09"));
    }

    #[test]
    fn serve_defaults_to_a_loopback_address() {
        let cli = parse(&["okf", "serve", "--mcp"]);
        let args = serve_args(cli.command).expect("serve command");
        assert_eq!(args.addr, "127.0.0.1:7878");
        assert_eq!(args.bundle, PathBuf::from("."));
        assert!(args.mcp);
    }

    #[test]
    fn mcp_and_web_conflict() {
        assert!(Cli::try_parse_from(["okf", "serve", "--mcp", "--web"]).is_err());
    }

    #[test]
    fn parses_the_validate_command_with_defaults() {
        let args = check_args(parse(&["okf", "validate"]).command).expect("check command");
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
                "okf",
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
        assert!(Cli::try_parse_from(["okf", "frobnicate"]).is_err());
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

/// Serving a bundle over MCP or HTTP.
mod serve {
    use std::process::ExitCode;

    use okft_core::bundle::Bundle;
    use okft_mcp::OkfServer;
    use okft_web::Viewer;
    use rmcp::ServiceExt as _;

    use super::{EXIT_USAGE, ServeArgs, resolve_today};

    pub(super) fn run(args: &ServeArgs) -> ExitCode {
        if !args.mcp && !args.web {
            eprintln!("error: choose a transport: `--mcp` for agents, or `--web` for a browser");
            return ExitCode::from(EXIT_USAGE);
        }

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

        // The remaining failures below are all "the OS refused something the
        // process cannot continue without". They panic with a clear message
        // rather than being handled as ordinary errors, because there is no
        // useful recovery and no way to exercise them from a test.
        let runtime = tokio::runtime::Runtime::new().expect("failed to start the async runtime");

        if args.mcp {
            runtime.block_on(serve_mcp(bundle, today))
        } else {
            let name = args.bundle.display().to_string();
            runtime.block_on(serve_web(bundle, name, today, &args.addr))
        }
    }

    /// Serves MCP on stdio.
    ///
    /// Nothing may be written to stdout here: it carries the JSON-RPC stream,
    /// so any stray print would corrupt the protocol. Status goes to stderr.
    async fn serve_mcp(bundle: Bundle, today: okft_core::date::Date) -> ExitCode {
        eprintln!(
            "okf: serving {} concept(s) over MCP on stdio",
            bundle.concepts().count()
        );

        let server = OkfServer::new(bundle, today);
        let running = server
            .serve(rmcp::transport::io::stdio())
            .await
            .expect("failed to start the MCP server on stdio");
        running
            .waiting()
            .await
            .expect("the MCP session ended badly");
        ExitCode::SUCCESS
    }

    async fn serve_web(
        bundle: Bundle,
        name: String,
        today: okft_core::date::Date,
        addr: &str,
    ) -> ExitCode {
        let concepts = bundle.concepts().count();
        let viewer = Viewer::render(&bundle, &name, today);
        // The page embeds Cytoscape, Marked, and the bundle data, so reporting
        // its size makes it obvious that nothing else will be fetched.
        let page_kib = viewer.html().len() / 1024;
        let router = viewer.router();

        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!("error: could not bind {addr}: {error}");
                return ExitCode::from(EXIT_USAGE);
            }
        };

        let bound = listener
            .local_addr()
            .expect("a bound listener always has an address");
        println!(
            "okf: serving {concepts} concept(s) at http://{bound} ({page_kib} KiB, self-contained)"
        );

        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await
            .expect("the web server stopped unexpectedly");
        ExitCode::SUCCESS
    }
}
