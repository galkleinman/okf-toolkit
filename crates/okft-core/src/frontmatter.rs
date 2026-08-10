//! Typed accessors over frontmatter (§4.1, §5, §10).
//!
//! Every accessor is tolerant by design: §11 forbids rejecting a concept for a
//! missing or oddly-shaped optional field, so a field of the wrong type reads
//! as absent rather than as an error.

use crate::date::{Date, Timestamp};
use crate::document::Frontmatter;
use crate::span::Span;
use crate::trust::{Actor, Status, TrustTier};
use crate::value::Node;

/// The `type` value that makes a concept an Attested Computation (§10.1).
pub const ATTESTED_COMPUTATION: &str = "Attested Computation";

/// `generated: { by, at }` (§5.2).
#[derive(Debug, Clone)]
pub struct Generated<'a> {
    pub by: Option<&'a str>,
    pub at: Option<&'a str>,
    pub span: Span,
}

/// One entry of `verified` (§5.2).
#[derive(Debug, Clone)]
pub struct Verification<'a> {
    pub by: Option<&'a str>,
    pub at: Option<&'a str>,
    pub span: Span,
}

/// One entry of `sources` (§5.1).
#[derive(Debug, Clone)]
pub struct Source<'a> {
    pub id: Option<&'a str>,
    pub resource: Option<&'a str>,
    pub title: Option<&'a str>,
    pub author: Option<&'a str>,
    pub usage_count: Option<i64>,
    pub last_modified: Option<&'a str>,
    pub span: Span,
}

impl Frontmatter {
    fn string(&self, key: &str) -> Option<&str> {
        self.entries.get(key).and_then(Node::as_str)
    }

    /// The required `type` field (§4.1), empty values treated as absent.
    pub fn concept_type(&self) -> Option<&str> {
        self.string("type").filter(|value| !value.trim().is_empty())
    }

    pub fn title(&self) -> Option<&str> {
        self.string("title")
    }

    pub fn description(&self) -> Option<&str> {
        self.string("description")
    }

    pub fn resource(&self) -> Option<&str> {
        self.string("resource")
    }

    /// `okf_version`, permitted only in a bundle-root `index.md` (§12).
    pub fn okf_version(&self) -> Option<&str> {
        self.string("okf_version")
    }

    pub fn tags(&self) -> Vec<&str> {
        self.entries
            .get("tags")
            .and_then(Node::as_sequence)
            .map(|items| items.iter().filter_map(Node::as_str).collect())
            .unwrap_or_default()
    }

    pub fn status(&self) -> Status {
        self.string("status").map_or(Status::Stable, Status::parse)
    }

    pub fn stale_after(&self) -> Option<Date> {
        self.string("stale_after").and_then(Date::parse)
    }

    /// The raw `stale_after` value, for reporting a malformed date.
    pub fn stale_after_raw(&self) -> Option<&Node> {
        self.entries.get("stale_after")
    }

    /// The v0.1 `timestamp` field, superseded by `generated.at` (§13.1).
    pub fn legacy_timestamp(&self) -> Option<&Node> {
        self.entries.get("timestamp")
    }

    pub fn generated(&self) -> Option<Generated<'_>> {
        let node = self.entries.get("generated")?;
        let map = node.as_mapping()?;
        Some(Generated {
            by: map.get("by").and_then(Node::as_str),
            at: map.get("at").and_then(Node::as_str),
            span: node.span,
        })
    }

    /// Verification events, always as a list.
    ///
    /// §5.2 allows a single verifier to be written as a bare mapping, and §11
    /// requires consumers to treat that as a one-element list.
    pub fn verified(&self) -> Vec<Verification<'_>> {
        let Some(node) = self.entries.get("verified") else {
            return Vec::new();
        };

        let entries = match &node.value {
            crate::value::Value::Sequence(items) => items.iter().collect::<Vec<_>>(),
            crate::value::Value::Mapping(_) => vec![node],
            _ => return Vec::new(),
        };

        entries
            .into_iter()
            .filter_map(|entry| {
                let map = entry.as_mapping()?;
                Some(Verification {
                    by: map.get("by").and_then(Node::as_str),
                    at: map.get("at").and_then(Node::as_str),
                    span: entry.span,
                })
            })
            .collect()
    }

    pub fn sources(&self) -> Vec<Source<'_>> {
        self.entries
            .get("sources")
            .and_then(Node::as_sequence)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|entry| {
                        let map = entry.as_mapping()?;
                        Some(Source {
                            id: map.get("id").and_then(Node::as_str),
                            resource: map.get("resource").and_then(Node::as_str),
                            title: map.get("title").and_then(Node::as_str),
                            author: map.get("author").and_then(Node::as_str),
                            usage_count: map.get("usage_count").and_then(Node::as_int),
                            last_modified: map.get("last_modified").and_then(Node::as_str),
                            span: entry.span,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The trust tier implied by `verified` (§5.3).
    pub fn trust_tier(&self) -> TrustTier {
        let verifications = self.verified();
        if verifications.is_empty() {
            return TrustTier::Unverified;
        }
        if verifications
            .iter()
            .filter_map(|v| v.by)
            .any(|by| Actor::parse(by).is_human())
        {
            TrustTier::HumanReviewed
        } else {
            TrustTier::MachineConfirmed
        }
    }

    /// The most recent parseable `verified[].at` (§5.2).
    pub fn last_verified_at(&self) -> Option<Timestamp> {
        self.verified()
            .iter()
            .filter_map(|v| v.at)
            .filter_map(Timestamp::parse)
            .max()
    }

    /// Whether the concept is stale on `today`, i.e. `today >= stale_after` (§5.5).
    pub fn is_stale_on(&self, today: Date) -> bool {
        self.stale_after()
            .is_some_and(|stale_after| today >= stale_after)
    }

    /// Whether this concept is an Attested Computation (§10.1).
    pub fn is_attested_computation(&self) -> bool {
        self.concept_type() == Some(ATTESTED_COMPUTATION)
    }

    /// `runtime`, required for an Attested Computation (§10.2).
    pub fn runtime(&self) -> Option<&str> {
        self.string("runtime")
    }

    /// Every path-valued field, paired with the span to blame (§6.2).
    ///
    /// `sources[].resource` is excluded: §5.1 allows it to be a scope
    /// descriptor rather than a path, so it cannot be checked as one.
    pub fn path_fields(&self) -> Vec<(&str, Span)> {
        let mut paths = Vec::new();
        if let Some(node) = self.entries.get("computation")
            && let Some(text) = node.as_str()
        {
            paths.push((text, node.span));
        }
        for key in ["executor", "attester"] {
            if let Some(node) = self.entries.get(key)
                && let Some(resource) = node.as_mapping().and_then(|map| map.get("resource"))
                && let Some(text) = resource.as_str()
            {
                paths.push((text, resource.span));
            }
        }
        paths
    }

    /// Every actor string in the concept, with the span to blame for each.
    pub fn actors(&self) -> Vec<(&str, Span)> {
        let mut actors = Vec::new();
        if let Some(generated) = self.generated()
            && let Some(by) = generated.by
        {
            actors.push((by, generated.span));
        }
        for verification in self.verified() {
            if let Some(by) = verification.by {
                actors.push((by, verification.span));
            }
        }
        actors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;

    fn frontmatter(yaml: &str) -> Frontmatter {
        let source = format!("---\n{yaml}---\nbody\n");
        Document::parse(&source)
            .frontmatter
            .parsed()
            .expect("parsed")
            .clone()
    }

    #[test]
    fn reads_the_recommended_scalar_fields() {
        let fm = frontmatter(
            "type: BigQuery Table\ntitle: Orders\ndescription: One row per order.\n\
             resource: https://example.com/t\n",
        );
        assert_eq!(fm.concept_type(), Some("BigQuery Table"));
        assert_eq!(fm.title(), Some("Orders"));
        assert_eq!(fm.description(), Some("One row per order."));
        assert_eq!(fm.resource(), Some("https://example.com/t"));
    }

    #[test]
    fn blank_type_reads_as_absent() {
        assert_eq!(frontmatter("type: '   '\n").concept_type(), None);
        assert_eq!(frontmatter("type: ''\n").concept_type(), None);
        assert_eq!(frontmatter("title: only\n").concept_type(), None);
    }

    #[test]
    fn non_string_fields_read_as_absent() {
        let fm = frontmatter("type: [a]\ntitle: 12\ndescription: {a: b}\n");
        assert_eq!(fm.concept_type(), None);
        assert_eq!(fm.title(), None);
        assert_eq!(fm.description(), None);
        assert_eq!(fm.resource(), None);
    }

    #[test]
    fn reads_tags_skipping_non_strings() {
        assert_eq!(
            frontmatter("tags: [sales, orders]\n").tags(),
            ["sales", "orders"]
        );
        assert_eq!(frontmatter("tags: [ok, 12, [nested]]\n").tags(), ["ok"]);
        assert!(frontmatter("tags: not-a-list\n").tags().is_empty());
        assert!(frontmatter("type: X\n").tags().is_empty());
    }

    #[test]
    fn status_defaults_to_stable_when_absent() {
        assert_eq!(frontmatter("type: X\n").status(), Status::Stable);
        assert_eq!(frontmatter("status: draft\n").status(), Status::Draft);
        assert_eq!(frontmatter("status: [x]\n").status(), Status::Stable);
    }

    #[test]
    fn reads_stale_after_and_staleness() {
        let fm = frontmatter("stale_after: 2026-09-23\n");
        assert_eq!(fm.stale_after(), Date::parse("2026-09-23"));
        assert!(fm.stale_after_raw().is_some());

        assert!(fm.is_stale_on(Date::parse("2026-09-23").expect("valid")));
        assert!(fm.is_stale_on(Date::parse("2026-09-24").expect("valid")));
        assert!(!fm.is_stale_on(Date::parse("2026-09-22").expect("valid")));
    }

    #[test]
    fn a_concept_without_stale_after_is_never_stale() {
        let fm = frontmatter("type: X\n");
        assert!(!fm.is_stale_on(Date::parse("2099-01-01").expect("valid")));
        assert!(fm.stale_after().is_none());
        assert!(fm.stale_after_raw().is_none());
    }

    #[test]
    fn a_malformed_stale_after_is_readable_but_unparsed() {
        let fm = frontmatter("stale_after: soon\n");
        assert!(fm.stale_after().is_none());
        assert!(fm.stale_after_raw().is_some());
    }

    #[test]
    fn reads_generated() {
        let fm = frontmatter("generated: { by: agent/v1, at: 2026-06-20T22:53:05Z }\n");
        let generated = fm.generated().expect("generated");
        assert_eq!(generated.by, Some("agent/v1"));
        assert_eq!(generated.at, Some("2026-06-20T22:53:05Z"));
        assert_eq!(generated.span.start.line, 2);
    }

    #[test]
    fn generated_of_the_wrong_shape_is_absent() {
        assert!(frontmatter("generated: agent/v1\n").generated().is_none());
        assert!(frontmatter("type: X\n").generated().is_none());
        let fm = frontmatter("generated: { by: agent/v1 }\n");
        let partial = fm.generated().expect("present");
        assert_eq!(partial.at, None);
    }

    /// §5.2 and §11: a bare mapping must be read as a one-element list.
    #[test]
    fn a_bare_verified_mapping_becomes_a_one_element_list() {
        let fm = frontmatter("verified: { by: human:ahormati, at: 2026-06-25T09:00:00Z }\n");
        let verified = fm.verified();
        assert_eq!(verified.len(), 1);
        assert_eq!(verified[0].by, Some("human:ahormati"));
        assert_eq!(fm.trust_tier(), TrustTier::HumanReviewed);
    }

    #[test]
    fn reads_a_verified_list() {
        let fm = frontmatter(
            "verified:\n  - { by: human:a, at: 2026-06-25T09:00:00Z }\n  \
             - { by: process:n, at: 2026-06-26T02:00:00Z }\n",
        );
        assert_eq!(fm.verified().len(), 2);
        assert_eq!(fm.trust_tier(), TrustTier::HumanReviewed);
    }

    #[test]
    fn trust_tiers_follow_the_verifiers() {
        assert_eq!(frontmatter("type: X\n").trust_tier(), TrustTier::Unverified);
        assert_eq!(
            frontmatter("verified: [{ by: process:n, at: 2026-01-01T00:00:00Z }]\n").trust_tier(),
            TrustTier::MachineConfirmed
        );
        assert_eq!(
            frontmatter("verified: [{ by: agent/v1, at: 2026-01-01T00:00:00Z }]\n").trust_tier(),
            TrustTier::MachineConfirmed
        );
        assert_eq!(
            frontmatter("verified: [{ at: 2026-01-01T00:00:00Z }]\n").trust_tier(),
            TrustTier::MachineConfirmed
        );
    }

    #[test]
    fn malformed_verified_yields_no_entries() {
        assert!(
            frontmatter("verified: just-a-string\n")
                .verified()
                .is_empty()
        );
        assert!(frontmatter("verified: [plain, 12]\n").verified().is_empty());
        assert_eq!(
            frontmatter("verified: just-a-string\n").trust_tier(),
            TrustTier::Unverified
        );
    }

    #[test]
    fn finds_the_latest_verification() {
        let fm = frontmatter(
            "verified:\n  - { by: human:a, at: 2026-06-25T09:00:00Z }\n  \
             - { by: process:n, at: 2026-06-26T02:00:00Z }\n  - { by: process:m, at: bogus }\n",
        );
        assert_eq!(
            fm.last_verified_at().map(|t| t.date()),
            Date::parse("2026-06-26")
        );
        assert!(frontmatter("type: X\n").last_verified_at().is_none());
    }

    #[test]
    fn reads_sources_with_credibility_signals() {
        let fm = frontmatter(
            "sources:\n  - id: ga4-schema\n    resource: https://example.com/s\n    \
             title: GA4 schema\n    author: team:ga4-docs\n    usage_count: 5000\n    \
             last_modified: 2026-05-30\n",
        );
        let sources = fm.sources();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].id, Some("ga4-schema"));
        assert_eq!(sources[0].resource, Some("https://example.com/s"));
        assert_eq!(sources[0].title, Some("GA4 schema"));
        assert_eq!(sources[0].author, Some("team:ga4-docs"));
        assert_eq!(sources[0].usage_count, Some(5000));
        assert_eq!(sources[0].last_modified, Some("2026-05-30"));
    }

    #[test]
    fn malformed_sources_are_skipped() {
        assert!(frontmatter("sources: nope\n").sources().is_empty());
        assert!(
            frontmatter("sources: [plain-string]\n")
                .sources()
                .is_empty()
        );
        assert!(frontmatter("type: X\n").sources().is_empty());
        let fm = frontmatter("sources:\n  - title: no resource\n");
        let partial = fm.sources();
        assert_eq!(partial.len(), 1);
        assert_eq!(partial[0].resource, None);
    }

    #[test]
    fn collects_actors_from_both_families() {
        let fm = frontmatter(
            "generated: { by: agent/v1, at: 2026-01-01T00:00:00Z }\n\
             verified:\n  - { by: human:a, at: 2026-01-02T00:00:00Z }\n  \
             - { at: 2026-01-03T00:00:00Z }\n",
        );
        let actors: Vec<_> = fm.actors().into_iter().map(|(actor, _)| actor).collect();
        assert_eq!(actors, ["agent/v1", "human:a"]);
    }

    #[test]
    fn actors_is_empty_without_trust_fields() {
        assert!(frontmatter("type: X\n").actors().is_empty());
        assert!(
            frontmatter("generated: { at: 2026-01-01T00:00:00Z }\n")
                .actors()
                .is_empty()
        );
    }

    #[test]
    fn identifies_attested_computations_and_their_runtime() {
        let fm = frontmatter("type: Attested Computation\nruntime: bigquery\n");
        assert!(fm.is_attested_computation());
        assert_eq!(fm.runtime(), Some("bigquery"));

        let other = frontmatter("type: Metric\n");
        assert!(!other.is_attested_computation());
        assert_eq!(other.runtime(), None);
    }

    #[test]
    fn collects_path_valued_fields() {
        let fm = frontmatter(
            "computation: references/computations/revenue.sql\n\
             executor:\n  resource: references/skills/run-on-bq.md\n  \
             receipt: [job_id]\nattester:\n  resource: references/attesters/revenue.py\n",
        );
        let paths: Vec<_> = fm.path_fields().into_iter().map(|(path, _)| path).collect();
        assert_eq!(
            paths,
            [
                "references/computations/revenue.sql",
                "references/skills/run-on-bq.md",
                "references/attesters/revenue.py",
            ]
        );
    }

    #[test]
    fn path_fields_skips_absent_and_malformed_shapes() {
        assert!(frontmatter("type: X\n").path_fields().is_empty());
        assert!(
            frontmatter("computation: [a]\nexecutor: plain\n")
                .path_fields()
                .is_empty()
        );
        assert!(
            frontmatter("executor:\n  receipt: [job_id]\n")
                .path_fields()
                .is_empty()
        );
        assert!(
            frontmatter("attester:\n  resource: [nested]\n")
                .path_fields()
                .is_empty()
        );
    }

    #[test]
    fn reads_the_legacy_timestamp_and_okf_version() {
        assert!(
            frontmatter("timestamp: '2026-05-28T22:53:05+00:00'\n")
                .legacy_timestamp()
                .is_some()
        );
        assert!(frontmatter("type: X\n").legacy_timestamp().is_none());
        assert_eq!(
            frontmatter("okf_version: '0.2'\n").okf_version(),
            Some("0.2")
        );
        assert_eq!(frontmatter("type: X\n").okf_version(), None);
    }
}
