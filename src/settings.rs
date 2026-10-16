//! Settings screen data (v1): a read-only vim key reference.
//! Structured so later versions can add toggles (themes, counts, lessons).

pub struct KeyGroup {
    pub title: &'static str,
    pub keys: &'static [(&'static str, &'static str)],
}

pub const GROUPS: &[KeyGroup] = &[
    KeyGroup {
        title: "Cursor movement (lessons 1-5)",
        keys: &[
            ("h j k l", "left / down / up / right"),
            ("w b e", "next word / previous word / word end"),
            ("W B E", "same for BIG Words (punct included)"),
            ("f F t T", "to / back-to / till / back-till char"),
            ("; ,", "repeat last f/t, forward / backward"),
            ("0 ^ $", "line start / first word char / line end"),
            ("g_", "last non-blank char of line"),
            ("gg G", "first line / last line (5G = line 5)"),
            ("H M L", "top / middle / bottom of screen"),
            ("{ }", "previous / next paragraph"),
            ("3j 2w ...", "counts: repeat any motion"),
        ],
    },
    KeyGroup {
        title: "Game controls",
        keys: &[
            ("Enter", "start / next round"),
            ("Esc", "clear keys, close panels, back to menu"),
            ("? / F1", "help overlay"),
            ("F2", "this settings screen"),
            ("F3", "hint: optimal next motion"),
            (":q :next ...", "command bar (:help lists commands)"),
            ("Ctrl+C", "quit immediately"),
        ],
    },
    KeyGroup {
        title: "Coming later (editing drills)",
        keys: &[
            ("x s i a o", "delete char / substitute / insert modes"),
            ("d y c p u", "delete / yank / change / paste / undo"),
            ("iw aw i( ...", "text objects"),
            ("v V / n N", "visual mode / search"),
        ],
    },
];

pub const FOOTER: &str = "v1: read-only reference. Later: themes, difficulty, lesson toggles.";
