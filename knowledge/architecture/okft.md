---
type: Rust Crate
title: okf-toolkit (the okf binary)
description: The command-line front end that wraps the core and renders diagnostics for humans and CI.
resource: https://crates.io/crates/okft
tags: [architecture, crate, cli]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-09T00:00:00Z }
stale_after: 2027-02-09
---

# Responsibility

The binary is deliberately thin. It parses arguments, calls
[okft-core](okft-core.md), applies severity overrides, renders the
result, and picks an exit code. All rule logic lives in the core.

`main.rs` is a three-line shim delegating to `run()`, so the process entry point
carries no logic that integration tests cannot reach.

# Exit codes

| Code | Meaning                                                     |
|------|-------------------------------------------------------------|
| `0`  | No errors. Warnings and infos may still have been reported. |
| `1`  | At least one error. This is what fails CI.                  |
| `2`  | The run could not happen: bad flag, unknown rule, unreadable bundle. |

Separating `2` from `1` matters in CI: a misspelled rule name is a pipeline bug,
not a knowledge-base defect, and the two should not look alike.

# Output formats

`--format` selects `human`, `json`, `github`, or `sarif`. The `github` format
emits workflow commands so findings appear as inline annotations on the pull
request diff; `sarif` feeds GitHub code scanning. See
[the validate command](../commands/validate.md).
