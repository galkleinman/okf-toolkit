---
type: Log
title: okf-toolkit knowledge log
---

# Bundle history

## 2026-08-10

- **Initialization**: Created this bundle to document okf-toolkit as an OKF bundle rather than a `docs/` folder, so the project's CI validates its own documentation with the tool it ships.
- **Creation**: Documented the four crates under [architecture](architecture/index.md) and the command surface under [commands](commands/index.md).
- **Creation**: Recorded the five decisions that shape the codebase: [conformance vs lint](decisions/conformance-vs-lint.md), [binary naming](decisions/binary-naming.md), [the YAML parser choice](decisions/yaml-parser.md), [the coverage gate](decisions/coverage-gate.md), and [release automation](decisions/release-automation.md).
- **Creation**: Recorded [how a run picks an OKF revision](decisions/okf-versions.md) when `--okf-version` and the `okf-version` action input were added, so a bundle written against v0.1 can be linted without hand-silencing the rules v0.2 introduced.
- **Note**: No concept here carries a `verified` entry, so the whole bundle sits at the unverified trust tier. That is deliberate: recording a review that has not happened would misuse the exact field this project exists to check.
