---
type: CLI Command
title: okf rules
description: Lists every diagnostic rule with its tier, spec section, and default severity.
tags: [command, reference]
status: stable
stale_after: 2027-02-09
---

# Synopsis

```
okf rules [--json] [--okf-version 0.1|0.2]
```

# Behaviour

Prints the rule registry, split into the conformance rules that always fail
[validate](validate.md) and the advisory rules that [lint](lint.md) reports.
Each entry carries the OKF section it derives from, so a finding can be traced
back to the spec text that motivates it, and the earliest revision it applies
to as `v0.1+` or `v0.2+`.

`--okf-version` narrows the listing to the rules a run against that revision can
report, which is the same filter [lint](lint.md) applies — both read the `since`
field on the rule, so the listing cannot drift from the behaviour. See
[the versioning decision](../decisions/okf-versions.md).

`--json` emits the same data as an array, which is the supported way to discover
rule codes programmatically rather than parsing the table.

# Example

```sh
okf rules
okf rules --okf-version 0.1     # only the rules a v0.1 bundle can trip
okf rules --json | jq -r '.[] | select(.kind == "conformance") | .code'
```
