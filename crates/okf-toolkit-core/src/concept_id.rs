//! Concept identifiers (§2): a concept's path within the bundle, minus `.md`.

use std::path::Path;

/// A concept's path within its bundle with the `.md` suffix removed (§2).
///
/// Always slash-separated regardless of host platform, so IDs written on
/// Windows match the links inside documents.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConceptId(String);

impl ConceptId {
    /// Builds an ID from a bundle-relative path, dropping any `.md` suffix.
    ///
    /// Returns `None` for paths that escape the bundle root or are empty.
    pub fn from_relative_path(path: &Path) -> Option<Self> {
        let mut segments = Vec::new();
        for component in path.components() {
            match component {
                std::path::Component::Normal(part) => segments.push(part.to_str()?),
                std::path::Component::CurDir => {}
                _ => return None,
            }
        }

        let joined = segments.join("/");
        let trimmed = joined.strip_suffix(".md").unwrap_or(&joined);
        (!trimmed.is_empty()).then(|| Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The final path segment.
    pub fn name(&self) -> &str {
        self.0.rsplit_once('/').map_or(self.0.as_str(), |(_, name)| name)
    }

    /// The parent directory, or `None` for a concept at the bundle root.
    pub fn parent(&self) -> Option<&str> {
        self.0.rsplit_once('/').map(|(parent, _)| parent)
    }

    /// The path this ID corresponds to, relative to the bundle root.
    pub fn to_relative_path(&self) -> std::path::PathBuf {
        let mut path = std::path::PathBuf::new();
        for segment in self.0.split('/') {
            path.push(segment);
        }
        path.set_extension("md");
        path
    }
}

impl std::fmt::Display for ConceptId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(path: &str) -> ConceptId {
        ConceptId::from_relative_path(Path::new(path)).expect("valid id")
    }

    #[test]
    fn strips_the_md_suffix() {
        assert_eq!(id("tables/orders.md").as_str(), "tables/orders");
        assert_eq!(id("index.md").as_str(), "index");
    }

    #[test]
    fn keeps_paths_without_an_extension() {
        assert_eq!(id("tables/orders").as_str(), "tables/orders");
    }

    #[test]
    fn normalises_separators_to_slashes() {
        let path: std::path::PathBuf = ["a", "b", "c.md"].iter().collect();
        assert_eq!(id(path.to_str().expect("utf-8")).as_str(), "a/b/c");
    }

    #[test]
    fn ignores_current_directory_components() {
        assert_eq!(id("./tables/orders.md").as_str(), "tables/orders");
    }

    #[test]
    fn rejects_paths_that_escape_the_bundle() {
        assert!(ConceptId::from_relative_path(Path::new("../outside.md")).is_none());
        assert!(ConceptId::from_relative_path(Path::new("/absolute.md")).is_none());
    }

    #[test]
    fn rejects_empty_ids() {
        assert!(ConceptId::from_relative_path(Path::new("")).is_none());
        assert!(ConceptId::from_relative_path(Path::new(".md")).is_none());
    }

    #[test]
    fn exposes_name_and_parent() {
        let nested = id("a/b/c.md");
        assert_eq!(nested.name(), "c");
        assert_eq!(nested.parent(), Some("a/b"));

        let root = id("top.md");
        assert_eq!(root.name(), "top");
        assert_eq!(root.parent(), None);
    }

    #[test]
    fn round_trips_to_a_relative_path() {
        let original = id("tables/orders.md");
        let path = original.to_relative_path();
        assert_eq!(path, Path::new("tables").join("orders.md"));
        assert_eq!(ConceptId::from_relative_path(&path).expect("round trip"), original);
    }

    #[test]
    fn displays_as_the_bare_id() {
        assert_eq!(id("metrics/revenue.md").to_string(), "metrics/revenue");
    }

    #[test]
    fn ids_sort_and_hash() {
        let mut ids = vec![id("b.md"), id("a.md")];
        ids.sort();
        assert_eq!(ids, [id("a.md"), id("b.md")]);

        let set: std::collections::HashSet<_> = ids.iter().cloned().collect();
        assert!(set.contains(&id("a.md")));
    }
}
