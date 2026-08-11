//! Loading a bundle: a directory tree of markdown files (§3).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::concept_id::ConceptId;
use crate::document::Document;
use crate::links::{self, BodyLinks};
use crate::span::Span;

/// Filenames with defined meaning that are never concept documents (§3.1).
pub const RESERVED_FILENAMES: [&str; 2] = ["index.md", "log.md"];

/// The role a file plays in the bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A concept document: any non-reserved `.md` file.
    Concept,
    /// A directory listing (§8).
    Index,
    /// An update history (§9).
    Log,
}

impl EntryKind {
    /// Classifies by concept-ID name, i.e. the filename minus `.md`.
    fn for_stem(stem: &str) -> Self {
        match stem {
            "index" => Self::Index,
            "log" => Self::Log,
            _ => Self::Concept,
        }
    }
}

/// One markdown file in a bundle, parsed.
#[derive(Debug, Clone)]
pub struct Entry {
    pub id: ConceptId,
    pub kind: EntryKind,
    /// Path relative to the bundle root, always slash-separated.
    pub path: PathBuf,
    pub document: Document,
    pub body: BodyLinks,
}

impl Entry {
    /// Builds an entry from already-loaded source text.
    ///
    /// Exists so validation can be exercised without touching the filesystem.
    pub fn from_source(relative_path: &Path, source: &str) -> Option<Self> {
        let id = ConceptId::from_relative_path(relative_path)?;
        let document = Document::parse(source);
        let body = links::extract(&document.body);
        Some(Self {
            kind: EntryKind::for_stem(id.name()),
            id,
            path: relative_path.to_path_buf(),
            document,
            body,
        })
    }

    pub fn is_concept(&self) -> bool {
        self.kind == EntryKind::Concept
    }

    /// The directory this entry sits in, relative to the bundle root.
    pub fn directory(&self) -> &str {
        self.id.parent().unwrap_or("")
    }
}

/// A loaded bundle.
#[derive(Debug, Clone, Default)]
pub struct Bundle {
    root: PathBuf,
    entries: BTreeMap<ConceptId, Entry>,
    /// Every non-markdown file, for resolving path-valued fields (§6.2).
    assets: BTreeSet<String>,
}

/// Why a bundle could not be loaded.
#[derive(Debug)]
pub enum LoadError {
    NotADirectory(PathBuf),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotADirectory(path) => {
                write!(f, "{} is not a directory", path.display())
            }
            Self::Io { path, source } => write!(f, "cannot read {}: {source}", path.display()),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotADirectory(_) => None,
            Self::Io { source, .. } => Some(source),
        }
    }
}

impl Bundle {
    /// Builds a bundle from in-memory sources, keyed by bundle-relative path.
    pub fn from_sources<'a>(sources: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        Self::from_source_list(&sources.into_iter().collect::<Vec<_>>())
    }

    /// The non-generic body behind [`Bundle::from_sources`].
    ///
    /// Generic wrappers are kept to a single delegating line so the real work
    /// is compiled once, rather than once per caller's iterator type.
    fn from_source_list(sources: &[(&str, &str)]) -> Self {
        let mut bundle = Self::default();
        for (path, source) in sources {
            bundle.insert(Path::new(path), source);
        }
        bundle
    }

    /// Files that are not markdown are recorded as assets; markdown files whose
    /// path escapes the bundle root are skipped.
    fn insert(&mut self, relative: &Path, source: &str) {
        if relative.extension().is_some_and(|ext| ext == "md") {
            if let Some(entry) = Entry::from_source(relative, source) {
                self.entries.insert(entry.id.clone(), entry);
            }
        } else {
            self.assets
                .insert(relative.to_string_lossy().replace('\\', "/"));
        }
    }

    /// Reads a bundle from disk, skipping files ignored by `.gitignore` and
    /// the contents of dot-directories.
    ///
    /// # Errors
    ///
    /// Returns [`LoadError::NotADirectory`] if `root` is not a directory, and
    /// [`LoadError::Io`] if a file cannot be read or is not valid UTF-8.
    pub fn load(root: impl AsRef<Path>) -> Result<Self, LoadError> {
        Self::load_path(root.as_ref())
    }

    /// The non-generic body behind [`Bundle::load`].
    fn load_path(root: &Path) -> Result<Self, LoadError> {
        if !root.is_dir() {
            return Err(LoadError::NotADirectory(root.to_path_buf()));
        }

        let mut bundle = Self {
            root: root.to_path_buf(),
            ..Self::default()
        };

        // A bundle is often a plain directory rather than a git checkout, so
        // `.gitignore` is honoured without requiring git. The user's *global*
        // excludes are deliberately ignored: validation must not depend on the
        // machine it runs on.
        let walker = ignore::WalkBuilder::new(root)
            .hidden(false)
            .parents(false)
            .require_git(false)
            .git_global(false)
            .sort_by_file_path(Ord::cmp)
            .filter_entry(|entry| entry.depth() == 0 || !is_tooling_directory(entry))
            .build();

        for result in walker {
            // `ignore::Error` names the offending path in its own message.
            let entry = result.map_err(|error| LoadError::Io {
                path: root.to_path_buf(),
                source: std::io::Error::other(error),
            })?;

            if !entry
                .file_type()
                .is_some_and(|file_type| file_type.is_file())
            {
                continue;
            }

            let path = entry.path();
            // The walker only yields paths beneath `root`.
            let relative = path.strip_prefix(root).unwrap_or(path);

            let source = if path.extension().is_some_and(|ext| ext == "md") {
                std::fs::read_to_string(path).map_err(|source| LoadError::Io {
                    path: path.to_path_buf(),
                    source,
                })?
            } else {
                String::new()
            };
            bundle.insert(relative, &source);
        }

        Ok(bundle)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every markdown file, including reserved ones, in ID order.
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.values()
    }

    /// Concept documents only, excluding `index.md` and `log.md` (§3.1).
    pub fn concepts(&self) -> impl Iterator<Item = &Entry> {
        self.entries.values().filter(|entry| entry.is_concept())
    }

    pub fn get(&self, id: &ConceptId) -> Option<&Entry> {
        self.entries.get(id)
    }

    /// The bundle-root `index.md`, the one file permitted to carry frontmatter
    /// while being reserved (§8, §12).
    pub fn root_index(&self) -> Option<&Entry> {
        self.entries
            .values()
            .find(|entry| entry.kind == EntryKind::Index && entry.directory().is_empty())
    }

    /// The OKF revision the bundle declares, verbatim, with the span to blame.
    ///
    /// Returned unparsed: a declaration this toolkit does not recognise is a
    /// lint finding that has to quote what was written, not something to
    /// silently discard here.
    pub fn declared_okf_version(&self) -> Option<(&str, Span)> {
        let node = self
            .root_index()?
            .document
            .frontmatter
            .parsed()?
            .entries
            .get("okf_version")?;
        Some((node.as_str()?, node.span))
    }

    pub fn contains(&self, id: &ConceptId) -> bool {
        self.entries.contains_key(id)
    }

    /// Whether a bundle-relative path names a file that exists (§6.2).
    pub fn contains_path(&self, path: &str) -> bool {
        let normalized = path.trim_start_matches('/');
        self.assets.contains(normalized)
            || ConceptId::from_relative_path(Path::new(normalized))
                .is_some_and(|id| self.contains(&id))
    }

    /// Concepts that `id` links to, in document order, deduplicated.
    pub fn links_from(&self, id: &ConceptId) -> Vec<ConceptId> {
        let Some(entry) = self.get(id) else {
            return Vec::new();
        };
        let mut seen = BTreeSet::new();
        entry
            .body
            .links
            .iter()
            .filter_map(|link| link.resolve(id))
            .filter(|target| seen.insert(target.clone()))
            .collect()
    }

    /// Concepts that link to `id`.
    pub fn backlinks(&self, id: &ConceptId) -> Vec<ConceptId> {
        self.entries
            .values()
            .filter(|entry| &entry.id != id)
            .filter(|entry| self.links_from(&entry.id).iter().any(|target| target == id))
            .map(|entry| entry.id.clone())
            .collect()
    }

    /// Directories that contain at least one markdown file, plus the root.
    pub fn directories(&self) -> BTreeSet<&str> {
        let mut directories = BTreeSet::from([""]);
        for entry in self.entries.values() {
            let mut directory = entry.directory();
            directories.insert(directory);
            while let Some((parent, _)) = directory.rsplit_once('/') {
                directories.insert(parent);
                directory = parent;
            }
        }
        directories
    }
}

/// Whether a walked entry is a dot-directory, whose contents belong to some
/// tool rather than to the bundle.
///
/// §3.1 makes every non-reserved `.md` file in the tree a concept document,
/// which taken literally turns `.claude/skills/*/SKILL.md` and `.github/`
/// issue templates into concepts that fail §11 for want of a `type`. Those
/// files are configuration that happens to live under the bundle root, so
/// pruning them is the reading that matches what §3.1 is describing. Only the
/// directory name is examined: a dot-*file* is left alone, since a producer
/// who writes one has named an individual document rather than handed a
/// subtree to a tool.
///
/// The bundle root is exempt from this, so that a bundle may itself live in a
/// dot-directory such as `.okf/`.
fn is_tooling_directory(entry: &ignore::DirEntry) -> bool {
    entry.file_type().is_some_and(|kind| kind.is_dir())
        && entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with('.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(path: &str) -> ConceptId {
        ConceptId::from_relative_path(Path::new(path)).expect("valid id")
    }

    fn sample() -> Bundle {
        Bundle::from_sources([
            (
                "index.md",
                "# Subdirectories\n\n* [tables](tables/index.md)\n",
            ),
            ("log.md", "# History\n\n## 2026-01-01\n\n- **Created**\n"),
            (
                "tables/orders.md",
                "---\ntype: BigQuery Table\n---\n\nJoins [customers](/tables/customers.md).\n",
            ),
            (
                "tables/customers.md",
                "---\ntype: BigQuery Table\n---\n\nNo links.\n",
            ),
            ("tables/index.md", "# Tables\n\n* [orders](orders.md)\n"),
            ("attesters/check.py", "print('not markdown')\n"),
        ])
    }

    #[test]
    fn classifies_reserved_filenames() {
        let bundle = sample();
        assert_eq!(
            bundle.get(&id("index.md")).expect("index").kind,
            EntryKind::Index
        );
        assert_eq!(bundle.get(&id("log.md")).expect("log").kind, EntryKind::Log);
        assert_eq!(
            bundle.get(&id("tables/orders.md")).expect("orders").kind,
            EntryKind::Concept
        );
        assert_eq!(RESERVED_FILENAMES, ["index.md", "log.md"]);
    }

    #[test]
    fn concepts_exclude_reserved_files() {
        let bundle = sample();
        let concepts: Vec<_> = bundle.concepts().map(|e| e.id.to_string()).collect();
        assert_eq!(concepts, ["tables/customers", "tables/orders"]);
        assert_eq!(bundle.entries().count(), 5);
        assert_eq!(bundle.len(), 5);
        assert!(!bundle.is_empty());
    }

    #[test]
    fn resolves_the_link_graph_in_both_directions() {
        let bundle = sample();
        assert_eq!(
            bundle.links_from(&id("tables/orders.md")),
            [id("tables/customers.md")]
        );
        assert_eq!(
            bundle.backlinks(&id("tables/customers.md")),
            [id("tables/orders.md")]
        );
        assert!(
            bundle
                .backlinks(&id("tables/orders.md"))
                .contains(&id("tables/index.md"))
        );
    }

    #[test]
    fn links_from_deduplicates_repeated_targets() {
        let bundle = Bundle::from_sources([(
            "a.md",
            "---\ntype: X\n---\n[one](/b.md) and [again](/b.md) and [c](/c.md)\n",
        )]);
        assert_eq!(bundle.links_from(&id("a.md")), [id("b.md"), id("c.md")]);
    }

    #[test]
    fn links_from_an_unknown_concept_is_empty() {
        assert!(sample().links_from(&id("nope.md")).is_empty());
    }

    #[test]
    fn tracks_non_markdown_assets_for_path_fields() {
        let bundle = sample();
        assert!(bundle.contains_path("attesters/check.py"));
        assert!(bundle.contains_path("/attesters/check.py"));
        assert!(bundle.contains_path("tables/orders.md"));
        assert!(!bundle.contains_path("attesters/missing.py"));
        assert!(!bundle.contains_path(""));
    }

    #[test]
    fn lists_directories_including_ancestors_and_root() {
        let bundle = Bundle::from_sources([
            ("a/b/c/deep.md", "---\ntype: X\n---\n"),
            ("top.md", "---\ntype: X\n---\n"),
        ]);
        let directories = bundle.directories();
        assert!(directories.contains(""));
        assert!(directories.contains("a"));
        assert!(directories.contains("a/b"));
        assert!(directories.contains("a/b/c"));
    }

    #[test]
    fn entry_reports_its_directory() {
        let bundle = sample();
        assert_eq!(
            bundle.get(&id("tables/orders.md")).expect("e").directory(),
            "tables"
        );
        assert_eq!(bundle.get(&id("index.md")).expect("e").directory(), "");
    }

    #[test]
    fn from_source_rejects_paths_outside_the_bundle() {
        assert!(Entry::from_source(Path::new("../escape.md"), "").is_none());
        assert!(Entry::from_source(Path::new(""), "").is_none());
    }

    #[test]
    fn markdown_paths_that_escape_the_bundle_are_skipped() {
        let bundle = Bundle::from_sources([
            ("../escape.md", "---\ntype: X\n---\n"),
            ("kept.md", "---\ntype: X\n---\n"),
        ]);
        assert_eq!(bundle.len(), 1);
        assert!(bundle.contains(&id("kept.md")));
    }

    #[test]
    fn reads_the_declared_okf_version_from_the_root_index() {
        let bundle = Bundle::from_sources([
            ("index.md", "---\nokf_version: '0.1'\n---\n# Bundle\n"),
            ("tables/index.md", "# Tables\n"),
        ]);
        let (declared, span) = bundle.declared_okf_version().expect("declared");
        assert_eq!(declared, "0.1");
        assert_eq!(span.start.line, 2);
        assert_eq!(bundle.root_index().expect("root index").directory(), "");
    }

    #[test]
    fn a_bundle_that_declares_nothing_has_no_version() {
        assert!(sample().declared_okf_version().is_none());
        assert!(Bundle::default().declared_okf_version().is_none());
        assert!(Bundle::default().root_index().is_none());
    }

    /// A nested `index.md` cannot speak for the bundle (§12).
    #[test]
    fn a_nested_declaration_is_not_the_bundles() {
        let bundle = Bundle::from_sources([("tables/index.md", "---\nokf_version: '0.1'\n---\n")]);
        assert!(bundle.declared_okf_version().is_none());
    }

    #[test]
    fn a_malformed_declaration_reads_as_absent() {
        let unparseable = Bundle::from_sources([("index.md", "---\n[unclosed\n---\n")]);
        assert!(unparseable.declared_okf_version().is_none());

        let non_string = Bundle::from_sources([("index.md", "---\nokf_version: [0.1]\n---\n")]);
        assert!(non_string.declared_okf_version().is_none());
    }

    #[test]
    fn contains_and_get_agree() {
        let bundle = sample();
        assert!(bundle.contains(&id("tables/orders.md")));
        assert!(!bundle.contains(&id("tables/missing.md")));
        assert!(bundle.get(&id("tables/missing.md")).is_none());
    }

    #[test]
    fn an_empty_bundle_has_no_entries() {
        let bundle = Bundle::default();
        assert!(bundle.is_empty());
        assert_eq!(bundle.len(), 0);
        assert_eq!(bundle.root(), Path::new(""));
        assert_eq!(bundle.directories(), BTreeSet::from([""]));
    }

    mod from_disk {
        use super::*;

        fn write(dir: &Path, relative: &str, contents: &str) {
            let path = dir.join(relative);
            std::fs::create_dir_all(path.parent().expect("has parent")).expect("mkdir");
            std::fs::write(path, contents).expect("write");
        }

        #[test]
        fn loads_a_directory_tree() {
            let dir = tempfile::tempdir().expect("tempdir");
            write(dir.path(), "index.md", "# Index\n");
            write(
                dir.path(),
                "tables/orders.md",
                "---\ntype: BigQuery Table\n---\nbody\n",
            );
            write(dir.path(), "attesters/check.py", "print()\n");

            let bundle = Bundle::load(dir.path()).expect("loads");
            assert_eq!(bundle.len(), 2);
            assert_eq!(bundle.concepts().count(), 1);
            assert!(bundle.contains_path("attesters/check.py"));
            assert_eq!(bundle.root(), dir.path());
        }

        #[test]
        fn respects_gitignore() {
            let dir = tempfile::tempdir().expect("tempdir");
            write(dir.path(), ".gitignore", "ignored/\n");
            write(dir.path(), "kept.md", "---\ntype: X\n---\n");
            write(dir.path(), "ignored/hidden.md", "---\ntype: X\n---\n");

            let bundle = Bundle::load(dir.path()).expect("loads");
            assert!(bundle.contains(&id("kept.md")));
            assert!(!bundle.contains(&id("ignored/hidden.md")));
        }

        /// Real bundles keep agent and CI configuration under the bundle root;
        /// treating it as knowledge is how `.claude/skills/*/SKILL.md` turns
        /// into a §11 error in the wild.
        #[test]
        fn skips_the_contents_of_dot_directories() {
            let dir = tempfile::tempdir().expect("tempdir");
            write(dir.path(), "kept.md", "---\ntype: X\n---\n");
            write(
                dir.path(),
                ".claude/skills/bookkeeping/SKILL.md",
                "# Skill\n",
            );
            write(dir.path(), ".github/workflows/ci.yml", "on: push\n");
            write(dir.path(), "nested/.cursor/rules/style.md", "# Rules\n");

            let bundle = Bundle::load(dir.path()).expect("loads");
            assert_eq!(bundle.len(), 1);
            assert!(bundle.contains(&id("kept.md")));
            assert!(!bundle.contains_path(".github/workflows/ci.yml"));
        }

        /// A dot-file is a document the producer named, not a subtree handed
        /// to a tool, so only directories are pruned.
        #[test]
        fn keeps_dot_files() {
            let dir = tempfile::tempdir().expect("tempdir");
            write(dir.path(), ".hidden.md", "---\ntype: X\n---\n");

            let bundle = Bundle::load(dir.path()).expect("loads");
            assert!(bundle.contains(&id(".hidden.md")));
        }

        /// `.okf/` is a common bundle location, so the rule must not prune the
        /// root out from under itself.
        #[test]
        fn loads_a_bundle_that_is_itself_a_dot_directory() {
            let dir = tempfile::tempdir().expect("tempdir");
            write(
                dir.path(),
                ".okf/index.md",
                "---\nokf_version: '0.2'\n---\n",
            );
            write(dir.path(), ".okf/tables/orders.md", "---\ntype: X\n---\n");

            let bundle = Bundle::load(dir.path().join(".okf")).expect("loads");
            assert_eq!(bundle.len(), 2);
            assert!(bundle.contains(&id("tables/orders.md")));
        }

        #[test]
        fn rejects_a_path_that_is_not_a_directory() {
            let dir = tempfile::tempdir().expect("tempdir");
            write(dir.path(), "file.md", "---\ntype: X\n---\n");
            let file = dir.path().join("file.md");

            let error = Bundle::load(&file).expect_err("should reject");
            assert!(matches!(error, LoadError::NotADirectory(_)));
            assert!(error.to_string().contains("is not a directory"));
            assert!(std::error::Error::source(&error).is_none());
        }

        #[test]
        fn rejects_a_missing_directory() {
            let dir = tempfile::tempdir().expect("tempdir");
            let error = Bundle::load(dir.path().join("nope")).expect_err("should reject");
            assert!(matches!(error, LoadError::NotADirectory(_)));
        }

        /// Permissions are the portable-enough way to make the walker itself
        /// fail, as opposed to a single file read failing.
        #[cfg(unix)]
        #[test]
        fn reports_directories_it_cannot_traverse() {
            use std::os::unix::fs::PermissionsExt;

            let dir = tempfile::tempdir().expect("tempdir");
            write(dir.path(), "locked/inner.md", "---\ntype: X\n---\n");
            let locked = dir.path().join("locked");
            std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000))
                .expect("chmod");

            let result = Bundle::load(dir.path());

            std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755))
                .expect("restore");

            let error = result.expect_err("should fail");
            assert!(matches!(error, LoadError::Io { .. }));
            assert!(std::error::Error::source(&error).is_some());
        }

        #[test]
        fn reports_unreadable_files() {
            let dir = tempfile::tempdir().expect("tempdir");
            // Invalid UTF-8 is the portable way to make a read fail.
            std::fs::write(dir.path().join("bad.md"), [0x66, 0x80, 0x6f]).expect("write");

            let error = Bundle::load(dir.path()).expect_err("should fail");
            assert!(matches!(error, LoadError::Io { .. }));
            assert!(error.to_string().starts_with("cannot read"));
            assert!(std::error::Error::source(&error).is_some());
        }
    }
}
