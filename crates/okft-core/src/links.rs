//! Cross-link and footnote extraction from a document body (§5.1, §6.1).

use pulldown_cmark::{Event, Options, Parser, Tag};

use crate::concept_id::ConceptId;
use crate::document::Body;
use crate::span::Span;

/// Which of the two forms in §6.1 a link uses, or an external target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    /// Begins with `/`; resolved against the bundle root. The recommended form.
    BundleAbsolute,
    /// A normal markdown relative path, resolved against the document.
    Relative,
    /// Anything with a URI scheme; never resolved against the bundle.
    External,
}

/// A markdown link found in a document body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// The destination exactly as written, fragment included.
    pub target: String,
    pub kind: LinkKind,
    pub span: Span,
}

/// A `[^label]` reference, whose label joins to `sources[].id` (§5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FootnoteReference {
    pub label: String,
    pub span: Span,
}

/// A markdown heading, with the level needed by the `log.md` rules (§9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub span: Span,
}

/// Everything extracted from one body in a single parse.
#[derive(Debug, Clone, Default)]
pub struct BodyLinks {
    pub links: Vec<Link>,
    pub footnotes: Vec<FootnoteReference>,
    /// Labels of `[^label]:` definitions present in the body.
    pub footnote_definitions: Vec<String>,
    pub headings: Vec<Heading>,
}

impl BodyLinks {
    /// Headings at a given level, in document order.
    pub fn headings_at(&self, level: u8) -> impl Iterator<Item = &Heading> {
        self.headings
            .iter()
            .filter(move |heading| heading.level == level)
    }
}

impl Link {
    /// Resolves the link to a concept in the bundle.
    ///
    /// `from` is the linking document's own ID, used as the base for relative
    /// paths. Returns `None` for external links, non-`.md` targets, and paths
    /// that climb out of the bundle root.
    pub fn resolve(&self, from: &ConceptId) -> Option<ConceptId> {
        let path = self.target.split(['#', '?']).next()?;
        // Deliberately case-sensitive: `.md` is the extension the spec uses, and
        // a link to `README.MD` should not silently resolve to a concept. An
        // empty target has no extension, so it is rejected here too.
        if std::path::Path::new(path)
            .extension()
            .is_none_or(|extension| extension != "md")
        {
            return None;
        }

        let segments: Vec<&str> = match self.kind {
            LinkKind::External => return None,
            LinkKind::BundleAbsolute => path.trim_start_matches('/').split('/').collect(),
            LinkKind::Relative => {
                let mut base: Vec<&str> = from
                    .parent()
                    .map(|p| p.split('/').collect())
                    .unwrap_or_default();
                base.extend(path.split('/'));
                base
            }
        };

        let mut resolved: Vec<&str> = Vec::with_capacity(segments.len());
        for segment in segments {
            match segment {
                "." | "" => {}
                ".." => {
                    resolved.pop()?;
                }
                other => resolved.push(other),
            }
        }

        // Segments are already normalised, so this only rejects empty results.
        ConceptId::from_relative_path(std::path::Path::new(&resolved.join("/")))
    }
}

fn classify(target: &str) -> LinkKind {
    if target.starts_with('/') {
        LinkKind::BundleAbsolute
    } else if has_uri_scheme(target) {
        LinkKind::External
    } else {
        LinkKind::Relative
    }
}

/// Detects a URI scheme without treating a Windows drive letter or a bare
/// `path:with:colons` as one.
fn has_uri_scheme(target: &str) -> bool {
    let Some((scheme, rest)) = target.split_once(':') else {
        return false;
    };
    !scheme.is_empty()
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        && (scheme.len() > 1 || rest.starts_with("//"))
}

/// Extracts links, footnotes, and headings from a body in one pass.
pub fn extract(body: &Body) -> BodyLinks {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let mut found = BodyLinks::default();
    let mut open_heading: Option<(u8, Span)> = None;
    let mut heading_text = String::new();

    for (event, range) in Parser::new_ext(&body.text, options).into_offset_iter() {
        match event {
            Event::Start(Tag::Link { dest_url, .. }) => {
                let target = dest_url.into_string();
                found.links.push(Link {
                    kind: classify(&target),
                    target,
                    span: body.span_at(range),
                });
            }
            Event::FootnoteReference(label) => found.footnotes.push(FootnoteReference {
                label: label.into_string(),
                span: body.span_at(range),
            }),
            Event::Start(Tag::FootnoteDefinition(label)) => {
                found.footnote_definitions.push(label.into_string());
            }
            Event::Start(Tag::Heading { level, .. }) => {
                heading_text.clear();
                open_heading = Some((level as u8, body.span_at(range)));
            }
            Event::End(pulldown_cmark::TagEnd::Heading(_)) => {
                if let Some((level, span)) = open_heading.take() {
                    found.headings.push(Heading {
                        level,
                        text: heading_text.trim().to_owned(),
                        span,
                    });
                }
                heading_text.clear();
            }
            Event::Text(text) | Event::Code(text) if open_heading.is_some() => {
                heading_text.push_str(&text);
            }
            _ => {}
        }
    }

    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(text: &str) -> Body {
        Body {
            text: text.to_owned(),
            start_line: 1,
        }
    }

    fn id(path: &str) -> ConceptId {
        ConceptId::from_relative_path(std::path::Path::new(path)).expect("valid id")
    }

    #[test]
    fn classifies_the_two_link_forms_and_external_targets() {
        let found = extract(&body(
            "[a](/tables/orders.md) [b](./other.md) [c](https://example.com) [d](mailto:x@y.z)\n",
        ));
        let kinds: Vec<_> = found.links.iter().map(|l| l.kind).collect();
        assert_eq!(
            kinds,
            [
                LinkKind::BundleAbsolute,
                LinkKind::Relative,
                LinkKind::External,
                LinkKind::External
            ]
        );
    }

    /// The reference implementation drops bundle-absolute links, which §6.1
    /// actually *recommends*; both forms must resolve here.
    #[test]
    fn resolves_both_recommended_and_relative_forms() {
        let found = extract(&body("[a](/tables/orders.md) [b](../metrics/revenue.md)\n"));
        let from = id("tables/customers.md");
        assert_eq!(found.links[0].resolve(&from), Some(id("tables/orders.md")));
        assert_eq!(
            found.links[1].resolve(&from),
            Some(id("metrics/revenue.md"))
        );
    }

    #[test]
    fn resolves_relative_links_from_the_bundle_root() {
        let found = extract(&body("[a](tables/orders.md)\n"));
        assert_eq!(
            found.links[0].resolve(&id("index.md")),
            Some(id("tables/orders.md"))
        );
    }

    #[test]
    fn normalises_dot_segments() {
        let found = extract(&body("[a](./sub/../orders.md) [b](/./a/b/../c.md)\n"));
        let from = id("tables/customers.md");
        assert_eq!(found.links[0].resolve(&from), Some(id("tables/orders.md")));
        assert_eq!(found.links[1].resolve(&from), Some(id("a/c.md")));
    }

    #[test]
    fn strips_fragments_and_queries_before_resolving() {
        let found = extract(&body(
            "[a](/tables/orders.md#schema) [b](/tables/orders.md?v=1)\n",
        ));
        let from = id("index.md");
        assert_eq!(found.links[0].resolve(&from), Some(id("tables/orders.md")));
        assert_eq!(found.links[1].resolve(&from), Some(id("tables/orders.md")));
    }

    #[test]
    fn does_not_resolve_external_or_non_markdown_targets() {
        let found = extract(&body(
            "[a](https://example.com/x.md) [b](/tables/orders.csv) [c](subdir/) [d](#anchor)\n",
        ));
        let from = id("index.md");
        for link in &found.links {
            assert_eq!(
                link.resolve(&from),
                None,
                "{} should not resolve",
                link.target
            );
        }
    }

    #[test]
    fn refuses_to_climb_out_of_the_bundle() {
        let found = extract(&body("[a](../../outside.md)\n"));
        assert_eq!(found.links[0].resolve(&id("tables/orders.md")), None);
    }

    #[test]
    fn resolves_to_none_when_the_path_is_only_dots() {
        let found = extract(&body("[a](./.md)\n"));
        assert_eq!(found.links[0].resolve(&id("index.md")), None);
    }

    #[test]
    fn records_link_spans_against_the_file() {
        let found = extract(&Body {
            text: "line one\n\nsee [orders](/tables/orders.md)\n".to_owned(),
            start_line: 5,
        });
        assert_eq!(found.links.len(), 1);
        assert_eq!(found.links[0].span.start.line, 7);
    }

    #[test]
    fn collects_footnote_references_and_definitions() {
        let found = extract(&body(
            "Sharded daily.[^ga4-schema]\n\n[^ga4-schema]: GA4 BigQuery Export schema\n",
        ));
        assert_eq!(found.footnotes.len(), 1);
        assert_eq!(found.footnotes[0].label, "ga4-schema");
        assert_eq!(found.footnote_definitions, ["ga4-schema"]);
        assert_eq!(found.footnotes[0].span.start.line, 1);
    }

    #[test]
    fn collects_headings_with_their_levels() {
        let found = extract(&body("# Schema\n\n## Columns\n\n# Citations\n\ntext\n"));
        let levels: Vec<_> = found
            .headings
            .iter()
            .map(|h| (h.level, h.text.as_str()))
            .collect();
        assert_eq!(levels, [(1, "Schema"), (2, "Columns"), (1, "Citations")]);

        let top: Vec<_> = found.headings_at(1).map(|h| h.text.as_str()).collect();
        assert_eq!(top, ["Schema", "Citations"]);
        assert_eq!(found.headings_at(2).count(), 1);
        assert_eq!(found.headings_at(3).count(), 0);
    }

    #[test]
    fn heading_text_includes_inline_code() {
        let found = extract(&body("# The `orders` table\n"));
        assert_eq!(found.headings[0].text, "The orders table");
    }

    #[test]
    fn heading_spans_point_at_the_file() {
        let found = extract(&Body {
            text: "intro\n\n## 2026-05-22\n".to_owned(),
            start_line: 6,
        });
        let heading = found.headings_at(2).next().expect("heading");
        assert_eq!(heading.span.start.line, 8);
    }

    #[test]
    fn links_inside_headings_are_still_collected() {
        let found = extract(&body("# See [orders](/t/orders.md)\n"));
        assert_eq!(found.links.len(), 1);
        assert_eq!(found.headings[0].text, "See orders");
    }

    #[test]
    fn ignores_links_inside_fenced_code_blocks() {
        let found = extract(&body("```\n[not a link](/x.md)\n```\n"));
        assert!(found.links.is_empty());
    }

    #[test]
    fn finds_links_inside_tables() {
        let found = extract(&body(
            "| Column | Description |\n|---|---|\n| a | [c](/t/c.md) |\n",
        ));
        assert_eq!(found.links.len(), 1);
        assert_eq!(found.links[0].resolve(&id("index.md")), Some(id("t/c.md")));
    }

    #[test]
    fn empty_body_yields_nothing() {
        let found = extract(&body(""));
        assert!(found.links.is_empty());
        assert!(found.footnotes.is_empty());
        assert!(found.footnote_definitions.is_empty());
        assert!(found.headings.is_empty());
    }

    #[test]
    fn distinguishes_schemes_from_colons_in_paths() {
        assert!(has_uri_scheme("https://example.com"));
        assert!(has_uri_scheme("mailto:someone@example.com"));
        assert!(has_uri_scheme("c://drive"));
        assert!(!has_uri_scheme("C:/Users/file.md"));
        assert!(!has_uri_scheme("plain.md"));
        assert!(!has_uri_scheme(":leading.md"));
        assert!(!has_uri_scheme("1scheme:x"));
        assert!(!has_uri_scheme("has space:x"));
    }

    #[test]
    fn resolve_returns_none_for_an_empty_target() {
        let link = Link {
            target: String::new(),
            kind: LinkKind::Relative,
            span: Span::default(),
        };
        assert_eq!(link.resolve(&id("index.md")), None);
    }
}
