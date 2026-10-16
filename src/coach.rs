//! The coach: an ASCII guide that teaches each lesson and reacts to play.

use crate::engine::motion::Motion;

#[allow(dead_code)]
pub const MASCOT: [&str; 4] = [
    r#"  .-"-."#,
    r#" ( o_o )  <"#,
    r#"  | ^ |"#,
    r#"  `-^-'"#,
];

/// Lesson intro lines shown next to the mascot.
pub fn praise(streak: u32) -> &'static str {
    const LINES: [&str; 6] = [
        "Clean! On to the next.",
        "Sharp. Keep flowing.",
        "Nice one!",
        "Efficient. I saw that.",
        "Textbook motion!",
        "Flawless. The streak grows.",
    ];
    LINES[(streak as usize) % LINES.len()]
}

pub fn over_par(seg_keys: usize, seg_par: usize, hint: Option<&str>) -> String {
    match hint {
        Some(h) => format!("{seg_keys} keys for a {seg_par}-par hop. Try `{h}` next time."),
        None => format!("{seg_keys} keys for a {seg_par}-par hop. Look for a bigger jump."),
    }
}

pub fn round_done(stars: u8) -> &'static str {
    match stars {
        3 => "PERFECT PAR. You move like water.",
        2 => "Solid! Study the par line and shave a key.",
        _ => "Cleared! Replay the round and hunt for bigger motions.",
    }
}

/// One-line "why" for an optimal motion: teaches the principle behind it.
pub fn why_for(m: &Motion, count: usize) -> &'static str {
    if count > 1 && !matches!(m, Motion::GotoLine) {
        return "Counts multiply: `3j` does in 2 keys what `j j j` does in 3.";
    }
    match m {
        Motion::WordFwdSmall
        | Motion::WordFwdBig
        | Motion::WordBackSmall
        | Motion::WordBackBig
        | Motion::WordEndSmall
        | Motion::WordEndBig
        | Motion::WordEndBackSmall
        | Motion::WordEndBackBig => "Word jumps cover ground; tapping h/l walks cell by cell.",
        Motion::MatchPair => "% jumps directly between matching brackets (), [], {}.",
        Motion::Find(..) => "f/F/t/T jumps straight to the char; no walking needed.",
        Motion::GotoLine | Motion::FileTop | Motion::FileBottom => {
            "G/gg teleports between lines: one hop, any distance."
        }
        Motion::LineStart | Motion::FirstNonBlank | Motion::LineEnd | Motion::LastNonBlank => {
            "0 ^ $ own the whole line in a single key."
        }
        Motion::ScreenTop | Motion::ScreenMid | Motion::ScreenBot => {
            "H/M/L lands on screen rows instantly."
        }
        Motion::ParaFwd | Motion::ParaBack => "{ and } hop whole paragraphs at once.",
        Motion::Left | Motion::Down | Motion::Up | Motion::Right => {
            "Short walk here: hjkl is the right tool."
        }
    }
}
