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
    pub fn add_tokens(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if self.classes[byte as usize] != Class::Separator {
                self.classes[byte as usize] = Class::Token;
            }
        }
    }
    pub fn add_separators(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.classes[byte as usize] = Class::Separator;
        }
    }
}
pub(crate) struct Input<'a> {
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
    pub fn position_at(&self, offset: usize) -> Position {
        let prefix = &self.bytes[..offset];
        Position {
            offset,
            line: prefix.iter().filter(|&&b| b == b'\n').count() + 1,
            column: prefix
                .iter()
                .rposition(|&b| b == b'\n')
                .map_or(offset + 1, |lf| offset - lf),
        }
    }
    fn advance_to(&mut self, end: usize) {
        if end != self.cursor.offset {
            self.next_data = None;
        }
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
    pub fn skip_to_data(&mut self) {
        loop {
            let mut end = self.cursor.offset;
            while end < self.bytes.len()
                && self.settings.classes[self.bytes[end] as usize] == Class::Separator
            {
                end += 1;
            }
            self.advance_to(end);
            if end == self.bytes.len() || !self.cursor.line_blank {
                return;
            }
            let tail = &self.bytes[end..];
            if self
                .settings
                .comments
                .iter()
                .any(|p| prefix_matches(tail, p))
            {
                let end = tail
                    .iter()
                    .position(|&b| b == b'\n')
                    .map_or(self.bytes.len(), |n| end + n);
                self.advance_to(end);
            } else if self.settings.stops.iter().any(|p| prefix_matches(tail, p)) {
                self.advance_to(self.bytes.len());
                return;
            } else {
                return;
            }
        }
    }
    /// Skip separators on this physical line, leaving its LF unconsumed.
    pub fn skip_to_line_data(&mut self) -> bool {
        let mut end = self.cursor.offset;
        while end < self.bytes.len()
            && self.bytes[end] != b'\n'
            && self.settings.classes[self.bytes[end] as usize] == Class::Separator
        {
            end += 1;
        }
        self.advance_to(end);
        end < self.bytes.len() && self.bytes[end] != b'\n'
    }
    pub fn peek_byte(&self) -> Option<u8> {
        self.bytes.get(self.cursor.offset).copied()
    }
    pub fn consume_byte(&mut self) {
        self.advance_to(self.cursor.offset + 1);
    }
    pub fn next_token(&mut self) -> Option<Token<'a>> {
        self.skip_to_data();
        if self.cursor.offset == self.bytes.len() {
            return None;
        }
        let start = self.position();
        let mut end = self.cursor.offset + 1;
        if self.settings.classes[self.bytes[self.cursor.offset] as usize] == Class::Plain {
            while end < self.bytes.len()
                && self.settings.classes[self.bytes[end] as usize] == Class::Plain
            {
                end += 1;
            }
        }
        self.advance_to(end);
        Some(Token {
            bytes: &self.bytes[start.offset..end],
            start,
            end: self.position(),
        })
    }
    pub fn settings_changed(&mut self) {
        self.next_data = None;
    }
    pub fn next_data_position(&mut self) -> Position {
        if let Some(position) = self.next_data {
            return position;
        }
        let cursor = self.cursor;
        self.skip_to_data();
        let position = self.position();
        self.cursor = cursor;
        self.next_data = Some(position);
        position
    }
    pub fn peek_token(&mut self) -> Option<Token<'a>> {
        let cursor = self.cursor;
        let next_data = self.next_data;
        let token = self.next_token();
        self.cursor = cursor;
        self.next_data = next_data;
        token
    }
    pub fn read_rest(&mut self) -> Option<Token<'a>> {
        self.skip_to_data();
        if self.cursor.offset == self.bytes.len() {
            return None;
        }
        let start = self.position();
        let end = self.bytes[self.cursor.offset..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(self.bytes.len(), |n| self.cursor.offset + n);
        let mut trimmed = end;
        while trimmed > self.cursor.offset && is_line_blank(self.bytes[trimmed - 1]) {
            trimmed -= 1;
        }
        self.advance_to(end);
        // The token holds no LF, so its end lies on its start's line.
        let end = Position {
            offset: trimmed,
            column: start.column + (trimmed - start.offset),
            ..start
        };
        Some(Token {
            bytes: &self.bytes[start.offset..trimmed],
            start,
            end,
        })
    }
}
fn is_line_blank(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r')
}
fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
fn prefix_matches(tail: &[u8], prefix: &[u8]) -> bool {
    !prefix.is_empty()
        && tail.starts_with(prefix)
        && (!is_word_byte(prefix[prefix.len() - 1])
            || tail.get(prefix.len()).is_none_or(|b| !is_word_byte(*b)))
}
