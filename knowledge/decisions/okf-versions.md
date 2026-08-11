---
type: Decision
title: Older spec revisions withhold rules rather than add them
description: Targeting OKF v0.1 silences the rules whose constructs v0.2 introduced; conformance is identical in both.
tags: [decision, validation, versioning]
status: stable
stale_after: 2027-02-09
sources:
  - id: spec-versioning
    resource: https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md
    title: OKF v0.2 §12 Versioning and §13 Changes from v0.1
    author: team:google-cloud
    last_modified: 2026-07-01
---

# Decision

Every rule in the registry records the earliest revision it says anything
about, as `since`. A run targets one revision, and a rule is skipped when the
target predates its `since`. Nothing else in the checker is version-aware.

`--okf-version` picks the target explicitly. Without it, the bundle's own
`okf_version` decides (§12), and a bundle that declares nothing is checked
against the newest revision the toolkit supports.

| Targeting | Rules that run |
|-----------|----------------|
| v0.2 (default) | All of them |
| v0.1 | Only those with `since: 0.1` |

`okf rules --okf-version 0.1` prints exactly the set a v0.1 run can report.

# Rationale

§13 makes v0.2 a superset of v0.1 apart from two supersessions, so an older
revision means *fewer* applicable rules, never different ones.[^spec-versioning]
`legacy-timestamp` is the clearest case: `timestamp` only became legacy when
v0.2 replaced it with `generated.at`, so reporting it against a v0.1 bundle
would be a false positive against the revision that bundle actually targets.
The same reasoning covers `sources`, `verified`, `stale_after`, the actor
conventions, and Attested Computations — none of them exist in v0.1, so no rule
about them can be a finding there.

Recording `since` on the rule rather than inside each check means the registry
is the single source of truth. `okf rules --okf-version` and a real run cannot
disagree, because both read the same field, and adding a rule means answering
"which revision introduced this?" once.

# Conformance does not move

`okf validate` ignores the target revision. §11 states the same three
requirements in both revisions, so a version-dependent conformance result would
be an invention rather than a reading of the spec. `--okf-version` is accepted
by `validate` so the same invocation works for both commands, but it changes
nothing there.

The one exception the checker makes in both directions is `okf_version` itself
in a bundle-root `index.md`: it is how a bundle names its revision, so it is
accepted whichever revision is being targeted.

# Consequences

- A v0.1 bundle can be checked strictly in CI without silencing rules by hand.
- A bundle whose declaration is not a revision this toolkit knows is reported
  as `version-mismatch` and checked against the newest one, rather than being
  refused; §11 has no requirement to reject on it.
- Requesting a revision that contradicts the bundle's declaration is also
  `version-mismatch`, because the two disagreeing is worth surfacing even
  though the explicit request wins.
- A new revision means one new enum variant and a `since` on the rules it
  introduces. See [conformance vs lint](conformance-vs-lint.md) for the tier
  split that this layers on top of.

[^spec-versioning]: OKF v0.2 §12 Versioning and §13 Changes from v0.1
