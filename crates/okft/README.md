# okft

The `okf` command line tool for [Open Knowledge Format][spec] bundles: validate
them in CI, lint them for hygiene, and serve them to AI agents over MCP or to a
browser as a graph.

```sh
cargo install okft     # installs a binary named `okf`

okf validate ./knowledge     # OKF §11 conformance only
okf lint ./knowledge --strict
okf serve --mcp ./knowledge
okf serve --web ./knowledge
```

`validate` reports only the three §11 conformance rules, because the spec
forbids rejecting a bundle for broken links, unknown keys, or missing optional
fields. `lint` adds the advisory rules, and `--strict` or `-D <rule>` promotes
them if you want CI to gate on them.

Runs target spec v0.2. `--okf-version 0.1` lints a bundle against the older
revision instead, withholding the rules about constructs v0.2 introduced; a
bundle declaring `okf_version` in its root `index.md` is detected without the
flag.

The crate is `okft` but the command is `okf`. The unrelated `okf` crate installs
a binary of the same name, so installing both leaves whichever came last on your
`PATH`.

See the [repository](https://github.com/galkleinman/okf-toolkit) for the GitHub
Action and full documentation. Apache-2.0.

[spec]: https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md
