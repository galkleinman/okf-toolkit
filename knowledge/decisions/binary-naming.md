---
type: Decision
title: The command is okf, the crates are okft
description: The command keeps the obvious name; the crates carry a distinct one because the obvious crate names were already taken.
tags: [decision, distribution, naming]
status: stable
stale_after: 2027-02-10
---

# Decision

The published crates are `okft`, `okft-core`, `okft-mcp`, and `okft-web`. The
installed command is **`okf`**.

So `cargo install okft` gives you a binary called `okf`.

# Rationale

Three of the obvious crate names were already registered on crates.io by
unrelated projects when this repository was started: `okf` (a separate pure-Rust
OKF implementation), `okf-cli`, and `okf-mcp`. Crate names are globally unique
and first-come, so those were simply unavailable.

Binary names are not globally unique, only unique per `PATH`. That asymmetry is
what makes this split possible: the crates take a name that was free, and the
command takes the name people will actually type.

`okf validate ./knowledge` is what a reader expects a tool for the Open
Knowledge Format to be called, and it is typed in every workflow file, every
README example, and every terminal session. Optimising that at the cost of a
less obvious `cargo install` line is the right trade.

# The collision this accepts

The `okf` crate installs a binary that is also called `okf`. Anyone who installs
both tools ends up running whichever they installed most recently, with no
warning.

The two tools are alternatives rather than companions, so few people will want
both at once, and the cost falls only on those who do. The daily ergonomics of
the command outweigh a collision most users never encounter.

# Consequences

- Documentation must be careful to distinguish the crate (`okft`) from the
  command (`okf`); they are not interchangeable, and `cargo install okf` gets
  somebody else's tool.
- The README states the collision plainly rather than hiding it, so anyone who
  does hit it can recognise what happened.
- The repository stays named `okf-toolkit`, so the
  [GitHub Action](../architecture/github-action.md) is still referenced as
  `galkleinman/okf-toolkit@v1`.
- Release archives are named `okft-<target>`, and the binary inside them is
  `okf`; the action relies on both.
