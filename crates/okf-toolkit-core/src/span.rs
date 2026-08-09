/// A 1-indexed position within a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Default for Position {
    fn default() -> Self {
        Self { line: 1, column: 1 }
    }
}

impl Position {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// A range between two [`Position`]s within a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

impl Span {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    /// A zero-width span at `position`.
    pub fn at(position: Position) -> Self {
        Self {
            start: position,
            end: position,
        }
    }

    /// Shifts both endpoints down by `lines`.
    ///
    /// Frontmatter is parsed as a standalone YAML document, so its spans are
    /// relative to the start of the block rather than the file. Shifting by the
    /// number of lines the opening `---` occupies maps them back onto the file.
    pub fn offset_lines(self, lines: usize) -> Self {
        Self {
            start: Position::new(self.start.line + lines, self.start.column),
            end: Position::new(self.end.line + lines, self.end.column),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_position_is_one_indexed() {
        assert_eq!(Position::default(), Position::new(1, 1));
    }

    #[test]
    fn default_span_starts_and_ends_at_origin() {
        let span = Span::default();
        assert_eq!(span.start, Position::new(1, 1));
        assert_eq!(span.end, Position::new(1, 1));
    }

    #[test]
    fn at_builds_a_zero_width_span() {
        let span = Span::at(Position::new(4, 7));
        assert_eq!(span.start, span.end);
        assert_eq!(span.start, Position::new(4, 7));
    }

    #[test]
    fn new_keeps_both_endpoints() {
        let span = Span::new(Position::new(1, 2), Position::new(3, 4));
        assert_eq!(span.start, Position::new(1, 2));
        assert_eq!(span.end, Position::new(3, 4));
    }

    #[test]
    fn offset_lines_shifts_lines_but_not_columns() {
        let span = Span::new(Position::new(1, 5), Position::new(2, 9)).offset_lines(3);
        assert_eq!(span.start, Position::new(4, 5));
        assert_eq!(span.end, Position::new(5, 9));
    }

    #[test]
    fn positions_order_by_line_then_column() {
        assert!(Position::new(1, 9) < Position::new(2, 1));
        assert!(Position::new(2, 1) < Position::new(2, 2));
    }
}
