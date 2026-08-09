---
type: Rust Crate
title: okf-toolkit-mcp
description: An MCP server that exposes a bundle's concepts to AI agents as both resources and tools.
resource: https://crates.io/crates/okf-toolkit-mcp
tags: [architecture, crate, mcp, agents]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-09T00:00:00Z }
stale_after: 2027-02-09
---

# Responsibility

Serves a loaded bundle over the Model Context Protocol on stdio, so a coding
agent can query a knowledge base that lives next to the code it is editing.

# Why both resources and tools

Concepts are exposed twice: as MCP resources under `okf://<concept-id>`, and as
tools (`search`, `read`, `list`, `related`, `trust`).

The duplication is deliberate. Resources are the conceptually correct model for
read-only documents, but many MCP clients never surface them to the model
without an explicit user action. Tools are always visible. Exposing only
resources produces a server that looks correct and does nothing useful in
practice.

# Testing

The server is exercised in-process over an in-memory duplex stream rather than
by spawning a subprocess, so every handler and error branch is reachable from
tests. See [the coverage gate](../decisions/coverage-gate.md).
