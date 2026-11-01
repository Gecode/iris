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
    offset: usize,
    line: usize,
    line_start: usize,
    pub settings: Settings,
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
            offset: 0,
            line: 1,
            line_start: 0,
            settings: Settings::default(),
        }
    }
    pub fn position(&self) -> Position {
        Position {
            offset: self.offset,
            line: self.line,
            column: self.offset - self.line_start + 1,
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
        for offset in self.offset..end {
            if self.bytes[offset] == b'\n' {
                self.line += 1;
                self.line_start = offset + 1;
            }
        }
        self.offset = end;
    }
    pub fn skip_to_data(&mut self) {
        loop {
            let mut end = self.offset;
            while end < self.bytes.len()
                && self.settings.classes[self.bytes[end] as usize] == Class::Separator
            {
                end += 1;
            }
            self.advance_to(end);
            if end == self.bytes.len()
                || !self.bytes[self.line_start..end]
                    .iter()
                    .all(|b| is_line_blank(*b))
            {
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
    pub fn next_token(&mut self) -> Option<Token<'a>> {
        self.skip_to_data();
        if self.offset == self.bytes.len() {
            return None;
        }
        let start = self.position();
        let mut end = self.offset + 1;
        if self.settings.classes[self.bytes[self.offset] as usize] == Class::Plain {
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
    pub fn read_rest(&mut self) -> Option<Token<'a>> {
        self.skip_to_data();
        if self.offset == self.bytes.len() {
            return None;
        }
        let start = self.position();
        let end = self.bytes[self.offset..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(self.bytes.len(), |n| self.offset + n);
        let mut trimmed = end;
        while trimmed > self.offset && is_line_blank(self.bytes[trimmed - 1]) {
            trimmed -= 1;
        }
        self.advance_to(end);
        Some(Token {
            bytes: &self.bytes[start.offset..trimmed],
            start,
            end: self.position_at(trimmed),
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
