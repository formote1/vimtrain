//! Key state machine: counts, g-prefix, f/t-prefix, ;/, repeats.
//!
//! The app feeds raw key events here; when a complete motion is formed
//! this returns `Feed::Done(motion, count)`.

use crossterm::event::{KeyCode, KeyModifiers};

use super::motion::{FindKind, Motion};

pub enum Feed {
    /// More keys needed (digit, `g`, `f`...).
    NeedMore,
    /// A complete motion with its count (for GotoLine the count is the line).
    Done(Motion, usize),
    /// Pending state was cleared (Esc).
    Cancelled,
    /// Key is not a motion key.
    Invalid,
}

pub struct KeyState {
    count: Option<usize>,
    g_pending: bool,
    find_pending: Option<FindKind>,
    pub last_find: Option<(FindKind, char)>,
}

impl KeyState {
    pub fn new() -> Self {
        Self {
            count: None,
            g_pending: false,
            find_pending: None,
            last_find: None,
        }
    }

    pub fn clear(&mut self) {
        self.count = None;
        self.g_pending = false;
        self.find_pending = None;
    }

    pub fn is_pending(&self) -> bool {
        self.count.is_some() || self.g_pending || self.find_pending.is_some()
    }

    /// Short pending description for the HUD, e.g. "3", "g", "f", "2f".
    pub fn pending_text(&self) -> String {
        let mut s = String::new();
        if let Some(n) = self.count {
            s.push_str(&n.to_string());
        }
        if self.g_pending {
            s.push('g');
        }
        if let Some(k) = self.find_pending {
            s.push(k.key());
        }
        s
    }

    pub fn feed(&mut self, code: KeyCode, mods: KeyModifiers) -> Feed {        if mods.contains(KeyModifiers::CONTROL) || mods.contains(KeyModifiers::ALT) {
            return Feed::Invalid;
        }
        match code {
            KeyCode::Esc => {
                if self.is_pending() {
                    self.clear();
                    Feed::Cancelled
                } else {
                    Feed::Cancelled
                }
            }
            KeyCode::Char(c) => self.feed_char(c),
            _ => Feed::Invalid,
        }
    }

    fn feed_char(&mut self, c: char) -> Feed {
        // Awaiting the target char of f/F/t/T.
        if let Some(kind) = self.find_pending.take() {
            let n = self.count.take().unwrap_or(1);
            self.last_find = Some((kind, c));
            return Feed::Done(Motion::Find(kind, c), n);
        }

        // Awaiting the second key of a g-prefix.
        if self.g_pending {
            self.g_pending = false;
            match c {
                'g' => match self.count.take() {
                    Some(line) => return Feed::Done(Motion::GotoLine, line),
                    None => return Feed::Done(Motion::FileTop, 1),
                },
                '_' => {
                    let n = self.count.take().unwrap_or(1);
                    return Feed::Done(Motion::LastNonBlank, n);
                }
                'e' => {
                    let n = self.count.take().unwrap_or(1);
                    return Feed::Done(Motion::WordEndBackSmall, n);
                }
                'E' => {
                    let n = self.count.take().unwrap_or(1);
                    return Feed::Done(Motion::WordEndBackBig, n);
                }
                _ => {
                    self.count = None;
                    return Feed::Invalid;
                }
            }
        }

        // Bare 0 is LineStart; otherwise it extends a count.
        if c == '0' && self.count.is_none() {
            return Feed::Done(Motion::LineStart, 1);
        }
        if c.is_ascii_digit() {
            let d = (c as u8 - b'0') as usize;
            self.count = Some(self.count.unwrap_or(0) * 10 + d);
            return Feed::NeedMore;
        }

        if c == 'g' {
            self.g_pending = true;
            return Feed::NeedMore;
        }
        if c == 'f' {
            self.find_pending = Some(FindKind::Fwd);
            return Feed::NeedMore;
        }
        if c == 'F' {
            self.find_pending = Some(FindKind::Back);
            return Feed::NeedMore;
        }
        if c == 't' {
            self.find_pending = Some(FindKind::TillFwd);
            return Feed::NeedMore;
        }
        if c == 'T' {
            self.find_pending = Some(FindKind::TillBack);
            return Feed::NeedMore;
        }

        let cnt = self.count.take();
        let n = cnt.unwrap_or(1);
        match c {
            'h' => Feed::Done(Motion::Left, n),
            'j' => Feed::Done(Motion::Down, n),
            'k' => Feed::Done(Motion::Up, n),
            'l' => Feed::Done(Motion::Right, n),
            'w' => Feed::Done(Motion::WordFwdSmall, n),
            'W' => Feed::Done(Motion::WordFwdBig, n),
            'b' => Feed::Done(Motion::WordBackSmall, n),
            'B' => Feed::Done(Motion::WordBackBig, n),
            'e' => Feed::Done(Motion::WordEndSmall, n),
            'E' => Feed::Done(Motion::WordEndBig, n),
            '^' | '_' => Feed::Done(Motion::FirstNonBlank, 1),
            '$' => Feed::Done(Motion::LineEnd, 1),
            '%' => Feed::Done(Motion::MatchPair, 1),
            'G' => match cnt {
                Some(line) => Feed::Done(Motion::GotoLine, line.max(1)),
                None => Feed::Done(Motion::FileBottom, 1),
            },
            '{' => Feed::Done(Motion::ParaBack, n),
            '}' => Feed::Done(Motion::ParaFwd, n),
            ';' => match self.last_find {
                Some((kind, ch)) => Feed::Done(Motion::Find(kind, ch), n),
                None => Feed::Invalid,
            },
            ',' => match self.last_find {
                Some((kind, ch)) => Feed::Done(Motion::Find(kind.reversed(), ch), n),
                None => Feed::Invalid,
            },
            'H' => Feed::Done(Motion::ScreenTop, 1),
            'M' => Feed::Done(Motion::ScreenMid, 1),
            'L' => Feed::Done(Motion::ScreenBot, 1),
            _ => Feed::Invalid,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(ks: &mut KeyState, c: char) -> Feed {
        ks.feed(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn is_done(f: Feed, m: Motion, n: usize) -> bool {
        matches!(f, Feed::Done(mm, nn) if mm == m && nn == n)
    }

    #[test]
    fn counts_accumulate_then_fire() {
        let mut ks = KeyState::new();
        assert!(matches!(press(&mut ks, '3'), Feed::NeedMore));
        assert_eq!(ks.pending_text(), "3");
        assert!(is_done(press(&mut ks, 'j'), Motion::Down, 3));
        assert!(!ks.is_pending());
    }

    #[test]
    fn bare_zero_is_line_start_but_10_is_a_count() {
        let mut ks = KeyState::new();
        assert!(is_done(press(&mut ks, '0'), Motion::LineStart, 1));
        let mut ks = KeyState::new();
        assert!(matches!(press(&mut ks, '1'), Feed::NeedMore));
        assert!(matches!(press(&mut ks, '0'), Feed::NeedMore));
        assert!(is_done(press(&mut ks, 'G'), Motion::GotoLine, 10));
    }

    #[test]
    fn g_prefix_and_find_prefix() {
        let mut ks = KeyState::new();
        assert!(matches!(press(&mut ks, 'g'), Feed::NeedMore));
        assert!(is_done(press(&mut ks, 'g'), Motion::FileTop, 1));
        let mut ks = KeyState::new();
        assert!(matches!(press(&mut ks, '5'), Feed::NeedMore));
        assert!(matches!(press(&mut ks, 'g'), Feed::NeedMore));
        assert!(is_done(press(&mut ks, 'g'), Motion::GotoLine, 5));
        let mut ks = KeyState::new();
        assert!(matches!(press(&mut ks, 'f'), Feed::NeedMore));
        assert_eq!(ks.pending_text(), "f");
        assert!(is_done(press(&mut ks, 'x'), Motion::Find(FindKind::Fwd, 'x'), 1));
        // last_find remembered for ; and ,
        assert!(is_done(
            press(&mut ks, ';'),
            Motion::Find(FindKind::Fwd, 'x'),
            1
        ));
        assert!(is_done(
            press(&mut ks, ','),
            Motion::Find(FindKind::Back, 'x'),
            1
        ));
    }

    #[test]
    fn repeat_without_find_is_invalid_and_esc_clears() {
        let mut ks = KeyState::new();
        assert!(matches!(press(&mut ks, ';'), Feed::Invalid));
        let mut ks = KeyState::new();
        assert!(matches!(press(&mut ks, '2'), Feed::NeedMore));
        assert!(matches!(
            ks.feed(KeyCode::Esc, KeyModifiers::NONE),
            Feed::Cancelled
        ));
        assert!(!ks.is_pending());
        // After clear, plain j has no count.
        assert!(is_done(press(&mut ks, 'j'), Motion::Down, 1));
    }

    #[test]
    fn new_key_combinations() {
        let mut ks = KeyState::new();
        assert!(matches!(press(&mut ks, 'g'), Feed::NeedMore));
        assert!(is_done(press(&mut ks, 'e'), Motion::WordEndBackSmall, 1));

        let mut ks = KeyState::new();
        assert!(matches!(press(&mut ks, '3'), Feed::NeedMore));
        assert!(matches!(press(&mut ks, 'g'), Feed::NeedMore));
        assert!(is_done(press(&mut ks, 'e'), Motion::WordEndBackSmall, 3));

        let mut ks = KeyState::new();
        assert!(matches!(press(&mut ks, 'g'), Feed::NeedMore));
        assert!(is_done(press(&mut ks, 'E'), Motion::WordEndBackBig, 1));

        let mut ks = KeyState::new();
        assert!(is_done(press(&mut ks, '%'), Motion::MatchPair, 1));

        let mut ks = KeyState::new();
        assert!(is_done(press(&mut ks, '_'), Motion::FirstNonBlank, 1));
    }

    #[test]
    fn ctrl_combos_never_form_motions() {
        let mut ks = KeyState::new();
        assert!(matches!(
            ks.feed(KeyCode::Char('j'), KeyModifiers::CONTROL),
            Feed::Invalid
        ));
    }
}
