# okf-toolkit-web

A local graph viewer for [Open Knowledge Format][spec] v0.2 bundles, adapted from
Google's reference visualizer.

One deliberate change from upstream: the reference template loads Cytoscape and
Marked from a CDN, which makes the viewer useless offline and pings a third
party every time someone inspects a private knowledge base. Both libraries are
embedded in the binary, so the page renders with the network disabled.

```sh
okft serve --web ./knowledge
```

Part of [okf-toolkit](https://github.com/galkleinman/okf-toolkit). Apache-2.0.
Vendored third-party material is recorded in the repository's NOTICE file.

[spec]: https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md
