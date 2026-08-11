---
type: CLI Command
title: okf validate
description: Checks a bundle against the three OKF v0.2 conformance rules and exits non-zero on any error.
tags: [command, validation, ci]
status: stable
stale_after: 2027-02-09
---

# Synopsis

```
okf validate [BUNDLE] [--format human|json|github|sarif] [--strict]
              [-D RULE] [-A RULE] [--today YYYY-MM-DD] [--okf-version 0.1|0.2]
```

`BUNDLE` defaults to the current directory.

# Behaviour

Reports only the three conformance rules described in
[conformance vs lint](../decisions/conformance-vs-lint.md). Exits `1` if any
error was reported, `2` if the run could not happen at all, and `0` otherwise.

A broken cross-link does not fail this command. That is deliberate, not an
oversight; use [okf lint](lint.md) with `--strict` if you want to gate on links.

The result does not depend on `--okf-version`: §11 states the same three
requirements in every revision. The flag is accepted so one set of arguments
works for both commands, and it only changes what [okf lint](lint.md) reports.
See [the versioning decision](../decisions/okf-versions.md).

# Examples

```sh
okf validate ./knowledge
okf validate ./knowledge --format github     # inline PR annotations
okf validate ./knowledge --format sarif > results.sarif
```

# Flags

| Flag              | Effect                                                        |
|-------------------|---------------------------------------------------------------|
| `--format`        | Output rendering. `github` emits workflow commands.           |
| `--strict`        | Promotes warnings to errors. No effect unless linting.        |
| `-D`, `--deny`    | Promotes one rule to an error. Repeatable.                    |
| `-A`, `--allow`   | Silences one rule. Cannot silence a conformance rule.         |
| `--today`         | Pins the date staleness is judged against, for reproducibility.|
| `--okf-version`   | OKF revision to check against. No effect unless linting.      |
