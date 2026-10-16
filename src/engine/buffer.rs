//! Text buffer with a char-grid cursor.

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Pos {
    pub row: usize,
    pub col: usize,
}

impl Pos {
    pub fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }
}

/// A simple line buffer. Lines are stored as char vectors so that
/// column indices are always char indices (never byte indices).
#[derive(Clone, Debug)]
pub struct Buffer {
    lines: Vec<Vec<char>>,
}

impl Buffer {
    pub fn new(lines: Vec<String>) -> Self {
        Self {
            lines: lines.into_iter().map(|s| s.chars().collect()).collect(),
        }
    }

    pub fn row_count(&self) -> usize {
        self.lines.len()
    }

    pub fn line_len(&self, row: usize) -> usize {
        self.lines.get(row).map(|l| l.len()).unwrap_or(0)
    }

    pub fn char_at(&self, row: usize, col: usize) -> Option<char> {
        self.lines.get(row).and_then(|l| l.get(col)).copied()
    }

    pub fn line_chars(&self, row: usize) -> Vec<char> {
        self.lines.get(row).cloned().unwrap_or_default()
    }

    pub fn line_slice(&self, row: usize) -> &[char] {
        self.lines.get(row).map(|l| l.as_slice()).unwrap_or(&[])
    }

    pub fn line_string(&self, row: usize) -> String {
        self.lines
            .get(row)
            .map(|l| l.iter().collect())
            .unwrap_or_default()
    }

    /// Clamp a position to a valid cursor cell.
    pub fn clamp(&self, pos: Pos) -> Pos {
        if self.lines.is_empty() {
            return Pos::new(0, 0);
        }
        let row = pos.row.min(self.lines.len() - 1);
        let len = self.line_len(row);
        let col = if len == 0 { 0 } else { pos.col.min(len - 1) };
        Pos::new(row, col)
    }

    pub fn first_non_blank(&self, row: usize) -> usize {
        for (i, c) in self.line_chars(row).iter().enumerate() {
            if *c != ' ' && *c != '\t' {
                return i;
            }
        }
        0
    }

    pub fn last_non_blank(&self, row: usize) -> usize {
        let line = self.line_chars(row);
        for (i, c) in line.iter().enumerate().rev() {
            if *c != ' ' && *c != '\t' {
                return i;
            }
        }
        0
    }

    pub fn is_blank_line(&self, row: usize) -> bool {
        match self.lines.get(row) {
            None => true,
            Some(l) => l.is_empty() || l.iter().all(|c| *c == ' ' || *c == '\t'),
        }
    }

    pub fn last_pos(&self) -> Pos {
        if self.lines.is_empty() {
            return Pos::new(0, 0);
        }
        let r = self.lines.len() - 1;
        Pos::new(r, self.line_len(r).saturating_sub(1))
    }
}

pub fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

pub fn is_blank(c: char) -> bool {
    c == ' ' || c == '\t'
}

/// Character class for small words: 0 = blank, 1 = word, 2 = punctuation.
pub fn class_small(c: char) -> u8 {
    if is_blank(c) {
        0
    } else if is_word_char(c) {
        1
    } else {
        2
    }
}

/// Character class for big WORDS: 0 = blank, 1 = non-blank.
pub fn class_big(c: char) -> u8 {
    if is_blank(c) {
        0
    } else {
        1
    }
}
