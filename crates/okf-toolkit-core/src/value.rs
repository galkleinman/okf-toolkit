//! An order-preserving, span-annotated YAML value model.
//!
//! OKF §4.1 lets producers put arbitrary keys in frontmatter and requires
//! consumers to preserve unknown keys when round-tripping, so frontmatter is
//! modelled as a generic tree rather than a fixed struct. Mappings keep their
//! source order (and any duplicate keys) so diagnostics can point at the exact
//! entry that caused them.

use saphyr::{LoadableYamlNode, MarkedYamlOwned, ScalarOwned, YamlDataOwned};

use crate::span::{Position, Span};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Sequence(Vec<Node>),
    Mapping(Mapping),
}

/// A [`Value`] together with the source range it was parsed from.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub value: Value,
    pub span: Span,
}

/// A mapping that preserves its source key order.
///
/// Duplicate keys are collapsed by the YAML loader before they reach here
/// (last value wins, first position kept), so entries are unique by key in
/// practice; lookups still scan linearly because frontmatter is small and the
/// key nodes are needed for span anchoring.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mapping {
    entries: Vec<(Node, Node)>,
}

/// Failure to parse a YAML document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub position: Position,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseError {}

impl Mapping {
    pub fn from_entries(entries: Vec<(Node, Node)>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &[(Node, Node)] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns the first value whose key is the string `key`.
    pub fn get(&self, key: &str) -> Option<&Node> {
        self.entry(key).map(|(_, value)| value)
    }

    /// Returns the first key/value pair whose key is the string `key`.
    ///
    /// The key node is what diagnostics anchor to when the problem is the
    /// presence or name of a field rather than its value.
    pub fn entry(&self, key: &str) -> Option<(&Node, &Node)> {
        self.entries
            .iter()
            .find(|(k, _)| k.as_str() == Some(key))
            .map(|(k, v)| (k, v))
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entry(key).is_some()
    }

    /// Keys in source order, skipping any that are not strings.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().filter_map(|(k, _)| k.as_str())
    }
}

impl Node {
    pub fn new(value: Value, span: Span) -> Self {
        Self { value, span }
    }

    pub fn as_str(&self) -> Option<&str> {
        match &self.value {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match &self.value {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match &self.value {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_sequence(&self) -> Option<&[Node]> {
        match &self.value {
            Value::Sequence(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_mapping(&self) -> Option<&Mapping> {
        match &self.value {
            Value::Mapping(map) => Some(map),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self.value, Value::Null)
    }

    /// Renders a scalar the way it appeared in the source.
    ///
    /// Used by messages that quote a field's value back to the reader; returns
    /// `None` for collections, which have no meaningful one-line rendering.
    pub fn scalar_text(&self) -> Option<String> {
        match &self.value {
            Value::Null => Some("null".to_owned()),
            Value::Bool(b) => Some(b.to_string()),
            Value::Int(i) => Some(i.to_string()),
            Value::Float(f) => Some(f.to_string()),
            Value::String(s) => Some(s.clone()),
            Value::Sequence(_) | Value::Mapping(_) => None,
        }
    }

    /// Parses a single YAML document.
    ///
    /// An empty document yields [`Value::Null`], matching YAML's own reading of
    /// a blank stream and letting callers treat "no frontmatter keys" as an
    /// empty mapping rather than an error.
    pub fn parse(source: &str) -> Result<Self, ParseError> {
        let documents = MarkedYamlOwned::load_from_str(source).map_err(|error| ParseError {
            message: error.to_string(),
            position: marker_position(error.marker()),
        })?;

        Ok(documents
            .first()
            .map_or_else(|| Node::new(Value::Null, Span::default()), convert))
    }
}

fn marker_position(marker: &saphyr::Marker) -> Position {
    Position::new(marker.line(), marker.col() + 1)
}

fn convert(node: &MarkedYamlOwned) -> Node {
    let span = Span::new(
        marker_position(&node.span.start),
        marker_position(&node.span.end),
    );
    let value = match &node.data {
        YamlDataOwned::Value(scalar) => convert_scalar(scalar),
        YamlDataOwned::Sequence(items) => Value::Sequence(items.iter().map(convert).collect()),
        YamlDataOwned::Mapping(map) => Value::Mapping(Mapping::from_entries(
            map.iter().map(|(k, v)| (convert(k), convert(v))).collect(),
        )),
        // OKF gives tags no meaning, so a tagged node is its inner value.
        YamlDataOwned::Tagged(_, inner) => convert(inner).value,
        YamlDataOwned::Representation(text, ..) => Value::String(text.clone()),
        // A scalar the loader could not build (`!!binary`, for one). Treated as
        // absent rather than fatal: §11 only requires the block to *parse*.
        YamlDataOwned::BadValue => Value::Null,
        // Anchors are resolved into their target during loading.
        YamlDataOwned::Alias(_) => Value::Null,
    };
    Node::new(value, span)
}

fn convert_scalar(scalar: &ScalarOwned) -> Value {
    match scalar {
        ScalarOwned::Null => Value::Null,
        ScalarOwned::Boolean(b) => Value::Bool(*b),
        ScalarOwned::Integer(i) => Value::Int(*i),
        ScalarOwned::FloatingPoint(f) => Value::Float(f.into_inner()),
        ScalarOwned::String(s) => Value::String(s.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Node {
        Node::parse(source).expect("parses")
    }

    #[test]
    fn parses_scalars_into_typed_values() {
        let node = parse("s: hello\nb: true\ni: 42\nf: 1.5\nn: null\n");
        let map = node.as_mapping().expect("mapping");
        assert_eq!(map.get("s").and_then(Node::as_str), Some("hello"));
        assert_eq!(map.get("b").and_then(Node::as_bool), Some(true));
        assert_eq!(map.get("i").and_then(Node::as_int), Some(42));
        assert_eq!(
            map.get("f").map(|n| n.value.clone()),
            Some(Value::Float(1.5))
        );
        assert!(map.get("n").expect("n").is_null());
    }

    #[test]
    fn preserves_key_order() {
        let node = parse("z: 1\na: 2\nm: 3\n");
        let map = node.as_mapping().expect("mapping");
        assert_eq!(map.keys().collect::<Vec<_>>(), ["z", "a", "m"]);
        assert_eq!(map.len(), 3);
        assert!(!map.is_empty());
    }

    #[test]
    fn records_spans_for_nested_values() {
        let node = parse("type: Metric\ntags:\n  - finance\n");
        let map = node.as_mapping().expect("mapping");
        assert_eq!(map.get("type").expect("type").span.start.line, 1);

        let tags = map
            .get("tags")
            .expect("tags")
            .as_sequence()
            .expect("sequence");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].span.start.line, 3);
    }

    #[test]
    fn entry_exposes_the_key_node_for_anchoring() {
        let node = parse("alpha: 1\nbeta: 2\n");
        let map = node.as_mapping().expect("mapping");
        let (key, value) = map.entry("beta").expect("beta");
        assert_eq!(key.as_str(), Some("beta"));
        assert_eq!(value.as_int(), Some(2));
        assert_eq!(key.span.start.line, 2);
        assert!(map.contains_key("alpha"));
        assert!(!map.contains_key("gamma"));
        assert!(map.entry("gamma").is_none());
    }

    #[test]
    fn duplicate_keys_collapse_to_the_last_value() {
        let node = parse("{ a: 1, a: 2 }");
        let map = node.as_mapping().expect("mapping");
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("a").and_then(Node::as_int), Some(2));
    }

    #[test]
    fn non_string_keys_are_skipped_by_keys() {
        let node = parse("{ 1: one, real: yes }");
        let map = node.as_mapping().expect("mapping");
        assert_eq!(map.keys().collect::<Vec<_>>(), ["real"]);
    }

    #[test]
    fn empty_document_is_null() {
        let node = parse("");
        assert!(node.is_null());
        assert_eq!(node.span, Span::default());
    }

    #[test]
    fn reports_position_of_malformed_yaml() {
        let error = Node::parse("a: [1, 2\nb: 3\n").expect_err("should fail");
        assert!(error.position.line >= 1);
        assert!(!error.message.is_empty());
        assert_eq!(error.to_string(), error.message);
        // Exercises the std::error::Error impl.
        let _: &dyn std::error::Error = &error;
    }

    #[test]
    fn accessors_return_none_for_other_shapes() {
        let node = parse("seq: [1]\nmap: { a: 1 }\ntext: hi\n");
        let map = node.as_mapping().expect("mapping");
        let seq = map.get("seq").expect("seq");
        let inner = map.get("map").expect("map");
        let text = map.get("text").expect("text");

        assert!(seq.as_str().is_none());
        assert!(seq.as_bool().is_none());
        assert!(seq.as_int().is_none());
        assert!(seq.as_mapping().is_none());
        assert!(!seq.is_null());
        assert!(inner.as_sequence().is_none());
        assert!(text.as_sequence().is_none());
        assert!(text.as_mapping().is_none());
        assert!(inner.as_mapping().is_some());
    }

    #[test]
    fn scalar_text_renders_scalars_only() {
        let node = parse("s: hi\nb: false\ni: 7\nf: 2.5\nn: null\nseq: [1]\nmap: { a: 1 }\n");
        let map = node.as_mapping().expect("mapping");
        assert_eq!(
            map.get("s").and_then(Node::scalar_text).as_deref(),
            Some("hi")
        );
        assert_eq!(
            map.get("b").and_then(Node::scalar_text).as_deref(),
            Some("false")
        );
        assert_eq!(
            map.get("i").and_then(Node::scalar_text).as_deref(),
            Some("7")
        );
        assert_eq!(
            map.get("f").and_then(Node::scalar_text).as_deref(),
            Some("2.5")
        );
        assert_eq!(
            map.get("n").and_then(Node::scalar_text).as_deref(),
            Some("null")
        );
        assert!(map.get("seq").and_then(Node::scalar_text).is_none());
        assert!(map.get("map").and_then(Node::scalar_text).is_none());
    }

    #[test]
    fn empty_mapping_helpers() {
        let map = Mapping::default();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
        assert!(map.entries().is_empty());
        assert!(map.get("anything").is_none());
    }

    #[test]
    fn node_new_pairs_value_with_span() {
        let span = Span::at(Position::new(2, 3));
        let node = Node::new(Value::Bool(true), span);
        assert_eq!(node.as_bool(), Some(true));
        assert_eq!(node.span, span);
    }

    #[test]
    fn aliases_resolve_to_their_anchor() {
        let node = parse("a: &x 1\nb: *x\n");
        let map = node.as_mapping().expect("mapping");
        assert_eq!(map.get("b").and_then(Node::as_int), Some(1));
    }

    #[test]
    fn custom_tags_unwrap_to_their_inner_value() {
        let node = parse("tagged: !Custom 7\n");
        let map = node.as_mapping().expect("mapping");
        assert_eq!(map.get("tagged").and_then(Node::as_int), Some(7));
    }

    #[test]
    fn unbuildable_scalars_become_null_instead_of_failing() {
        let node = parse("data: !!binary aGk=\n");
        let map = node.as_mapping().expect("mapping");
        assert!(map.get("data").expect("data").is_null());
    }

    /// `Alias` and `Representation` never survive `load_from_str`, so they are
    /// converted directly to keep the match exhaustive and fully exercised.
    #[test]
    fn loader_internal_variants_convert() {
        let mut alias = MarkedYamlOwned::value_from_str("placeholder");
        alias.data = YamlDataOwned::Alias(0);
        assert!(convert(&alias).is_null());

        let mut representation = MarkedYamlOwned::value_from_str("placeholder");
        representation.data =
            YamlDataOwned::Representation("raw".to_owned(), saphyr::ScalarStyle::Plain, None);
        assert_eq!(convert(&representation).as_str(), Some("raw"));
    }
}
