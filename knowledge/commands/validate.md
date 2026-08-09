---
type: CLI Command
title: okft validate
description: Checks a bundle against the three OKF v0.2 conformance rules and exits non-zero on any error.
tags: [command, validation, ci]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-09T00:00:00Z }
stale_after: 2027-02-09
---

# Synopsis

```
okft validate [BUNDLE] [--format human|json|github|sarif] [--strict]
              [-D RULE] [-A RULE] [--today YYYY-MM-DD]
```

`BUNDLE` defaults to the current directory.

# Behaviour

Reports only the three conformance rules described in
[conformance vs lint](../decisions/conformance-vs-lint.md). Exits `1` if any
error was reported, `2` if the run could not happen at all, and `0` otherwise.

A broken cross-link does not fail this command. That is deliberate, not an
oversight; use [okft lint](lint.md) with `--strict` if you want to gate on links.

# Examples

```sh
okft validate ./knowledge
okft validate ./knowledge --format github     # inline PR annotations
okft validate ./knowledge --format sarif > results.sarif
```

# Flags

| Flag              | Effect                                                        |
|-------------------|---------------------------------------------------------------|
| `--format`        | Output rendering. `github` emits workflow commands.           |
| `--strict`        | Promotes warnings to errors. No effect unless linting.        |
| `-D`, `--deny`    | Promotes one rule to an error. Repeatable.                    |
| `-A`, `--allow`   | Silences one rule. Cannot silence a conformance rule.         |
| `--today`         | Pins the date staleness is judged against, for reproducibility.|
