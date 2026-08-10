---
type: Decision
title: Frontmatter is parsed with saphyr, not a serde YAML crate
description: OKF requires preserving arbitrary unknown keys with source spans, which typed deserialization cannot provide.
tags: [decision, parsing, dependencies]
status: stable
stale_after: 2027-02-09
---

# Decision

Frontmatter is parsed into a generic, span-annotated tree using `saphyr`, and
that tree is wrapped in this project's own `Value` and `Node` types rather than
exposed directly.

# Rationale

The obvious choice, `serde_yaml`, is deprecated and unmaintained, as is its
`serde_yml` fork. The maintained serde-based option deserializes into typed
structs and offers no generic document model at all.

That rules it out on requirements, not just taste. OKF frontmatter is
open-ended: producers may add arbitrary keys, and consumers are asked to
preserve unknown ones when round-tripping. A validator also needs a source
position for every node so a diagnostic can point at the offending line.
`saphyr`'s marked tree provides both.

`saphyr` is pre-1.0, so its types are kept out of this project's public API. A
breaking change upstream then costs an internal conversion rather than a
semver-major release here.

# Consequences

- Duplicate frontmatter keys cannot be reported: the underlying loader collapses
  them before this crate sees them, keeping the last value.
- The `Value` tree is a small amount of code this project owns and tests, in
  exchange for insulation from a 0.0.x dependency.
