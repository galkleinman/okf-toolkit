//! Splitting a markdown file into OKF frontmatter and body (§4).

use crate::span::{Position, Span};
use crate::value::{Mapping, Node, ParseError, Value};

const DELIMITER: &str = "---";

/// A parsed OKF markdown file: YAML frontmatter plus a markdown body (§4).
#[derive(Debug, Clone)]
pub struct Document {
    pub frontmatter: FrontmatterState,
    pub body: Body,
}

/// Why a document has no usable frontmatter.
///
/// Kept distinct from "absent" because §11.1 requires a *parseable* block in
/// concept documents, while reserved files (§8, §9) legitimately have none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontmatterError {
    /// An opening `---` with no closing `---`.
    Unterminated { span: Span },
    /// The block is present but is not valid YAML.
    Invalid { message: String, span: Span },
    /// The block parsed, but its top level is not a key/value mapping.
    NotAMapping { span: Span },
}

impl FrontmatterError {
    pub fn span(&self) -> Span {
        match self {
            Self::Unterminated { span }
            | Self::Invalid { span, .. }
            | Self::NotAMapping { span } => *span,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Unterminated { .. } => {
                "frontmatter block is opened with `---` but never closed".to_owned()
            }
            Self::Invalid { message, .. } => format!("frontmatter is not valid YAML: {message}"),
            Self::NotAMapping { .. } => {
                "frontmatter must be a mapping of keys to values".to_owned()
            }
        }
    }
}

/// The three frontmatter outcomes a validator must tell apart.
#[derive(Debug, Clone)]
pub enum FrontmatterState {
    /// No opening `---`; the file is body-only.
    Absent,
    Parsed(Frontmatter),
    Malformed(FrontmatterError),
}

impl FrontmatterState {
    pub fn parsed(&self) -> Option<&Frontmatter> {
        match self {
            Self::Parsed(frontmatter) => Some(frontmatter),
            Self::Absent | Self::Malformed(_) => None,
        }
    }

    pub fn error(&self) -> Option<&FrontmatterError> {
        match self {
            Self::Malformed(error) => Some(error),
            Self::Absent | Self::Parsed(_) => None,
        }
    }

    pub fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }
}

/// A parsed frontmatter mapping and the source range it occupied.
#[derive(Debug, Clone)]
pub struct Frontmatter {
    pub entries: Mapping,
    pub span: Span,
}

/// A document body, retaining the line it starts on so spans map to the file.
#[derive(Debug, Clone)]
pub struct Body {
    pub text: String,
    pub start_line: usize,
}

impl Body {
    /// Converts a byte offset within [`Body::text`] into a file position.
    pub fn position_at(&self, offset: usize) -> Position {
        let clamped = offset.min(self.text.len());
        let preceding = &self.text[..clamped];
        let newlines = preceding.matches('\n').count();
        let column = preceding.rfind('\n').map_or(clamped, |i| clamped - i - 1);
        Position::new(self.start_line + newlines, column + 1)
    }

    /// Converts a byte range within [`Body::text`] into a file span.
    pub fn span_at(&self, range: std::ops::Range<usize>) -> Span {
        Span::new(self.position_at(range.start), self.position_at(range.end))
    }
}

impl Document {
    /// Splits `source` into frontmatter and body.
    ///
    /// Never fails: a malformed block is reported through
    /// [`FrontmatterState::Malformed`] so callers still get the body, and so a
    /// single file can produce several diagnostics rather than only the first.
    pub fn parse(source: &str) -> Self {
        let source = source.strip_prefix('\u{feff}').unwrap_or(source);

        let Some(after_open) = strip_opening_delimiter(source) else {
            return Self {
                frontmatter: FrontmatterState::Absent,
                body: Body {
                    text: source.to_owned(),
                    start_line: 1,
                },
            };
        };

        let Some((yaml, body, body_start_line)) = split_at_closing_delimiter(after_open) else {
            return Self {
                frontmatter: FrontmatterState::Malformed(FrontmatterError::Unterminated {
                    span: Span::at(Position::new(1, 1)),
                }),
                body: Body {
                    text: String::new(),
                    start_line: 1,
                },
            };
        };

        let yaml_line_count = yaml.lines().count();
        let block_span = Span::new(
            Position::new(1, 1),
            Position::new(yaml_line_count + 2, DELIMITER.len() + 1),
        );

        let frontmatter = match Node::parse(yaml) {
            Err(ParseError { message, position }) => {
                FrontmatterState::Malformed(FrontmatterError::Invalid {
                    message,
                    span: Span::at(Position::new(position.line + 1, position.column)),
                })
            }
            // An empty block is an empty mapping: no keys, but structurally present.
            Ok(node) if node.is_null() => FrontmatterState::Parsed(Frontmatter {
                entries: Mapping::default(),
                span: block_span,
            }),
            Ok(node) => match node.value {
                Value::Mapping(entries) => FrontmatterState::Parsed(Frontmatter {
                    entries: shift_mapping(entries),
                    span: block_span,
                }),
                _ => FrontmatterState::Malformed(FrontmatterError::NotAMapping {
                    span: node.span.offset_lines(1),
                }),
            },
        };

        Self {
            frontmatter,
            body: Body {
                text: body.to_owned(),
                start_line: body_start_line,
            },
        }
    }
}

/// Re-anchors frontmatter spans onto the file, past the opening `---`.
fn shift_mapping(mapping: Mapping) -> Mapping {
    Mapping::from_entries(
        mapping
            .entries()
            .iter()
            .map(|(key, value)| (shift_node(key), shift_node(value)))
            .collect(),
    )
}

fn shift_node(node: &Node) -> Node {
    let value = match &node.value {
        Value::Sequence(items) => Value::Sequence(items.iter().map(shift_node).collect()),
        Value::Mapping(map) => Value::Mapping(shift_mapping(map.clone())),
        other => other.clone(),
    };
    Node::new(value, node.span.offset_lines(1))
}

fn strip_opening_delimiter(source: &str) -> Option<&str> {
    let rest = source.strip_prefix(DELIMITER)?;
    match rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))
    {
        Some(after) => Some(after),
        // A file consisting of exactly `---` opens a block that never closes.
        None if rest.is_empty() => Some(rest),
        None => None,
    }
}

/// Finds the closing `---` line, returning the YAML, the body, and the body's
/// first line number.
fn split_at_closing_delimiter(after_open: &str) -> Option<(&str, &str, usize)> {
    let mut offset = 0;
    let mut line_number = 2;

    for line in after_open.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == DELIMITER {
            let yaml = &after_open[..offset];
            let body = &after_open[offset + line.len()..];
            return Some((yaml, body, line_number + 1));
        }
        offset += line.len();
        line_number += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_frontmatter_from_body() {
        let doc = Document::parse("---\ntype: Metric\n---\n# Heading\n\nText.\n");
        let frontmatter = doc.frontmatter.parsed().expect("parsed");
        assert_eq!(
            frontmatter.entries.get("type").and_then(Node::as_str),
            Some("Metric")
        );
        assert_eq!(doc.body.text, "# Heading\n\nText.\n");
        assert_eq!(doc.body.start_line, 4);
        assert!(doc.frontmatter.error().is_none());
        assert!(!doc.frontmatter.is_absent());
    }

    #[test]
    fn frontmatter_spans_point_at_file_lines() {
        let doc = Document::parse("---\ntype: Metric\ntitle: T\n---\nbody\n");
        let frontmatter = doc.frontmatter.parsed().expect("parsed");
        assert_eq!(
            frontmatter
                .entries
                .get("type")
                .expect("type")
                .span
                .start
                .line,
            2
        );
        assert_eq!(
            frontmatter
                .entries
                .get("title")
                .expect("title")
                .span
                .start
                .line,
            3
        );
        assert_eq!(frontmatter.span.start.line, 1);
    }

    #[test]
    fn nested_spans_are_shifted_too() {
        let doc = Document::parse("---\nsources:\n  - id: a\n    resource: r\ntags: [x]\n---\n");
        let frontmatter = doc.frontmatter.parsed().expect("parsed");
        let sources = frontmatter.entries.get("sources").expect("sources");
        let first = &sources.as_sequence().expect("sequence")[0];
        assert_eq!(first.span.start.line, 3);
        let id = first.as_mapping().expect("mapping").get("id").expect("id");
        assert_eq!(id.span.start.line, 3);
        let tags = frontmatter.entries.get("tags").expect("tags");
        assert_eq!(tags.as_sequence().expect("seq")[0].span.start.line, 5);
    }

    #[test]
    fn absent_frontmatter_keeps_whole_file_as_body() {
        let doc = Document::parse("# Just a heading\n");
        assert!(doc.frontmatter.is_absent());
        assert!(doc.frontmatter.parsed().is_none());
        assert!(doc.frontmatter.error().is_none());
        assert_eq!(doc.body.text, "# Just a heading\n");
        assert_eq!(doc.body.start_line, 1);
    }

    #[test]
    fn a_leading_rule_that_is_not_a_delimiter_is_body() {
        let doc = Document::parse("----\nnot frontmatter\n");
        assert!(doc.frontmatter.is_absent());
    }

    #[test]
    fn unterminated_block_is_malformed() {
        let doc = Document::parse("---\ntype: Metric\nno closing delimiter\n");
        let error = doc.frontmatter.error().expect("error");
        assert_eq!(
            *error,
            FrontmatterError::Unterminated {
                span: Span::at(Position::new(1, 1))
            }
        );
        assert!(error.message().contains("never closed"));
        assert_eq!(error.span().start.line, 1);
    }

    #[test]
    fn bare_delimiter_only_file_is_unterminated() {
        let doc = Document::parse("---");
        assert!(matches!(
            doc.frontmatter.error(),
            Some(FrontmatterError::Unterminated { .. })
        ));
    }

    #[test]
    fn invalid_yaml_is_malformed_with_a_shifted_position() {
        let doc = Document::parse("---\ntype: [unclosed\n---\nbody\n");
        let error = doc.frontmatter.error().expect("error");
        assert!(
            matches!(error, FrontmatterError::Invalid { .. }),
            "got {error:?}"
        );
        assert!(
            error.span().start.line >= 2,
            "position should be shifted past the opening ---"
        );
        assert!(error.message().starts_with("frontmatter is not valid YAML"));
    }

    #[test]
    fn non_mapping_frontmatter_is_malformed() {
        let doc = Document::parse("---\n- one\n- two\n---\nbody\n");
        let error = doc.frontmatter.error().expect("error");
        assert!(matches!(error, FrontmatterError::NotAMapping { .. }));
        assert_eq!(
            error.message(),
            "frontmatter must be a mapping of keys to values"
        );
        assert_eq!(error.span().start.line, 2);
    }

    #[test]
    fn empty_frontmatter_is_an_empty_mapping() {
        let doc = Document::parse("---\n---\nbody\n");
        let frontmatter = doc.frontmatter.parsed().expect("parsed");
        assert!(frontmatter.entries.is_empty());
        assert_eq!(doc.body.text, "body\n");
    }

    #[test]
    fn handles_crlf_line_endings() {
        let doc = Document::parse("---\r\ntype: Metric\r\n---\r\nbody\r\n");
        let frontmatter = doc.frontmatter.parsed().expect("parsed");
        assert_eq!(
            frontmatter.entries.get("type").and_then(Node::as_str),
            Some("Metric")
        );
        assert_eq!(doc.body.text, "body\r\n");
    }

    #[test]
    fn strips_a_utf8_byte_order_mark() {
        let doc = Document::parse("\u{feff}---\ntype: Metric\n---\n");
        assert!(doc.frontmatter.parsed().is_some());
    }

    #[test]
    fn body_without_trailing_newline_parses() {
        let doc = Document::parse("---\ntype: Metric\n---\nlast line");
        assert_eq!(doc.body.text, "last line");
    }

    #[test]
    fn empty_source_has_no_frontmatter() {
        let doc = Document::parse("");
        assert!(doc.frontmatter.is_absent());
        assert_eq!(doc.body.text, "");
    }

    #[test]
    fn body_position_maps_offsets_to_file_lines() {
        let doc = Document::parse("---\ntype: Metric\n---\nalpha\nbravo\n");
        let body = &doc.body;
        assert_eq!(body.position_at(0), Position::new(4, 1));
        assert_eq!(body.position_at(2), Position::new(4, 3));
        assert_eq!(body.position_at(6), Position::new(5, 1));
        assert_eq!(body.position_at(9), Position::new(5, 4));
    }

    #[test]
    fn body_position_clamps_past_the_end() {
        let doc = Document::parse("---\ntype: Metric\n---\nabc\n");
        assert_eq!(
            doc.body.position_at(9_999),
            doc.body.position_at(doc.body.text.len())
        );
    }

    #[test]
    fn body_span_covers_a_range() {
        let doc = Document::parse("---\ntype: Metric\n---\nalpha\nbravo\n");
        let span = doc.body.span_at(6..11);
        assert_eq!(span.start, Position::new(5, 1));
        assert_eq!(span.end, Position::new(5, 6));
    }

    #[test]
    fn closing_delimiter_with_trailing_spaces_is_not_a_delimiter() {
        let doc = Document::parse("---\ntype: Metric\n--- \nbody\n");
        assert!(matches!(
            doc.frontmatter.error(),
            Some(FrontmatterError::Unterminated { .. })
        ));
    }
}
