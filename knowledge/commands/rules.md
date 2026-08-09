---
type: CLI Command
title: okft rules
description: Lists every diagnostic rule with its tier, spec section, and default severity.
tags: [command, reference]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-09T00:00:00Z }
stale_after: 2027-02-09
---

# Synopsis

```
okft rules [--json]
```

# Behaviour

Prints the rule registry, split into the conformance rules that always fail
[validate](validate.md) and the advisory rules that [lint](lint.md) reports.
Each entry carries the OKF section it derives from, so a finding can be traced
back to the spec text that motivates it.

`--json` emits the same data as an array, which is the supported way to discover
rule codes programmatically rather than parsing the table.

# Example

```sh
okft rules
okft rules --json | jq -r '.[] | select(.kind == "conformance") | .code'
```
