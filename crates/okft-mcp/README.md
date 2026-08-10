# okft-mcp

A [Model Context Protocol](https://modelcontextprotocol.io/) server that exposes
an [Open Knowledge Format][spec] v0.2 bundle to AI agents.

Concepts are published as MCP resources (`okf://<concept-id>`) *and* as tools
(`search`, `list`, `read`, `related`, `trust`). Both, because many clients never
surface resources to the model without an explicit user action, so a
resource-only server looks correct and does nothing useful in practice.

The `trust` tool reports a concept's trust tier, lifecycle status, staleness,
and sources from OKF's provenance frontmatter, so an agent can decide whether a
concept is worth quoting.

Usually reached through the CLI:

```sh
okf serve --mcp ./knowledge
```

Part of [okf-toolkit](https://github.com/galkleinman/okf-toolkit). Apache-2.0.

[spec]: https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md
