---
type: Decision
title: Coverage is gated at 100% of lines
description: The gate went in before any logic, and it shapes how the code is structured rather than being measured after the fact.
tags: [decision, testing, ci]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-09T00:00:00Z }
stale_after: 2027-02-09
---

# Decision

CI fails below 100% line coverage across the workspace. The gate was added to
the pipeline before any rule logic existed.

# Rationale

A coverage target added after the fact measures whatever the tests happen to
reach. A gate added first is a design constraint: code that cannot be tested
does not get written, because it cannot be merged.

In practice it changed the structure of several things:

- `main.rs` is a three-line shim, so the process entry point holds no untestable
  logic.
- The [MCP server](../architecture/okf-toolkit-mcp.md) is tested over an
  in-memory duplex stream rather than a spawned subprocess.
- Unreachable match arms and defensive fallbacks were deleted rather than
  excluded, because a branch nothing can reach is dead code.

# Gotcha: generic monomorphisation

`cargo llvm-cov` attributes line coverage per monomorphisation. A generic
function whose body returns early for one instantiation reports its remaining
lines as uncovered even when another instantiation executes them all.

The fix is to keep generic wrappers to a single delegating line and put the real
work in a non-generic function. `Bundle::load` and `Bundle::from_sources` are
both written that way. Diagnosing this cost real time; it looks like a phantom
uncovered line that no test can reach.

# Consequences

- Region coverage is reported but not gated, because `#[derive]` expansions
  produce regions no test can reach.
- Every diagnostic rule ships with a fixture that triggers it and one that does
  not.
