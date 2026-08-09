# okf-toolkit

The `okft` command line tool for [Open Knowledge Format][spec] v0.2 bundles:
validate them in CI, lint them for hygiene, and serve them to AI agents over MCP
or to a browser as a graph.

```sh
cargo install okf-toolkit     # installs a binary named `okft`

okft validate ./knowledge     # OKF §11 conformance only
okft lint ./knowledge --strict
okft serve --mcp ./knowledge
okft serve --web ./knowledge
```

`validate` reports only the three §11 conformance rules, because the spec
forbids rejecting a bundle for broken links, unknown keys, or missing optional
fields. `lint` adds the advisory rules, and `--strict` or `-D <rule>` promotes
them if you want CI to gate on them.

The binary is `okft`, not `okf`: an unrelated crate already installs a binary by
that name.

See the [repository](https://github.com/galkleinman/okf-toolkit) for the GitHub
Action and full documentation. Apache-2.0.

[spec]: https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md
