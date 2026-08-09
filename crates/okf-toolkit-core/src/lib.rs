//! Parser, conformance validator, and linter for [OKF v0.2][spec] bundles.
//!
//! [spec]: https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md

pub mod bundle;
pub mod concept_id;
pub mod date;
pub mod diagnostic;
pub mod document;
pub mod frontmatter;
pub mod links;
pub mod span;
pub mod trust;
pub mod value;
