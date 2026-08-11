---
type: CLI Command
title: okf lint
description: Reports advisory findings about bundle hygiene alongside the conformance rules.
tags: [command, lint]
status: stable
stale_after: 2027-02-09
---

# Synopsis

```
okf lint [BUNDLE] [--strict] [-D RULE] [-A RULE] [--today YYYY-MM-DD]
          [--okf-version 0.1|0.2]
```

# Behaviour

Runs the conformance rules *and* the advisory ones: broken links, missing
recommended fields, staleness, superseded v0.1 constructs, unrecognised actors,
unresolved footnotes, and more. Run [okf rules](rules.md) for the full list.

By default only conformance errors change the exit code, so `okf lint` on a
bundle with warnings still exits `0`. Use `--strict` to fail on warnings, or
`-D <rule>` to fail on one specific rule.

# Spec revisions

This is the only command `--okf-version` changes. Targeting `0.1` withholds
every rule about a construct v0.2 introduced, so a v0.1 bundle can be linted
strictly without silencing rules one at a time. Without the flag, the bundle's
own `okf_version` decides, and a bundle that declares nothing is read as the
newest revision. See [the versioning decision](../decisions/okf-versions.md).

# Examples

```sh
okf lint ./knowledge                        # report everything, exit 0
okf lint ./knowledge --strict               # warnings become errors
okf lint ./knowledge -D broken-link         # gate on links only
okf lint ./knowledge -A orphan-concept      # silence one rule
okf lint ./knowledge --today 2026-08-09     # reproducible staleness
okf lint ./legacy --okf-version 0.1         # judge it as an OKF v0.1 bundle
```

# Severity tiers

`--strict` promotes warnings but leaves infos advisory, so a bundle is never
failed for an observation like an orphaned concept or a missing `index.md`.
See [the conformance decision](../decisions/conformance-vs-lint.md).
