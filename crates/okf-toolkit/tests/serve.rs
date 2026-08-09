//! End-to-end tests for `okft serve`.
//!
//! These spawn the real binary because the point is the process-level
//! behaviour: which transport it picks, what it writes to stdout versus
//! stderr, and how it exits.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use assert_cmd::cargo::CommandCargoExt as _;
use predicates::prelude::*;

const EXIT_USAGE: i32 = 2;

fn okft() -> Command {
    Command::cargo_bin("okft").expect("binary builds")
}

fn assert_cmd_okft() -> assert_cmd::Command {
    assert_cmd::Command::cargo_bin("okft").expect("binary builds")
}

/// Carries every v0.2 family, so serving it exercises the full graph builder
/// rather than only the paths a bare concept reaches.
fn bundle() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("index.md"), "# Bundle\n\n* [a](a.md)\n").expect("write");
    std::fs::write(
        dir.path().join("a.md"),
        "---\ntype: Metric\ntitle: A metric\ndescription: Something measurable.\n\
         resource: https://example.com/metric\ntags: [finance, revenue]\nstatus: stable\n\
         generated: { by: agent/v1, at: 2026-01-01T00:00:00Z }\n\
         verified:\n  - { by: human:gal, at: 2026-01-02T00:00:00Z }\n\
         stale_after: 2026-12-31\n\
         sources:\n  - id: s1\n    resource: https://example.com\n    title: A source\n    \
         author: team:data\n    usage_count: 7\n    last_modified: 2026-01-01\n---\n\n\
         Links to [b](/b.md).\n",
    )
    .expect("write");
    std::fs::write(
        dir.path().join("b.md"),
        "---\ntype: Metric\ntitle: B metric\ndescription: Another one.\n---\n",
    )
    .expect("write");
    // No title, so the viewer must fall back to the concept id.
    std::fs::write(dir.path().join("untitled.md"), "---\ntype: Metric\n---\n").expect("write");
    // Unparseable frontmatter: §11 does not let the viewer refuse to draw it.
    std::fs::write(dir.path().join("broken.md"), "---\n[unclosed\n---\nbody\n").expect("write");
    dir
}

#[test]
fn serve_requires_a_transport() {
    let dir = bundle();
    assert_cmd_okft()
        .arg("serve")
        .arg(dir.path())
        .assert()
        .code(EXIT_USAGE)
        .stderr(predicate::str::contains("--mcp").and(predicate::str::contains("--web")));
}

#[test]
fn mcp_and_web_are_mutually_exclusive() {
    let dir = bundle();
    assert_cmd_okft()
        .args(["serve", "--mcp", "--web"])
        .arg(dir.path())
        .assert()
        .code(EXIT_USAGE);
}

#[test]
fn serve_rejects_a_missing_bundle() {
    assert_cmd_okft()
        .args(["serve", "--mcp", "/definitely/not/a/bundle"])
        .assert()
        .code(EXIT_USAGE)
        .stderr(predicate::str::contains("is not a directory"));
}

#[test]
fn serve_rejects_a_malformed_today() {
    let dir = bundle();
    assert_cmd_okft()
        .args(["serve", "--mcp", "--today", "nope"])
        .arg(dir.path())
        .assert()
        .code(EXIT_USAGE)
        .stderr(predicate::str::contains("not a YYYY-MM-DD date"));
}

#[test]
fn serve_web_rejects_an_unusable_address() {
    let dir = bundle();
    assert_cmd_okft()
        .args(["serve", "--web", "--addr", "256.256.256.256:1"])
        .arg(dir.path())
        .assert()
        .code(EXIT_USAGE)
        .stderr(predicate::str::contains("could not bind"));
}

/// Port 0 lets the OS choose, so the test never collides with a busy port.
///
/// The server is interrupted rather than killed so it exits through its own
/// graceful-shutdown path, which is what the test is really checking.
#[cfg(unix)]
#[test]
fn serve_web_binds_and_serves_the_viewer() {
    let dir = bundle();
    let mut child = okft()
        .args(["serve", "--web", "--addr", "127.0.0.1:0"])
        .arg(dir.path())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawns");

    let stdout = child.stdout.take().expect("stdout");
    let mut line = String::new();
    BufReader::new(stdout)
        .read_line(&mut line)
        .expect("reads the address line");

    assert!(line.contains("4 concept(s)"), "unexpected banner: {line}");
    assert!(
        line.contains("self-contained"),
        "banner should report the page size: {line}"
    );
    // The banner is `... at http://HOST:PORT (N KiB, self-contained)`.
    let address = line
        .split("http://")
        .nth(1)
        .expect("an address")
        .split_whitespace()
        .next()
        .expect("a host:port")
        .to_owned();

    let response = minimal_http_get(&address, "/");
    let health = minimal_http_get(&address, "/healthz");

    interrupt(&child);
    let status = child.wait().expect("reaps the server");
    assert!(
        status.success(),
        "graceful shutdown should exit zero, got {status}"
    );

    assert!(
        response.contains("200 OK"),
        "unexpected response: {}",
        &response[..60.min(response.len())]
    );
    assert!(response.contains("OKF Bundle Viewer"));
    assert!(
        !response.contains("cdn.jsdelivr.net"),
        "the served page loaded a CDN"
    );
    assert!(
        health.trim_end().ends_with("ok"),
        "health check failed: {health}"
    );
}

/// Sends SIGINT, which the server handles as a graceful shutdown.
#[cfg(unix)]
fn interrupt(child: &std::process::Child) {
    let status = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .expect("sends SIGINT");
    assert!(status.success(), "kill -INT failed");
}

/// Speaks just enough HTTP to fetch one page, so the test needs no client crate.
fn minimal_http_get(address: &str, path: &str) -> String {
    use std::io::Read as _;
    let mut stream = std::net::TcpStream::connect(address).expect("connects");
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .expect("sends the request");
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .expect("reads the response");
    String::from_utf8_lossy(&response).into_owned()
}

/// The MCP transport owns stdout, so the banner must go to stderr; anything
/// else would corrupt the JSON-RPC stream.
#[test]
fn serve_mcp_speaks_json_rpc_on_stdout_and_status_on_stderr() {
    let dir = bundle();
    let mut child = okft()
        .args(["serve", "--mcp", "--today", "2026-08-09"])
        .arg(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawns");

    let mut stdin = child.stdin.take().expect("stdin");
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2024-11-05","capabilities":{{}},"clientInfo":{{"name":"test","version":"1"}}}}}}"#
    )
    .expect("sends initialize");
    stdin.flush().expect("flushes");

    let stdout = child.stdout.take().expect("stdout");
    let mut line = String::new();
    BufReader::new(stdout)
        .read_line(&mut line)
        .expect("reads a response");

    // Closing stdin ends the JSON-RPC stream, so the server exits on its own
    // and flushes coverage; killing it would skip both.
    drop(stdin);
    let output = child.wait_with_output().expect("reaps");
    assert!(
        output.status.success(),
        "clean EOF should exit zero, got {}",
        output.status
    );

    let response: serde_json::Value = serde_json::from_str(line.trim())
        .unwrap_or_else(|e| panic!("stdout was not JSON-RPC: {e}: {line}"));
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["result"]["serverInfo"]["name"], "okft");
    assert!(response["result"]["capabilities"]["tools"].is_object());
    assert!(response["result"]["capabilities"]["resources"].is_object());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("over MCP on stdio"),
        "status banner missing from stderr: {stderr}"
    );
}
