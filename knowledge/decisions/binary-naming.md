---
type: Decision
title: The binary is okft and the crates are namespaced
description: The obvious names were already taken on crates.io by unrelated projects, so everything moved under okf-toolkit-*.
tags: [decision, distribution, naming]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-09T00:00:00Z }
stale_after: 2027-02-09
---

# Decision

The published crates are `okf-toolkit`, `okf-toolkit-core`, `okf-toolkit-mcp`,
and `okf-toolkit-web`. The installed binary is `okft`.

# Rationale

Three of the obvious names were already registered on crates.io by unrelated
projects when this repository was started: `okf` (a separate pure-Rust OKF
implementation), `okf-cli`, and `okf-mcp`. The `okf` crate installs a binary
called `okf`.

Naming this project's binary `okf` would therefore collide on `PATH` for anyone
who had installed either tool, producing a shell that runs whichever was
installed most recently. `okft` is unambiguous and still short enough to type in
a workflow file on every run.

Taking the whole `okf-toolkit-*` namespace at once also avoids a second round of
renaming if a future crate is added.

# Consequences

- Documentation and the [GitHub Action](../architecture/github-action.md) must
  say `okft`, never `okf`.
- `cargo install okf-toolkit` installs a binary whose name does not match the
  crate, which is worth stating explicitly in the README quickstart.
