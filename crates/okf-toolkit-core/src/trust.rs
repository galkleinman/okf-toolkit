//! Actors (§7), trust tiers (§5.3), and lifecycle status (§5.4).

/// An identity recorded in `generated.by` or `verified[].by` (§7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actor<'a> {
    /// `human:<id>`. The only form that yields [`TrustTier::HumanReviewed`].
    Human(&'a str),
    /// `process:<id>`.
    Process(&'a str),
    /// `<producer>/<version>`.
    Agent { producer: &'a str, version: &'a str },
    /// Matches none of the three conventions.
    Unrecognized(&'a str),
}

impl<'a> Actor<'a> {
    pub fn parse(text: &'a str) -> Self {
        if let Some(id) = text.strip_prefix("human:") {
            if !id.is_empty() {
                return Self::Human(id);
            }
        }
        if let Some(id) = text.strip_prefix("process:") {
            if !id.is_empty() {
                return Self::Process(id);
            }
        }
        if let Some((producer, version)) = text.split_once('/') {
            if !producer.is_empty() && !version.is_empty() && !version.contains('/') {
                return Self::Agent { producer, version };
            }
        }
        Self::Unrecognized(text)
    }

    pub fn is_human(self) -> bool {
        matches!(self, Self::Human(_))
    }

    /// Whether the actor matches one of the three §7 conventions.
    pub fn is_recognized(self) -> bool {
        !matches!(self, Self::Unrecognized(_))
    }
}

/// A concept's trust level, derived from `verified` rather than stored (§5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrustTier {
    Unverified,
    MachineConfirmed,
    HumanReviewed,
}

impl TrustTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unverified => "unverified",
            Self::MachineConfirmed => "machine-confirmed",
            Self::HumanReviewed => "human-reviewed",
        }
    }
}

impl std::fmt::Display for TrustTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Lifecycle status (§5.4). Absent `status` means [`Status::Stable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    Draft,
    #[default]
    Stable,
    Deprecated,
    /// A value outside the three the spec names; consumers must tolerate it.
    Unknown,
}

impl Status {
    pub fn parse(text: &str) -> Self {
        match text {
            "draft" => Self::Draft,
            "stable" => Self::Stable,
            "deprecated" => Self::Deprecated,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Stable => "stable",
            Self::Deprecated => "deprecated",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_three_actor_conventions() {
        assert_eq!(Actor::parse("human:ahormati"), Actor::Human("ahormati"));
        assert_eq!(
            Actor::parse("process:finance-nightly"),
            Actor::Process("finance-nightly")
        );
        assert_eq!(
            Actor::parse("reference_agent/gemini-2.5-pro"),
            Actor::Agent {
                producer: "reference_agent",
                version: "gemini-2.5-pro"
            }
        );
    }

    #[test]
    fn parses_human_ids_containing_an_at_sign() {
        assert_eq!(
            Actor::parse("human:jsmith@acme"),
            Actor::Human("jsmith@acme")
        );
    }

    #[test]
    fn treats_other_shapes_as_unrecognized() {
        for text in [
            "",
            "plain",
            "human:",
            "process:",
            "team:finance",
            "a/",
            "/b",
            "a/b/c",
        ] {
            assert_eq!(Actor::parse(text), Actor::Unrecognized(text), "{text}");
            assert!(!Actor::parse(text).is_recognized());
        }
    }

    #[test]
    fn only_human_actors_are_human() {
        assert!(Actor::parse("human:x").is_human());
        assert!(!Actor::parse("process:x").is_human());
        assert!(!Actor::parse("agent/v1").is_human());
        assert!(!Actor::parse("nonsense").is_human());
    }

    #[test]
    fn recognized_actors_are_the_three_conventions() {
        assert!(Actor::parse("human:x").is_recognized());
        assert!(Actor::parse("process:x").is_recognized());
        assert!(Actor::parse("agent/v1").is_recognized());
    }

    #[test]
    fn trust_tiers_order_from_unverified_upwards() {
        assert!(TrustTier::Unverified < TrustTier::MachineConfirmed);
        assert!(TrustTier::MachineConfirmed < TrustTier::HumanReviewed);
    }

    #[test]
    fn trust_tiers_render_with_spec_wording() {
        assert_eq!(TrustTier::Unverified.to_string(), "unverified");
        assert_eq!(TrustTier::MachineConfirmed.as_str(), "machine-confirmed");
        assert_eq!(TrustTier::HumanReviewed.as_str(), "human-reviewed");
    }

    #[test]
    fn parses_the_three_statuses() {
        assert_eq!(Status::parse("draft"), Status::Draft);
        assert_eq!(Status::parse("stable"), Status::Stable);
        assert_eq!(Status::parse("deprecated"), Status::Deprecated);
    }

    #[test]
    fn unknown_statuses_are_tolerated() {
        assert_eq!(Status::parse("retired"), Status::Unknown);
        assert_eq!(Status::parse("Stable"), Status::Unknown);
    }

    #[test]
    fn absent_status_defaults_to_stable() {
        assert_eq!(Status::default(), Status::Stable);
    }

    #[test]
    fn statuses_render() {
        assert_eq!(Status::Draft.to_string(), "draft");
        assert_eq!(Status::Stable.as_str(), "stable");
        assert_eq!(Status::Deprecated.as_str(), "deprecated");
        assert_eq!(Status::Unknown.as_str(), "unknown");
    }
}
