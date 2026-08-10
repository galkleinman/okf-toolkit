# okf-toolkit

[![CI](https://github.com/galkleinman/okf-toolkit/actions/workflows/ci.yml/badge.svg)](https://github.com/galkleinman/okf-toolkit/actions/workflows/ci.yml)
[![Knowledge bundle](https://github.com/galkleinman/okf-toolkit/actions/workflows/validate.yml/badge.svg)](https://github.com/galkleinman/okf-toolkit/actions/workflows/validate.yml)
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

## Validate vs. lint: the important part

Most OKF tools treat "things I dislike about your bundle" as conformance
failures. OKF §11 does not allow that. It lists exactly three requirements, and
then explicitly **forbids** consumers from rejecting a bundle for missing
optional fields, unknown `type` values, unknown keys, **broken cross-links**, or
a missing `index.md`. §6.1 is blunt about links in particular: a link to a
concept that does not exist "is not malformed; it may simply represent
not-yet-written knowledge."

So the two tiers are separate here:

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
test in this repository asserts that on every commit. That is the anti-false-positive
net; a rule stricter than the spec is the worst defect this tool could ship.

## GitHub Action

```yaml
- uses: galkleinman/okf-toolkit@v1
  with:
    path: knowledge
```

It downloads a prebuilt static binary (no Rust toolchain, no Docker build) and
annotates findings inline on the pull request diff. Full inputs:

```yaml
- uses: galkleinman/okf-toolkit@v1
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

`trust` is the one worth knowing about. It reports a concept's trust tier,
lifecycle status, staleness, and sources, derived from OKF's provenance
frontmatter, so an agent can check whether a concept is worth quoting before it
does.

Registering it with Claude Code:

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

The crate is `okft`; the command it installs is `okf`. Be aware that the
unrelated [`okf` crate](https://crates.io/crates/okf) also installs a binary
called `okf`, so if you install both, whichever came last wins on your `PATH`.
See [the decision log](knowledge/decisions/binary-naming.md).

## Where this sits in the ecosystem

OKF launched in June 2026 and the space filled up quickly. Worth knowing about:

- [`W4G1/okf`](https://github.com/W4G1/okf) — a zero-dependency pure-Rust OKF library and CLI.
- [`jyjeanne/okf-rs`](https://github.com/jyjeanne/okf-rs) — generates bundles from source code, with its own MCP server.
- [`scaccogatto/okf-skills`](https://github.com/scaccogatto/okf-skills) — Claude Code plugin, agent skills, and an Action.

What this project does differently: it separates §11 conformance from opinion
and refuses to conflate them, ships CI-native output (annotations and SARIF),
serves a viewer that works offline, and proves the whole thing on its own
documentation.

## Development

```sh
cargo test --workspace                                   # 310 tests
cargo clippy --workspace --all-targets -- -D warnings
cargo llvm-cov --workspace --fail-under-lines 100        # the gate CI enforces
```

Coverage is gated at 100% of lines and was wired up before any rule logic
existed, so it shapes the code rather than measuring it after the fact. See
[the coverage decision](knowledge/decisions/coverage-gate.md) for the
`cargo-llvm-cov` monomorphisation gotcha that costs an afternoon if you hit it
unprepared.

Google's four sample bundles are vendored under `tests/fixtures/upstream/`;
refresh them with `scripts/refresh-fixtures.sh`.

## Licence

Apache-2.0, permanently, for everything in this repository. See [LICENSE](LICENSE)
and [NOTICE](NOTICE) for the vendored third-party material.

[spec]: https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md
