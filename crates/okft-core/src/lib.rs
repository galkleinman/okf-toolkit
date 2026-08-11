//! Parser, conformance validator, and linter for [OKF][spec] bundles.
//!
//! Section references throughout are to the v0.2 specification, the revision
//! this crate targets by default. Bundles written against v0.1 are supported
//! too: see [`version`] for how a run picks a revision and what changes.
//!
//! [spec]: https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md

pub mod bundle;
pub mod concept_id;
pub mod conformance;
pub mod date;
pub mod diagnostic;
pub mod document;
pub mod frontmatter;
pub mod links;
pub mod lint;
pub mod span;
pub mod trust;
pub mod value;
pub mod version;
