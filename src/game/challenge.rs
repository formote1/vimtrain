//! Challenge generation plus par (optimal keystroke) computation.
//!
//! Par is computed with Dijkstra over the cursor grid using exactly the
//! motions the current skill's kit allows, so par is always achievable
//! with collected keys. Skill boards spotlight one key at a time.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use rand::seq::IndexedRandom;
use rand::Rng;

use crate::engine::buffer::{Buffer, Pos};
use crate::engine::motion::{apply, label, FindKind, Motion};

use super::curriculum::{
    kit_for, K_BIGB, K_BIGE, K_BIGF, K_BIGT, K_BIGW, K_B, K_CARET, K_COUNTS, K_DOLLAR, K_E, K_F, K_G,
    K_GE, K_GG, K_GUNDER, K_H, K_HML, K_J, K_K, K_L, K_PARA, K_PERCENT, K_T, K_W, K_ZERO, SKILL_COUNT,
    SKILLS,
};

// Word classification matching the motion engine (engine/buffer.rs)
fn is_blank(c: char) -> bool {
    c == ' ' || c == '\t'
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

pub struct Challenge {
    pub lines: Vec<String>,
    pub start: Pos,
    pub goals: Vec<Pos>,
}

pub fn digits(mut n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    let mut d = 0;
    while n > 0 {
        d += 1;
        n /= 10;
    }
    d
}

// ---------------------------------------------------------------------------
// par via Dijkstra
// ---------------------------------------------------------------------------

/// Candidate (motion, count, keystroke cost) edges from `from`, using only
/// motions present in the skill kit. Reuses `out` buffer to avoid allocation.
fn fill_edges(buf: &Buffer, from: Pos, kit: u32, out: &mut Vec<(Motion, usize, usize)>) {
    out.clear();
    let has = |b: u32| kit & b != 0;
    if has(K_H) {
        out.push((Motion::Left, 1, 1));
    }
    if has(K_J) {
        out.push((Motion::Down, 1, 1));
    }
    if has(K_K) {
        out.push((Motion::Up, 1, 1));
    }
    if has(K_L) {
        out.push((Motion::Right, 1, 1));
    }

    if has(K_W) {
        out.push((Motion::WordFwdSmall, 1, 1));
    }
    if has(K_BIGW) {
        out.push((Motion::WordFwdBig, 1, 1));
    }
    if has(K_B) {
        out.push((Motion::WordBackSmall, 1, 1));
    }
    if has(K_BIGB) {
        out.push((Motion::WordBackBig, 1, 1));
    }
    if has(K_E) {
        out.push((Motion::WordEndSmall, 1, 1));
    }
    if has(K_BIGE) {
        out.push((Motion::WordEndBig, 1, 1));
    }
    if has(K_GE) {
        out.push((Motion::WordEndBackSmall, 1, 2));
        if has(K_BIGE) {
            out.push((Motion::WordEndBackBig, 1, 2));
        }
    }

    if has(K_ZERO) {
        out.push((Motion::LineStart, 1, 1));
    }
    if has(K_CARET) {
        out.push((Motion::FirstNonBlank, 1, 1));
    }
    if has(K_DOLLAR) {
        out.push((Motion::LineEnd, 1, 1));
    }
    if has(K_GUNDER) {
        out.push((Motion::LastNonBlank, 1, 2));
    }
    if has(K_PERCENT) {
        out.push((Motion::MatchPair, 1, 1));
    }
    if has(K_GG) {
        out.push((Motion::FileTop, 1, 2));
    }
    if has(K_G) {
        out.push((Motion::FileBottom, 1, 1));
    }
    if has(K_HML) {
        out.push((Motion::ScreenTop, 1, 1));
        out.push((Motion::ScreenMid, 1, 1));
        out.push((Motion::ScreenBot, 1, 1));
    }
    if has(K_G) {
        // {n}G to every line.
        for ln in 1..=buf.row_count().max(1) {
            out.push((Motion::GotoLine, ln, digits(ln) + 1));
        }
    }

    if has(K_PARA) {
        out.push((Motion::ParaFwd, 1, 1));
        out.push((Motion::ParaBack, 1, 1));
    }

    let find_bits = [
        (K_F, FindKind::Fwd),
        (K_BIGF, FindKind::Back),
        (K_T, FindKind::TillFwd),
        (K_BIGT, FindKind::TillBack),
    ];
    let find_any = find_bits.iter().any(|(b, _)| has(*b));
    let line = if find_any { buf.line_slice(from.row) } else { &[] };
    let fwd_slice = if (has(K_F) || has(K_T)) && from.col + 1 < line.len() {
        &line[from.col + 1..]
    } else {
        &[]
    };
    let back_slice = if (has(K_BIGF) || has(K_BIGT)) && from.col > 0 {
        &line[..from.col.min(line.len())]
    } else {
        &[]
    };

    if find_any {
        let mut seen = [false; 128];
        for &ch in line {
            if (ch as usize) < 128 {
                if seen[ch as usize] {
                    continue;
                }
                seen[ch as usize] = true;
            }
            for (b, kind) in &find_bits {
                if !has(*b) {
                    continue;
                }
                let reachable = match kind {
                    FindKind::Fwd | FindKind::TillFwd => fwd_slice.contains(&ch),
                    FindKind::Back | FindKind::TillBack => back_slice.contains(&ch),
                };
                if reachable {
                    out.push((Motion::Find(*kind, ch), 1, 2));
                }
            }
        }
    }

    if has(K_COUNTS) {
        for n in 2..=9usize {
            for (b, m, key_len) in [
                (K_H, Motion::Left, 1),
                (K_J, Motion::Down, 1),
                (K_K, Motion::Up, 1),
                (K_L, Motion::Right, 1),
                (K_W, Motion::WordFwdSmall, 1),
                (K_BIGW, Motion::WordFwdBig, 1),
                (K_B, Motion::WordBackSmall, 1),
                (K_BIGB, Motion::WordBackBig, 1),
                (K_E, Motion::WordEndSmall, 1),
                (K_BIGE, Motion::WordEndBig, 1),
                (K_GE, Motion::WordEndBackSmall, 2),
                (K_GE, Motion::WordEndBackBig, 2),
                (K_PARA, Motion::ParaFwd, 1),
            ] {
                if has(b) {
                    out.push((m, n, digits(n) + key_len));
                }
            }
            if has(K_PARA) {
                out.push((Motion::ParaBack, n, digits(n) + 1));
            }
        }
        if find_any {
            let mut seen_cnt = [false; 128];
            for &ch in line {
                if (ch as usize) < 128 {
                    if seen_cnt[ch as usize] {
                        continue;
                    }
                    seen_cnt[ch as usize] = true;
                }
                for (b, kind) in &find_bits {
                    if !has(*b) {
                        continue;
                    }
                    let reachable = match kind {
                        FindKind::Fwd | FindKind::TillFwd => fwd_slice.contains(&ch),
                        FindKind::Back | FindKind::TillBack => back_slice.contains(&ch),
                    };
                    if reachable {
                        for n in 2..=3usize {
                            out.push((Motion::Find(*kind, ch), n, digits(n) + 2));
                        }
                    }
                }
            }
        }
    }
}

fn dijkstra_cost_or_path(
    buf: &Buffer,
    from: Pos,
    goal: Pos,
    kit: u32,
    want_path: bool,
) -> Option<(usize, Vec<(Motion, usize)>)> {
    let from = buf.clamp(from);
    let goal = buf.clamp(goal);
    if from == goal {
        return Some((0, Vec::new()));
    }
    let max_len = (0..buf.row_count()).map(|r| buf.line_len(r)).max().unwrap_or(0);
    let stride = max_len.max(64) + 1;
    let total_cells = buf.row_count() * stride;
    let mut dist = vec![usize::MAX; total_cells];
    let mut prev = if want_path {
        vec![None; total_cells]
    } else {
        Vec::new()
    };
    let mut heap: BinaryHeap<(Reverse<usize>, usize, usize)> = BinaryHeap::new();
    let mut edges_buf = Vec::with_capacity(128);

    let from_idx = from.row * stride + from.col;
    dist[from_idx] = 0;
    heap.push((Reverse(0), from.row, from.col));

    while let Some((Reverse(d), r, c)) = heap.pop() {
        let p = Pos::new(r, c);
        if p == goal {
            if want_path {
                let mut path = VecDeque::new();
                let mut cur = goal;
                while cur != from {
                    let cur_idx = cur.row * stride + cur.col;
                    let (pp, m, n) = prev[cur_idx].clone()?;
                    path.push_front((m, n));
                    cur = pp;
                }
                return Some((d, path.into_iter().collect()));
            } else {
                return Some((d, Vec::new()));
            }
        }
        let p_idx = r * stride + c;
        if d > dist[p_idx] {
            continue;
        }
        fill_edges(buf, p, kit, &mut edges_buf);
        for &(ref m, n, cost) in &edges_buf {
            let q = apply(buf, p, m, n);
            if q == p {
                continue;
            }
            let nd = d + cost;
            let q_idx = q.row * stride + q.col;
            if nd < dist[q_idx] {
                dist[q_idx] = nd;
                if want_path {
                    prev[q_idx] = Some((p, m.clone(), n));
                }
                heap.push((Reverse(nd), q.row, q.col));
            }
        }
    }
    None
}

fn dijkstra(
    buf: &Buffer,
    from: Pos,
    goal: Pos,
    kit: u32,
) -> Option<(usize, Vec<(Motion, usize)>)> {
    dijkstra_cost_or_path(buf, from, goal, kit, true)
}

fn manhattan(a: Pos, b: Pos) -> usize {
    a.row.abs_diff(b.row) + a.col.abs_diff(b.col)
}

/// Total par plus per-goal-segment pars for a challenge under a kit.
pub fn par_for(ch: &Challenge, kit: u32) -> (usize, Vec<usize>) {
    let buf = Buffer::new(ch.lines.clone());
    let mut total = 0;
    let mut segs = Vec::new();
    let mut from = ch.start;
    for g in &ch.goals {
        let seg = dijkstra_cost_or_path(&buf, from, *g, kit, false)
            .map(|(cost, _)| cost)
            .unwrap_or_else(|| manhattan(from, *g));
        total += seg;
        segs.push(seg);
        from = *g;
    }
    (total, segs)
}

/// Optimal full path under a kit: cost plus the (motion, count) steps.
pub fn path_for(buf: &Buffer, from: Pos, goal: Pos, kit: u32) -> Option<(usize, Vec<(Motion, usize)>)> {
    dijkstra(buf, from, goal, kit)
}

/// Optimal keystroke(s) step-by-step, e.g. ["2w", "e"].
pub fn optimal_labels(buf: &Buffer, from: Pos, goal: Pos, kit: u32) -> Option<Vec<String>> {
    let (_, path) = path_for(buf, from, goal, kit)?;
    if path.is_empty() {
        return None;
    }
    Some(path.iter().map(|(m, n)| label(m, *n)).collect())
}

/// Optimal first keystroke(s), e.g. "w", "3j".
pub fn hint_for(buf: &Buffer, from: Pos, goal: Pos, kit: u32) -> Option<String> {
    optimal_labels(buf, from, goal, kit)?.into_iter().next()
}

// ---------------------------------------------------------------------------
// skill boards
// ---------------------------------------------------------------------------

const WORDS: &[&str] = &[
    "vim", "motion", "cursor", "buffer", "window", "yank", "paste", "delete",
    "change", "jump", "word", "line", "file", "search", "mark", "macro",
    "rust", "terminal", "keyboard", "speed", "train", "drill", "score",
    "streak", "par", "goal", "target", "coach", "lesson", "packet",
    "river", "mountain", "forest", "keyboard", "planet", "rocket", "garden",
    "bridge", "cloud", "stone", "flame", "ocean", "desert", "harbor",
    "lantern", "mirror", "needle", "orchard", "piano", "quilt", "raven",
    "saddle", "tiger", "umbrella", "valley", "whale", "xenon", "yacht",
    "zebra", "anchor", "baker", "cider", "drift", "ember", "fable",
];

fn words_line_exact(rng: &mut impl Rng, target_len: usize) -> String {
    if target_len < 3 {
        return ".".repeat(target_len);
    }
    // We want sum of (word_len + 1) to equal target_len + 1.
    // Word lengths in WORDS are 3..=8, so step sizes (w.len() + 1) are in 4..=9.
    let target = target_len + 1;
    let mut words: Vec<&'static str> = Vec::new();
    let mut rem = target;
    while rem > 0 {
        let valid_steps: Vec<usize> = (4..=9)
            .filter(|&s| s <= rem && (rem == s || rem >= s + 4))
            .collect();
        if valid_steps.is_empty() {
            break;
        }
        let s = *valid_steps.choose(rng).unwrap();
        let word_len = s - 1;
        let cands: Vec<&'static str> = WORDS
            .iter()
            .copied()
            .filter(|w| w.len() == word_len)
            .collect();
        let w = *cands.choose(rng).unwrap_or(&"vim");
        words.push(w);
        rem -= s;
    }
    let res = words.join(" ");
    if res.len() == target_len {
        res
    } else {
        format!("{:<width$}", res, width = target_len)
    }
}

fn words_line(rng: &mut impl Rng, _min_w: usize, _max_w: usize, max_width: usize) -> String {
    words_line_exact(rng, max_width)
}

/// Word starts for SMALL words (w/e/b): alphanumeric + underscore only.
fn word_starts_small(line: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut in_word = false;
    for (i, c) in line.char_indices() {
        if is_blank(c) {
            in_word = false;
        } else if is_word_char(c) {
            if !in_word {
                in_word = true;
                out.push(i);
            }
        } else {
            // Punctuation breaks words for small-word motions
            in_word = false;
        }
    }
    out
}

/// Word ends for SMALL words: end of alphanumeric/underscore sequences.
fn word_ends_small(line: &str) -> Vec<usize> {
    let starts = word_starts_small(line);
    starts
        .iter()
        .map(|s| {
            let mut e = *s;
            let chars: Vec<char> = line.chars().collect();
            while e + 1 < chars.len() && is_word_char(chars[e + 1]) {
                e += 1;
            }
            e
        })
        .collect()
}

/// Word starts for BIG words (W/E/B): any non-blank.
fn word_starts_big(line: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut in_word = false;
    for (i, c) in line.char_indices() {
        if is_blank(c) {
            in_word = false;
        } else if !in_word {
            in_word = true;
            out.push(i);
        }
    }
    out
}

/// Word ends for BIG words: last non-blank before a blank.
fn word_ends_big(line: &str) -> Vec<usize> {
    let starts = word_starts_big(line);
    let chars: Vec<char> = line.chars().collect();
    starts
        .iter()
        .map(|s| {
            let mut e = *s;
            while e + 1 < chars.len() && !is_blank(chars[e + 1]) {
                e += 1;
            }
            e
        })
        .collect()
}

/// Legacy aliases for backward compatibility (used in BIG-word skills).
fn word_starts(line: &str) -> Vec<usize> {
    word_starts_big(line)
}

fn word_ends(line: &str) -> Vec<usize> {
    word_ends_big(line)
}

fn tmp_buf(lines: &[String]) -> Buffer {
    Buffer::new(lines.to_vec())
}

fn goal_on_line(lines: &[String], row: usize, col: usize) -> Pos {
    tmp_buf(lines).clamp(Pos::new(row, col))
}

/// Build a board for a skill; rejects trivial boards below its floor.
pub fn generate_skill(skill_idx: usize) -> Challenge {
    let idx = skill_idx.min(SKILL_COUNT - 1);
    let floor = SKILLS[idx].floor;
    let mut ch = build_once(idx);
    for _ in 0..15 {
        let (par, _) = par_for(&ch, kit_for(idx));
        if par >= floor && !ch.goals.is_empty() {
            return ch;
        }
        ch = build_once(idx);
    }
    ch
}

fn build_once(skill_idx: usize) -> Challenge {
    let mut rng = rand::rng();
    match skill_idx + 1 {
        1 => b_corridor_h(&mut rng),
        2 => b_corridor_v(&mut rng),
        3 => b_field(&mut rng, 8, 44, 4, 6, 10),
        4 => b_words_start(&mut rng, 4),
        5 => b_words_end(&mut rng, 4),
        6 => b_words_back(&mut rng, 4),
        7 => b_words_mix(&mut rng, 5),
        8 => b_bigwords(&mut rng, 5),
        9 => b_bigwords(&mut rng, 6),
        10 => b_find(&mut rng, true, 4),
        11 => b_find(&mut rng, false, 4),
        12 => b_find_till(&mut rng, 4),
        13 => b_find_repeat(&mut rng, 4),
        14 => b_find(&mut rng, true, 5),
        15 => b_edges(&mut rng, 5),
        16 => b_carets(&mut rng, 4),
        17 => b_jumps(&mut rng, 5),
        18 => b_thirds(&mut rng, 5),
        19 => b_line_mix(&mut rng, 6),
        20 => b_counts_field(&mut rng, 4),
        21 => b_paras(&mut rng, 5),
        22 => b_power_mix(&mut rng, 6),
        23 => b_capstone(&mut rng, 6),
        24 => b_capstone(&mut rng, 7),
        _ => b_capstone(&mut rng, 8),
    }
}

// --- movement drills ---

fn b_corridor_h(rng: &mut impl Rng) -> Challenge {
    let width = 44;
    let rows = 8;
    let lines = vec![".".repeat(width); rows];
    // Zigzag: alternate direction every hop so both h and l are required.
    let mut goals = Vec::new();
    let mut c = 22usize;
    let mut left = rng.random_bool(0.5);
    for _ in 0..4 {
        let step = rng.random_range(6..=12);
        c = if left {
            c.saturating_sub(step)
        } else {
            (c + step).min(width - 1)
        };
        goals.push(Pos::new(3, c));
        left = !left;
        if c == 0 {
            left = false;
        } else if c == width - 1 {
            left = true;
        }
    }
    Challenge {
        lines,
        start: Pos::new(3, 22),
        goals,
    }
}

fn b_corridor_v(rng: &mut impl Rng) -> Challenge {
    let rows = 8;
    let width = 44;
    let lines: Vec<String> = (0..rows).map(|_| ".".repeat(width)).collect();
    // Zigzag vertically so both j and k are required.
    let mut goals = Vec::new();
    let (mut r, mut c) = (3usize, 22usize);
    let mut up = rng.random_bool(0.5);
    for _ in 0..4 {
        let step = rng.random_range(2..=4);
        r = if up {
            r.saturating_sub(step)
        } else {
            (r + step).min(rows - 1)
        };
        c = ((c as i32) + rng.random_range(-2..=2)).clamp(0, width as i32 - 1) as usize;
        goals.push(Pos::new(r, c));
        up = !up;
        if r == 0 {
            up = false;
        } else if r == rows - 1 {
            up = true;
        }
    }
    Challenge {
        lines,
        start: Pos::new(3, 22),
        goals,
    }
}

fn b_field(rng: &mut impl Rng, rows: usize, cols: usize, ngoals: usize, lo: usize, hi: usize) -> Challenge {
    let lines: Vec<String> = (0..rows).map(|_| ".".repeat(cols)).collect();
    let mut goals = Vec::new();
    let mut p = Pos::new(0, 0);
    for _ in 0..ngoals {
        let mut q = p;
        for _ in 0..rng.random_range(lo..=hi) {
            match rng.random_range(0..4) {
                0 => q.col = q.col.saturating_sub(1),
                1 => q.col = (q.col + 1).min(cols - 1),
                2 => q.row = q.row.saturating_sub(1),
                _ => q.row = (q.row + 1).min(rows - 1),
            }
        }
        if q == p {
            q = Pos::new((p.row + 3) % rows, (p.col + 5) % cols);
        }
        goals.push(q);
        p = q;
    }
    Challenge {
        lines,
        start: Pos::new(0, 0),
        goals,
    }
}

// --- word drills ---

fn word_board_lines(rng: &mut impl Rng, nlines: usize) -> Vec<String> {
    (0..nlines).map(|_| words_line(rng, 5, 7, 44)).collect()
}

fn spread_rows(nlines: usize, ngoals: usize, skip_first: bool) -> Vec<usize> {
    let mut rows: Vec<usize> = (0..nlines).collect();
    if skip_first && !rows.is_empty() {
        rows.remove(0);
    }
    // Spread out: stride through the list instead of clustering at top.
    let mut out = Vec::new();
    let mut i = rows.len() / ngoals.max(1);
    while out.len() < ngoals && !rows.is_empty() {
        let idx = i.min(rows.len() - 1);
        out.push(rows.remove(idx));
        i += rows.len().max(1) / ngoals.max(1) + 1;
    }
    out
}

fn b_words_start(rng: &mut impl Rng, ngoals: usize) -> Challenge {
    let lines = word_board_lines(rng, 8);
    let mut goals = Vec::new();
    for row in spread_rows(8, ngoals, true) {
        let starts = word_starts_small(&lines[row]);
        if starts.len() < 2 {
            continue;
        }
        let col = starts[rng.random_range(1..starts.len())];
        goals.push(goal_on_line(&lines, row, col));
    }
    Challenge {
        lines,
        start: Pos::new(0, 0),
        goals,
    }
}

fn b_words_end(rng: &mut impl Rng, ngoals: usize) -> Challenge {
    let lines = word_board_lines(rng, 8);
    let mut goals = Vec::new();
    for row in spread_rows(8, ngoals, true) {
        let ends = word_ends_small(&lines[row]);
        if ends.len() < 2 {
            continue;
        }
        let col = ends[rng.random_range(1..ends.len())];
        goals.push(goal_on_line(&lines, row, col));
    }
    Challenge {
        lines,
        start: Pos::new(0, 0),
        goals,
    }
}

fn b_words_back(rng: &mut impl Rng, ngoals: usize) -> Challenge {
    let lines = word_board_lines(rng, 8);
    let last = lines.len() - 1;
    let mut goals = Vec::new();
    // Start at the end; every goal sits behind you.
    for row in spread_rows(8, ngoals, false).into_iter().rev().skip(1) {
        let starts = word_starts_small(&lines[row]);
        if starts.is_empty() {
            continue;
        }
        let col = starts[rng.random_range(0..starts.len())];
        goals.push(goal_on_line(&lines, row, col));
    }
    let buf = tmp_buf(&lines);
    Challenge {
        lines,
        start: buf.clamp(Pos::new(last, buf.line_len(last).saturating_sub(1))),
        goals,
    }
}

fn b_words_mix(rng: &mut impl Rng, ngoals: usize) -> Challenge {
    let lines = word_board_lines(rng, 8);
    let mut goals = Vec::new();
    for (k, row) in spread_rows(8, ngoals, true).into_iter().enumerate() {
        if k % 2 == 0 {
            let starts = word_starts_small(&lines[row]);
            if starts.len() < 2 {
                continue;
            }
            goals.push(goal_on_line(&lines, row, starts[rng.random_range(1..starts.len())]));
        } else {
            let ends = word_ends_small(&lines[row]);
            if ends.len() < 2 {
                continue;
            }
            goals.push(goal_on_line(&lines, row, ends[rng.random_range(1..ends.len())]));
        }
    }
    Challenge {
        lines,
        start: Pos::new(0, 0),
        goals,
    }
}

fn punct_line(rng: &mut impl Rng, max_width: usize) -> String {
    let mut s = words_line(rng, 4, 6, max_width);
    let mut chars: Vec<char> = s.chars().collect();
    for _ in 0..rng.random_range(1..=3) {
        if chars.len() > 8 {
            let i = rng.random_range(2..chars.len() - 2);
            if chars[i] != ' ' {
                chars[i] = *['(', ')', ',', '.', ':'].choose(rng).unwrap_or(&',');
            }
        }
    }
    s = chars.into_iter().collect();
    s
}

fn b_bigwords(rng: &mut impl Rng, ngoals: usize) -> Challenge {
    let lines: Vec<String> = (0..10).map(|_| punct_line(rng, 44)).collect();
    let mut goals = Vec::new();
    for (k, row) in spread_rows(10, ngoals, true).into_iter().enumerate() {
        if k % 2 == 0 {
            let starts = word_starts(&lines[row]);
            if starts.len() < 2 {
                continue;
            }
            goals.push(goal_on_line(&lines, row, starts[rng.random_range(1..starts.len())]));
        } else {
            let ends = word_ends(&lines[row]);
            if ends.len() < 2 {
                continue;
            }
            goals.push(goal_on_line(&lines, row, ends[rng.random_range(1..ends.len())]));
        }
    }
    Challenge {
        lines,
        start: Pos::new(0, 0),
        goals,
    }
}

// --- find drills ---

const FIND_TARGETS: [char; 6] = ['x', 'm', 'a', ',', '(', ')'];

fn find_soup(rng: &mut impl Rng, nlines: usize, width: usize, same: bool) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for _ in 0..nlines {
        let mut chars: Vec<char> = (0..width)
            .map(|i| {
                if i % 7 == 6 {
                    ' '
                } else {
                    rng.random_range(b'b'..=b'g') as char
                }
            })
            .collect();
        if same {
            // One repeated target: repeat territory.
            let t = *FIND_TARGETS.choose(rng).unwrap_or(&'x');
            for i in (2..width).step_by(3).take(rng.random_range(4..=6)) {
                chars[i] = t;
            }
        } else {
            let t = *FIND_TARGETS.choose(rng).unwrap_or(&'x');
            for _ in 0..rng.random_range(3..=5) {
                chars[rng.random_range(1..width)] = t;
            }
        }
        lines.push(chars.into_iter().collect());
    }
    lines
}

fn occurrences(line: &[char], target: char) -> Vec<usize> {
    line.iter()
        .enumerate()
        .filter(|(i, c)| *i > 0 && **c == target)
        .map(|(i, _)| i)
        .collect()
}

fn b_find(rng: &mut impl Rng, fwd: bool, ngoals: usize) -> Challenge {
    let lines = find_soup(rng, 8, 36, false);
    let rows = spread_rows(8, ngoals, true);
    let mut goals = Vec::new();
    for row in rows {
        let line: Vec<char> = lines[row].chars().collect();
        let mut spots = Vec::new();
        for t in FIND_TARGETS {
            let occ = occurrences(&line, t);
            if fwd {
                spots.extend(occ.into_iter().filter(|i| *i > 2));
            } else {
                spots.extend(occ.into_iter().filter(|i| *i + 3 < line.len()));
            }
        }
        if spots.is_empty() {
            continue;
        }
        goals.push(goal_on_line(&lines, row, *spots.choose(rng).unwrap_or(&4)));
    }
    let buf = tmp_buf(&lines);
    let start = if fwd {
        Pos::new(0, 0)
    } else {
        buf.clamp(Pos::new(0, buf.line_len(0).saturating_sub(1)))
    };
    Challenge { lines, start, goals }
}

fn b_find_till(rng: &mut impl Rng, ngoals: usize) -> Challenge {
    // Chain per line: t-spot then T-spot, so till in both directions wins.
    let width = 36;
    let mut lines: Vec<String> = Vec::new();
    for _ in 0..6 {
        let mut chars: Vec<char> = (0..width)
            .map(|i| {
                if i % 8 == 7 {
                    ' '
                } else {
                    rng.random_range(b'b'..=b'g') as char
                }
            })
            .collect();
        let t = *FIND_TARGETS.choose(rng).unwrap_or(&'x');
        let a = rng.random_range(10..16);
        let b = rng.random_range(22..30);
        chars[a] = t;
        chars[b] = t;
        lines.push(chars.into_iter().collect());
    }
    let mut goals = Vec::new();
    for row in spread_rows(6, ngoals / 2 + 1, true) {
        let line: Vec<char> = lines[row].chars().collect();
        let mut ts: Vec<usize> = line
            .iter()
            .enumerate()
            .filter(|(_, c)| FIND_TARGETS.contains(c))
            .map(|(i, _)| i)
            .collect();
        ts.sort_unstable();
        if ts.len() < 2 {
            continue;
        }
        let (a, b) = (ts[ts.len() / 3], ts[2 * ts.len() / 3]);
        if a + 1 >= b {
            continue;
        }
        goals.push(goal_on_line(&lines, row, b - 1)); // t-spot
        goals.push(goal_on_line(&lines, row, a + 1)); // T-spot
        if goals.len() >= ngoals {
            break;
        }
    }
    Challenge {
        lines,
        start: Pos::new(0, 0),
        goals,
    }
}

fn b_find_repeat(rng: &mut impl Rng, ngoals: usize) -> Challenge {
    let lines = find_soup(rng, 8, 36, true);
    let mut goals = Vec::new();
    for row in spread_rows(8, ngoals, true) {
        let line: Vec<char> = lines[row].chars().collect();
        let mut best: Vec<usize> = Vec::new();
        for t in FIND_TARGETS {
            let occ = occurrences(&line, t);
            if occ.len() >= 3 && occ.len() > best.len() {
                best = occ;
            }
        }
        if best.len() < 3 {
            continue;
        }
        // 3rd+ occurrence: fx then ;;; territory.
        goals.push(goal_on_line(&lines, row, best[2.min(best.len() - 1)]));
    }
    Challenge {
        lines,
        start: Pos::new(0, 0),
        goals,
    }
}

// --- line drills ---

fn indent_lines(rng: &mut impl Rng, nlines: usize) -> Vec<String> {
    (0..nlines)
        .map(|_| {
            let ind_len = rng.random_range(0..=6);
            let indent = " ".repeat(ind_len);
            format!("{indent}{}", words_line_exact(rng, 44 - ind_len))
        })
        .collect()
}

fn b_edges(rng: &mut impl Rng, ngoals: usize) -> Challenge {
    let lines = indent_lines(rng, 10);
    let buf = tmp_buf(&lines);
    let last = lines.len() - 1;
    let cands = [
        Pos::new(7, buf.line_len(7).saturating_sub(1)),
        Pos::new(1, 0),
        Pos::new(5, buf.line_len(5).saturating_sub(1)),
        Pos::new(3, 0),
        Pos::new(9.min(last), buf.line_len(9.min(last)).saturating_sub(1)),
        Pos::new(0, 0),
    ];
    let goals = cands.into_iter().take(ngoals).map(|p| buf.clamp(p)).collect();
    Challenge {
        lines,
        start: buf.clamp(Pos::new(4, buf.first_non_blank(4))),
        goals,
    }
}

fn b_carets(rng: &mut impl Rng, ngoals: usize) -> Challenge {
    let lines = indent_lines(rng, 10);
    let buf = tmp_buf(&lines);
    let goals = spread_rows(10, ngoals, true)
        .into_iter()
        .map(|r| buf.clamp(Pos::new(r, buf.first_non_blank(r))))
        .collect();
    Challenge {
        lines,
        start: Pos::new(0, 0),
        goals,
    }
}

fn b_jumps(rng: &mut impl Rng, ngoals: usize) -> Challenge {
    let lines = indent_lines(rng, 12);
    let buf = tmp_buf(&lines);
    let last = lines.len() - 1;
    let cands = [
        Pos::new(0, buf.first_non_blank(0)),
        Pos::new(last, buf.first_non_blank(last)),
        Pos::new(5, buf.first_non_blank(5)),
        Pos::new(2, buf.first_non_blank(2)),
        Pos::new(9.min(last), buf.first_non_blank(9.min(last))),
        Pos::new(7.min(last), buf.first_non_blank(7.min(last))),
    ];
    let goals = cands.into_iter().take(ngoals).map(|p| buf.clamp(p)).collect();
    let _ = rng;
    Challenge {
        lines,
        start: buf.clamp(Pos::new(6, buf.first_non_blank(6))),
        goals,
    }
}

fn b_thirds(rng: &mut impl Rng, ngoals: usize) -> Challenge {
    let lines = indent_lines(rng, 12);
    let buf = tmp_buf(&lines);
    let last = lines.len() - 1;
    let mid = last / 2;
    let cycle = [
        Pos::new(0, buf.first_non_blank(0)),
        Pos::new(mid, buf.first_non_blank(mid)),
        Pos::new(last, buf.first_non_blank(last)),
    ];
    let goals = (0..ngoals)
        .map(|k| buf.clamp(cycle[k % cycle.len()]))
        .collect();
    let _ = rng;
    Challenge {
        lines,
        start: buf.clamp(Pos::new(mid, buf.first_non_blank(mid))),
        goals,
    }
}

fn b_line_mix(rng: &mut impl Rng, ngoals: usize) -> Challenge {
    let lines = indent_lines(rng, 12);
    let buf = tmp_buf(&lines);
    let last = lines.len() - 1;
    let cands = [
        Pos::new(8, buf.line_len(8).saturating_sub(1)),
        Pos::new(2, buf.first_non_blank(2)),
        Pos::new(last, buf.first_non_blank(last)),
        Pos::new(0, buf.first_non_blank(0)),
        Pos::new(5, 0),
        Pos::new(10.min(last), buf.line_len(10.min(last)).saturating_sub(1)),
        Pos::new(mid_of(last), buf.first_non_blank(mid_of(last))),
    ];
    let goals = cands.into_iter().take(ngoals).map(|p| buf.clamp(p)).collect();
    Challenge {
        lines,
        start: buf.clamp(Pos::new(4, buf.first_non_blank(4))),
        goals,
    }
}

fn mid_of(last: usize) -> usize {
    last / 2
}

// --- power drills ---

fn b_counts_field(rng: &mut impl Rng, ngoals: usize) -> Challenge {
    // Narrow word columns, goals stacked far apart: multiply or crawl.
    let lines: Vec<String> = (0..12).map(|_| words_line(rng, 2, 3, 20)).collect();
    let buf = tmp_buf(&lines);
    let mut goals = Vec::new();
    for row in (2..12).step_by(9 / ngoals.max(1) + 1).take(ngoals) {
        goals.push(buf.clamp(Pos::new(row, buf.first_non_blank(row))));
    }
    Challenge {
        lines,
        start: Pos::new(0, 0),
        goals,
    }
}

fn para_block(rng: &mut impl Rng, per: usize, width: usize) -> Vec<String> {
    (0..per).map(|_| words_line(rng, 3, 6, width)).collect()
}

fn b_paras(rng: &mut impl Rng, ngoals: usize) -> Challenge {
    let mut lines = Vec::new();
    for block in 0..3 {
        lines.extend(para_block(rng, 3, 46));
        if block < 2 {
            lines.push(String::new());
        }
    }
    // rows: 0,1,2, blank 3, 4,5,6, blank 7, 8,9,10
    // Goals sit ON the blank separators too: } lands there in one key
    // while counted-G needs two, so paragraph jumps are strictly optimal.
    let buf = tmp_buf(&lines);
    let cands = [
        Pos::new(3, 0),
        Pos::new(4, buf.first_non_blank(4)),
        Pos::new(7, 0),
        Pos::new(8, buf.first_non_blank(8)),
        Pos::new(10, buf.line_len(10).saturating_sub(1)),
        Pos::new(1, 0),
    ];
    let goals = cands
        .into_iter()
        .take(ngoals)
        .map(|p| buf.clamp(p))
        .collect();
    Challenge {
        lines,
        start: Pos::new(0, 0),
        goals,
    }
}

fn b_power_mix(rng: &mut impl Rng, ngoals: usize) -> Challenge {
    let mut lines = Vec::new();
    for block in 0..3 {
        for _ in 0..3 {
            let ind_len = if rng.random_bool(0.3) {
                rng.random_range(2..=6)
            } else {
                0
            };
            let indent = " ".repeat(ind_len);
            lines.push(format!("{indent}{}", words_line_exact(rng, 44 - ind_len)));
        }
        if block < 2 {
            lines.push(String::new());
        }
    }
    let buf = tmp_buf(&lines);
    let cands = [
        Pos::new(4, buf.first_non_blank(4)),
        Pos::new(6, buf.line_len(6).saturating_sub(1)),
        Pos::new(8, buf.first_non_blank(8)),
        Pos::new(10, buf.line_len(10).saturating_sub(1)),
        Pos::new(0, buf.line_len(0).saturating_sub(1)),
        Pos::new(2, buf.first_non_blank(2)),
    ];
    let goals = cands
        .into_iter()
        .take(ngoals)
        .map(|p| buf.clamp(p))
        .collect();
    Challenge {
        lines,
        start: Pos::new(0, 0),
        goals,
    }
}

fn b_capstone(rng: &mut impl Rng, ngoals: usize) -> Challenge {
    let mut lines = Vec::new();
    for i in 0..12 {
        if i == 4 || i == 8 {
            lines.push(String::new());
            continue;
        }
        let ind_len = if rng.random_bool(0.35) {
            rng.random_range(2..=6)
        } else {
            0
        };
        let indent = " ".repeat(ind_len);
        lines.push(format!("{indent}{}", punct_line(rng, 44 - ind_len)));
    }
    let buf = tmp_buf(&lines);
    let rows = [1usize, 3, 5, 9, 11, 2, 6, 10];
    let mut goals = Vec::new();
    for (k, row) in rows.into_iter().take(ngoals).enumerate() {
        if k % 2 == 0 {
            let starts = word_starts(&buf.line_string(row));
            let col = starts
                .get(rng.random_range(0..starts.len().max(1)))
                .copied()
                .unwrap_or(0);
            goals.push(buf.clamp(Pos::new(row, col)));
        } else {
            let ends = word_ends(&buf.line_string(row));
            let col = ends
                .get(rng.random_range(0..ends.len().max(1)))
                .copied()
                .unwrap_or(0);
            goals.push(buf.clamp(Pos::new(row, col)));
        }
    }
    Challenge {
        lines,
        start: Pos::new(0, 0),
        goals,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use crate::game::curriculum::{spot_hit, Spot};
    use crate::game::curriculum::kit_for;

    fn buf(lines: &[&str]) -> Buffer {
        Buffer::new(lines.iter().map(|s| s.to_string()).collect())
    }

    const HJKL: u32 = K_H | K_J | K_K | K_L;

    #[test]
    fn par_hjkl_is_manhattan() {
        let b = buf(&["....", "....", "...."]);
        let (cost, _) = dijkstra(&b, Pos::new(0, 0), Pos::new(2, 3), HJKL).unwrap();
        assert_eq!(cost, 5);
    }

    #[test]
    fn par_words_beats_walking() {
        let b = buf(&["foo bar baz"]);
        let (cost, _) = dijkstra(&b, Pos::new(0, 0), Pos::new(0, 8), HJKL | K_W).unwrap();
        assert_eq!(cost, 2); // w w
    }

    #[test]
    fn hint_exists_for_kits() {
        let b = buf(&["foo bar"]);
        assert!(hint_for(&b, Pos::new(0, 0), Pos::new(0, 4), HJKL | K_W).is_some());
        assert!(hint_for(&b, Pos::new(0, 0), Pos::new(0, 4), HJKL).is_some());
    }

    fn motion_kind(m: &Motion) -> u8 {        match m {
            Motion::Left => 0,
            Motion::Down => 1,
            Motion::Up => 2,
            Motion::Right => 3,
            Motion::WordFwdSmall => 4,
            Motion::WordFwdBig => 5,
            Motion::WordBackSmall => 6,
            Motion::WordBackBig => 7,
            Motion::WordEndSmall => 8,
            Motion::WordEndBig => 9,
            Motion::LineStart => 10,
            Motion::FirstNonBlank => 11,
            Motion::LineEnd => 12,
            Motion::LastNonBlank => 13,
            Motion::FileTop => 14,
            Motion::FileBottom => 15,
            Motion::GotoLine => 16,
            Motion::ScreenTop => 17,
            Motion::ScreenMid => 18,
            Motion::ScreenBot => 19,
            Motion::Find(FindKind::Fwd, _) => 20,
            Motion::Find(FindKind::Back, _) => 21,
            Motion::Find(FindKind::TillFwd, _) => 22,
            Motion::Find(FindKind::TillBack, _) => 23,
            Motion::ParaFwd => 24,
            Motion::ParaBack => 25,
            Motion::WordEndBackSmall => 26,
            Motion::WordEndBackBig => 27,
            Motion::MatchPair => 28,
        }
    }

    fn solve_kinds(ch: &Challenge, kit: u32) -> HashSet<u8> {
        let buf = Buffer::new(ch.lines.clone());
        let mut kinds = HashSet::new();
        let mut from = ch.start;
        for g in &ch.goals {
            if let Some((_, p)) = path_for(&buf, from, *g, kit) {
                kinds.extend(p.iter().map(|(m, _)| motion_kind(m)));
            }
            from = *g;
        }
        kinds
    }

    #[test]
    fn corridors_require_both_directions() {
        // Skill 1 must need h AND l; skill 2 must need j AND k.
        // Zigzag construction makes this hold on every board, not by luck.
        for _ in 0..10 {
            let mut rng = rand::rng();
            let kinds = solve_kinds(&b_corridor_h(&mut rng), kit_for(0));
            assert!(kinds.contains(&motion_kind(&Motion::Left)), "h unused");
            assert!(kinds.contains(&motion_kind(&Motion::Right)), "l unused");
            let kinds = solve_kinds(&b_corridor_v(&mut rng), kit_for(1));
            assert!(kinds.contains(&motion_kind(&Motion::Up)), "k unused");
            assert!(kinds.contains(&motion_kind(&Motion::Down)), "j unused");
        }
    }

    #[test]
    fn every_skill_board_is_valid_and_floored() {
        for (i, sk) in SKILLS.iter().enumerate() {
            let ch = generate_skill(i);
            assert!(!ch.lines.is_empty(), "skill {}", i + 1);
            assert!(!ch.goals.is_empty(), "skill {}", i + 1);
            assert!(ch.lines.len() <= 12, "board fits skill {}", i + 1);
            let b = Buffer::new(ch.lines.clone());
            assert_eq!(b.clamp(ch.start), ch.start, "start in bounds {}", i + 1);
            // Blank-line goals are legal only where paragraph jumps land.
            let para_skill = sk.spots.contains(&Spot::Para);
            for g in &ch.goals {
                assert_eq!(b.clamp(*g), *g, "goal in bounds {}", i + 1);
                assert!(
                    para_skill || !b.is_blank_line(g.row),
                    "goal not blank {}",
                    i + 1
                );
            }
            let kit = kit_for(i);
            let (par, segs) = par_for(&ch, kit);
            assert_eq!(segs.len(), ch.goals.len());
            assert!(par > 0, "non-trivial par {}", i + 1);
            assert!(
                par >= sk.floor,
                "meets floor skill {}: par {par} < {}",
                i + 1,
                sk.floor
            );
        }
    }

    #[test]
    fn demo_rooms_are_solvable_spotlights() {
        // Every demo must be solvable and its optimal solution must use
        // the skill's spotlight key — the demo IS the first proof.
        for (i, sk) in SKILLS.iter().enumerate() {
            let Some(d) = &sk.demo else { continue };
            let lines: Vec<String> = d.lines.iter().map(|s| s.to_string()).collect();
            let buf = Buffer::new(lines.clone());
            let start = buf.clamp(Pos::new(d.start.0, d.start.1));
            let goals: Vec<Pos> = d
                .goals
                .iter()
                .map(|(r, c)| buf.clamp(Pos::new(*r, *c)))
                .collect();
            assert!(!goals.is_empty(), "demo goals skill {}", i + 1);
            let kit = kit_for(i);
            let ch = Challenge {
                lines,
                start,
                goals: goals.clone(),
            };
            let (par, _) = par_for(&ch, kit);
            assert!(par > 0, "demo unsolvable skill {}", i + 1);
            let mut from = start;
            let mut used = false;
            for g in &goals {
                if let Some((_, p)) = path_for(&buf, from, *g, kit) {
                    used |= sk.spots.iter().any(|s| spot_hit(*s, &p));
                }
                from = *g;
            }
            assert!(used, "demo doesn't use spotlight skill {}", i + 1);
        }
    }

    #[test]
    fn spotlight_skills_reward_their_key() {
        // For each single-spotlight skill, at least one of several boards
        // must be optimally solved with the spotlight key in the path.
        for (i, sk) in SKILLS.iter().enumerate() {
            if sk.spots.is_empty() {
                continue;
            }
            let kit = kit_for(i);
            let mut found = false;
            for _ in 0..20 {
                let ch = build_once(i);
                let buf = Buffer::new(ch.lines.clone());
                let mut from = ch.start;
                let mut path_all = Vec::new();
                for g in &ch.goals {
                    if let Some((_, p)) = path_for(&buf, from, *g, kit) {
                        path_all.extend(p);
                    }
                    from = *g;
                }
                if sk.spots.iter().any(|s| spot_hit(*s, &path_all)) {
                    found = true;
                    break;
                }
            }
            assert!(found, "spotlight never optimal in skill {}", i + 1);
        }
    }

    #[test]
    fn fluency_skills_force_variety() {
        // Combo boards must reward at least 4 distinct motion kinds.
        for (i, sk) in SKILLS.iter().enumerate() {
            if !sk.spots.is_empty() {
                continue;
            }
            let kit = kit_for(i);
            let mut found = false;
            for _ in 0..20 {
                let ch = build_once(i);
                let buf = Buffer::new(ch.lines.clone());
                let mut kinds = HashSet::new();
                let mut from = ch.start;
                for g in &ch.goals {
                    if let Some((_, p)) = path_for(&buf, from, *g, kit) {
                        kinds.extend(p.iter().map(|(m, _)| motion_kind(m)));
                    }
                    from = *g;
                }
                if kinds.len() >= 4 {
                    found = true;
                    break;
                }
            }
            assert!(found, "no varied solution in skill {}", i + 1);
        }
    }

    #[test]
    fn word_end_motion_visits_every_word_end() {
        let ch = generate_skill(4); // Skill 5: Word End
        let b = Buffer::new(ch.lines.clone());
        let mut p = Pos::new(0, 0);
        let mut visited = Vec::new();
        for _ in 0..20 {
            let next_p = apply(&b, p, &Motion::WordEndSmall, 1);
            if next_p == p {
                break;
            }
            visited.push(next_p);
            p = next_p;
        }
        assert!(!visited.is_empty());
        for pos in &visited {
            let len = b.line_len(pos.row);
            let at_word_char = b
                .char_at(pos.row, pos.col)
                .map(|c| c.is_alphanumeric() || c == '_')
                .unwrap_or(false);
            assert!(at_word_char, "Word end must land on a word char: {:?}", pos);
            let next_is_not_word = if pos.col + 1 < len {
                !b.char_at(pos.row, pos.col + 1)
                    .map(|c| c.is_alphanumeric() || c == '_')
                    .unwrap_or(false)
            } else {
                true // end of line is end of word
            };
            assert!(
                next_is_not_word,
                "Word end must be at end of word: {:?}",
                pos
            );
        }
    }

    #[test]
    fn words_line_exact_produces_equal_length_lines() {
        let mut rng = rand::rng();
        for _ in 0..50 {
            let line = words_line_exact(&mut rng, 44);
            assert_eq!(line.len(), 44, "Line must have exact length 44");
            for word in line.split(' ') {
                assert!(!word.is_empty(), "No adjacent spaces");
                assert!(
                    WORDS.contains(&word),
                    "Must contain valid words from WORDS: {}",
                    word
                );
            }
        }
    }
}
