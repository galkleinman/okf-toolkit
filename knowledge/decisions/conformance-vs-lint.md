---
type: Decision
title: Conformance and lint are separate tiers
description: Only the three OKF v0.2 §11 rules can fail a bundle; every other finding is advisory and opt-in.
tags: [decision, validation, conformance]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-09T00:00:00Z }
stale_after: 2027-02-09
sources:
  - id: spec-conformance
    resource: https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md
    title: OKF v0.2 §11, Conformance
    author: team:google-cloud
    last_modified: 2026-07-01
---

# Decision

`okf validate` reports exactly three rules, and they are the only findings that
can ever carry error severity:

| Code           | Requirement                                                  |
|----------------|--------------------------------------------------------------|
| `okf-parse`    | Every non-reserved `.md` file has a parseable frontmatter block. |
| `okf-type`     | Every frontmatter block has a non-empty `type`.              |
| `okf-reserved` | `index.md` and `log.md` follow their specified structure.    |

Everything else, including broken links, is a lint finding. `okf lint` reports
them; `--strict` or `-D <rule>` promotes them to errors for callers who want to
gate on them.

# Rationale

§11 does not merely omit the other checks, it forbids them.[^spec-conformance] A
consumer must not reject a bundle because of missing optional frontmatter
fields, unknown `type` values, unknown additional keys, broken cross-links, or
missing `index.md` files. §6.1 is explicit that a link to a nonexistent concept
"is not malformed; it may simply represent not-yet-written knowledge."

Tools that fail a build on a broken link are therefore not implementing the
spec, they are implementing an opinion. Both are useful, but conflating them
means a bundle that Google's own samples satisfy can be reported as
non-conformant. This split lets the tool be honest about the spec and still
useful as a CI gate.

# Consequences

- `-A` cannot silence a conformance rule. Allowing that would let a repository
  configure its way to a green build on a genuinely broken bundle.
- `--strict` promotes warnings but not infos, so bundles are not failed for
  observations like an orphaned concept.
- A regression test asserts no lint rule defaults to error severity, and the
  four published Google bundles must produce zero errors.

[^spec-conformance]: OKF v0.2 §11, Conformance
