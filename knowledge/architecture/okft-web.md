---
type: Rust Crate
title: okft-web
description: A local, fully offline graph viewer for a bundle, adapted from Google's reference visualizer.
resource: https://crates.io/crates/okft-web
tags: [architecture, crate, web]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-09T00:00:00Z }
stale_after: 2027-02-09
sources:
  - id: reference-viewer
    resource: https://github.com/GoogleCloudPlatform/knowledge-catalog/tree/main/okf/src/reference_agent/viewer
    title: OKF reference agent viewer
    author: team:google-cloud
    last_modified: 2026-07-01
---

# Responsibility

Serves a force-directed graph of a bundle on a local port, reusing Google's
reference visualizer markup and styling rather than rebuilding it.[^reference-viewer]

# Difference from the reference implementation

The upstream template loads Cytoscape and Marked from a CDN. That makes the
viewer useless offline and leaks a request to a third party every time someone
inspects a private knowledge base. Both libraries are vendored into the binary
instead, so `okf serve --web` works with the network disabled.

The graph builder also differs: the reference link extractor skips
bundle-absolute links, which the spec actually recommends as the stable form, so
edges written the recommended way are missing from the upstream graph. Both link
forms are resolved here.

[^reference-viewer]: OKF reference agent viewer
