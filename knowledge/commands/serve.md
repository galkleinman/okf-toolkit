---
type: CLI Command
title: okf serve
description: Serves a bundle to AI agents over MCP, or to a browser as an offline graph viewer.
tags: [command, mcp, web, agents]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-09T00:00:00Z }
stale_after: 2027-02-09
---

# Synopsis

```
okf serve [BUNDLE] (--mcp | --web) [--addr HOST:PORT] [--today YYYY-MM-DD]
```

Exactly one transport must be chosen; the two are mutually exclusive.

# --mcp

Serves the bundle over the Model Context Protocol on stdio, exposing concepts as
resources (`okf://<concept-id>`) and as the tools `search`, `list`, `read`,
`related`, and `trust`. See [the MCP crate](../architecture/okft-mcp.md)
for why both.

Stdout carries the JSON-RPC stream, so nothing else is ever written there;
status messages go to stderr. Closing stdin ends the session cleanly.

```json
{
  "mcpServers": {
    "knowledge": { "command": "okf", "args": ["serve", "--mcp", "./knowledge"] }
  }
}
```

# --web

Serves a force-directed graph of the bundle on a local port, defaulting to
`127.0.0.1:7878`. The page embeds its own JavaScript, so it renders with the
network disabled and never contacts a third party. See
[the web crate](../architecture/okft-web.md).

The banner reports the rendered page size, which is how you can tell at a glance
that the page is self-contained rather than fetching libraries at load time.

```sh
okf serve --web ./knowledge
okf serve --web ./knowledge --addr 0.0.0.0:9000
```

`/healthz` answers `ok`, for scripting a readiness check.

# Staleness

`--today` pins the date staleness is reported against and is fixed for the life
of the process, so a long-running server does not silently change its answers at
midnight.
