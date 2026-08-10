# okf-toolkit

[![CI](https://github.com/galkleinman/okf-toolkit/actions/workflows/ci.yml/badge.svg)](https://github.com/galkleinman/okf-toolkit/actions/workflows/ci.yml)
[![Knowledge bundle](https://github.com/galkleinman/okf-toolkit/actions/workflows/validate.yml/badge.svg)](https://github.com/galkleinman/okf-toolkit/actions/workflows/validate.yml)
[![codecov](https://codecov.io/gh/galkleinman/okf-toolkit/branch/main/graph/badge.svg)](https://codecov.io/gh/galkleinman/okf-toolkit)
[![crates.io](https://img.shields.io/crates/v/okft.svg)](https://crates.io/crates/okft)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

A single-binary toolkit for [Google's Open Knowledge Format][spec] (OKF) v0.2:
**validate** bundles in CI, **lint** them for hygiene, and **serve** them to AI
agents over MCP or to a browser as a graph.

This repository documents itself as an OKF bundle in [`/knowledge`](knowledge/),
and its CI validates that bundle using the binary it ships.

## Quickstart

```sh
cargo install okft          # installs a binary named `okf`

okf validate ./knowledge          # OKF §11 conformance; exits non-zero on error
okf lint ./knowledge --strict     # hygiene too, warnings promoted to errors
okf rules                         # every rule, its tier, and its severity
```

Point a coding agent at a knowledge base:

```sh
okf serve --mcp ./knowledge       # Model Context Protocol, on stdio
```

Browse it as a graph, with no network access required:

```sh
okf serve --web ./knowledge       # http://127.0.0.1:7878
```

## Conformance and linting

OKF §11 lists exactly three conformance requirements, and then explicitly
**forbids** consumers from rejecting a bundle for missing optional fields,
unknown `type` values, unknown keys, **broken cross-links**, or a missing
`index.md`. §6.1 is specific about links: one pointing at a concept that does
not exist "is not malformed; it may simply represent not-yet-written knowledge."

Conformance and opinion are therefore kept apart:

| | Rules | Fails the build? |
|---|---|---|
| `okf validate` | `okf-parse`, `okf-type`, `okf-reserved` | Always, on any error |
| `okf lint` | 13 advisory rules, each with a stable code | Only with `--strict` or `-D <rule>` |

```sh
okf validate ./bundle           # a broken link does NOT fail this
okf lint ./bundle --strict      # …but it does fail this
okf lint ./bundle -D broken-link   # or gate on just that one rule
```

`-A` silences a lint rule, but it **cannot** silence a conformance rule: no
configuration should be able to make a genuinely broken bundle look clean.

All four of Google's published sample bundles validate with zero errors, and a
test asserts that on every commit. A rule stricter than the specification is the
worst defect a validator can ship, so that test is the guard against it.

## GitHub Action

```yaml
- uses: galkleinman/okf-toolkit@v0
  with:
    path: knowledge
```

It downloads a prebuilt static binary (no Rust toolchain, no Docker build) and
annotates findings inline on the pull request diff. Full inputs:

```yaml
- uses: galkleinman/okf-toolkit@v0
  with:
    path: knowledge        # bundle directory
    command: lint          # `validate` (default) or `lint`
    strict: "true"         # promote lint warnings to errors
    deny: broken-link      # space-separated rule codes to fail on
    allow: orphan-concept  # space-separated rule codes to silence
    today: "2026-08-09"    # pin the date staleness is judged against
    format: github         # `github`, `human`, `json`, or `sarif`
```

## Output formats

`--format` selects how findings are rendered:

- `human` — grouped by file, with line numbers and help text.
- `github` — workflow commands, so findings appear as inline PR annotations.
- `sarif` — SARIF 2.1.0 for GitHub code scanning.
- `json` — for anything else.

## Serving a bundle to agents

`okf serve --mcp` exposes concepts as MCP **resources** (`okf://<concept-id>`)
and as **tools**: `search`, `list`, `read`, `related`, and `trust`. Both, because
many MCP clients never surface resources to the model without an explicit user
action, so a resource-only server looks correct and does nothing useful.

`trust` reports a concept's trust tier, lifecycle status, staleness, and
sources, derived from OKF's provenance frontmatter, so an agent can check
whether a concept is worth quoting before it does.

The server speaks stdio, so it registers with any MCP client:

```json
{
  "mcpServers": {
    "knowledge": { "command": "okf", "args": ["serve", "--mcp", "./knowledge"] }
  }
}
```

## Crates

| Crate | Purpose |
|---|---|
| [`okft`](crates/okft) | The `okf` binary |
| [`okft-core`](crates/okft-core) | Parsing, conformance, lint, link graph. No CLI or server dependencies |
| [`okft-mcp`](crates/okft-mcp) | MCP server over a bundle |
| [`okft-web`](crates/okft-web) | Self-contained local graph viewer |

The crate is `okft`; the command it installs is `okf`. An unrelated crate named
`okf` also installs a binary of that name, so installing both leaves whichever
came last on your `PATH`.

## Development

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo llvm-cov --workspace --fail-under-lines 100
```

Coverage is gated at 100% of lines. `cargo llvm-cov --fail-under-lines 100` is
the gate; the Codecov upload is reporting only and is deliberately non-fatal, so
a reporting outage cannot fail a build whose coverage passed.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the commit convention and the release
process. See
[the coverage decision](knowledge/decisions/coverage-gate.md) for the
`cargo-llvm-cov` monomorphisation gotcha that costs an afternoon if you hit it
unprepared.

Google's four sample bundles are vendored under `tests/fixtures/upstream/`;
refresh them with `scripts/refresh-fixtures.sh`.

## Licence

Apache-2.0, permanently, for everything in this repository. See [LICENSE](LICENSE)
and [NOTICE](NOTICE) for the vendored third-party material.

[spec]: https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md
