use iris_sema::Analysis;
use iris_syntax::{LanguageVersion, Parse, ParseError, SyntaxNode, TextRange, TextSize};
use lsp_types::{Position, PositionEncodingKind, Range, TextDocumentContentChangeEvent};
use std::time::{Duration, Instant};

const DIAGNOSTIC_DELAY: Duration = Duration::from_millis(150);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PositionEncoding {
    Utf8,
    Utf16,
}

impl PositionEncoding {
    pub(crate) fn protocol_kind(self) -> PositionEncodingKind {
        match self {
            Self::Utf8 => PositionEncodingKind::UTF8,
            Self::Utf16 => PositionEncodingKind::UTF16,
        }
    }

    fn character_len(self, character: char) -> usize {
        match self {
            Self::Utf8 => character.len_utf8(),
            Self::Utf16 => character.len_utf16(),
        }
    }
}

pub(crate) struct Document {
    pub(crate) source: String,
    pub(crate) version: i32,
    pub(crate) lines: LineIndex,
    pub(crate) deadline: Option<Instant>,
    snapshot: Option<AnalysisSnapshot>,
}

struct AnalysisSnapshot {
    // Keep the lossless tree with its semantic model for later LSP features.
    parsed: Parse,
    analysis: Analysis,
}

impl Document {
    pub(crate) fn new(source: String, version: i32) -> Self {
        Self {
            lines: LineIndex::new(&source),
            source,
            version,
            deadline: None,
            snapshot: None,
        }
    }

    /// Apply the batch atomically; later ranges refer to earlier edits' results.
    pub(crate) fn change(
        &mut self,
        version: i32,
        changes: Vec<TextDocumentContentChangeEvent>,
        encoding: PositionEncoding,
        now: Instant,
    ) -> Result<(), &'static str> {
        if version <= self.version {
            return Ok(());
        }
        let mut source = self.source.clone();
        let mut lines = self.lines.clone();
        for change in changes {
            if let Some(range) = change.range {
                let range = lines.byte_range(&source, range, encoding)?;
                source.replace_range(range, &change.text);
            } else {
                source = change.text;
            }
            lines = LineIndex::new(&source);
        }
        self.source = source;
        self.lines = lines;
        self.version = version;
        self.snapshot = None;
        self.deadline = Some(now + DIAGNOSTIC_DELAY);
        Ok(())
    }

    pub(crate) fn analyze(&mut self, origin: &str) -> Result<&Analysis, ParseError> {
        if self.snapshot.is_none() {
            let parsed = iris_syntax::parse(self.source.as_bytes(), origin, LanguageVersion::V0_1)?;
            let analysis = iris_sema::analyze(&parsed);
            self.snapshot = Some(AnalysisSnapshot { parsed, analysis });
        }
        Ok(&self
            .snapshot
            .as_ref()
            .expect("analysis was just cached")
            .analysis)
    }

    pub(crate) fn cached_analysis(&self) -> Option<&Analysis> {
        self.snapshot.as_ref().map(|snapshot| &snapshot.analysis)
    }

    pub(crate) fn cached_syntax(&self) -> Option<SyntaxNode> {
        self.snapshot
            .as_ref()
            .map(|snapshot| snapshot.parsed.syntax())
    }
}

/// Request-local positions for bulk range conversion. Indexing byte boundaries
/// avoids rescanning a long line for every symbol, fold, or semantic token.
pub(crate) struct Positions(Vec<Position>);

impl Positions {
    pub(crate) fn new(source: &str, lines: &LineIndex, encoding: PositionEncoding) -> Self {
        let mut positions = vec![Position::default(); source.len() + 1];
        for (number, line) in lines.lines.iter().enumerate() {
            let mut position = Position::new(number as u32, 0);
            for (offset, scalar) in source[line.start..line.end].char_indices() {
                let start = line.start + offset;
                positions[start..start + scalar.len_utf8()].fill(position);
                position.character += encoding.character_len(scalar) as u32;
            }
            let next = lines
                .lines
                .get(number + 1)
                .map_or(source.len() + 1, |l| l.start);
            positions[line.end..next].fill(position);
        }
        Self(positions)
    }

    pub(crate) fn at(&self, offset: TextSize) -> Position {
        self.0[usize::from(offset)]
    }

    pub(crate) fn range(&self, range: TextRange) -> Range {
        Range::new(self.at(range.start()), self.at(range.end()))
    }
}

#[derive(Clone)]
pub(crate) struct LineIndex {
    lines: Vec<Line>,
}

#[derive(Clone)]
struct Line {
    start: usize,
    end: usize,
}

impl LineIndex {
    pub(crate) fn new(source: &str) -> Self {
        let bytes = source.as_bytes();
        let mut lines = Vec::new();
        let mut start = 0;
        let mut offset = 0;
        while offset < bytes.len() {
            if matches!(bytes[offset], b'\r' | b'\n') {
                lines.push(Line { start, end: offset });
                if bytes[offset] == b'\r' && bytes.get(offset + 1) == Some(&b'\n') {
                    offset += 1;
                }
                start = offset + 1;
            }
            offset += 1;
        }
        lines.push(Line {
            start,
            end: bytes.len(),
        });
        Self { lines }
    }

    pub(crate) fn position(
        &self,
        source: &str,
        offset: usize,
        encoding: PositionEncoding,
    ) -> Result<Position, &'static str> {
        if offset > source.len() || !source.is_char_boundary(offset) {
            return Err("invalid source byte offset");
        }
        let line_number = self.lines.partition_point(|line| line.start <= offset) - 1;
        let line = &self.lines[line_number];
        let prefix = &source[line.start..offset.min(line.end)];
        let character = match encoding {
            PositionEncoding::Utf8 => prefix.len(),
            PositionEncoding::Utf16 => prefix.encode_utf16().count(),
        };
        Ok(Position::new(
            u32::try_from(line_number).map_err(|_| "too many source lines")?,
            u32::try_from(character).map_err(|_| "source line is too long")?,
        ))
    }

    pub(crate) fn byte_offset(
        &self,
        source: &str,
        position: Position,
        encoding: PositionEncoding,
    ) -> Result<usize, &'static str> {
        let line = self
            .lines
            .get(position.line as usize)
            .ok_or("nonexistent line")?;
        let mut character = 0;
        for (offset, scalar) in source[line.start..line.end].char_indices() {
            if character == position.character as usize {
                return Ok(line.start + offset);
            }
            character += encoding.character_len(scalar);
            if character > position.character as usize {
                return Err("position splits a Unicode scalar");
            }
        }
        // LSP positions beyond the line's length clamp to the end of the line.
        Ok(line.end)
    }

    pub(crate) fn byte_range(
        &self,
        source: &str,
        range: Range,
        encoding: PositionEncoding,
    ) -> Result<std::ops::Range<usize>, &'static str> {
        if range.start > range.end {
            return Err("reversed edit range");
        }
        let start = self.byte_offset(source, range.start, encoding)?;
        let end = self.byte_offset(source, range.end, encoding)?;
        Ok(start..end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_follow_physical_lines_and_negotiated_units() {
        let source = "é😀\t\r\nx\ry\n";
        let lines = LineIndex::new(source);
        for (offset, utf16, utf8) in [
            (0, (0, 0), (0, 0)),
            (2, (0, 1), (0, 2)),
            (6, (0, 3), (0, 6)),
            (7, (0, 4), (0, 7)),
            (8, (0, 4), (0, 7)),
            (9, (1, 0), (1, 0)),
            (10, (1, 1), (1, 1)),
            (11, (2, 0), (2, 0)),
            (13, (3, 0), (3, 0)),
        ] {
            for (encoding, (line, character)) in [
                (PositionEncoding::Utf16, utf16),
                (PositionEncoding::Utf8, utf8),
            ] {
                let positions = Positions::new(source, &lines, encoding);
                assert_eq!(
                    positions.at(TextSize::new(offset as u32)),
                    Position::new(line, character)
                );
                assert_eq!(
                    lines.position(source, offset, encoding),
                    Ok(Position::new(line, character))
                );
            }
        }
        for encoding in [PositionEncoding::Utf16, PositionEncoding::Utf8] {
            assert_eq!(
                lines.byte_offset(source, Position::new(0, 99), encoding),
                Ok(7)
            );
            assert_eq!(
                lines.byte_offset(source, Position::new(3, 0), encoding),
                Ok(13)
            );
            assert!(
                lines
                    .byte_offset(source, Position::new(4, 0), encoding)
                    .is_err()
            );
            assert!(
                lines
                    .byte_range(
                        source,
                        Range::new(Position::new(1, 0), Position::new(0, 0)),
                        encoding
                    )
                    .is_err()
            );
        }
        assert!(
            lines
                .byte_offset(source, Position::new(0, 2), PositionEncoding::Utf16)
                .is_err()
        );
        assert!(
            lines
                .byte_offset(source, Position::new(0, 1), PositionEncoding::Utf8)
                .is_err()
        );
        assert_eq!(
            lines.byte_offset(source, Position::new(0, 6), PositionEncoding::Utf8),
            Ok(6)
        );
        assert_eq!(
            lines.byte_offset(source, Position::new(0, 3), PositionEncoding::Utf16),
            Ok(6)
        );
        assert_eq!(
            lines.byte_range(
                source,
                Range::new(Position::new(0, 3), Position::new(2, 1)),
                PositionEncoding::Utf16
            ),
            Ok(6..12)
        );
        let empty = LineIndex::new("");
        assert_eq!(
            empty.position("", 0, PositionEncoding::Utf16),
            Ok(Position::new(0, 0))
        );
        assert_eq!(
            empty.byte_offset("", Position::new(0, 0), PositionEncoding::Utf16),
            Ok(0)
        );
    }

    #[test]
    fn changes_invalidate_analysis_and_reset_only_accepted_deadlines() {
        let mut document = Document::new("x:unknown".into(), 1);
        assert!(!document.analyze("untitled:test").unwrap().is_valid());
        let now = Instant::now();
        document
            .change(2, vec![], PositionEncoding::Utf16, now)
            .unwrap();
        assert!(document.snapshot.is_none());
        assert_eq!(document.deadline, Some(now + Duration::from_millis(150)));
        document
            .change(
                2,
                vec![],
                PositionEncoding::Utf16,
                now + Duration::from_secs(1),
            )
            .unwrap();
        assert_eq!(document.deadline, Some(now + Duration::from_millis(150)));
        document
            .change(
                3,
                vec![],
                PositionEncoding::Utf16,
                now + Duration::from_millis(20),
            )
            .unwrap();
        assert_eq!(document.deadline, Some(now + Duration::from_millis(170)));
    }
}
