---
type: Rust Crate
title: okft-core
description: Parsing, conformance validation, linting, and the link graph, with no CLI or server dependencies.
resource: https://crates.io/crates/okft-core
tags: [architecture, crate, validation]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-09T00:00:00Z }
stale_after: 2027-02-09
sources:
  - id: okf-spec
    resource: https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md
    title: Open Knowledge Format specification v0.2
    author: team:google-cloud
    last_modified: 2026-07-01
---

# Responsibility

`okft-core` owns everything that understands OKF. It reads a directory of
markdown files, splits frontmatter from body, applies the conformance and lint
rules, and exposes the cross-link graph. It has no dependency on `clap`, `axum`,
or `rmcp`, so the [MCP server](okft-mcp.md) and the
[web viewer](okft-web.md) reuse it directly rather than shelling out to
the [CLI](okft.md).

# Modules

| Module        | Responsibility                                                     |
|---------------|--------------------------------------------------------------------|
| `value`       | An order-preserving, span-annotated YAML tree.                     |
| `document`    | Splitting a file into frontmatter and body.                        |
| `frontmatter` | Typed accessors over the provenance, trust, and lifecycle families.|
| `bundle`      | Loading a directory tree and resolving the link graph.             |
| `links`       | Markdown link, footnote, and heading extraction.                   |
| `conformance` | The three rules that can fail a bundle.                            |
| `lint`        | The advisory rules that cannot.                                    |
| `diagnostic`  | Severities and the rule registry.                                  |

# Design notes

Frontmatter is modelled as a generic tree rather than a fixed struct because the
spec lets producers add arbitrary keys and asks consumers to preserve unknown
ones when round-tripping.[^okf-spec] Every accessor is tolerant: a field of the
wrong type reads as absent rather than raising an error, because rejecting a
concept for a malformed optional field is exactly what the spec forbids.

Loading honours `.gitignore` without requiring a git checkout, and deliberately
ignores the user's global excludes, so validation results never depend on the
machine the tool runs on.

[^okf-spec]: Open Knowledge Format specification v0.2
