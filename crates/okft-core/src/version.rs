//! The OKF revision a run targets (§12).
//!
//! §12 versions the specification as `<major>.<minor>` and lets a bundle
//! declare the revision it targets with `okf_version` in its bundle-root
//! `index.md`. §13 makes v0.2 a superset of v0.1 apart from the two constructs
//! §13.1 supersedes, so targeting an older revision means *withholding* rules
//! rather than adding them. Which rules those are is recorded once, on
//! [`crate::diagnostic::Rule::since`].

use std::fmt;
use std::str::FromStr;

/// A revision of the Open Knowledge Format (§12).
///
/// Ordered oldest to newest so `target >= rule.since` decides whether a rule
/// has anything to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum OkfVersion {
    /// v0.1: none of the `sources`, `generated`, `verified`, `status`,
    /// `stale_after`, or Attested Computation families exist yet (§13.2), and
    /// provenance lives in `timestamp` and a body `# Citations` list (§13.1).
    V0_1,
    /// v0.2, the revision this toolkit implements.
    #[default]
    V0_2,
}

/// Every revision this toolkit understands, oldest first.
pub const SUPPORTED: [OkfVersion; 2] = [OkfVersion::V0_1, OkfVersion::V0_2];

/// The newest supported revision, and the default when nothing says otherwise.
pub const LATEST: OkfVersion = OkfVersion::V0_2;

impl OkfVersion {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V0_1 => "0.1",
            Self::V0_2 => "0.2",
        }
    }

    /// Parses a declared or requested revision, tolerating a `v` prefix.
    ///
    /// Bundles write `okf_version: "0.2"` while people type `v0.2`, and
    /// rejecting one of those spellings would be pedantry rather than rigour.
    pub fn parse(text: &str) -> Option<Self> {
        let trimmed = text.trim();
        let normalized = trimmed.strip_prefix(['v', 'V']).unwrap_or(trimmed);
        SUPPORTED
            .into_iter()
            .find(|version| version.as_str() == normalized)
    }

    /// The supported revisions as `0.1, 0.2`, for error messages.
    fn supported_list() -> String {
        SUPPORTED.map(OkfVersion::as_str).join(", ")
    }
}

impl fmt::Display for OkfVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for OkfVersion {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text).ok_or_else(|| {
            format!(
                "unknown OKF version `{text}`; supported versions are {}",
                Self::supported_list()
            )
        })
    }
}

/// Resolves the revision a run targets.
///
/// An explicit request wins: pinning a version is a deliberate statement about
/// which consumer is being emulated, and it has to be able to contradict a
/// bundle whose declaration is what the caller is investigating. Otherwise the
/// bundle's own `okf_version` decides (§12), and an absent or unrecognised
/// declaration falls back to the latest revision.
pub fn resolve(requested: Option<OkfVersion>, declared: Option<&str>) -> OkfVersion {
    requested
        .or_else(|| declared.and_then(OkfVersion::parse))
        .unwrap_or(LATEST)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revisions_are_ordered_oldest_first() {
        assert!(OkfVersion::V0_1 < OkfVersion::V0_2);
        assert_eq!(SUPPORTED, [OkfVersion::V0_1, OkfVersion::V0_2]);
        assert_eq!(LATEST, OkfVersion::V0_2);
        assert_eq!(OkfVersion::default(), LATEST);
    }

    #[test]
    fn renders_as_a_bare_version_number() {
        assert_eq!(OkfVersion::V0_1.as_str(), "0.1");
        assert_eq!(OkfVersion::V0_2.to_string(), "0.2");
    }

    #[test]
    fn parses_every_spelling_a_caller_might_use() {
        assert_eq!(OkfVersion::parse("0.1"), Some(OkfVersion::V0_1));
        assert_eq!(OkfVersion::parse("v0.2"), Some(OkfVersion::V0_2));
        assert_eq!(OkfVersion::parse("V0.2"), Some(OkfVersion::V0_2));
        assert_eq!(OkfVersion::parse("  0.2  "), Some(OkfVersion::V0_2));
    }

    #[test]
    fn rejects_revisions_it_does_not_implement() {
        assert_eq!(OkfVersion::parse("0.3"), None);
        assert_eq!(OkfVersion::parse("1.0"), None);
        assert_eq!(OkfVersion::parse(""), None);
        assert_eq!(OkfVersion::parse("vv0.2"), None);
        assert_eq!(OkfVersion::parse("0.2.0"), None);
    }

    #[test]
    fn from_str_names_the_supported_revisions() {
        assert_eq!("0.1".parse(), Ok(OkfVersion::V0_1));
        let error = "0.9".parse::<OkfVersion>().expect_err("unsupported");
        assert!(error.contains("`0.9`"));
        assert!(error.contains("0.1, 0.2"));
    }

    #[test]
    fn an_explicit_request_beats_the_declaration() {
        assert_eq!(
            resolve(Some(OkfVersion::V0_1), Some("0.2")),
            OkfVersion::V0_1
        );
    }

    #[test]
    fn the_declaration_is_used_when_nothing_was_requested() {
        assert_eq!(resolve(None, Some("0.1")), OkfVersion::V0_1);
        assert_eq!(resolve(None, Some("v0.2")), OkfVersion::V0_2);
    }

    #[test]
    fn an_absent_or_unreadable_declaration_falls_back_to_the_latest() {
        assert_eq!(resolve(None, None), LATEST);
        assert_eq!(resolve(None, Some("0.9")), LATEST);
    }
}
