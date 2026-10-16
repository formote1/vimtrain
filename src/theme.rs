//! Tokyo-night-like theme. All UI colors flow through here so that
//! future themes (and a settings toggle) only touch this module.

use crossterm::style::Color;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb { r, g, b }
}

pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub dim: Color,
    pub faint: Color,
    pub green: Color,
    pub yellow: Color,
    pub red: Color,
    pub cyan: Color,
    pub magenta: Color,
    pub blue: Color,
    pub sel_bg: Color,
    pub cursor_fg: Color,
    pub cursor_bg: Color,
    pub goal_fg: Color,
    pub goal_bg: Color,
}

/// Tokyo Night inspired palette.
pub const T: Theme = Theme {
    bg: rgb(0x1a, 0x1b, 0x26),
    fg: rgb(0xc0, 0xca, 0xf5),
    dim: rgb(0x9a, 0xa1, 0xb8),
    faint: rgb(0x56, 0x5f, 0x89),
    green: rgb(0x9e, 0xce, 0x6a),
    yellow: rgb(0xe0, 0xaf, 0x68),
    red: rgb(0xf7, 0x76, 0x8e),
    cyan: rgb(0x7d, 0xcf, 0xff),
    magenta: rgb(0xbb, 0x9a, 0xf7),
    blue: rgb(0x7a, 0xa2, 0xf7),
    sel_bg: rgb(0x29, 0x2e, 0x42),
    cursor_fg: rgb(0x1a, 0x1b, 0x26),
    cursor_bg: rgb(0xc0, 0xca, 0xf5),
    goal_fg: rgb(0x1a, 0x1b, 0x26),
    goal_bg: rgb(0x9e, 0xce, 0x6a),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_is_dark_background() {
        // Sanity: bg must be dark, fg light, goal readable on bg.
        let dark = |c: Color| match c {
            Color::Rgb { r, g, b } => (r as u16 + g as u16 + b as u16) < 200,
            _ => false,
        };
        assert!(dark(T.bg));
        assert!(!dark(T.fg));
        assert!(!dark(T.goal_bg));
    }
}
