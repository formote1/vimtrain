//! All terminal rendering: tokyo-night theme, everything centered.
//!
//! Visual language: the motions being learned are boxed and centered
//! (chips for keys, cards for skill groups); app chrome lives only in
//! the bottom footer, dim and out of focus.

use std::io::{self, Write};

use crossterm::{
    cursor::MoveTo,
    queue,
    style::{
        Attribute, Color, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
    },
    terminal::{self, Clear, ClearType},
};

use crate::app::{App, Screen};
use crate::game::curriculum::{SKILL_COUNT, SKILLS, WINS_PER_SKILL};
use crate::game::lesson::LESSONS;
use crate::settings;
use crate::theme::T;

#[derive(Clone)]
struct Cell {
    ch: char,
    fg: Color,
    bg: Option<Color>,
    bold: bool,
}

type Row = Vec<Cell>;

fn cell(ch: char, fg: Color) -> Cell {
    Cell {
        ch,
        fg,
        bg: None,
        bold: false,
    }
}

fn text_row(s: &str, fg: Color, bold: bool) -> Row {
    s.chars()
        .map(|ch| {
            let mut c = cell(ch, fg);
            c.bold = bold;
            c
        })
        .collect()
}

fn mixed(parts: &[(&str, Color, bool)]) -> Row {
    let mut row = Row::new();
    for (s, fg, bold) in parts {
        for ch in s.chars() {
            row.push(Cell {
                ch,
                fg: *fg,
                bg: None,
                bold: *bold,
            });
        }
    }
    row
}

fn blank() -> Row {
    Row::new()
}

fn gap() -> Vec<Row> {
    Vec::new()
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(10);
    let mut lines = Vec::new();
    for para in text.split('\n') {
        if para.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut cur = String::new();
        let mut cur_len = 0;
        for word in para.split(' ') {
            let wlen = word.chars().count();
            if cur.is_empty() {
                cur.push_str(word);
                cur_len = wlen;
            } else if cur_len + 1 + wlen <= width {
                cur.push(' ');
                cur.push_str(word);
                cur_len += 1 + wlen;
            } else {
                lines.push(cur);
                cur = word.to_string();
                cur_len = wlen;
            }
        }
        lines.push(cur);
    }
    lines
}

/// Tracks the terminal's current SGR state so paint_row only emits an
/// escape sequence when a cell actually changes it. Naively emitting
/// fg+bg (truecolor: ~19 bytes each) plus resets for *every* cell made a
/// full repaint ~20-30 KB of escape codes; the game repaints on every
/// keystroke, and `f`/`t` find motions take two keystrokes, so lesson 3
/// paid that cost twice per move. This keeps the same pixels but slashes
/// the bytes.
#[derive(Default)]
struct Style {
    fg: Option<Color>,
    bg: Option<Color>,
    bold: bool,
}

/// Draw one centered row at terminal row `y`, with the cursor left wherever
/// Draw row starting at exact coordinate (x, y)
fn paint_row_at(
    out: &mut impl Write,
    row: &Row,
    x: usize,
    y: u16,
    tw: usize,
    st: &mut Style,
) -> io::Result<()> {
    if row.is_empty() || x >= tw {
        return Ok(());
    }
    queue!(out, MoveTo(x as u16, y))?;
    for (cx, c) in (x..).zip(row.iter()) {
        if cx >= tw {
            break;
        }
        let bg = c.bg.unwrap_or(T.bg);
        if st.fg != Some(c.fg) {
            queue!(out, SetForegroundColor(c.fg))?;
            st.fg = Some(c.fg);
        }
        if st.bg != Some(bg) {
            queue!(out, SetBackgroundColor(bg))?;
            st.bg = Some(bg);
        }
        if st.bold != c.bold {
            queue!(
                out,
                SetAttribute(if c.bold {
                    Attribute::Bold
                } else {
                    Attribute::NormalIntensity
                })
            )?;
            st.bold = c.bold;
        }
        write!(out, "{}", c.ch)?;
    }
    Ok(())
}

/// Draw one centered row at terminal row `y`.
fn paint_row(
    out: &mut impl Write,
    row: &Row,
    y: u16,
    tw: usize,
    st: &mut Style,
) -> io::Result<()> {
    if row.is_empty() {
        return Ok(());
    }
    let x0 = tw.saturating_sub(row.len()) / 2;
    paint_row_at(out, row, x0, y, tw, st)
}

/// Dedicated top navigation bar at row 1: brand logo top-left, nav links top-right (smassh design).
fn blit_top_bar(out: &mut impl Write, app: &App, tw: usize, st: &mut Style) -> io::Result<()> {
    let logo = mixed(&[("VIMTRAIN", T.green, true)]);
    paint_row_at(out, &logo, 2, 1, tw, st)?;

    // Mutually exclusive: exactly ONE nav button can be selected at any time.
    let (is_home, is_settings, is_help) = if app.help_open {
        (false, false, true)
    } else if app.screen == Screen::Settings {
        (false, true, false)
    } else {
        (true, false, false)
    };

    let nav = mixed(&[
        (if is_home { "[● home]" } else { "  home  " }, if is_home { T.fg } else { T.dim }, is_home),
        ("   ", T.faint, false),
        (if is_settings { "[● settings]" } else { "  settings  " }, if is_settings { T.cyan } else { T.dim }, is_settings),
        ("   ", T.faint, false),
        (if is_help { "[● help]" } else { "  help  " }, if is_help { T.yellow } else { T.dim }, is_help),
    ]);

    let rx = tw.saturating_sub(nav.len() + 2);
    if rx > logo.len() + 4 {
        paint_row_at(out, &nav, rx, 1, tw, st)?;
    }
    Ok(())
}

/// Centered mode & settings pill bar at row 2: difficulty modes + chapter badge (smassh knockoff).
fn blit_mode_bar(out: &mut impl Write, app: &App, tw: usize, st: &mut Style) -> io::Result<()> {
    let casual_sel = app.difficulty == crate::game::progress::Difficulty::Casual;
    let normal_sel = app.difficulty == crate::game::progress::Difficulty::Normal;
    let strict_sel = app.difficulty == crate::game::progress::Difficulty::Strict;

    let ch = &LESSONS[app.chapter_idx()];
    let sk = app.skill();

    let pills = mixed(&[
        (if casual_sel { "[● Casual]" } else { "  Casual  " }, if casual_sel { T.cyan } else { T.faint }, casual_sel),
        (" ", T.faint, false),
        (if normal_sel { "[● Normal]" } else { "  Normal  " }, if normal_sel { T.green } else { T.faint }, normal_sel),
        (" ", T.faint, false),
        (if strict_sel { "[● Strict]" } else { "  Strict  " }, if strict_sel { T.red } else { T.faint }, strict_sel),
        ("   │   ", T.faint, false),
        (&format!("Ch.{} {}", ch.id, ch.name), T.yellow, false),
        (" · ", T.faint, false),
        (sk.name, T.dim, false),
        ("   │   ", T.faint, false),
        ("Tab: diff", T.dim, false),
    ]);

    paint_row(out, &pills, 2, tw, st)?;
    Ok(())
}

/// Paint background, top bar at row 1, centered settings pill bar at row 2,
/// then draw body rows cleanly without overflowing the terminal.
fn blit(out: &mut impl Write, app: &App, rows: &[Row]) -> io::Result<()> {
    let (tw, th) = terminal::size().unwrap_or((80, 24));
    queue!(out, SetAttribute(Attribute::Reset), SetBackgroundColor(T.bg), Clear(ClearType::All))?;
    let mut st = Style {
        bg: Some(T.bg),
        ..Style::default()
    };

    // Pinned top navigation bar at row 1
    blit_top_bar(out, app, tw as usize, &mut st)?;

    // Centered mode & settings pill bar at row 2
    blit_mode_bar(out, app, tw as usize, &mut st)?;

    // Body content starting at row 4 (or centered between row 3 and footer)
    let h = rows.len();
    let avail = (th as usize).saturating_sub(5);
    let oy = if avail > h {
        4 + (avail - h) / 3
    } else {
        4
    };
    for (i, row) in rows.iter().enumerate() {
        let y = oy + i;
        if y >= (th as usize).saturating_sub(1) {
            break;
        }
        paint_row(out, row, y as u16, tw as usize, &mut st)?;
    }
    queue!(out, ResetColor, SetAttribute(Attribute::Reset))?;
    out.flush()?;
    Ok(())
}

/// Footer pinned to the last terminal row (hidden while the cmd bar is open).
fn blit_footer(out: &mut impl Write, row: &Row) -> io::Result<()> {
    let (tw, th) = terminal::size().unwrap_or((80, 24));
    if th == 0 {
        return Ok(());
    }
    let y = th.saturating_sub(1);
    let mut st = Style::default();
    queue!(out, SetAttribute(Attribute::Reset))?;
    paint_row(out, row, y, tw as usize, &mut st)?;
    queue!(out, ResetColor, SetAttribute(Attribute::Reset))?;
    out.flush()?;
    Ok(())
}

pub fn render(out: &mut impl Write, app: &App) -> io::Result<()> {
    let (tw, th) = terminal::size().unwrap_or((80, 24));
    if tw < 60 || th < 24 {
        queue!(out, SetAttribute(Attribute::Reset), SetBackgroundColor(T.bg), Clear(ClearType::All))?;
        let mut st = Style::default();
        paint_row(
            out,
            &text_row(
                &format!("Terminal too small ({tw}x{th}, need 60x24)."),
                T.red,
                true,
            ),
            th / 2,
            tw as usize,
            &mut st,
        )?;
        out.flush()?;
        return Ok(());
    }
    let rows: Vec<Row> = match app.screen {
        Screen::Menu => render_menu(app),
        Screen::Intro => render_intro(app),
        Screen::Playing => render_playing(app),
        Screen::RoundDone => render_done(app),
        Screen::Settings => render_settings(app),
    };
    if app.help_open {
        blit(out, app, &render_help(app))?;
    } else {
        blit(out, app, &rows)?;
    }
    if app.cmd_open {
        render_cmdbar(out, app)?;
    } else {
        blit_footer(out, &footer(app))?;
    }
    out.flush()?;
    Ok(())
}

// ---------------------------------------------------------------- shared ---

/// Boxed key chips with the meaning centered underneath each box:
/// key on top, function below. Clean design: no bottom border.
fn labeled_chips(items: &[(&str, Color, &str)]) -> Vec<Row> {
    let mut top = Row::new();
    let mut mid = Row::new();
    let mut sub = Row::new();
    for (i, (k, col, meaning)) in items.iter().enumerate() {
        if i > 0 {
            for r in [&mut top, &mut mid, &mut sub] {
                for _ in 0..4 {
                    r.push(cell(' ', T.fg));
                }
            }
        }
        let colw = k.chars().count().max(meaning.chars().count()) + 2;
        top.push(cell('┌', T.faint));
        for _ in 0..colw {
            top.push(cell('─', T.faint));
        }
        top.push(cell('┐', T.faint));
        mid.push(cell('│', T.faint));
        let kl = k.chars().count();
        for _ in 0..(colw - kl) / 2 {
            mid.push(cell(' ', T.fg));
        }
        for ch in k.chars() {
            mid.push(Cell {
                ch,
                fg: *col,
                bg: None,
                bold: true,
            });
        }
        while mid.len() < top.len() - 1 {
            mid.push(cell(' ', T.fg));
        }
        mid.push(cell('│', T.faint));
        let total = colw + 2;
        let ml = meaning.chars().count();
        for _ in 0..(total - ml) / 2 {
            sub.push(cell(' ', T.fg));
        }
        for ch in meaning.chars() {
            sub.push(cell(ch, T.dim));
        }
        while sub.len() < top.len() {
            sub.push(cell(' ', T.fg));
        }
    }
    // Do not trim trailing spaces on the meaning row; they keep the meaning centered
    // and the row width aligned with the box above.
    vec![top, mid, sub]
}

/// Chip color: hjkl keep their signature colors, everything else yellow.
fn chip_color(key: &str) -> Color {
    match key {
        "h" => T.cyan,
        "j" => T.green,
        "k" => T.yellow,
        "l" => T.magenta,
        _ => T.yellow,
    }
}

/// Big spaced hjkl legend: colored keys with meanings underneath.
fn hjkl_hero() -> Vec<Row> {
    labeled_chips(&[
        ("h", T.cyan, "left"),
        ("j", T.green, "down"),
        ("k", T.yellow, "up"),
        ("l", T.magenta, "right"),
    ])
}

/// Labeled chips for a skill's keys (empty when the skill teaches none).
fn skill_chips(sk: &crate::game::curriculum::Skill) -> Vec<Row> {
    if sk.labels.is_empty() {
        return Vec::new();
    }
    let items: Vec<(&str, Color, &str)> = sk
        .labels
        .iter()
        .map(|(k, m)| (*k, chip_color(k), *m))
        .collect();
    labeled_chips(&items)
}

/// Bordered card: centered colored header, separator, wrapped body.
/// Width includes borders; every row is exactly `width` cells.
fn card(header: &str, hfg: Color, body: &[String], width: usize) -> Vec<Row> {
    let w = width.max(12);
    let inner = w - 2;
    let rule = |l: char, fill: char, r: char| -> Row {
        let mut row = vec![cell(l, T.faint)];
        for _ in 0..inner {
            row.push(cell(fill, T.faint));
        }
        row.push(cell(r, T.faint));
        row
    };
    let mut rows = vec![rule('┌', '─', '┐')];
    let h: String = header.chars().take(inner).collect();
    let pad = inner.saturating_sub(h.chars().count());
    let left = pad / 2;
    let mut hr = vec![cell('│', T.faint)];
    for _ in 0..left {
        hr.push(cell(' ', hfg));
    }
    for ch in h.chars() {
        hr.push(Cell {
            ch,
            fg: hfg,
            bg: None,
            bold: true,
        });
    }
    while hr.len() < w - 1 {
        hr.push(cell(' ', hfg));
    }
    hr.push(cell('│', T.faint));
    rows.push(hr);
    for line in body {
        for wl in wrap(line, inner.saturating_sub(2).max(8)) {
            let mut r = vec![cell('│', T.faint), cell(' ', T.fg)];
            for ch in wl.chars().take(inner.saturating_sub(2)) {
                r.push(cell(ch, T.fg));
            }
            while r.len() < w - 1 {
                r.push(cell(' ', T.fg));
            }
            r.push(cell('│', T.faint));
            rows.push(r);
        }
    }
    rows
}

/// Lay cards out in a grid; incomplete rows are padded with blank panels.
fn grid(cards: &[Vec<Row>], cols: usize, gap: usize) -> Vec<Row> {
    let mut rows = Vec::new();
    if cards.is_empty() {
        return rows;
    }
    let height = cards.iter().map(Vec::len).max().unwrap_or(0);
    let width = cards
        .first()
        .and_then(|c| c.first())
        .map(Row::len)
        .unwrap_or(0);
    for chunk in cards.chunks(cols.max(1)) {
        for i in 0..height {
            let mut row = Row::new();
            for (k, cd) in chunk.iter().enumerate() {
                if k > 0 {
                    for _ in 0..gap {
                        row.push(cell(' ', T.fg));
                    }
                }
                match cd.get(i) {
                    Some(r) => row.extend(r.iter().cloned()),
                    None => {
                        row.push(cell('│', T.faint));
                        for _ in 0..width.saturating_sub(2) {
                            row.push(cell(' ', T.fg));
                        }
                        row.push(cell('│', T.faint));
                    }
                }
            }
            rows.push(row);
        }
        rows.push(blank());
    }
    rows.pop();
    rows
}

/// Compact legend for the playing footer.
fn hjkl_mini() -> Row {
    let mut row = Row::new();
    for (ch, col) in [
        ('h', T.cyan),
        ('j', T.green),
        ('k', T.yellow),
        ('l', T.magenta),
    ] {
        row.push(Cell {
            ch,
            fg: col,
            bg: None,
            bold: true,
        });
        row.push(cell(' ', T.fg));
    }
    for c in "move".chars() {
        row.push(cell(c, T.dim));
    }
    row
}

fn footer(app: &App) -> Row {
    if app.help_open {
        return text_row("Esc / ? closes  ·  Tab diff  ·  1-3 direct  ·  j/k scroll", T.faint, false);
    }
    match app.screen {
        Screen::Menu => {
            text_row("↑/↓ select  ·  1-6 chapters  ·  Enter start  ·  Tab diff  ·  s settings  ·  ? help  ·  q quit", T.faint, false)
        }
        Screen::Intro => text_row("Space start  ·  Tab diff  ·  ? help  ·  Esc menu", T.faint, false),
        Screen::Playing => {
            let mut row = hjkl_mini();
            for c in "  ·  Tab diff  ·  : cmd  ·  ? help  ·  Esc menu".chars() {
                row.push(cell(c, T.faint));
            }
            row
        }
        Screen::RoundDone => {
            if app.round_failed {
                text_row(
                    "Enter / r retry  ·  Tab diff  ·  ? help  ·  Esc menu  ·  q quit",
                    T.faint,
                    false,
                )
            } else {
                text_row(
                    "Enter next  ·  r replay  ·  Tab diff  ·  ? help  ·  Esc menu  ·  q quit",
                    T.faint,
                    false,
                )
            }
        }
        Screen::Settings => {
            text_row("Esc back  ·  Tab diff  ·  ? help  ·  q quit", T.faint, false)
        }
    }
}

// ---------------------------------------------------------------- menu ---

/// (done skills, total skills, resume skill idx, chapter unlocked)
fn chapter_stats(app: &App, ch: usize) -> (usize, usize, usize, bool) {
    let id = (ch + 1) as u8;
    let idxs: Vec<usize> = SKILLS
        .iter()
        .enumerate()
        .filter(|(_, s)| s.lesson == id)
        .map(|(i, _)| i)
        .collect();
    let done = idxs.iter().filter(|i| app.mastered(**i)).count();
    let resume = idxs
        .iter()
        .find(|i| !app.mastered(**i))
        .copied()
        .unwrap_or_else(|| idxs.first().copied().unwrap_or(0));
    let unlocked = idxs.first().map(|i| app.unlocked_skill(*i)).unwrap_or(false);
    (done, idxs.len(), resume, unlocked)
}

fn render_menu(app: &App) -> Vec<Row> {
    let mut rows = vec![
        mixed(&[
            ("SELECT CHAPTER", T.yellow, true),
            ("  ·  learn vim motions", T.faint, false),
        ]),
        text_row("collect vim keys one skill at a time (vim.rtorr.com)", T.faint, false),
        blank(),
    ];
    for (i, l) in LESSONS.iter().enumerate() {
        let sel = app.menu_sel == i;
        let marker = if sel { "▸ " } else { "  " };
        let (done, total, resume, unlocked) = chapter_stats(app, i);
        let mut row = Row::new();

        // 1. Selection indicator (2 cols)
        for ch in marker.chars() {
            row.push(Cell {
                ch,
                fg: if unlocked { T.green } else { T.faint },
                bg: if sel { Some(T.sel_bg) } else { None },
                bold: sel,
            });
        }

        // 2. Chapter number badge (4 cols) - perfectly left-aligned
        let badge = format!("[{}] ", l.id);
        for ch in badge.chars() {
            row.push(Cell {
                ch,
                fg: if unlocked { T.cyan } else { T.faint },
                bg: if sel { Some(T.sel_bg) } else { None },
                bold: sel,
            });
        }

        // 3. Chapter Name (16 cols)
        let name_col = format!("{:<16}", l.name);
        for ch in name_col.chars() {
            row.push(Cell {
                ch,
                fg: if sel || unlocked { T.fg } else { T.faint },
                bg: if sel { Some(T.sel_bg) } else { None },
                bold: sel,
            });
        }

        // 4. Middle keys (32 cols) - centered in the middle part!
        let keys_str = l.keys;
        let k_len = keys_str.chars().count();
        let mid_width = 32usize;
        let (pad_left, pad_right, take_len) = if k_len <= mid_width {
            let pad = mid_width - k_len;
            (pad / 2, pad - pad / 2, k_len)
        } else {
            (0, 0, mid_width)
        };

        for _ in 0..pad_left {
            row.push(Cell {
                ch: ' ',
                fg: T.dim,
                bg: if sel { Some(T.sel_bg) } else { None },
                bold: false,
            });
        }
        for ch in keys_str.chars().take(take_len) {
            row.push(Cell {
                ch,
                fg: if unlocked { T.dim } else { T.faint },
                bg: if sel { Some(T.sel_bg) } else { None },
                bold: false,
            });
        }
        for _ in 0..pad_right {
            row.push(Cell {
                ch: ' ',
                fg: T.dim,
                bg: if sel { Some(T.sel_bg) } else { None },
                bold: false,
            });
        }

        // 5. Right status / skill name (22 cols) - ends at the exact same right endline
        let right_width = 22usize;
        let start_len = row.len(); // exactly 2 + 4 + 16 + 32 = 54
        if unlocked {
            let sk = &SKILLS[resume];
            let stat_str = format!("[{done}/{total}]");
            for ch in stat_str.chars() {
                row.push(Cell {
                    ch,
                    fg: if done == total { T.green } else { T.yellow },
                    bg: if sel { Some(T.sel_bg) } else { None },
                    bold: sel,
                });
            }
            let next_str = format!(" · {}", sk.name);
            let rem = right_width.saturating_sub(stat_str.chars().count());
            for ch in next_str.chars().take(rem) {
                row.push(Cell {
                    ch,
                    fg: if sel { T.yellow } else { T.dim },
                    bg: if sel { Some(T.sel_bg) } else { None },
                    bold: sel,
                });
            }
        } else {
            let lock_str = "[ locked ]";
            for ch in lock_str.chars().take(right_width) {
                row.push(Cell {
                    ch,
                    fg: T.faint,
                    bg: if sel { Some(T.sel_bg) } else { None },
                    bold: false,
                });
            }
        }

        // Uniform container width of 76 cells so the div centers cleanly
        while row.len() < start_len + right_width {
            row.push(Cell {
                ch: ' ',
                fg: T.fg,
                bg: if sel { Some(T.sel_bg) } else { None },
                bold: false,
            });
        }

        rows.push(row);
    }
    if !app.menu_msg.is_empty() {
        rows.push(blank());
        rows.push(text_row(&app.menu_msg, T.red, false));
    }
    rows
}

// ---------------------------------------------------------------- intro ---

fn render_intro(app: &App) -> Vec<Row> {
    let mut rows = Vec::new();
    let sk = app.skill();
    let ch = &LESSONS[app.chapter_idx()];
    let w = app.wins[app.skill_idx];

    rows.push(mixed(&[
        (&format!("Skill {}/{}  ·  ", app.skill_idx + 1, SKILL_COUNT), T.green, true),
        (sk.name, T.yellow, true),
        (&format!("   [ Ch.{} {} ]", ch.id, ch.name), T.dim, false),
    ]));
    rows.push(blank());

    if !sk.labels.is_empty() {
        for r in skill_chips(sk) {
            rows.push(r);
        }
    } else if sk.new == 0 {
        rows.push(text_row("Fluency practice: mix all collected keys.", T.cyan, true));
    }

    if let Some(tip) = sk.teach.iter().find(|t| !t.starts_with("Try it:")) {
        rows.push(blank());
        rows.push(text_row(tip, T.fg, false));
    }

    rows.push(blank());
    rows.push(mixed(&[
        ("Press ", T.faint, false),
        ("[ Space ]", T.green, true),
        (" to start", T.faint, false),
    ]));

    // Middle bottom mastery status
    rows.push(blank());
    rows.push(blank());
    rows.push(mixed(&[
        (&format!("Mastery: {}/{WINS_PER_SKILL} wins", w.min(WINS_PER_SKILL)), T.blue, false),
    ]));
    rows
}

// ---------------------------------------------------------------- playing ---

fn render_playing(app: &App) -> Vec<Row> {
    let mut rows = Vec::new();
    let pending = app.keys.pending_text();
    let ngoals = app.goals.len().max(1);
    let moves_str = if let Some(max) = app.max_moves() {
        format!("moves {}/{}", app.keys_pressed, max)
    } else {
        format!("moves {}", app.keys_pressed)
    };
    let moves_color = if let Some(max) = app.max_moves() {
        if app.keys_pressed >= max {
            T.red
        } else if app.keys_pressed >= app.par {
            T.yellow
        } else {
            T.fg
        }
    } else {
        T.fg
    };
    let goal_str = format!("goal {}/{}", (app.goal_idx + 1).min(ngoals), app.goals.len());
    let par_str = format!("par {}", app.par);
    let time_str = format!("{}s", app.elapsed_secs());
    let diff_str = app.difficulty.name();

    let mut stat_parts: Vec<(String, Color, bool)> = vec![
        (moves_str, moves_color, true),
        (" · ".to_string(), T.faint, false),
        (goal_str, T.cyan, false),
        (" · ".to_string(), T.faint, false),
        (par_str, T.dim, false),
        (" · ".to_string(), T.faint, false),
        (time_str, T.dim, false),
        (" · ".to_string(), T.faint, false),
        (diff_str.to_string(), T.faint, false),
    ];
    let pending_buf;
    if !pending.is_empty() {
        pending_buf = format!("  [ {pending} ]");
        stat_parts.push((pending_buf, T.magenta, true));
    }
    if app.demo {
        stat_parts.push(("  [ demo ]".to_string(), T.cyan, true));
    }
    let stat_refs: Vec<(&str, Color, bool)> = stat_parts
        .iter()
        .map(|(s, c, b)| (s.as_str(), *c, *b))
        .collect();
    rows.push(mixed(&stat_refs));
    rows.push(blank());

    // Board rows: gutter + content
    let mut board: Vec<Row> = Vec::new();
    for r in 0..app.buf.row_count() {
        let mut row: Row = Vec::new();
        let cur_row = app.cursor.row == r;
        let gutter = format!("{:>3} ", r + 1);
        let gfg = if cur_row { T.yellow } else { T.faint };
        for ch in gutter.chars() {
            let mut c = cell(ch, gfg);
            c.bold = cur_row;
            row.push(c);
        }
        let len = app.buf.line_len(r);
        for c in 0..len {
            let ch = app.buf.char_at(r, c).unwrap_or(' ');
            let is_cursor = app.cursor.row == r && app.cursor.col == c;
            let goal_pos = app.goals.iter().position(|g| g.row == r && g.col == c);
            let (fg, bg, bold) = if is_cursor {
                (T.cursor_fg, Some(T.cursor_bg), true)
            } else if let Some(gi) = goal_pos {
                if gi < app.goal_idx {
                    (T.faint, None, false)
                } else if gi == app.goal_idx {
                    (T.goal_fg, Some(T.goal_bg), true)
                } else {
                    (T.green, None, false)
                }
            } else {
                (T.fg, None, false)
            };
            row.push(Cell {
                ch,
                fg,
                bg,
                bold,
            });
        }
        board.push(row);
    }

    // Natural board width from actual content:
    let max_len = board.iter().map(Row::len).max().unwrap_or(48);
    let board_w = max_len;

    // Pad every row to board_w so words align from the left across all lines
    for row in &mut board {
        while row.len() < board_w {
            row.push(cell(' ', T.fg));
        }
    }

    rows.extend(shelf_rows(app, board_w));
    rows.push(blank());
    rows.extend(board);
    rows.push(blank());

    // Status wrapped to the board width so the block stays aligned.
    let status = if app.status.is_empty() {
        "Reach the green goal.".to_string()
    } else {
        app.status.clone()
    };
    let sfg = if app.status_err { T.red } else { T.yellow };
    let status_w = board_w.max(50);
    for wl in wrap(&status, status_w).into_iter().take(2) {
        rows.push(text_row(&wl, sfg, false));
    }
    rows
}

/// Collected-keys shelf for the playing HUD: chapter keys with their
/// one-word meanings. Active skill high-contrast highlighted, unlocked clear, future dimmed.
fn shelf_rows(app: &App, width: usize) -> Vec<Row> {
    let id = app.skill().lesson;
    let entries: Vec<(String, Color, Option<Color>, bool)> = SKILLS
        .iter()
        .enumerate()
        .filter(|(_, s)| s.lesson == id)
        .map(|(i, s)| {
            if i == app.skill_idx {
                let txt = format!("[● {}:{} {}]", i + 1, s.keys, s.meaning);
                (txt, T.bg, Some(T.yellow), true)
            } else if i < app.skill_idx {
                let txt = format!("[ {}:{} {} ]", i + 1, s.keys, s.meaning);
                (txt, T.cyan, None, false)
            } else {
                let txt = format!("[ {}:{} ]", i + 1, s.keys);
                (txt, T.faint, None, false)
            }
        })
        .collect();
    let width = width.max(20);
    let mut rows = Vec::new();
    let mut cur = Row::new();
    for (txt, fg, bg, bold) in entries {
        let cells: Vec<Cell> = txt
            .chars()
            .map(|ch| Cell {
                ch,
                fg,
                bg,
                bold,
            })
            .collect();
        if !cur.is_empty() && cur.len() + 2 + cells.len() > width {
            rows.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(cell(' ', T.fg));
            cur.push(cell(' ', T.fg));
        }
        cur.extend(cells);
    }
    if !cur.is_empty() {
        rows.push(cur);
    }
    rows
}

// ---------------------------------------------------------------- done ---

/// Renders 3 large multi-line ASCII/block stars side-by-side with earned stars in bright gold.
fn big_stars_rows(earned: u8) -> Vec<Row> {
    let earned = earned.clamp(1, 3) as usize;
    let patterns = [
        ("    ★    ", "    ☆    "),
        ("  ▄███▄  ", "  ▄▀ ▀▄  "),
        ("█████████", "█▀     ▀█"),
        (" ▀██ ██▀ ", " ▀█   █▀ "),
    ];

    let mut rows = Vec::new();
    for row_pat in &patterns {
        let mut r = Row::new();
        for i in 0..3 {
            if i > 0 {
                for _ in 0..5 {
                    r.push(cell(' ', T.fg));
                }
            }
            let is_earned = i < earned;
            let (part, fg, bold) = if is_earned {
                (row_pat.0, T.yellow, true)
            } else {
                (row_pat.1, T.faint, false)
            };
            for ch in part.chars() {
                r.push(Cell {
                    ch,
                    fg,
                    bg: None,
                    bold,
                });
            }
        }
        rows.push(r);
    }
    rows
}

fn render_done(app: &App) -> Vec<Row> {
    let mut rows = Vec::new();
    let sk = app.skill();
    if app.round_failed {
        rows.push(mixed(&[
            ("[ RETRY ]", T.red, true),
            ("  ·  ", T.faint, false),
            (sk.name, T.fg, true),
        ]));
        rows.push(blank());
        let limit = app.max_moves().unwrap_or(app.keys_pressed);
        rows.push(text_row(
            &format!("Moves exceeded: {} / {} allowed  (par: {})", app.keys_pressed, limit, app.par),
            T.yellow,
            false,
        ));
        rows.push(blank());
        rows.push(mixed(&[
            ("Press ", T.faint, false),
            ("[ Enter / r ]", T.green, true),
            (" to retry level  ·  ", T.faint, false),
            ("[ Tab ]", T.cyan, true),
            (" change difficulty", T.faint, false),
        ]));
    } else {
        rows.push(mixed(&[
            ("[ CLEARED ]", T.green, true),
            ("  ·  ", T.faint, false),
            (sk.name, T.yellow, true),
        ]));
        rows.push(blank());
        rows.extend(big_stars_rows(app.last_stars));
        rows.push(blank());
        let rating = match app.last_stars {
            3 => "PERFECT PAR · ★★★",
            2 => "SOLID RUN · ★★☆",
            _ => "CLEARED · ★☆☆",
        };
        rows.push(text_row(rating, if app.last_stars == 3 { T.green } else { T.yellow }, true));
        rows.push(blank());
        rows.push(text_row(
            &format!(
                "moves {} / par {}  ·  {}s  ·  +{} pts  ·  total {}",
                app.keys_pressed, app.par, app.last_secs, app.last_points, app.total_score
            ),
            T.fg,
            false,
        ));
        rows.push(blank());
        rows.push(mixed(&[
            ("Press ", T.faint, false),
            ("[ Enter / Space ]", T.green, true),
            (" continue  ·  ", T.faint, false),
            ("[ r ]", T.cyan, true),
            (" replay", T.faint, false),
        ]));
    }
    if !app.status.is_empty() {
        rows.push(blank());
        for wl in wrap(&app.status, 58).into_iter().take(2) {
            rows.push(text_row(&wl, if app.round_failed { T.red } else { T.dim }, false));
        }
    }
    rows
}

// ---------------------------------------------------------------- settings ---

fn render_settings(_app: &App) -> Vec<Row> {
    let mut content: Vec<Row> = vec![
        mixed(&[
            ("SETTINGS", T.cyan, true),
            ("  ·  game controls", T.faint, false),
        ]),
        blank(),
    ];

    let controls = &[
        ("Enter", "start / next round"),
        ("Esc", "clear keys, close panels, back to menu"),
        ("Tab", "cycle difficulty (Casual / Normal / Strict)"),
        ("? / F1", "help overlay (collected keys & reference)"),
        ("F2", "settings (game controls)"),
        ("F3", "hint: optimal next motion"),
        (":q :next ...", "command bar (:help lists commands)"),
        ("Ctrl+C", "quit immediately"),
    ];

    let body: Vec<String> = controls.iter().map(|(k, d)| format!("{k:<14} {d}")).collect();
    content.extend(card("Game controls", T.yellow, &body, 62));
    content.extend(gap());
    content.push(text_row("Esc back  ·  Tab diff  ·  ? help  ·  q quit", T.faint, false));
    content
}

// ---------------------------------------------------------------- help/cmd ---

/// Cards for this chapter's skills: bright collected keys, dim locked ones.
fn chapter_cards(app: &App) -> Vec<Vec<Row>> {
    let id = app.skill().lesson;
    SKILLS
        .iter()
        .enumerate()
        .filter(|(_, s)| s.lesson == id)
        .map(|(i, s)| {
            if i <= app.skill_idx {
                // Card body mirrors the chips: one key-meaning line each.
                // Fluency mixes (no labels) show their advice lines instead.
                let body: Vec<String> = if s.labels.is_empty() {
                    s.teach.iter().map(|l| l.to_string()).collect()
                } else {
                    s.labels.iter().map(|(k, m)| format!("{k} - {m}")).collect()
                };
                card(&format!("skill {}: {}", i + 1, s.keys), T.yellow, &body, 34)
            } else {
                card(
                    &format!("skill {}: ???", i + 1),
                    T.faint,
                    &["master previous skills".to_string()],
                    34,
                )
            }
        })
        .collect()
}

fn render_help(app: &App) -> Vec<Row> {
    let mut content: Vec<Row> = Vec::new();

    // 1. Header
    content.push(mixed(&[
        ("HELP", T.yellow, true),
        ("  ·  collected keys & vim reference", T.faint, false),
    ]));
    content.push(blank());

    // 2. Collected keys for current chapter
    let ch = &LESSONS[app.chapter_idx()];
    let sk = app.skill();
    content.push(mixed(&[
        ("COLLECTED KEYS", T.green, true),
        (&format!("  ·  Ch.{} {} (Skill {}/{}: {})", ch.id, ch.name, app.skill_idx + 1, SKILL_COUNT, sk.name), T.faint, false),
    ]));
    content.extend(gap());
    content.extend(hjkl_hero());
    content.extend(gap());

    let (tw, th) = terminal::size().unwrap_or((80, 24));
    let cols = if tw >= 78 { 2 } else { 1 };
    content.extend(grid(&chapter_cards(app), cols, 4));
    content.extend(gap());

    // 3. Full Key Reference (excluding game controls which are in settings)
    content.push(text_row("KEY REFERENCE (vim.rtorr.com)", T.yellow, true));
    for g in settings::GROUPS {
        if g.title == "Game controls" {
            continue;
        }
        content.extend(gap());
        let body: Vec<String> = g.keys.iter().map(|(k, d)| format!("{k:<14} {d}")).collect();
        content.extend(card(g.title, T.yellow, &body, 62));
    }
    content.extend(gap());
    content.push(text_row(settings::FOOTER, T.faint, false));

    // Scroll window when content exceeds available height
    let h = th as usize;
    let max_rows = h.saturating_sub(6).max(8);
    let max_scroll = content.len().saturating_sub(max_rows);
    let scroll = app.settings_scroll.min(max_scroll);
    content.into_iter().skip(scroll).take(max_rows).collect()
}

fn render_cmdbar(out: &mut impl Write, app: &App) -> io::Result<()> {
    let (_, th) = terminal::size().unwrap_or((80, 24));
    queue!(out, MoveTo(0, th.saturating_sub(1)))?;
    queue!(out, SetAttribute(Attribute::Bold))?;
    queue!(out, SetForegroundColor(T.yellow))?;
    queue!(out, SetBackgroundColor(T.bg))?;
    write!(out, ":{} ", app.cmd_buf)?;
    queue!(out, ResetColor, SetAttribute(Attribute::Reset))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::curriculum::WINS_PER_SKILL;

    #[test]
    fn all_screens_render_without_panic() {
        let mut app = App::new();
        app.persist = false;
        let mut out = Vec::new();
        render(&mut out, &app).unwrap(); // menu
        // Unlock as we sweep so every skill intro is reachable.
        for i in 0..SKILL_COUNT {
            if i > 0 {
                app.wins[i - 1] = WINS_PER_SKILL;
            }
            app.start_skill(i);
            render(&mut out, &app).unwrap(); // intro
            app.begin_round();
            render(&mut out, &app).unwrap(); // playing
        }
        app.help_open = true;
        render(&mut out, &app).unwrap(); // help overlay
        app.help_open = false;
        app.cmd_open = true;
        app.cmd_buf = "skill 2".to_string();
        render(&mut out, &app).unwrap(); // command bar
        app.cmd_open = false;
        app.screen = Screen::Settings;
        render(&mut out, &app).unwrap(); // settings
        app.screen = Screen::RoundDone;
        render(&mut out, &app).unwrap(); // round clear
        assert!(!out.is_empty());
        let rendered = String::from_utf8_lossy(&out);
        assert!(!rendered.contains("\x1b[21m"), "must never emit SGR 21 (double underline)");
    }

    #[test]
    fn playing_board_rows_have_equal_length_and_align_left() {
        let mut app = App::new();
        app.persist = false;
        for i in 0..SKILL_COUNT {
            app.start_skill(i);
            app.begin_round();
            let rows = render_playing(&app);
            // Collect rows that form the board (gutter rows starting with line numbers or tildes)
            let board_rows: Vec<&Row> = rows
                .iter()
                .filter(|r| {
                    if r.len() < 4 {
                        return false;
                    }
                    let prefix: String = r[0..4].iter().map(|c| c.ch).collect();
                    prefix.ends_with(' ') && (prefix.trim().chars().all(|c| c.is_ascii_digit()) || prefix.contains('~'))
                })
                .collect();
            assert!(board_rows.len() >= app.buf.row_count(), "board has buffer rows in skill {}", i + 1);
            let expected_len = board_rows[0].len();
            for (idx, r) in board_rows.iter().enumerate() {
                assert_eq!(
                    r.len(),
                    expected_len,
                    "row {} in skill {} board must have equal length",
                    idx,
                    i + 1
                );
            }
        }
    }

    #[test]
    fn footer_exists_for_every_screen() {
        for screen in [
            Screen::Menu,
            Screen::Intro,
            Screen::Playing,
            Screen::RoundDone,
            Screen::Settings,
        ] {
            let mut a = App::new();
            a.persist = false;
            a.screen = screen;
            let f = footer(&a);
            assert!(!f.is_empty(), "footer for {screen:?}");
        }
    }

    #[test]
    fn hero_names_all_four_keys_with_meanings_underneath() {
        let rows = hjkl_hero();
        // 3 rows: box top, key row, meaning (no bottom border)
        assert_eq!(rows.len(), 3, "box top, key, meaning");
        let text: String = rows.iter().flat_map(|r| r.iter().map(|c| c.ch)).collect();
        for k in ['h', 'j', 'k', 'l'] {
            assert!(text.contains(k));
        }
        assert!(text.contains("left") && text.contains("right"));
        // Box rows align with each other.
        assert_eq!(rows[0].len(), rows[1].len());
        assert_eq!(rows[0].len(), rows[2].len());
        // Meaning row (sub) exists and contains the meanings.
        let sub: String = rows[2].iter().map(|c| c.ch).collect();
        assert!(sub.contains("left") && sub.contains("right"));
    }

    #[test]
    fn cards_have_exact_equal_widths() {
        let c = card("w", T.yellow, &["word start fwd".to_string()], 30);
        assert!(c.len() >= 3);
        for r in &c {
            assert_eq!(r.len(), 30, "ragged card row");
        }
        // Top corners intact.
        let flat: Vec<String> = c
            .iter()
            .map(|r| r.iter().map(|x| x.ch).collect())
            .collect();
        assert!(flat[0].starts_with('┌') && flat[0].ends_with('┐'));
        // No bottom border row (clean design).
        assert!(!flat[flat.len() - 1].starts_with('└'));
    }

    #[test]
    fn chips_show_key_over_meaning() {
        let sk = &SKILLS[3]; // Word Forward: [("w", "next word")]
        let rows = skill_chips(sk);
        // 3 rows: box top, key row, meaning (no bottom border)
        assert_eq!(rows.len(), 3);
        let mid: String = rows[1].iter().map(|c| c.ch).collect();
        let sub: String = rows[2].iter().map(|c| c.ch).collect();
        assert!(mid.contains('w'), "key in the box: {mid}");
        assert!(sub.contains("next word"), "meaning under it: {sub}");
        // Box rows (top and key) align.
        assert_eq!(rows[0].len(), rows[1].len());
    }

    #[test]
    fn cards_mirror_chip_meanings() {
        let mut a = App::new();
        a.persist = false;
        a.skill_idx = 3; // chapter 2: first card is skill 4 (w)
        let cards = chapter_cards(&a);
        assert!(!cards.is_empty());
        let body: String = cards[0].iter().flat_map(|r| r.iter().map(|c| c.ch)).collect();
        assert!(body.contains("w - next word"), "card mirrors chip: {body}");
    }

    #[test]
    fn wrap_counts_chars_not_bytes() {
        let lines = wrap("ab cd ef ghij", 10);
        assert_eq!(lines, vec!["ab cd ef", "ghij"]);
    }

    #[test]
    fn big_stars_rows_renders_large_stars() {
        for stars in 1..=3 {
            let rows = big_stars_rows(stars);
            assert_eq!(rows.len(), 4, "4 rows of big stars");
            for r in &rows {
                let text: String = r.iter().map(|c| c.ch).collect();
                assert!(text.contains('★') || text.contains('█') || text.contains('▀') || text.contains('▄'));
            }
        }
    }

    #[test]
    fn menu_chapter_numbers_aligned_in_centered_div() {
        let mut app = App::new();
        app.persist = false;

        // Fresh state (Ch 1 unlocked, Ch 2-6 locked)
        let rows = render_menu(&app);
        assert_eq!(rows.len(), 3 + LESSONS.len());
        for (i, l) in LESSONS.iter().enumerate() {
            let r = &rows[3 + i];
            assert_eq!(r.len(), 76, "Ch {} row must be exactly 76 cols wide", l.id);
            // Badge `[N] ` starts at index 2
            assert_eq!(r[2].ch, '[');
            assert_eq!(r[3].ch, (b'0' + l.id) as char);
            assert_eq!(r[4].ch, ']');
            assert_eq!(r[5].ch, ' ');
        }

        // All mastered state
        for w in &mut app.wins {
            *w = WINS_PER_SKILL;
        }
        let rows = render_menu(&app);
        for (i, l) in LESSONS.iter().enumerate() {
            let r = &rows[3 + i];
            assert_eq!(r.len(), 76, "Ch {} mastered row must be exactly 76 cols wide", l.id);
            assert_eq!(r[2].ch, '[');
            assert_eq!(r[3].ch, (b'0' + l.id) as char);
            assert_eq!(r[4].ch, ']');
            assert_eq!(r[5].ch, ' ');
        }
    }
}

