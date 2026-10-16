//! Vim motion engine: resolve a motion + count to a new cursor position.

use super::buffer::{class_big, class_small, Buffer, Pos};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FindKind {
    Fwd,      // f
    Back,     // F
    TillFwd,  // t
    TillBack, // T
}

impl FindKind {
    pub fn reversed(self) -> Self {
        match self {
            FindKind::Fwd => FindKind::Back,
            FindKind::Back => FindKind::Fwd,
            FindKind::TillFwd => FindKind::TillBack,
            FindKind::TillBack => FindKind::TillFwd,
        }
    }

    pub fn key(self) -> char {
        match self {
            FindKind::Fwd => 'f',
            FindKind::Back => 'F',
            FindKind::TillFwd => 't',
            FindKind::TillBack => 'T',
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Motion {
    Left,
    Down,
    Up,
    Right,
    WordFwdSmall, // w
    WordFwdBig,   // W
    WordBackSmall, // b
    WordBackBig,  // B
    WordEndSmall, // e
    WordEndBig,   // E
    WordEndBackSmall, // ge
    WordEndBackBig,   // gE
    MatchPair,    // %
    LineStart,    // 0
    FirstNonBlank, // ^
    LineEnd,      // $
    LastNonBlank, // g_
    FileTop,      // gg
    FileBottom,   // G
    GotoLine,     // {n}G / {n}gg (count holds the 1-based line number)
    ScreenTop,    // H
    ScreenMid,    // M
    ScreenBot,    // L
    Find(FindKind, char),
    ParaFwd,  // }
    ParaBack, // {
}

/// Short human label for a motion, used for par hints (e.g. "3j", "fx", "5G").
pub fn label(m: &Motion, count: usize) -> String {
    if let Motion::GotoLine = m {
        return format!("{count}G");
    }
    let prefix = if count <= 1 {
        String::new()
    } else {
        count.to_string()
    };
    match m {
        Motion::Left => format!("{prefix}h"),
        Motion::Down => format!("{prefix}j"),
        Motion::Up => format!("{prefix}k"),
        Motion::Right => format!("{prefix}l"),
        Motion::WordFwdSmall => format!("{prefix}w"),
        Motion::WordFwdBig => format!("{prefix}W"),
        Motion::WordBackSmall => format!("{prefix}b"),
        Motion::WordBackBig => format!("{prefix}B"),
        Motion::WordEndSmall => format!("{prefix}e"),
        Motion::WordEndBig => format!("{prefix}E"),
        Motion::WordEndBackSmall => format!("{prefix}ge"),
        Motion::WordEndBackBig => format!("{prefix}gE"),
        Motion::MatchPair => "%".to_string(),
        Motion::LineStart => "0".to_string(),
        Motion::FirstNonBlank => format!("{prefix}^"),
        Motion::LineEnd => format!("{prefix}$"),
        Motion::LastNonBlank => format!("{prefix}g_"),
        Motion::FileTop => "gg".to_string(),
        Motion::FileBottom => "G".to_string(),
        Motion::GotoLine => format!("{count}G"),
        Motion::ScreenTop => "H".to_string(),
        Motion::ScreenMid => "M".to_string(),
        Motion::ScreenBot => "L".to_string(),
        Motion::Find(kind, ch) => format!("{prefix}{}{ch}", kind.key()),
        Motion::ParaFwd => format!("{prefix}}}"),
        Motion::ParaBack => format!("{prefix}{{"),
    }
}

/// Apply a motion `count` times (for GotoLine, `count` is the line number).
pub fn apply(buf: &Buffer, from: Pos, motion: &Motion, count: usize) -> Pos {
    let from = buf.clamp(from);
    let n = count.max(1);
    match motion {
        Motion::Left => Pos::new(from.row, from.col.saturating_sub(n)),
        Motion::Right => {
            let len = buf.line_len(from.row);
            if len == 0 {
                from
            } else {
                Pos::new(from.row, (from.col + n).min(len - 1))
            }
        }
        Motion::Down => {
            let row = (from.row + n).min(buf.row_count().saturating_sub(1));
            buf.clamp(Pos::new(row, from.col))
        }
        Motion::Up => {
            let row = from.row.saturating_sub(n);
            buf.clamp(Pos::new(row, from.col))
        }
        Motion::WordFwdSmall => repeat(buf, from, n, false, word_fwd_once),
        Motion::WordFwdBig => repeat(buf, from, n, true, word_fwd_once),
        Motion::WordBackSmall => repeat(buf, from, n, false, word_back_once),
        Motion::WordBackBig => repeat(buf, from, n, true, word_back_once),
        Motion::WordEndSmall => repeat(buf, from, n, false, word_end_once),
        Motion::WordEndBig => repeat(buf, from, n, true, word_end_once),
        Motion::WordEndBackSmall => repeat(buf, from, n, false, word_end_back_once),
        Motion::WordEndBackBig => repeat(buf, from, n, true, word_end_back_once),
        Motion::MatchPair => match_pair(buf, from),
        Motion::LineStart => Pos::new(from.row, 0),
        Motion::FirstNonBlank => {
            let r = (from.row + n - 1).min(buf.row_count().saturating_sub(1));
            Pos::new(r, buf.first_non_blank(r))
        }
        Motion::LineEnd => {
            let r = (from.row + n - 1).min(buf.row_count().saturating_sub(1));
            let len = buf.line_len(r);
            Pos::new(r, len.saturating_sub(1))
        }
        Motion::LastNonBlank => {
            let r = (from.row + n - 1).min(buf.row_count().saturating_sub(1));
            Pos::new(r, buf.last_non_blank(r))
        }
        Motion::FileTop => Pos::new(0, buf.first_non_blank(0)),
        Motion::FileBottom => {
            let r = buf.row_count().saturating_sub(1);
            Pos::new(r, buf.first_non_blank(r))
        }
        Motion::GotoLine => {
            let rows = buf.row_count().max(1);
            let r = count.clamp(1, rows) - 1;
            Pos::new(r, buf.first_non_blank(r))
        }
        Motion::ScreenTop => Pos::new(0, buf.first_non_blank(0)),
        Motion::ScreenMid => {
            let r = buf.row_count().saturating_sub(1) / 2;
            Pos::new(r, buf.first_non_blank(r))
        }
        Motion::ScreenBot => {
            let r = buf.row_count().saturating_sub(1);
            Pos::new(r, buf.first_non_blank(r))
        }
        Motion::Find(kind, ch) => find(buf, from, *kind, *ch, n),
        Motion::ParaFwd => {
            let mut p = from;
            for _ in 0..n {
                let q = para_fwd(buf, p);
                if q == p {
                    break;
                }
                p = q;
            }
            p
        }
        Motion::ParaBack => {
            let mut p = from;
            for _ in 0..n {
                let q = para_back(buf, p);
                if q == p {
                    break;
                }
                p = q;
            }
            p
        }
    }
}

fn repeat(
    buf: &Buffer,
    from: Pos,
    n: usize,
    big: bool,
    f: fn(&Buffer, Pos, bool) -> Pos,
) -> Pos {
    let mut p = from;
    for _ in 0..n {
        let q = f(buf, p, big);
        if q == p {
            break;
        }
        p = q;
    }
    p
}

fn next_pos(buf: &Buffer, p: Pos) -> Option<Pos> {
    let len = buf.line_len(p.row);
    if p.col + 1 < len {
        Some(Pos::new(p.row, p.col + 1))
    } else if p.row + 1 < buf.row_count() {
        Some(Pos::new(p.row + 1, 0))
    } else {
        None
    }
}

fn prev_pos(buf: &Buffer, p: Pos) -> Option<Pos> {
    if p.col > 0 {
        Some(Pos::new(p.row, p.col - 1))
    } else if p.row > 0 {
        let pr = p.row - 1;
        Some(Pos::new(pr, buf.line_len(pr).saturating_sub(1)))
    } else {
        None
    }
}

fn class_at(buf: &Buffer, pos: Pos, big: bool) -> u8 {
    match buf.char_at(pos.row, pos.col) {
        None => 0, // empty line counts as blank
        Some(c) => {
            if big {
                class_big(c)
            } else {
                class_small(c)
            }
        }
    }
}

fn word_fwd_once(buf: &Buffer, pos: Pos, big: bool) -> Pos {
    let cls = class_at(buf, pos, big);
    if cls == 0 {
        // On whitespace: skip blanks, land on next word start.
        let mut p = pos;
        loop {
            match next_pos(buf, p) {
                None => return buf.last_pos(),
                Some(nxt) => {
                    p = nxt;
                    if class_at(buf, nxt, big) != 0 {
                        return p;
                    }
                }
            }
        }
    } else {
        // On a word: skip the rest of this class run, then blanks.
        let mut p = pos;
        loop {
            match next_pos(buf, p) {
                None => return buf.last_pos(),
                Some(nxt) => {
                    if nxt.row != p.row || class_at(buf, nxt, big) != cls {
                        p = nxt;
                        break;
                    }
                    p = nxt;
                }
            }
        }
        if class_at(buf, p, big) == 0 {
            loop {
                match next_pos(buf, p) {
                    None => return buf.last_pos(),
                    Some(nxt) => {
                        p = nxt;
                        if class_at(buf, nxt, big) != 0 {
                            return p;
                        }
                    }
                }
            }
        } else {
            p
        }
    }
}

fn start_of_word_backward(buf: &Buffer, pos: Pos, big: bool) -> Pos {
    let cls = class_at(buf, pos, big);
    let mut p = pos;
    while let Some(nxt) = prev_pos(buf, p) {
        if nxt.row != p.row || class_at(buf, nxt, big) != cls {
            break;
        }
        p = nxt;
    }
    p
}

fn word_back_once(buf: &Buffer, pos: Pos, big: bool) -> Pos {
    let cls = class_at(buf, pos, big);
    if cls == 0 {
        // On whitespace: skip blanks back, then to start of that word.
        let mut p = pos;
        loop {
            match prev_pos(buf, p) {
                None => return Pos::new(0, 0),
                Some(nxt) => {
                    p = nxt;
                    if class_at(buf, nxt, big) != 0 {
                        break;
                    }
                }
            }
        }
        start_of_word_backward(buf, p, big)
    } else {
        let at_start = match prev_pos(buf, pos) {
            None => true,
            Some(pr) => pr.row != pos.row || class_at(buf, pr, big) != cls,
        };
        if at_start {
            // Already at word start: jump to previous word start.
            let mut p = match prev_pos(buf, pos) {
                None => return pos,
                Some(nxt) => nxt,
            };
            while class_at(buf, p, big) == 0 {
                match prev_pos(buf, p) {
                    None => return Pos::new(0, 0),
                    Some(nxt) => p = nxt,
                }
            }
            start_of_word_backward(buf, p, big)
        } else {
            start_of_word_backward(buf, pos, big)
        }
    }
}

fn end_of_word_forward(buf: &Buffer, pos: Pos, big: bool) -> Pos {
    let cls = class_at(buf, pos, big);
    let mut p = pos;
    while let Some(nxt) = next_pos(buf, p) {
        if nxt.row != p.row || class_at(buf, nxt, big) != cls {
            break;
        }
        p = nxt;
    }
    p
}

fn word_end_once(buf: &Buffer, pos: Pos, big: bool) -> Pos {
    let cls = class_at(buf, pos, big);
    if cls == 0 {
        // On whitespace: go to next word, then its end.
        let mut p = pos;
        loop {
            match next_pos(buf, p) {
                None => return buf.last_pos(),
                Some(nxt) => {
                    p = nxt;
                    if class_at(buf, nxt, big) != 0 {
                        break;
                    }
                }
            }
        }
        end_of_word_forward(buf, p, big)
    } else {
        let next_same = match next_pos(buf, pos) {
            Some(nxt) => nxt.row == pos.row && class_at(buf, nxt, big) == cls,
            None => false,
        };
        if next_same {
            end_of_word_forward(buf, pos, big)
        } else {
            // At end of word: advance to next word, then its end.
            let mut p = pos;
            loop {
                match next_pos(buf, p) {
                    None => return buf.last_pos(),
                    Some(nxt) => {
                        p = nxt;
                        if class_at(buf, nxt, big) != 0 {
                            break;
                        }
                    }
                }
            }
            end_of_word_forward(buf, p, big)
        }
    }
}

fn word_end_back_once(buf: &Buffer, pos: Pos, big: bool) -> Pos {
    let cls = class_at(buf, pos, big);
    let mut p = pos;
    if cls != 0 {
        // If on a word, move backwards past this current word run first
        while let Some(pr) = prev_pos(buf, p) {
            if pr.row != p.row || class_at(buf, pr, big) != cls {
                p = pr;
                break;
            }
            p = pr;
        }
        if p == Pos::new(0, 0) && class_at(buf, p, big) == cls {
            return p;
        }
    }
    // Now on whitespace or previous word boundary: skip whitespace backwards until reaching a word
    while class_at(buf, p, big) == 0 {
        match prev_pos(buf, p) {
            None => return Pos::new(0, 0),
            Some(pr) => p = pr,
        }
    }
    p
}

fn match_pair(buf: &Buffer, pos: Pos) -> Pos {
    let line = buf.line_chars(pos.row);
    if line.is_empty() {
        return pos;
    }
    let col = match line
        .iter()
        .enumerate()
        .skip(pos.col)
        .find(|(_, &ch)| matches!(ch, '(' | ')' | '[' | ']' | '{' | '}'))
    {
        Some((c, _)) => c,
        None => return pos,
    };
    let ch = line[col];
    let (target, forward) = match ch {
        '(' => (')', true),
        '[' => (']', true),
        '{' => ('}', true),
        ')' => ('(', false),
        ']' => ('[', false),
        '}' => ('{', false),
        _ => return pos,
    };
    let mut depth = 1usize;
    let mut curr = Pos::new(pos.row, col);
    if forward {
        while let Some(nxt) = next_pos(buf, curr) {
            curr = nxt;
            if let Some(c) = buf.char_at(curr.row, curr.col) {
                if c == ch {
                    depth += 1;
                } else if c == target {
                    depth -= 1;
                    if depth == 0 {
                        return curr;
                    }
                }
            }
        }
    } else {
        while let Some(pr) = prev_pos(buf, curr) {
            curr = pr;
            if let Some(c) = buf.char_at(curr.row, curr.col) {
                if c == ch {
                    depth += 1;
                } else if c == target {
                    depth -= 1;
                    if depth == 0 {
                        return curr;
                    }
                }
            }
        }
    }
    Pos::new(pos.row, col)
}

fn find(buf: &Buffer, pos: Pos, kind: FindKind, target: char, count: usize) -> Pos {
    let line = buf.line_chars(pos.row);
    match kind {
        FindKind::Fwd => {
            let mut seen = 0;
            let mut i = pos.col + 1;
            while i < line.len() {
                if line[i] == target {
                    seen += 1;
                    if seen == count {
                        return Pos::new(pos.row, i);
                    }
                }
                i += 1;
            }
            pos
        }
        FindKind::Back => {
            let mut seen = 0;
            let mut i = pos.col as isize - 1;
            while i >= 0 {
                if line[i as usize] == target {
                    seen += 1;
                    if seen == count {
                        return Pos::new(pos.row, i as usize);
                    }
                }
                i -= 1;
            }
            pos
        }
        FindKind::TillFwd => {
            let mut seen = 0;
            let mut i = pos.col + 1;
            while i < line.len() {
                if line[i] == target {
                    seen += 1;
                    if seen == count {
                        if i == pos.col + 1 {
                            return pos; // already just before target
                        }
                        return Pos::new(pos.row, i - 1);
                    }
                }
                i += 1;
            }
            pos
        }
        FindKind::TillBack => {
            let mut seen = 0;
            let mut i = pos.col as isize - 1;
            while i >= 0 {
                if line[i as usize] == target {
                    seen += 1;
                    if seen == count {
                        if i as usize + 1 == pos.col {
                            return pos; // already just after target
                        }
                        return Pos::new(pos.row, i as usize + 1);
                    }
                }
                i -= 1;
            }
            pos
        }
    }
}

fn para_fwd(buf: &Buffer, pos: Pos) -> Pos {
    let mut r = pos.row + 1;
    while r < buf.row_count() {
        if buf.is_blank_line(r) {
            return Pos::new(r, 0);
        }
        r += 1;
    }
    let lr = buf.row_count().saturating_sub(1);
    buf.clamp(Pos::new(lr, pos.col))
}

fn para_back(buf: &Buffer, pos: Pos) -> Pos {
    if pos.row == 0 {
        return buf.clamp(Pos::new(0, pos.col));
    }
    let mut r = pos.row - 1;
    loop {
        if buf.is_blank_line(r) {
            return Pos::new(r, 0);
        }
        if r == 0 {
            break;
        }
        r -= 1;
    }
    buf.clamp(Pos::new(0, pos.col))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(lines: &[&str]) -> Buffer {
        Buffer::new(lines.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn hjkl_basics() {
        let b = buf(&["abc", "de", "fghi"]);
        let p = Pos::new(0, 0);
        assert_eq!(apply(&b, p, &Motion::Right, 1), Pos::new(0, 1));
        assert_eq!(apply(&b, p, &Motion::Right, 99), Pos::new(0, 2));
        assert_eq!(apply(&b, p, &Motion::Left, 5), Pos::new(0, 0));
        assert_eq!(apply(&b, p, &Motion::Down, 1), Pos::new(1, 0));
        // col clamps to shorter line
        assert_eq!(apply(&b, Pos::new(0, 2), &Motion::Down, 1), Pos::new(1, 1));
        assert_eq!(apply(&b, Pos::new(2, 0), &Motion::Down, 9), Pos::new(2, 0));
        assert_eq!(apply(&b, Pos::new(2, 3), &Motion::Up, 2), Pos::new(0, 2));
    }

    #[test]
    fn word_motions_small() {
        let b = buf(&["foo bar", "  baz(qux)  end"]);
        // w from start -> start of "bar"
        assert_eq!(
            apply(&b, Pos::new(0, 0), &Motion::WordFwdSmall, 1),
            Pos::new(0, 4)
        );
        // w from "bar" -> next line "baz"
        assert_eq!(
            apply(&b, Pos::new(0, 4), &Motion::WordFwdSmall, 1),
            Pos::new(1, 2)
        );
        // w over punctuation: baz -> ( -> qux
        assert_eq!(
            apply(&b, Pos::new(1, 2), &Motion::WordFwdSmall, 1),
            Pos::new(1, 5)
        );
        assert_eq!(
            apply(&b, Pos::new(1, 5), &Motion::WordFwdSmall, 1),
            Pos::new(1, 6)
        );
        // b back to "("
        assert_eq!(
            apply(&b, Pos::new(1, 6), &Motion::WordBackSmall, 1),
            Pos::new(1, 5)
        );
        // e: start of foo -> end of foo
        assert_eq!(
            apply(&b, Pos::new(0, 0), &Motion::WordEndSmall, 1),
            Pos::new(0, 2)
        );
        // e again -> end of bar
        assert_eq!(
            apply(&b, Pos::new(0, 2), &Motion::WordEndSmall, 1),
            Pos::new(0, 6)
        );
    }

    #[test]
    fn word_motions_across_lines_without_spaces() {
        let b = buf(&["ember lantern", "vim quilt"]);
        // from 'm' in "ember" -> 'r' in "ember"
        assert_eq!(
            apply(&b, Pos::new(0, 1), &Motion::WordEndSmall, 1),
            Pos::new(0, 4)
        );
        // from 'r' in "ember" -> 'n' in "lantern" (must not skip lantern)
        assert_eq!(
            apply(&b, Pos::new(0, 4), &Motion::WordEndSmall, 1),
            Pos::new(0, 12)
        );
        // from 'n' in "lantern" -> 'm' in "vim" on next line
        assert_eq!(
            apply(&b, Pos::new(0, 12), &Motion::WordEndSmall, 1),
            Pos::new(1, 2)
        );
        // w from 'l' in "lantern" -> 'v' in "vim"
        assert_eq!(
            apply(&b, Pos::new(0, 6), &Motion::WordFwdSmall, 1),
            Pos::new(1, 0)
        );
        // b from 'm' in "vim" -> 'v' in "vim"
        assert_eq!(
            apply(&b, Pos::new(1, 2), &Motion::WordBackSmall, 1),
            Pos::new(1, 0)
        );
        // b from 'v' in "vim" -> 'l' in "lantern"
        assert_eq!(
            apply(&b, Pos::new(1, 0), &Motion::WordBackSmall, 1),
            Pos::new(0, 6)
        );
    }

    #[test]
    fn big_words_skip_punct() {
        let b = buf(&["foo(bar) baz"]);
        // W treats "foo(bar)" as one WORD
        assert_eq!(
            apply(&b, Pos::new(0, 0), &Motion::WordFwdBig, 1),
            Pos::new(0, 9)
        );
        assert_eq!(
            apply(&b, Pos::new(0, 0), &Motion::WordEndBig, 1),
            Pos::new(0, 7)
        );
    }

    #[test]
    fn line_motions() {
        let b = buf(&["   hello   ", "", "x"]);
        assert_eq!(
            apply(&b, Pos::new(0, 5), &Motion::LineStart, 1),
            Pos::new(0, 0)
        );
        assert_eq!(
            apply(&b, Pos::new(0, 0), &Motion::FirstNonBlank, 1),
            Pos::new(0, 3)
        );
        assert_eq!(
            apply(&b, Pos::new(0, 0), &Motion::LineEnd, 1),
            Pos::new(0, 10)
        );
        assert_eq!(
            apply(&b, Pos::new(0, 0), &Motion::LastNonBlank, 1),
            Pos::new(0, 7)
        );
        assert_eq!(
            apply(&b, Pos::new(2, 0), &Motion::FileTop, 1),
            Pos::new(0, 3)
        );
        assert_eq!(
            apply(&b, Pos::new(0, 0), &Motion::FileBottom, 1),
            Pos::new(2, 0)
        );
        assert_eq!(
            apply(&b, Pos::new(2, 0), &Motion::GotoLine, 2),
            Pos::new(1, 0)
        );
        // Counted line motions: [count]$ / ^ / g_ move [count] - 1 lines down
        assert_eq!(
            apply(&b, Pos::new(0, 0), &Motion::LineEnd, 3),
            Pos::new(2, 0)
        );
        assert_eq!(
            apply(&b, Pos::new(0, 0), &Motion::LastNonBlank, 3),
            Pos::new(2, 0)
        );
        assert_eq!(
            apply(&b, Pos::new(0, 0), &Motion::FirstNonBlank, 3),
            Pos::new(2, 0)
        );
    }

    #[test]
    fn find_motions() {
        let b = buf(&["abracadabra"]);
        assert_eq!(
            apply(&b, Pos::new(0, 0), &Motion::Find(FindKind::Fwd, 'a'), 1),
            Pos::new(0, 3)
        );
        assert_eq!(
            apply(&b, Pos::new(0, 0), &Motion::Find(FindKind::Fwd, 'a'), 2),
            Pos::new(0, 5)
        );
        assert_eq!(
            apply(&b, Pos::new(0, 0), &Motion::Find(FindKind::TillFwd, 'a'), 1),
            Pos::new(0, 2)
        );
        assert_eq!(
            apply(&b, Pos::new(0, 10), &Motion::Find(FindKind::Back, 'a'), 1),
            Pos::new(0, 7)
        );
        // missing char: no movement
        assert_eq!(
            apply(&b, Pos::new(0, 0), &Motion::Find(FindKind::Fwd, 'z'), 1),
            Pos::new(0, 0)
        );
    }

    #[test]
    fn paragraphs() {
        let b = buf(&["one", "two", "", "three", "", "four"]);
        assert_eq!(
            apply(&b, Pos::new(0, 0), &Motion::ParaFwd, 1),
            Pos::new(2, 0)
        );
        assert_eq!(
            apply(&b, Pos::new(2, 0), &Motion::ParaFwd, 1),
            Pos::new(4, 0)
        );
        assert_eq!(
            apply(&b, Pos::new(5, 0), &Motion::ParaBack, 1),
            Pos::new(4, 0)
        );
    }

    /// Full scripted playthrough on a tricky buffer: every step hand-verified.
    #[test]
    fn scripted_word_playthrough() {
        let b = buf(&["foo bar", "  baz(qux)  end"]);
        let mut p = Pos::new(0, 0);
        let step = |b: &Buffer, p: Pos, m: Motion, n: usize| apply(b, p, &m, n);

        p = step(&b, p, Motion::WordFwdSmall, 1);
        assert_eq!(p, Pos::new(0, 4)); // w: bar start
        p = step(&b, p, Motion::WordEndSmall, 1);
        assert_eq!(p, Pos::new(0, 6)); // e: bar end
        p = step(&b, p, Motion::WordEndSmall, 1);
        assert_eq!(p, Pos::new(1, 4)); // e: over the line break to baz end
        p = step(&b, p, Motion::WordBackSmall, 1);
        assert_eq!(p, Pos::new(1, 2)); // b: baz start
        p = step(&b, p, Motion::WordBackSmall, 1);
        assert_eq!(p, Pos::new(0, 4)); // b: back up to bar start
        p = step(&b, p, Motion::WordFwdSmall, 1);
        assert_eq!(p, Pos::new(1, 2)); // w: down to baz start
        p = step(&b, p, Motion::WordFwdSmall, 1);
        assert_eq!(p, Pos::new(1, 5)); // w: ( is its own word
        p = step(&b, p, Motion::WordEndSmall, 1);
        assert_eq!(p, Pos::new(1, 8)); // e: qux end
        p = step(&b, p, Motion::WordBackSmall, 1);
        assert_eq!(p, Pos::new(1, 6)); // b: qux start
        p = step(&b, p, Motion::WordBackSmall, 1);
        assert_eq!(p, Pos::new(1, 5)); // b: back onto (
        p = step(&b, p, Motion::WordFwdSmall, 1);
        assert_eq!(p, Pos::new(1, 6)); // w: adjacent class is a new start
        // Big words: foo(bar) is one WORD.
        p = Pos::new(0, 0);
        let bb = buf(&["foo(bar) baz"]);
        p = step(&bb, p, Motion::WordFwdBig, 1);
        assert_eq!(p, Pos::new(0, 9));
        p = step(&bb, p, Motion::WordBackBig, 1);
        assert_eq!(p, Pos::new(0, 0));
    }

    /// Walls: motions at boundaries must stay put (reported as "no move").
    #[test]
    fn boundary_motions_stay_put() {
        let b = buf(&["ab", "", "  ", "c"]);
        assert_eq!(apply(&b, Pos::new(0, 0), &Motion::Left, 9), Pos::new(0, 0));
        assert_eq!(apply(&b, Pos::new(0, 1), &Motion::Right, 9), Pos::new(0, 1));
        assert_eq!(apply(&b, Pos::new(0, 1), &Motion::Down, 9), Pos::new(3, 0));
        assert_eq!(apply(&b, Pos::new(3, 0), &Motion::Up, 9), Pos::new(0, 0));
        // w at end of file, b at start of file.
        let last = b.last_pos();
        assert_eq!(apply(&b, last, &Motion::WordFwdSmall, 1), last);
        assert_eq!(apply(&b, Pos::new(0, 0), &Motion::WordBackSmall, 1), Pos::new(0, 0));
        // f with a missing char.
        assert_eq!(
            apply(&b, Pos::new(0, 0), &Motion::Find(FindKind::Fwd, 'z'), 1),
            Pos::new(0, 0)
        );
    }

    /// Fuzz: every motion from every cell of nasty buffers stays in bounds.
    #[test]
    fn motions_never_leave_the_buffer() {
        use FindKind::*;
        let buffers = vec![
            buf(&[""]),
            buf(&["", "", ""]),
            buf(&["   ", "\t", ""]),
            buf(&["a"]),
            buf(&["ab cd(ef)  gh", "", "   ij", "klmno p qrs", "t"]),
            buf(&["héllo wörld", "  ünïcodé!?", "x"]),
        ];
        let find_chars = ['a', 'e', ' ', '(', 'z', 'é'];
        for b in &buffers {
            for row in 0..b.row_count() {
                for col in 0..b.line_len(row).max(1) {
                    let p = Pos::new(row, col);
                    let mut motions = vec![
                        Motion::Left,
                        Motion::Down,
                        Motion::Up,
                        Motion::Right,
                        Motion::WordFwdSmall,
                        Motion::WordFwdBig,
                        Motion::WordBackSmall,
                        Motion::WordBackBig,
                        Motion::WordEndSmall,
                        Motion::WordEndBig,
                        Motion::LineStart,
                        Motion::FirstNonBlank,
                        Motion::LineEnd,
                        Motion::LastNonBlank,
                        Motion::FileTop,
                        Motion::FileBottom,
                        Motion::GotoLine,
                        Motion::ScreenTop,
                        Motion::ScreenMid,
                        Motion::ScreenBot,
                        Motion::ParaFwd,
                        Motion::ParaBack,
                    ];
                    for ch in find_chars {
                        for kind in [Fwd, Back, TillFwd, TillBack] {
                            motions.push(Motion::Find(kind, ch));
                        }
                    }
                    for m in &motions {
                        for n in [1usize, 2, 3, 12] {
                            let count = if matches!(m, Motion::GotoLine) {
                                n.min(b.row_count().max(1))
                            } else {
                                n
                            };
                            let q = apply(b, p, m, count);
                            assert_eq!(b.clamp(q), q, "escaped: {m:?} x{count} from {p:?}");
                            // Deterministic: same input, same output.
                            assert_eq!(apply(b, p, m, count), q);
                        }
                    }
                }
            }
        }
    }
}
