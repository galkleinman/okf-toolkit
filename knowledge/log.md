---
type: Log
title: okf-toolkit knowledge log
---

# Bundle history

## 2026-08-10

- **Creation**: Recorded [release-automation](decisions/release-automation.md) after adopting Conventional Commits, `release-plz` for versioning and publishing, and `git-cliff` for the release notes.
- **Update**: Renamed the command to `okf` while the crates became `okft*`, and rewrote [binary-naming](decisions/binary-naming.md), which had argued the opposite.

## 2026-08-09

- **Initialization**: Created this bundle to document okf-toolkit as an OKF bundle rather than a `docs/` folder, so the project's own CI validates it with the tool it ships.
- **Creation**: Documented the four crates under [architecture](architecture/index.md) and the command surface under [commands](commands/index.md), including [serve](commands/serve.md) once the MCP server and web viewer landed.
- **Creation**: Recorded the four decisions that shape the codebase: [conformance vs lint](decisions/conformance-vs-lint.md), [binary naming](decisions/binary-naming.md), [the YAML parser choice](decisions/yaml-parser.md), and [the coverage gate](decisions/coverage-gate.md).
- **Note**: Every concept here is `generated` by `claude-code/opus-5` and carries no `verified` entry, so the whole bundle sits at the unverified trust tier until a human reviews it. That is deliberate: claiming review that has not happened would misuse the exact field this project exists to check.
