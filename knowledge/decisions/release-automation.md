---
type: Decision
title: Releases are driven by Conventional Commits
description: release-plz owns versioning and publishing; git-cliff owns the release notes, because release-plz's changelogs are path-scoped and drop commits.
tags: [decision, release, ci]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-10T00:00:00Z }
stale_after: 2027-02-10
---

# Decision

Commit messages follow Conventional Commits, enforced by `committed` in CI and
by a local `commit-msg` hook sharing one `committed.toml`.

Two tools split the release, and the split is not arbitrary:

| Tool | Owns |
|---|---|
| `release-plz` | Version bumps, the release pull request, crates.io publishing, the git tag, the GitHub release |
| `git-cliff` | The body of the GitHub release |

All four crates share one version through `version_group`, and only `okft`
carries the tag, so a release produces one `vX.Y.Z` rather than four colliding
ones.

# Why the release notes are generated separately

`release-plz` attributes a commit to a package by **file path**. That is right
for a per-crate `CHANGELOG.md` published to crates.io, and wrong for a release
note, because it silently drops anything outside a crate directory.

Two configurations were tried against a probe branch of five conventional
commits before settling:

- Pointing every package at one shared `changelog_path` made the packages
  overwrite one another's section. Of two commits, only one survived.
- `changelog_include = ["okft-core", "okft-mcp", "okft-web"]` on `okft`, the
  documented aggregation field, did not pull the libraries in.

Under both, a `docs(knowledge)` commit vanished entirely, because
[the knowledge bundle](../index.md) sits outside every crate directory. Running
`git-cliff` over the same five commits produced all five, so it generates the
release body while `release-plz` keeps the per-crate changelogs.

The commit-classification groups are duplicated between `release-plz.toml` and
`cliff.toml` deliberately, so a commit is grouped the same way in both.

# Why the binaries are built in the release workflow

`release-plz` tags using the default `GITHUB_TOKEN`, and a tag pushed with that
token does not start another workflow run. A separate tag-triggered build would
therefore never fire, and
[the GitHub Action](../architecture/github-action.md) would keep downloading
binaries from a release that has none. Building them in the same workflow avoids
needing a personal access token purely to work around that.

# Consequences

- The pull request title is checked too. This repository allows squash merging,
  where the title becomes the commit message, so checking only commits would
  leave that path unguarded.
- History predating the convention is not conventional and was left alone; only
  the commits a pull request adds are checked, and `git-cliff` filters
  unconventional commits out of the notes.
- The action reads its default version from `[workspace.package]` in
  `Cargo.toml` rather than a hand-maintained file, so the version `release-plz`
  bumps is automatically the version the action downloads.
- crates.io Trusted Publishing cannot create a crate that does not exist, so the
  first version of each crate must be published manually once. Until then the
  automation is held behind the `RELEASE_AUTOMATION` repository variable, which
  is unset: a publish job that runs before that first publication can only
  fail, and failing on every push to `main` would be worse than being inert.
