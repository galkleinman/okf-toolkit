# okft-core

Parser, conformance validator, and linter for [Open Knowledge Format][spec] v0.2
bundles. No CLI or server dependencies, so it can be embedded directly.

The important design point: OKF §11 lists exactly three conformance
requirements and then forbids rejecting a bundle for anything else, including
broken cross-links. This crate keeps that split honest — `conformance::validate`
returns only errors for those three rules, and `lint::lint` returns advisory
findings that can never be errors by default.

```rust
use okft_core::{bundle::Bundle, conformance};

let bundle = Bundle::load("./knowledge")?;
let errors = conformance::validate(&bundle);
if errors.is_empty() {
    println!("{} concepts, all conformant", bundle.concepts().count());
}
# Ok::<(), okft_core::bundle::LoadError>(())
```

Part of [okf-toolkit](https://github.com/galkleinman/okf-toolkit). Apache-2.0.

[spec]: https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md
