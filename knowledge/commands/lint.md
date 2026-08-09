---
type: CLI Command
title: okft lint
description: Reports advisory findings about bundle hygiene alongside the conformance rules.
tags: [command, lint]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-09T00:00:00Z }
stale_after: 2027-02-09
---

# Synopsis

```
okft lint [BUNDLE] [--strict] [-D RULE] [-A RULE] [--today YYYY-MM-DD]
```

# Behaviour

Runs the conformance rules *and* the advisory ones: broken links, missing
recommended fields, staleness, superseded v0.1 constructs, unrecognised actors,
unresolved footnotes, and more. Run [okft rules](rules.md) for the full list.

By default only conformance errors change the exit code, so `okft lint` on a
bundle with warnings still exits `0`. Use `--strict` to fail on warnings, or
`-D <rule>` to fail on one specific rule.

# Examples

```sh
okft lint ./knowledge                        # report everything, exit 0
okft lint ./knowledge --strict               # warnings become errors
okft lint ./knowledge -D broken-link         # gate on links only
okft lint ./knowledge -A orphan-concept      # silence one rule
okft lint ./knowledge --today 2026-08-09     # reproducible staleness
```

# Severity tiers

`--strict` promotes warnings but leaves infos advisory, so a bundle is never
failed for an observation like an orphaned concept or a missing `index.md`.
See [the conformance decision](../decisions/conformance-vs-lint.md).
