# Contributing

## Commit messages

Commits follow [Conventional Commits](https://www.conventionalcommits.org/).
This is not a style preference: `release-plz` derives the version bump and the
changelog from the commit history, so the message *is* the release note.

```
feat(mcp): add a trust tool
fix(core): treat a bare `verified` mapping as a one-element list
docs(knowledge): record the conventional-commits decision
```

| Type | Effect on the next release |
|---|---|
| `feat` | minor bump |
| `fix`, `perf`, `refactor`, `docs`, `test`, `build` | patch bump |
| `ci`, `chore` | no bump, kept out of the changelog |
| `!` after the type, or a `BREAKING CHANGE:` footer | major bump |

Scopes are `core`, `cli`, `mcp`, `web`, `action`, `knowledge`, `release`, or
`deps`. The full rule set lives in [`committed.toml`](committed.toml), which is
the single source of truth for both the local hook and CI.

### Checking messages locally

```sh
cargo install committed
git config core.hooksPath .githooks
```

The [`commit-msg` hook](.githooks/commit-msg) then rejects a non-conforming
message before it becomes a commit. Without `committed` installed the hook
passes with a warning and CI catches it instead.

Note that this repository allows squash merging, in which case the **pull
request title** becomes the commit message. CI checks the title for that
reason, so it must be a Conventional Commit too.

History predating this convention is not conventional and was deliberately left
alone rather than rewritten; only the commits a pull request adds are checked.

## Releasing

Releases are automated and nobody tags by hand.

1. Merge conventional commits to `main`.
2. `release-plz` opens a **release pull request** bumping the version and
   rewriting `CHANGELOG.md`. Review it like any other change.
3. Merging that pull request publishes all four crates to crates.io, tags the
   commit, creates the GitHub release with the changelog as its body, builds
   the binaries for five targets, attaches them to the release, and moves the
   floating `v1` tag.

All four crates share one version. They are developed as a unit, and a consumer
installing `okft` gets all of them, so independent versions would only create
combinations nobody tests.

The binaries are built in the same workflow as the release rather than by a
separate tag-triggered one. `release-plz` tags with the default `GITHUB_TOKEN`,
and a tag pushed with that token does not start another workflow, so a
tag-triggered build would silently never run.

### First publication

Release automation is **off by default**, because crates.io Trusted Publishing
cannot create a crate that does not exist: until each crate has been published
once by hand, the publish job can only fail.

To switch it on, once:

1. Publish each crate manually, in dependency order:
   `okft-core`, `okft-mcp`, `okft-web`, `okft`.
2. Configure Trusted Publishing on crates.io for all four, pointing at this
   repository and the `Release` workflow.
3. Set the repository variable `RELEASE_AUTOMATION` to `enabled`
   (`gh variable set RELEASE_AUTOMATION --body enabled`).

From then on, merging conventional commits to `main` is the only step.

## Standards

Every change must keep these green; CI enforces all of them.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo llvm-cov --workspace --fail-under-lines 100
okf lint ./knowledge --strict          # the repo's own knowledge bundle
```

Coverage is gated at 100% of lines. If a branch cannot be reached by a test,
the fix is to delete it or restructure the code, not to lower the gate. See
[the coverage decision](knowledge/decisions/coverage-gate.md), which also
documents the `cargo-llvm-cov` monomorphisation trap.

Changes to behaviour usually belong in the `/knowledge` bundle as well; it is
this project's real documentation, and CI validates it with the tool this
repository ships.

<!-- enforcement probe -->
