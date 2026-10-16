//! Skill-by-skill curriculum: one key at a time, then fluency mixes.
//!
//! Each teaching skill has a fixed demo room (copy the shown keys, feel
//! the motion, zero pressure) before the randomized challenge, plus
//! per-key labels shown as chips (key box, meaning underneath) — the same
//! header-over-body shape as the help cards.

use crate::engine::motion::{FindKind, Motion};

// ---------------------------------------------------------------- kits ---

// One bit per learnable motion. A skill's kit is the union of its own new
// bits plus every previous skill's bits (keys are collected, never lost).
pub const K_H: u32 = 1 << 0;
pub const K_L: u32 = 1 << 1;
pub const K_J: u32 = 1 << 2;
pub const K_K: u32 = 1 << 3;
pub const K_W: u32 = 1 << 4;
pub const K_E: u32 = 1 << 5;
pub const K_B: u32 = 1 << 6;
pub const K_BIGW: u32 = 1 << 7;
pub const K_BIGE: u32 = 1 << 8;
pub const K_BIGB: u32 = 1 << 9;
pub const K_F: u32 = 1 << 10;
pub const K_BIGF: u32 = 1 << 11;
pub const K_T: u32 = 1 << 12;
pub const K_BIGT: u32 = 1 << 13;
pub const K_SEMI: u32 = 1 << 14;
pub const K_COMMA: u32 = 1 << 15;
pub const K_ZERO: u32 = 1 << 16;
pub const K_DOLLAR: u32 = 1 << 17;
pub const K_CARET: u32 = 1 << 18;
pub const K_GUNDER: u32 = 1 << 19;
pub const K_GG: u32 = 1 << 20;
pub const K_G: u32 = 1 << 21;
pub const K_HML: u32 = 1 << 22;
pub const K_PARA: u32 = 1 << 23;
pub const K_COUNTS: u32 = 1 << 24;
pub const K_GE: u32 = 1 << 25;
pub const K_PERCENT: u32 = 1 << 26;

pub const SKILL_COUNT: usize = 25;

/// Cleared rounds needed to master one skill.
pub const WINS_PER_SKILL: u32 = 2;

/// Which kit bit a motion needs.
pub fn bit_for(m: &Motion) -> u32 {
    match m {
        Motion::Left => K_H,
        Motion::Down => K_J,
        Motion::Up => K_K,
        Motion::Right => K_L,
        Motion::WordFwdSmall => K_W,
        Motion::WordFwdBig => K_BIGW,
        Motion::WordBackSmall => K_B,
        Motion::WordBackBig => K_BIGB,
        Motion::WordEndSmall => K_E,
        Motion::WordEndBig => K_BIGE,
        Motion::WordEndBackSmall | Motion::WordEndBackBig => K_GE,
        Motion::MatchPair => K_PERCENT,
        Motion::LineStart => K_ZERO,
        Motion::FirstNonBlank => K_CARET,
        Motion::LineEnd => K_DOLLAR,
        Motion::LastNonBlank => K_GUNDER,
        Motion::FileTop => K_GG,
        Motion::FileBottom => K_G,
        Motion::GotoLine => K_G,
        Motion::ScreenTop | Motion::ScreenMid | Motion::ScreenBot => K_HML,
        Motion::Find(FindKind::Fwd, _) => K_F,
        Motion::Find(FindKind::Back, _) => K_BIGF,
        Motion::Find(FindKind::TillFwd, _) => K_T,
        Motion::Find(FindKind::TillBack, _) => K_BIGT,
        Motion::ParaFwd | Motion::ParaBack => K_PARA,
    }
}

pub fn kit_allows(kit: u32, m: &Motion) -> bool {
    kit & bit_for(m) != 0
}

/// Counts ride along with G-numbers; every other count needs K_COUNTS.
pub fn kit_allows_count(kit: u32, m: &Motion, count: usize) -> bool {
    if !kit_allows(kit, m) {
        return false;
    }
    if count <= 1 {
        return true;
    }
    match m {
        Motion::GotoLine => kit & K_G != 0,
        _ => kit & K_COUNTS != 0,
    }
}

// --------------------------------------------------------------- skills ---

/// Spotlight used to prove a board really teaches its skill.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Spot {
    H,
    J,
    K,
    L,
    W,
    BigW,
    B,
    BigB,
    E,
    BigE,
    F,
    BigF,
    T,
    BigT,
    Semi,
    Zero,
    Dollar,
    Caret,
    GG,
    G,
    Hml,
    Para,
    Counts,
}

/// Does an optimal path use this spotlight? (Find matches by kind;
/// Counts matches any multiplied motion, including `nG` line jumps.)
/// Test-only analysis helper: the game itself never calls this.
#[cfg(test)]
pub fn spot_hit(spot: Spot, path: &[(Motion, usize)]) -> bool {
    if spot == Spot::Counts {
        return path.iter().any(|(_, n)| *n > 1);
    }
    path.iter().any(|(m, _)| {
        matches!(
            (spot, m),
            (Spot::H, Motion::Left)
                | (Spot::J, Motion::Down)
                | (Spot::K, Motion::Up)
                | (Spot::L, Motion::Right)
                | (Spot::W, Motion::WordFwdSmall)
                | (Spot::BigW, Motion::WordFwdBig)
                | (Spot::B, Motion::WordBackSmall)
                | (Spot::BigB, Motion::WordBackBig)
                | (Spot::E, Motion::WordEndSmall)
                | (Spot::BigE, Motion::WordEndBig)
                | (Spot::F, Motion::Find(FindKind::Fwd, _))
                | (Spot::BigF, Motion::Find(FindKind::Back, _))
                | (Spot::T, Motion::Find(FindKind::TillFwd, _))
                | (Spot::BigT, Motion::Find(FindKind::TillBack, _))
                | (Spot::Semi, Motion::Find(_, _))
                | (Spot::Zero, Motion::LineStart)
                | (Spot::Dollar, Motion::LineEnd)
                | (Spot::Caret, Motion::FirstNonBlank)
                | (Spot::GG, Motion::FileTop)
                | (Spot::G, Motion::FileBottom)
                | (Spot::G, Motion::GotoLine)
                | (Spot::Hml, Motion::ScreenTop)
                | (Spot::Hml, Motion::ScreenMid)
                | (Spot::Hml, Motion::ScreenBot)
                | (Spot::Para, Motion::ParaFwd)
                | (Spot::Para, Motion::ParaBack)
        )
    })
}

/// Fixed demo room: copy the shown keys once, feel the motion, no pressure.
/// `|` marks the cursor in teach examples (not in demo boards).
pub struct Demo {
    pub lines: &'static [&'static str],
    pub start: (usize, usize),
    pub goals: &'static [(usize, usize)],
    pub press: &'static str,
}

pub struct Skill {
    /// Chapter 1-6 (matches LESSONS ids).
    pub lesson: u8,
    pub name: &'static str,
    /// Display keys, e.g. "w" or "{ }".
    pub keys: &'static str,
    /// One-word meaning for the playing HUD's key shelf.
    pub meaning: &'static str,
    /// Per-key labels: (key, meaning), rendered as chips with the meaning
    /// underneath. Empty for fluency mixes (nothing new to label).
    pub labels: &'static [(&'static str, &'static str)],
    /// Newly taught kit bits (0 for fluency/capstone mixes).
    pub new: u32,
    /// Spotlight motions this skill's boards must reward.
    /// Empty = fluency mix (proven by motion variety instead).
    /// Read by the pedagogy test-suite, not by gameplay code.
    #[allow(dead_code)]
    pub spots: &'static [Spot],
    /// Minimum total par a challenge board must have.
    pub floor: usize,
    /// Coach copy: worked example + when/pairing tip. Definitions live in
    /// the chips and cards, never duplicated here.
    pub teach: &'static [&'static str],
    /// Demo room before the challenge (None for fluency mixes).
    pub demo: Option<Demo>,
}

pub const SKILLS: [Skill; SKILL_COUNT] = [
    Skill {
        lesson: 1,
        name: "Side to Side",
        keys: "h l",
        meaning: "sideways",
        labels: &[("h", "left"), ("l", "right")],
        new: K_H | K_L,
        spots: &[Spot::H, Spot::L],
        floor: 4,
        teach: &[
            "Slide the zigzag corridor: every goal needs both keys.",
            "Plan each hop: fewer keys beats wandering.",
        ],
        demo: Some(Demo {
            lines: &[
                "............................................",
                "............................................",
                "............................................",
                "............................................",
                "............................................",
                "............................................",
                "............................................",
                "............................................",
            ],
            start: (3, 22),
            goals: &[(3, 14), (3, 30)],
            press: "h eight times, then l sixteen times",
        }),
    },
    Skill {
        lesson: 1,
        name: "Up and Down",
        keys: "j k",
        meaning: "vertical",
        labels: &[("j", "down"), ("k", "up")],
        new: K_J | K_K,
        spots: &[Spot::J, Spot::K],
        floor: 4,
        teach: &[
            "Climb and dive: the corridor zigzags up and down.",
            "With h l you roam all four directions now.",
        ],
        demo: Some(Demo {
            lines: &[
                "............................................",
                "............................................",
                "............................................",
                "............................................",
                "............................................",
                "............................................",
                "............................................",
                "............................................",
            ],
            start: (3, 22),
            goals: &[(1, 22), (6, 22)],
            press: "k two times, then j five times",
        }),
    },
    Skill {
        lesson: 1,
        name: "Open Field",
        keys: "h j k l",
        meaning: "roam",
        labels: &[],
        new: 0,
        spots: &[],
        floor: 8,
        teach: &[
            "No new keys. Just you and the open field.",
            "Plan each hop and match par to open the next chapter.",
        ],
        demo: None,
    },
    Skill {
        lesson: 2,
        name: "Word Forward",
        keys: "w",
        meaning: "next word",
        labels: &[("w", "next word")],
        new: K_W,
        spots: &[Spot::W],
        floor: 5,
        teach: &[
            "Try it: |aaa bbb goes w to aaa |bbb (| is your cursor).",
            "l walks cells, w leaps words. Pairs with b, coming next.",
        ],
        demo: Some(Demo {
            lines: &["aaa bbb ccc ddd"],
            start: (0, 0),
            goals: &[(0, 12)],
            press: "w w w",
        }),
    },
    Skill {
        lesson: 2,
        name: "Word End",
        keys: "e",
        meaning: "word end",
        labels: &[("e", "word end")],
        new: K_E,
        spots: &[Spot::E],
        floor: 5,
        teach: &[
            "Try it: |aaa goes e to aa|a.",
            "w lands on fronts, e on backs. You need both sights.",
        ],
        demo: Some(Demo {
            lines: &["aaa bbb ccc ddd"],
            start: (0, 0),
            goals: &[(0, 2), (0, 6)],
            press: "e e",
        }),
    },
    Skill {
        lesson: 2,
        name: "Step Back",
        keys: "b",
        meaning: "word back",
        labels: &[("b", "word back")],
        new: K_B,
        spots: &[Spot::B],
        floor: 5,
        teach: &[
            "Try it: aaa |bbb goes b to |aaa bbb.",
            "Overshot with w? One b steps home. Forward, back.",
        ],
        demo: Some(Demo {
            lines: &["aaa bbb ccc ddd"],
            start: (0, 14),
            goals: &[(0, 12), (0, 8)],
            press: "b b",
        }),
    },
    Skill {
        lesson: 2,
        name: "Word Weave",
        keys: "w e b",
        meaning: "weave",
        labels: &[],
        new: 0,
        spots: &[],
        floor: 8,
        teach: &[
            "Try it: |aaa bbb goes e e to aaa bb|b, then b to aaa |bbb.",
            "Fronts want w/b, backs want e. Read each hop.",
        ],
        demo: None,
    },
    Skill {
        lesson: 2,
        name: "BIG Words",
        keys: "W B E ge",
        meaning: "BIG word",
        labels: &[
            ("W", "big fwd"),
            ("B", "big back"),
            ("E", "big end"),
            ("ge", "end back"),
        ],
        new: K_BIGW | K_BIGE | K_BIGB | K_GE,
        spots: &[Spot::BigW, Spot::BigE, Spot::BigB],
        floor: 6,
        teach: &[
            "Try it: |foo(bar) baz goes W to foo(bar) |baz.",
            "Small words split on ( ) , BIG words swallow them. Look first.",
        ],
        demo: Some(Demo {
            lines: &["foo(bar) baz"],
            start: (0, 0),
            goals: &[(0, 9)],
            press: "W",
        }),
    },
    Skill {
        lesson: 2,
        name: "Word Fluency",
        keys: "words",
        meaning: "words",
        labels: &[],
        new: 0,
        spots: &[],
        floor: 10,
        teach: &[
            "Everything about words, mixed and longer.",
            "Small vs BIG: look at the punctuation before you move.",
        ],
        demo: None,
    },
    Skill {
        lesson: 3,
        name: "Seek",
        keys: "f",
        meaning: "find fwd",
        labels: &[("f x", "jump to x")],
        new: K_F,
        spots: &[Spot::F],
        floor: 5,
        teach: &[
            "Try it: |abracadabra goes fa to abr|acadabra.",
            "One jump beats five l-taps. Same line only.",
        ],
        demo: Some(Demo {
            lines: &["abracadabra"],
            start: (0, 0),
            goals: &[(0, 3)],
            press: "f a",
        }),
    },
    Skill {
        lesson: 3,
        name: "Seek Back",
        keys: "F",
        meaning: "find back",
        labels: &[("F x", "prev x")],
        new: K_BIGF,
        spots: &[Spot::BigF],
        floor: 5,
        teach: &[
            "Try it: abracadabr|a goes Fa to abracad|abra.",
            "Line ends are your start here: targets sit behind you.",
        ],
        demo: Some(Demo {
            lines: &["abracadabra"],
            start: (0, 10),
            goals: &[(0, 7)],
            press: "F a",
        }),
    },
    Skill {
        lesson: 3,
        name: "Till There",
        keys: "t T",
        meaning: "till",
        labels: &[("t", "till fwd"), ("T", "till back")],
        new: K_T | K_BIGT,
        spots: &[Spot::T, Spot::BigT],
        floor: 5,
        teach: &[
            "Try it: |abracadabra goes ta to ab|racadabra.",
            "Want the char? f. Want the cell before it? t.",
        ],
        demo: Some(Demo {
            lines: &["abracadabra"],
            start: (0, 0),
            goals: &[(0, 2)],
            press: "t a",
        }),
    },
    Skill {
        lesson: 3,
        name: "Again",
        keys: "; ,",
        meaning: "repeat",
        labels: &[(";", "again"), (",", "reverse")],
        new: K_SEMI | K_COMMA,
        spots: &[Spot::Semi],
        floor: 4,
        teach: &[
            "Try it: |axxxaxxxaxxx goes fa, then ; lands axxxaxxx|axxx.",
            "; repeats forward, , backwards. Find once, land often.",
        ],
        demo: Some(Demo {
            lines: &["axxxaxxxaxxx"],
            start: (0, 0),
            goals: &[(0, 8)],
            press: "f a, then ;",
        }),
    },
    Skill {
        lesson: 3,
        name: "Find Fluency",
        keys: "find",
        meaning: "find",
        labels: &[],
        new: 0,
        spots: &[],
        floor: 10,
        teach: &[
            "Every find trick mixed: f F t T with ; and ,.",
            "Spot the repeated letter, pick direction, repeat.",
        ],
        demo: None,
    },
    Skill {
        lesson: 4,
        name: "Line Edges",
        keys: "0 $ %",
        meaning: "edges",
        labels: &[
            ("0", "line start"),
            ("$", "line end"),
            ("%", "match pair"),
        ],
        new: K_ZERO | K_DOLLAR | K_PERCENT,
        spots: &[Spot::Zero, Spot::Dollar],
        floor: 5,
        teach: &[
            "Try it: ab|cd efgh goes $ to abcd efg|h, then 0 to |abcd efgh.",
            "Whole-line crosses in one key. Counts multiply them later.",
        ],
        demo: Some(Demo {
            lines: &["abcd efgh"],
            start: (0, 2),
            goals: &[(0, 8), (0, 0)],
            press: "$, then 0",
        }),
    },
    Skill {
        lesson: 4,
        name: "Indent Head",
        keys: "^ g_",
        meaning: "head",
        labels: &[("^", "head"), ("g_", "tail")],
        new: K_CARET | K_GUNDER,
        spots: &[Spot::Caret],
        floor: 4,
        teach: &[
            "Try it: ____aaa bb|b goes ^ to ____|aaa bbb.",
            "Code is indented: ^ lands where words start. g_ mirrors the end.",
        ],
        demo: Some(Demo {
            lines: &["    aaa bbb"],
            start: (0, 10),
            goals: &[(0, 4)],
            press: "^",
        }),
    },
    Skill {
        lesson: 4,
        name: "File Jumps",
        keys: "gg G",
        meaning: "jumps",
        labels: &[("gg", "first line"), ("G", "last line")],
        new: K_GG | K_G,
        spots: &[Spot::GG, Spot::G],
        floor: 5,
        teach: &[
            "Try it mid-file: gg lands line 1, then G lands last.",
            "Bonus: 5G jumps straight to line 5. G-numbers ride free.",
        ],
        demo: Some(Demo {
            lines: &["aaaa", "aaaa", "aaaa", "aaaa", "aaaa"],
            start: (2, 0),
            goals: &[(0, 0), (4, 0)],
            press: "g g, then G",
        }),
    },
    Skill {
        lesson: 4,
        name: "Screen Thirds",
        keys: "H M L",
        meaning: "thirds",
        labels: &[("H", "top"), ("M", "middle"), ("L", "bottom")],
        new: K_HML,
        spots: &[Spot::Hml],
        floor: 5,
        teach: &[
            "Try it: H lands top row, M middle, L bottom.",
            "Three landmarks, one key each. M is home base.",
        ],
        demo: Some(Demo {
            lines: &[
                "......",
                "......",
                "......",
                "......",
                "......",
                "......",
                "......",
                "......",
                "......",
            ],
            start: (4, 2),
            goals: &[(0, 0), (4, 0), (8, 0)],
            press: "H, then M, then L",
        }),
    },
    Skill {
        lesson: 4,
        name: "Line Fluency",
        keys: "lines",
        meaning: "lines",
        labels: &[],
        new: 0,
        spots: &[],
        floor: 10,
        teach: &[
            "Edges, heads, jumps and thirds, all mixed.",
            "Own the line, own the file.",
        ],
        demo: None,
    },
    Skill {
        lesson: 5,
        name: "Multiplier",
        keys: "counts",
        meaning: "times N",
        labels: &[("3j", "times 3"), ("2w", "times 2")],
        new: K_COUNTS,
        spots: &[Spot::Counts],
        floor: 6,
        teach: &[
            "Try it: 6j dives six rows in just 2 keys.",
            "3j beats j j j. Works on w e b f { } too.",
        ],
        demo: Some(Demo {
            lines: &[
                "........",
                "........",
                "........",
                "........",
                "........",
                "........",
                "........",
                "........",
                "........",
                "........",
            ],
            start: (0, 0),
            goals: &[(6, 0)],
            press: "6 j",
        }),
    },
    Skill {
        lesson: 5,
        name: "Paragraphs",
        keys: "{ }",
        meaning: "blocks",
        labels: &[("{", "para back"), ("}", "para fwd")],
        new: K_PARA,
        spots: &[Spot::Para],
        floor: 5,
        teach: &[
            "Try it: |aaa bbb ccc, blank, ddd goes } onto the blank.",
            "} lands on the blank, j steps off. 2} leaps two blocks.",
        ],
        demo: Some(Demo {
            lines: &["aaa", "bbb", "ccc", "", "ddd"],
            start: (0, 0),
            goals: &[(3, 0), (4, 0)],
            press: "}, then j",
        }),
    },
    Skill {
        lesson: 5,
        name: "Power Fluency",
        keys: "power",
        meaning: "power",
        labels: &[],
        new: 0,
        spots: &[],
        floor: 12,
        teach: &[
            "Counts plus paragraphs over longer files.",
            "Crush par: teleport, don't walk.",
        ],
        demo: None,
    },
    Skill {
        lesson: 6,
        name: "Expedition I",
        keys: "all",
        meaning: "all",
        labels: &[],
        new: 0,
        spots: &[],
        floor: 14,
        teach: &[
            "Full kit, real terrain: words, finds and jumps mixed.",
            "Read the board like a puzzle. Fewest keys wins.",
        ],
        demo: None,
    },
    Skill {
        lesson: 6,
        name: "Expedition II",
        keys: "all",
        meaning: "all",
        labels: &[],
        new: 0,
        spots: &[],
        floor: 16,
        teach: &[
            "Deeper in: punctuation, indents and blank lines.",
            "Small vs BIG words matters here. Look first.",
        ],
        demo: None,
    },
    Skill {
        lesson: 6,
        name: "Expedition III",
        keys: "all",
        meaning: "all",
        labels: &[],
        new: 0,
        spots: &[],
        floor: 18,
        teach: &[
            "The final expedition. Everything you collected.",
            "Move like water. Good luck.",
        ],
        demo: None,
    },
];

/// Cumulative kit for a skill: everything collected up to and including it.
pub fn kit_for(skill_idx: usize) -> u32 {
    let mut kit = 0;
    for s in SKILLS.iter().take(skill_idx + 1) {
        kit |= s.new;
    }
    kit
}

/// First skill that teaches this motion's key, if any.
pub fn teacher_of(m: &Motion) -> Option<usize> {
    let bit = bit_for(m);
    SKILLS.iter().position(|s| s.new & bit != 0)
}

/// Skill that teaches counts (for multiplied-motion lock messages).
pub fn counts_teacher() -> Option<usize> {
    SKILLS.iter().position(|s| s.new & K_COUNTS != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_motions() -> Vec<Motion> {
        vec![
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
            Motion::Find(FindKind::Fwd, 'x'),
            Motion::Find(FindKind::Back, 'x'),
            Motion::Find(FindKind::TillFwd, 'x'),
            Motion::Find(FindKind::TillBack, 'x'),
            Motion::WordEndBackSmall,
            Motion::WordEndBackBig,
            Motion::MatchPair,
            Motion::ParaFwd,
            Motion::ParaBack,
        ]
    }

    #[test]
    fn every_motion_has_a_teacher() {
        for m in all_motions() {
            assert!(
                teacher_of(&m).is_some(),
                "no skill teaches {m:?}; add it to a skill's `new` bits"
            );
        }
        assert!(counts_teacher().is_some());
    }

    #[test]
    fn kits_only_grow_and_end_complete() {
        let mut prev = 0;
        for i in 0..SKILL_COUNT {
            let kit = kit_for(i);
            assert_eq!(kit & prev, prev, "kit shrank at skill {}", i + 1);
            prev = kit;
        }
        for m in all_motions() {
            assert!(
                kit_allows(kit_for(SKILL_COUNT - 1), &m),
                "final kit missing {m:?}"
            );
        }
    }

    #[test]
    fn skill_content_holds_together() {
        for (i, s) in SKILLS.iter().enumerate() {
            let n = i + 1;
            assert!((1..=6).contains(&s.lesson), "skill {n} bad lesson");
            assert!(!s.name.is_empty() && !s.keys.is_empty());
            assert!(!s.meaning.is_empty(), "skill {n} needs a shelf meaning");
            // Teach is exactly example + tip; definitions live in chips/cards.
            assert_eq!(s.teach.len(), 2, "skill {n} teach must be [example, tip]");
            if s.new == 0 {
                assert!(s.demo.is_none(), "fluency skill {n} needs no demo");
                assert!(s.labels.is_empty(), "fluency skill {n} needs no labels");
            } else {
                assert!(s.demo.is_some(), "teaching skill {n} needs a demo");
                assert!(!s.labels.is_empty(), "teaching skill {n} needs labels");
                for (k, m) in s.labels {
                    assert!(!k.is_empty() && !m.is_empty(), "skill {n} bad label");
                }
            }
        }
        assert_eq!(SKILLS.len(), SKILL_COUNT);
    }
}
