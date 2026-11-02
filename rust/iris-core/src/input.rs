use crate::cancellation::{Cancelled, Control};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Position {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Plain,
    Token,
    Separator,
}
pub(crate) struct Settings {
    classes: [Class; 256],
    pub comments: Vec<Vec<u8>>,
    pub stops: Vec<Vec<u8>>,
    pub ignore_trailing: bool,
}
impl Default for Settings {
    fn default() -> Self {
        let mut classes = [Class::Plain; 256];
        for byte in b" \t\n\r\x0b\x0c" {
            classes[*byte as usize] = Class::Separator;
        }
        Self {
            classes,
            comments: vec![],
            stops: vec![],
            ignore_trailing: false,
        }
    }
}
impl Settings {
    pub fn add_tokens(&mut self, bytes: &[u8], control: &Control) -> Result<(), Cancelled> {
        for bytes in bytes.chunks(control.chunk_size()) {
            control.check()?;
            for &byte in bytes {
                if self.classes[byte as usize] != Class::Separator {
                    self.classes[byte as usize] = Class::Token;
                }
            }
        }
        Ok(())
    }
    pub fn add_separators(&mut self, bytes: &[u8], control: &Control) -> Result<(), Cancelled> {
        for bytes in bytes.chunks(control.chunk_size()) {
            control.check()?;
            for &byte in bytes {
                self.classes[byte as usize] = Class::Separator;
            }
        }
        Ok(())
    }
}
pub(crate) struct Input<'a> {
    pub control: Control,
    pub bytes: &'a [u8],
    pub origin: &'a str,
    cursor: Cursor,
    next_data: Option<Position>,
    pub settings: Settings,
}
#[derive(Clone, Copy)]
struct Cursor {
    offset: usize,
    line: usize,
    line_start: usize,
    line_blank: bool,
}
pub(crate) struct Line<'a> {
    pub bytes: &'a [u8],
    pub start: Position,
    pub physical_end: usize,
}
impl Line<'_> {
    pub fn position(&self, relative: usize) -> Position {
        Position {
            offset: self.start.offset + relative,
            column: self.start.column + relative,
            ..self.start
        }
    }
}
pub(crate) struct Token<'a> {
    pub bytes: &'a [u8],
    pub start: Position,
    pub end: Position,
}
impl<'a> Input<'a> {
    pub fn new(bytes: &'a [u8], origin: &'a str) -> Self {
        Self {
            bytes,
            origin,
            control: Control::default(),
            cursor: Cursor {
                offset: 0,
                line: 1,
                line_start: 0,
                line_blank: true,
            },
            next_data: None,
            settings: Settings::default(),
        }
    }
    pub fn position(&self) -> Position {
        Position {
            offset: self.cursor.offset,
            line: self.cursor.line,
            column: self.cursor.offset - self.cursor.line_start + 1,
        }
    }
    #[inline]
    pub fn advance_to(&mut self, end: usize) -> Result<(), Cancelled> {
        if self.control.is_active() {
            self.advance_to_with::<true>(end)
        } else {
            self.advance_to_with::<false>(end)
        }
    }
    #[inline]
    fn advance_to_with<const CANCEL: bool>(&mut self, end: usize) -> Result<(), Cancelled> {
        if end != self.cursor.offset {
            self.next_data = None;
        }
        if !CANCEL {
            self.advance_cursor(end);
        } else {
            for chunk in chunk_ranges(self.cursor.offset..end, self.control.chunk_size()) {
                self.control.check()?;
                self.advance_cursor(chunk.end);
            }
        }
        Ok(())
    }
    #[inline]
    fn advance_cursor(&mut self, end: usize) {
        self.control.worked(end - self.cursor.offset);
        for offset in self.cursor.offset..end {
            if self.bytes[offset] == b'\n' {
                self.cursor.line += 1;
                self.cursor.line_start = offset + 1;
                self.cursor.line_blank = true;
            } else if !is_line_blank(self.bytes[offset]) {
                self.cursor.line_blank = false;
            }
        }
        self.cursor.offset = end;
    }
    #[inline]
    pub fn skip_to_data(&mut self) -> Result<(), Cancelled> {
        if self.control.is_active() {
            self.skip_to_data_with::<true>()
        } else {
            self.skip_to_data_with::<false>()
        }
    }
    #[inline]
    fn skip_to_data_with<const CANCEL: bool>(&mut self) -> Result<(), Cancelled> {
        loop {
            let mut end = self.cursor.offset;
            end = self.scan_with::<CANCEL>(end, |b| {
                self.settings.classes[b as usize] == Class::Separator
            })?;
            self.advance_to_with::<CANCEL>(end)?;
            if end == self.bytes.len() || !self.cursor.line_blank {
                return Ok(());
            }
            let tail = &self.bytes[end..];
            if matches_any_prefix(tail, &self.settings.comments, &self.control)? {
                let end = self.scan_with::<CANCEL>(end, |b| b != b'\n')?;
                self.advance_to_with::<CANCEL>(end)?;
            } else if matches_any_prefix(tail, &self.settings.stops, &self.control)? {
                self.advance_to_with::<CANCEL>(self.bytes.len())?;
                return Ok(());
            } else {
                return Ok(());
            }
        }
    }
    /// Skip separators on this physical line, leaving its LF unconsumed.
    #[inline]
    pub fn skip_to_line_data(&mut self) -> Result<bool, Cancelled> {
        let mut end = self.cursor.offset;
        end = self.scan(end, |b| {
            b != b'\n' && self.settings.classes[b as usize] == Class::Separator
        })?;
        self.advance_to(end)?;
        Ok(end < self.bytes.len() && self.bytes[end] != b'\n')
    }
    pub fn peek_byte(&self) -> Option<u8> {
        self.bytes.get(self.cursor.offset).copied()
    }
    #[inline]
    pub fn consume_byte(&mut self) -> Result<(), Cancelled> {
        self.advance_to(self.cursor.offset + 1)
    }
    #[inline]
    pub fn next_token(&mut self) -> Result<Option<Token<'a>>, Cancelled> {
        if self.control.is_active() {
            self.next_token_with::<true>()
        } else {
            self.next_token_with::<false>()
        }
    }
    #[inline]
    fn next_token_with<const CANCEL: bool>(&mut self) -> Result<Option<Token<'a>>, Cancelled> {
        self.skip_to_data_with::<CANCEL>()?;
        if self.cursor.offset == self.bytes.len() {
            return Ok(None);
        }
        let start = self.position();
        let mut end = self.cursor.offset + 1;
        if self.settings.classes[self.bytes[self.cursor.offset] as usize] == Class::Plain {
            end = self
                .scan_with::<CANCEL>(end, |b| self.settings.classes[b as usize] == Class::Plain)?;
        }
        self.advance_to_with::<CANCEL>(end)?;
        Ok(Some(Token {
            bytes: &self.bytes[start.offset..end],
            start,
            end: self.position(),
        }))
    }
    #[inline]
    fn scan(&self, start: usize, keep: impl Fn(u8) -> bool) -> Result<usize, Cancelled> {
        if self.control.is_active() {
            self.scan_with::<true>(start, keep)
        } else {
            self.scan_with::<false>(start, keep)
        }
    }
    #[inline]
    fn scan_with<const CANCEL: bool>(
        &self,
        start: usize,
        keep: impl Fn(u8) -> bool,
    ) -> Result<usize, Cancelled> {
        if !CANCEL {
            let mut end = start;
            while end < self.bytes.len() && keep(self.bytes[end]) {
                end += 1;
            }
            self.control.worked(end - start);
            return Ok(end);
        }
        for range in chunk_ranges(start..self.bytes.len(), self.control.chunk_size()) {
            self.control.check()?;
            if let Some(n) = self.bytes[range.clone()].iter().position(|&b| !keep(b)) {
                self.control.worked(n + 1);
                return Ok(range.start + n);
            }
            self.control.worked(range.len());
        }
        Ok(self.bytes.len())
    }
    pub fn settings_changed(&mut self) {
        self.next_data = None;
    }
    pub fn next_data_position(&mut self) -> Result<Position, Cancelled> {
        if let Some(position) = self.next_data {
            return Ok(position);
        }
        let cursor = self.cursor;
        self.skip_to_data()?;
        let position = self.position();
        self.cursor = cursor;
        self.next_data = Some(position);
        Ok(position)
    }
    pub fn peek_token(&mut self) -> Result<Option<Token<'a>>, Cancelled> {
        let cursor = self.cursor;
        let next_data = self.next_data;
        let token = self.next_token();
        self.cursor = cursor;
        self.next_data = next_data;
        token
    }
    pub fn remaining_line(&mut self) -> Result<Option<Line<'a>>, Cancelled> {
        self.skip_to_data()?;
        if self.cursor.offset == self.bytes.len() {
            return Ok(None);
        }
        let start = self.position();
        let physical_end = self.scan(start.offset, |b| b != b'\n')?;
        let mut trimmed = physical_end;
        while trimmed > start.offset && is_line_blank(self.bytes[trimmed - 1]) {
            if (physical_end - trimmed).is_multiple_of(crate::cancellation::SCAN_CHUNK) {
                self.control.check()?;
            }
            trimmed -= 1;
        }
        Ok(Some(Line {
            bytes: &self.bytes[start.offset..trimmed],
            start,
            physical_end,
        }))
    }
    pub fn consume_line(&mut self, line: &Line<'_>) -> Result<(), Cancelled> {
        self.advance_to(line.physical_end)
    }
    pub fn skip_line(&mut self) -> Result<bool, Cancelled> {
        if let Some(line) = self.remaining_line()? {
            self.consume_line(&line)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub fn read_rest(&mut self) -> Result<Option<Token<'a>>, Cancelled> {
        let Some(line) = self.remaining_line()? else {
            return Ok(None);
        };
        self.consume_line(&line)?;
        Ok(Some(Token {
            bytes: line.bytes,
            start: line.start,
            end: line.position(line.bytes.len()),
        }))
    }
}
pub(super) fn is_line_blank(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r')
}
pub(super) fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
fn matches_any_prefix(
    tail: &[u8],
    prefixes: &[Vec<u8>],
    control: &Control,
) -> Result<bool, Cancelled> {
    for prefix in prefixes {
        control.check()?;
        if !prefix.is_empty()
            && control.starts_with(tail, prefix)?
            && (!is_word_byte(prefix[prefix.len() - 1])
                || tail.get(prefix.len()).is_none_or(|b| !is_word_byte(*b)))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn chunk_ranges(
    mut range: std::ops::Range<usize>,
    size: usize,
) -> impl Iterator<Item = std::ops::Range<usize>> {
    std::iter::from_fn(move || {
        if range.is_empty() {
            return None;
        }
        let start = range.start;
        range.start = start.saturating_add(size).min(range.end);
        Some(start..range.start)
    })
}
