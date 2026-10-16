//! App state and input handling: skill-by-skill progression.

use std::collections::VecDeque;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyModifiers};

use crate::coach;
use crate::engine::buffer::{Buffer, Pos};
use crate::engine::keymap::{Feed, KeyState};
use crate::engine::motion::{apply, label};
use crate::game::challenge::{self, hint_for, path_for};
use crate::game::curriculum::{
    counts_teacher, kit_allows, kit_allows_count, kit_for, teacher_of, SKILL_COUNT, SKILLS,
    WINS_PER_SKILL,
};
use crate::game::lesson::LESSONS;
use crate::game::progress::{Difficulty, Progress};
use crate::game::score;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Menu,
    Intro,
    Playing,
    RoundDone,
    Settings,
}

pub struct App {
    pub screen: Screen,
    pub settings_return: Screen,
    pub skill_idx: usize,
    pub menu_sel: usize,
    pub buf: Buffer,
    pub cursor: Pos,
    pub goals: Vec<Pos>,
    pub goal_idx: usize,
    pub keys: KeyState,
    pub keys_pressed: usize,
    pub seg_start_keys: usize,
    pub misses: usize,
    pub seg_pars: Vec<usize>,
    pub par: usize,
    pub round_start: Instant,
    pub status: String,
    pub status_err: bool,
    pub help_open: bool,
    pub cmd_open: bool,
    pub cmd_buf: String,
    pub settings_scroll: usize,
    pub total_score: u32,
    pub streak: u32,
    pub last_stars: u8,
    pub last_points: u32,
    pub last_secs: u64,
    pub hint_stage: u8, // 0 = none, 1 = +5 nudge shown, 2 = +10 answer shown
    pub should_quit: bool,
    // Difficulty mode & failure state
    pub difficulty: Difficulty,
    pub round_failed: bool,
    // Progression: cleared rounds per skill (persisted), recent key labels
    // for the over-par coach, menu notices. Saves stay off in tests.
    pub wins: [u32; SKILL_COUNT],
    pub recent: VecDeque<String>,
    pub recent_total: u64,
    pub seg_recent_mark: u64,
    pub menu_msg: String,
    pub persist: bool,
    /// Demo room active (copy the shown keys, no scoring, no misses).
    pub demo: bool,
    /// Demo rooms already played this session (one-time examples).
    pub demo_seen: [bool; SKILL_COUNT],
}

impl App {
    pub fn new() -> Self {
        let prog = Progress::load();
        Self {
            screen: Screen::Menu,
            settings_return: Screen::Menu,
            skill_idx: 0,
            menu_sel: 0,
            buf: Buffer::new(vec![]),
            cursor: Pos::new(0, 0),
            goals: Vec::new(),
            goal_idx: 0,
            keys: KeyState::new(),
            keys_pressed: 0,
            seg_start_keys: 0,
            misses: 0,
            seg_pars: Vec::new(),
            par: 0,
            round_start: Instant::now(),
            status: String::new(),
            status_err: false,
            help_open: false,
            cmd_open: false,
            cmd_buf: String::new(),
            settings_scroll: 0,
            total_score: 0,
            streak: 0,
            last_stars: 0,
            last_points: 0,
            last_secs: 0,
            hint_stage: 0,
            should_quit: false,
            difficulty: prog.difficulty,
            round_failed: false,
            wins: prog.wins,
            recent: VecDeque::new(),
            recent_total: 0,
            seg_recent_mark: 0,
            menu_msg: String::new(),
            persist: true,
            demo: false,
            demo_seen: [false; SKILL_COUNT],
        }
    }

    pub fn skill(&self) -> &'static crate::game::curriculum::Skill {
        &SKILLS[self.skill_idx.min(SKILL_COUNT - 1)]
    }

    pub fn kit(&self) -> u32 {
        kit_for(self.skill_idx)
    }

    /// Chapter index (0-based) of the current skill.
    pub fn chapter_idx(&self) -> usize {
        (self.skill().lesson as usize).saturating_sub(1).min(LESSONS.len() - 1)
    }

    pub fn menu_len(&self) -> usize {
        LESSONS.len()
    }

    pub fn elapsed_secs(&self) -> u64 {
        self.round_start.elapsed().as_secs()
    }

    pub fn current_goal(&self) -> Option<Pos> {
        self.goals.get(self.goal_idx).copied()
    }

    pub fn max_moves(&self) -> Option<usize> {
        if self.demo {
            None
        } else {
            self.difficulty.max_moves(self.skill_idx, self.par)
        }
    }

    pub fn fail_round(&mut self) {
        self.round_failed = true;
        self.streak = 0;
        self.last_stars = 0;
        self.last_points = 0;
        self.screen = Screen::RoundDone;
        let limit = self.max_moves().unwrap_or(self.keys_pressed);
        self.set_status(
            format!("Move limit exceeded ({}/{}). Press [Enter] or [r] to retry.", self.keys_pressed, limit),
            true,
        );
    }

    fn set_status(&mut self, msg: impl Into<String>, err: bool) {
        self.status = msg.into();
        self.status_err = err;
    }

    /// Skill 1 is always open; others need mastery of the previous skill.
    pub fn unlocked_skill(&self, idx: usize) -> bool {
        idx == 0 || self.wins[idx.saturating_sub(1)] >= WINS_PER_SKILL
    }

    pub fn mastered(&self, idx: usize) -> bool {
        self.wins[idx.min(SKILL_COUNT - 1)] >= WINS_PER_SKILL
    }

    /// First unmastered skill of a chapter (0-based chapter) for resume.
    pub fn resume_skill(&self, chapter: usize) -> usize {
        let id = (chapter + 1) as u8;
        SKILLS
            .iter()
            .enumerate()
            .find(|(i, s)| s.lesson == id && !self.mastered(*i))
            .map(|(i, _)| i)
            .unwrap_or_else(|| SKILLS.iter().position(|s| s.lesson == id).unwrap_or(0))
    }

    fn save_progress(&self) {
        if self.persist {
            Progress {
                wins: self.wins,
                difficulty: self.difficulty,
            }
            .save();
        }
    }

    pub fn start_skill(&mut self, idx: usize) {
        let idx = idx.min(SKILL_COUNT - 1);
        if !self.unlocked_skill(idx) {
            let prev = idx.saturating_sub(1);
            let need = WINS_PER_SKILL - self.wins[prev].min(WINS_PER_SKILL);
            self.menu_msg = format!(
                "Skill {} is locked: win {need} more round(s) in skill {} ({}).",
                idx + 1,
                prev + 1,
                SKILLS[prev].name
            );
            self.screen = Screen::Menu;
            return;
        }
        self.skill_idx = idx;
        self.menu_msg.clear();
        self.screen = Screen::Intro;
        self.help_open = false;
        self.cmd_open = false;
        self.cmd_buf.clear();
        self.keys.clear();
    }

    pub fn begin_round(&mut self) {
        self.round_failed = false;
        if self.skill().demo.is_some() && !self.demo_seen[self.skill_idx] {
            self.start_demo();
        } else {
            self.start_challenge();
        }
    }

    fn demo_hint(&self) -> String {
        match &self.skill().demo {
            Some(d) => format!("Try it: press {}. Misses don't count here.", d.press),
            None => "Reach the green goal.".to_string(),
        }
    }

    /// Fixed one-time example room: feel the new key, zero pressure.
    fn start_demo(&mut self) {
        let (lines, start, goals) = match &self.skill().demo {
            Some(d) => (
                d.lines.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                Pos::new(d.start.0, d.start.1),
                d.goals.to_vec(),
            ),
            None => {
                self.start_challenge();
                return;
            }
        };
        self.buf = Buffer::new(lines);
        self.cursor = self.buf.clamp(start);
        self.goals = goals
            .iter()
            .map(|(r, c)| self.buf.clamp(Pos::new(*r, *c)))
            .collect();
        let kit = self.kit();
        let ch = challenge::Challenge {
            lines: (0..self.buf.row_count()).map(|r| self.buf.line_string(r)).collect(),
            start: self.cursor,
            goals: self.goals.clone(),
        };
        let (par, seg_pars) = challenge::par_for(&ch, kit);
        self.goal_idx = 0;
        self.keys = KeyState::new();
        self.keys_pressed = 0;
        self.seg_start_keys = 0;
        self.misses = 0;
        self.seg_pars = seg_pars;
        self.par = par;
        self.round_start = Instant::now();
        self.help_open = false;
        self.cmd_open = false;
        self.cmd_buf.clear();
        self.hint_stage = 0;
        self.recent.clear();
        self.seg_recent_mark = self.recent_total;
        self.demo = true;
        self.screen = Screen::Playing;
        self.set_status(self.demo_hint(), false);
    }

    /// Demo cleared: on to the real challenge.
    fn finish_demo(&mut self) {
        self.demo_seen[self.skill_idx] = true;
        self.demo = false;
        self.start_challenge();
        self.set_status("Felt it? Real challenge now: match par!", false);
    }

    fn check_move_limit(&mut self) -> bool {
        if !self.demo {
            if let Some(max) = self.max_moves() {
                if self.keys_pressed > max {
                    self.fail_round();
                    return true;
                }
            }
        }
        false
    }

    fn start_challenge(&mut self) {
        self.demo = false;
        self.round_failed = false;
        let ch = challenge::generate_skill(self.skill_idx);
        let kit = self.kit();
        let (par, seg_pars) = challenge::par_for(&ch, kit);
        self.buf = Buffer::new(ch.lines);
        self.cursor = self.buf.clamp(ch.start);
        self.goals = ch.goals;
        self.goal_idx = 0;
        self.keys = KeyState::new();
        self.keys_pressed = 0;
        self.seg_start_keys = 0;
        self.misses = 0;
        self.seg_pars = seg_pars;
        self.par = par;
        self.round_start = Instant::now();
        self.help_open = false;
        self.cmd_open = false;
        self.cmd_buf.clear();
        self.hint_stage = 0;
        self.recent.clear();
        self.seg_recent_mark = self.recent_total;
        self.screen = Screen::Playing;
        let sk = self.skill();
        let w = self.wins[self.skill_idx];
        self.set_status(
            format!(
                "Skill {} ({}): {} goals, par {} keys. Wins {}/{WINS_PER_SKILL}. Go!",
                sk.name,
                sk.keys,
                self.goals.len(),
                par,
                w.min(WINS_PER_SKILL),
            ),
            false,
        );
    }

    pub fn open_settings(&mut self) {
        self.settings_return = if self.screen == Screen::Settings {
            Screen::Menu
        } else {
            self.screen
        };
        self.screen = Screen::Settings;
        self.help_open = false;
        self.settings_scroll = 0;
        self.cmd_open = false;
        self.cmd_buf.clear();
    }

    pub fn toggle_help(&mut self) {
        if self.help_open {
            self.help_open = false;
        } else {
            if self.screen == Screen::Settings {
                self.screen = if self.settings_return == Screen::Settings {
                    Screen::Menu
                } else {
                    self.settings_return
                };
            }
            self.help_open = true;
        }
        self.settings_scroll = 0;
    }

    fn push_recent(&mut self, s: impl Into<String>) {
        if self.recent.len() >= 12 {
            self.recent.pop_front();
        }
        self.recent.push_back(s.into());
        self.recent_total += 1;
    }

    /// Labels pressed since the current hop started, oldest first.
    fn recent_since_mark(&self) -> Vec<String> {
        let keep = (self.recent_total - self.seg_recent_mark).min(self.recent.len() as u64) as usize;
        self.recent.iter().skip(self.recent.len() - keep).cloned().collect()
    }

    /// Staged over-par coach: nudge at +5, full answer at +10.
    fn check_over_par(&mut self) {
        let spent = self.keys_pressed - self.seg_start_keys;
        let seg_par = *self.seg_pars.get(self.goal_idx).unwrap_or(&1);
        if spent == seg_par + 5 && self.hint_stage == 0 {
            self.hint_stage = 1;
            self.coach_over_par(spent, seg_par, false);
        } else if spent == seg_par + 10 && self.hint_stage == 1 {
            self.hint_stage = 2;
            self.coach_over_par(spent, seg_par, true);
        }
    }

    /// The +5/+10 over-par coach: optimal open, your keys, and the why.
    fn coach_over_par(&mut self, spent: usize, seg_par: usize, full: bool) {
        let kit = self.kit();
        let goal = match self.current_goal() {
            Some(g) => g,
            None => return,
        };
        let path = path_for(&self.buf, self.cursor, goal, kit);
        let (best, why): (Vec<String>, &'static str) = match path {
            Some((_, steps)) => {
                let labels = steps.iter().map(|(m, n)| label(m, *n)).collect();
                let w = steps
                    .into_iter()
                    .next()
                    .map(|(m, n)| coach::why_for(&m, n))
                    .unwrap_or("");
                (labels, w)
            }
            None => return,
        };
        if best.is_empty() {
            return;
        }
        let you = self.recent_since_mark();
        let you_s = if you.is_empty() {
            "-".to_string()
        } else {
            you.join(" ")
        };
        let msg = if full {
            format!(
                "Over par ({spent} vs {seg_par}). Full answer: {}. {why}",
                best.join(" "),
            )
        } else {
            let open: Vec<String> = best.into_iter().take(3).collect();
            format!(
                "Over par ({spent} vs {seg_par}). Best: {}. You did: {you_s}. {why}",
                open.join(" then "),
            )
        };
        self.set_status(msg, false);
    }

    fn show_hint(&mut self) {
        match self.current_goal() {
            None => self.set_status("No goals left.", false),
            Some(g) => match hint_for(&self.buf, self.cursor, g, self.kit()) {
                Some(h) => self.set_status(format!("Hint: try `{h}`."), false),
                None => self.set_status("You are on the goal.", false),
            },
        }
    }

    fn finish_goal(&mut self) {
        if self.demo {
            self.goal_idx += 1;
            if self.goal_idx >= self.goals.len() {
                self.finish_demo();
            } else {
                self.set_status("Good — next one.", false);
            }
            return;
        }
        let seg_keys = self.keys_pressed - self.seg_start_keys;
        let seg_par = *self.seg_pars.get(self.goal_idx).unwrap_or(&1);
        if seg_keys <= seg_par {
            self.set_status(coach::praise(self.streak), false);
        } else {
            // Hint for the next hop, if there is one.
            let cur_goal = self.goals.get(self.goal_idx).copied();
            let next_hint = self
                .goals
                .get(self.goal_idx + 1)
                .and_then(|nx| cur_goal.and_then(|cur| hint_for(&self.buf, cur, *nx, self.kit())));
            self.set_status(coach::over_par(seg_keys, seg_par, next_hint.as_deref()), false);
        }
        self.goal_idx += 1;
        self.seg_start_keys = self.keys_pressed;
        self.seg_recent_mark = self.recent_total;
        self.hint_stage = 0;
        if self.goal_idx >= self.goals.len() {
            self.finish_round();
        }
    }

    fn finish_round(&mut self) {
        self.round_failed = false;
        let secs = self.elapsed_secs();
        let st = score::stars(self.keys_pressed, self.par);
        let pts = score::points(self.keys_pressed, self.par, secs, self.streak);
        self.last_stars = st;
        self.last_points = pts;
        self.last_secs = secs;
        self.total_score = self.total_score.saturating_add(pts);
        self.streak = self.streak.saturating_add(1);
        self.wins[self.skill_idx] = self.wins[self.skill_idx].saturating_add(1);
        self.save_progress();
        self.screen = Screen::RoundDone;
        let w = self.wins[self.skill_idx];
        let progress_msg = if w >= WINS_PER_SKILL {
            if self.skill_idx + 1 < SKILL_COUNT {
                let nx = &SKILLS[self.skill_idx + 1];
                format!(" Skill mastered! Next: skill {} ({}).", self.skill_idx + 2, nx.name)
            } else {
                " Every skill mastered. You move like water.".to_string()
            }
        } else {
            format!(" Wins {w}/{WINS_PER_SKILL}: one more to master this skill!")
        };
        self.set_status(format!("{}{}", coach::round_done(st), progress_msg), false);
    }

    /// Enter on the round-clear screen: next skill if unlocked,
    /// otherwise a fresh round of this skill.
    fn advance(&mut self) {
        if self.skill_idx + 1 < SKILL_COUNT && self.unlocked_skill(self.skill_idx + 1) {
            self.start_skill(self.skill_idx + 1);
        } else {
            self.begin_round();
        }
    }

    fn run_command(&mut self) {
        let raw = self.cmd_buf.trim().to_string();
        self.cmd_open = false;
        self.cmd_buf.clear();
        if raw.is_empty() {
            return;
        }
        let mut it = raw.split_whitespace();
        let cmd = it.next().unwrap_or("");
        let arg = it.next().unwrap_or("");
        match cmd {
            "q" | "quit" | "exit" => self.should_quit = true,
            "menu" => {
                self.screen = Screen::Menu;
                self.help_open = false;
            }
            "next" => {
                if self.skill_idx + 1 < SKILL_COUNT {
                    if self.unlocked_skill(self.skill_idx + 1) {
                        self.start_skill(self.skill_idx + 1);
                    } else {
                        let need = WINS_PER_SKILL - self.wins[self.skill_idx].min(WINS_PER_SKILL);
                        self.set_status(
                            format!("Next skill is locked: win {need} more round(s) here first."),
                            true,
                        );
                    }
                } else {
                    self.set_status("That was the final skill.", true);
                }
            }
            "prev" | "previous" => {
                if self.skill_idx > 0 {
                    self.start_skill(self.skill_idx - 1);
                } else {
                    self.set_status("Already on the first skill.", true);
                }
            }
            "skill" | "s" => match arg.parse::<usize>() {
                Ok(n) if (1..=SKILL_COUNT).contains(&n) => self.start_skill(n - 1),
                _ => self.set_status(format!("Usage: :skill 1-{SKILL_COUNT}"), true),
            },
            "lesson" | "l" => match arg.parse::<usize>() {
                Ok(n) if (1..=LESSONS.len()).contains(&n) => {
                    self.start_skill(self.resume_skill(n - 1))
                }
                _ => self.set_status("Usage: :lesson 1-6", true),
            },
            "restart" | "retry" | "again" => {
                if matches!(self.screen, Screen::Playing | Screen::RoundDone) {
                    self.begin_round();
                } else {
                    self.set_status("Nothing to restart here.", true);
                }
            }
            "settings" | "set" => self.open_settings(),
            cmd if cmd == "diff"
                || cmd.starts_with("diff ")
                || cmd == "difficulty"
                || cmd.starts_with("difficulty ") =>
            {
                let arg = cmd
                    .strip_prefix("difficulty")
                    .or_else(|| cmd.strip_prefix("diff"))
                    .unwrap_or("")
                    .trim();
                match arg {
                    "casual" | "1" => self.difficulty = Difficulty::Casual,
                    "normal" | "2" => self.difficulty = Difficulty::Normal,
                    "strict" | "3" => self.difficulty = Difficulty::Strict,
                    "" => self.difficulty = self.difficulty.next(),
                    _ => {
                        self.set_status("Usage: :diff [casual|normal|strict]", true);
                        return;
                    }
                }
                self.save_progress();
                self.set_status(
                    format!(
                        "Difficulty: {} ({})",
                        self.difficulty.name(),
                        self.difficulty.description()
                    ),
                    false,
                );
            }
            "help" | "h" => {
                self.help_open = true;
                self.set_status(
                    "Commands: :q, :next, :prev, :skill N, :diff [casual|normal|strict], :restart, :settings",
                    false,
                );
            }
            "reset" => {
                self.wins = [0; SKILL_COUNT];
                self.skill_idx = 0;
                self.screen = Screen::Menu;
                self.save_progress();
                self.set_status("Progress reset: back to skill 1.", false);
            }
            _ => self.set_status(format!("Unknown command `:{raw}`. Try :help."), true),
        }
    }

    // ------------------------------------------------------------------
    // input
    // ------------------------------------------------------------------

    pub fn cycle_difficulty(&mut self) {
        self.difficulty = self.difficulty.next();
        self.save_progress();
        self.set_status(
            format!(
                "Difficulty: {} ({})",
                self.difficulty.name(),
                self.difficulty.description()
            ),
            false,
        );
    }

    pub fn handle_mouse(&mut self, event: crossterm::event::MouseEvent) -> bool {
        use crossterm::event::{MouseButton, MouseEventKind};
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return false;
        }
        let (tw, _th) = crossterm::terminal::size().unwrap_or((80, 24));
        let tw = tw as usize;
        let x = event.column as usize;
        let y = event.row as usize;

        // Mouse clicking ONLY works on row 1 (the topbar), nowhere else.
        if y != 1 {
            return false;
        }

        // Mouse clicks ONLY respond when pressed directly on top of ACTUAL buttons:
        // 1. Logo button: "VIMTRAIN" at cols 2..10
        if (2..10).contains(&x) {
            self.screen = Screen::Menu;
            self.help_open = false;
            return true;
        }

        // 2. Right navigation pills: [● home] (8 cols)   [● settings] (12 cols)   [● help] (8 cols)
        let nav_len = 34;
        let rx = tw.saturating_sub(nav_len + 2);
        if (rx..rx + nav_len).contains(&x) {
            let rel = x - rx;
            if rel < 8 {
                // Click is on the [ home ] button (cols rx..rx+8)
                self.screen = Screen::Menu;
                self.help_open = false;
                return true;
            } else if (11..23).contains(&rel) {
                // Click is on the [ settings ] button (cols rx+11..rx+23)
                if self.screen == Screen::Settings && !self.help_open {
                    self.screen = Screen::Menu;
                } else {
                    self.open_settings();
                }
                return true;
            } else if (26..34).contains(&rel) {
                // Click is on the [ help ] button (cols rx+26..rx+34)
                self.toggle_help();
                return true;
            }
        }

        false
    }

    pub fn handle_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        // Global hard quit.
        if mods.contains(KeyModifiers::CONTROL) {
            if let KeyCode::Char('c') | KeyCode::Char('C') = code {
                self.should_quit = true;
                return;
            }
            if let KeyCode::Char('g') | KeyCode::Char('G') = code {
                if matches!(self.screen, Screen::Playing) && !self.cmd_open {
                    self.show_hint();
                }
                return;
            }
        }
        if code == KeyCode::F(1) {
            self.toggle_help();
            return;
        }
        if code == KeyCode::F(2) {
            if self.screen == Screen::Settings && !self.help_open {
                self.screen = Screen::Menu;
            } else {
                self.open_settings();
            }
            return;
        }
        if code == KeyCode::F(3) {
            if matches!(self.screen, Screen::Playing) && !self.cmd_open {
                self.show_hint();
            }
            return;
        }

        // Help overlay traps input (Esc / F(1) / F(2) / ? / q closes it, j/k scrolls, Tab/1-3 changes difficulty)
        if self.help_open {
            match code {
                KeyCode::Esc | KeyCode::F(1) | KeyCode::F(2) => {
                    self.help_open = false;
                    self.settings_scroll = 0;
                }
                KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Char('Q')
                    if !mods.contains(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    self.help_open = false;
                    self.settings_scroll = 0;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.settings_scroll = self.settings_scroll.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.settings_scroll = (self.settings_scroll + 1).min(50);
                }
                KeyCode::PageUp => {
                    self.settings_scroll = self.settings_scroll.saturating_sub(10);
                }
                KeyCode::PageDown => {
                    self.settings_scroll = (self.settings_scroll + 10).min(50);
                }
                KeyCode::Home => {
                    self.settings_scroll = 0;
                }
                KeyCode::Tab
                | KeyCode::Char('d')
                | KeyCode::Char('D')
                | KeyCode::Right
                | KeyCode::Char('l') => {
                    self.cycle_difficulty();
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    self.difficulty = self.difficulty.prev();
                    self.save_progress();
                    self.set_status(
                        format!(
                            "Difficulty: {} ({})",
                            self.difficulty.name(),
                            self.difficulty.description()
                        ),
                        false,
                    );
                }
                KeyCode::Char('1') => {
                    self.difficulty = Difficulty::Casual;
                    self.save_progress();
                    self.set_status("Difficulty: Casual (Unlimited moves)", false);
                }
                KeyCode::Char('2') => {
                    self.difficulty = Difficulty::Normal;
                    self.save_progress();
                    self.set_status("Difficulty: Normal (Progressive limit)", false);
                }
                KeyCode::Char('3') => {
                    self.difficulty = Difficulty::Strict;
                    self.save_progress();
                    self.set_status("Difficulty: Strict (Exact par only)", false);
                }
                _ => {}
            }
            return;
        }

        // Command-line input captures everything except Ctrl+C.
        if self.cmd_open {
            match code {
                KeyCode::Esc => {
                    self.cmd_open = false;
                    self.cmd_buf.clear();
                }
                KeyCode::Enter => self.run_command(),
                KeyCode::Backspace => {
                    self.cmd_buf.pop();
                }
                KeyCode::Char(c)
                    if !mods.contains(KeyModifiers::CONTROL | KeyModifiers::ALT)
                        && self.cmd_buf.len() < 48 =>
                {
                    self.cmd_buf.push(c);
                }
                _ => {}
            }
            return;
        }

        // Tab difficulty changer works globally across all screens!
        if code == KeyCode::Tab || code == KeyCode::BackTab {
            self.cycle_difficulty();
            return;
        }

        match self.screen {
            Screen::Menu => self.menu_key(code, mods),
            Screen::Intro => self.intro_key(code, mods),
            Screen::Playing => self.playing_key(code, mods),
            Screen::RoundDone => self.done_key(code, mods),
            Screen::Settings => self.settings_key(code, mods),
        }
    }

    fn menu_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        let n = self.menu_len();
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.menu_sel = self.menu_sel.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.menu_sel = (self.menu_sel + 1).min(n - 1);
            }
            KeyCode::Enter => self.menu_activate(),
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Char(c)
                if !mods.contains(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(d) = c.to_digit(10) {
                    let ch = (d as usize).saturating_sub(1);
                    if ch < LESSONS.len() {
                        self.start_skill(self.resume_skill(ch));
                    }
                } else if c == 's' || c == 'S' {
                    self.open_settings();
                } else if c == 'q' || c == 'Q' {
                    self.should_quit = true;
                }
            }
            _ => {}
        }
    }

    fn menu_activate(&mut self) {
        if self.menu_sel < LESSONS.len() {
            self.start_skill(self.resume_skill(self.menu_sel));
        }
    }

    fn intro_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        match code {
            KeyCode::Enter | KeyCode::Char(' ') => self.begin_round(),
            KeyCode::Esc => {
                self.screen = Screen::Menu;
                self.help_open = false;
            }
            KeyCode::Char(c)
                if !mods.contains(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if c == '?' {
                    self.toggle_help();
                } else if c == ':' {
                    self.cmd_open = true;
                    self.cmd_buf.clear();
                } else if c == 'q' || c == 'Q' {
                    self.screen = Screen::Menu;
                }
            }
            _ => {}
        }
    }

    fn playing_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        // Help overlay blocks play input (except closers).
        if self.help_open {
            match code {
                KeyCode::Esc | KeyCode::F(1) => self.help_open = false,
                KeyCode::Char('?')
                    if !mods.contains(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    self.help_open = false;
                }
                _ => {}
            }
            return;
        }

        match code {
            KeyCode::Esc => {
                if self.keys.is_pending() {
                    self.keys.clear();
                    self.set_status("Cleared.", false);
                } else {
                    self.screen = Screen::Menu;
                }
            }
            KeyCode::Enter => self.set_status("Reach every green goal.", false),
            KeyCode::Char(c)
                if !mods.contains(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if c == ':' {
                    self.cmd_open = true;
                    self.cmd_buf.clear();
                    return;
                }
                if c == '?' {
                    self.toggle_help();
                    return;
                }
                self.feed_motion_key(code, mods);
            }
            KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
                self.keys_pressed += 1;
                if self.check_move_limit() {
                    return;
                }
                if self.demo {
                    self.set_status(self.demo_hint(), false);
                    return;
                }
                self.misses += 1;
                self.push_recent("arrow");
                self.set_status("Arrow keys are disabled here — use h j k l.", true);
                self.check_over_par();
            }
            _ => {}
        }
    }

    fn feed_motion_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        let c = match code {
            KeyCode::Char(ch) => ch,
            _ => return,
        };
        match self.keys.feed(code, mods) {
            Feed::NeedMore => {
                self.keys_pressed += 1;
                if self.check_move_limit() {
                    return;
                }
                if self.status_err {
                    self.set_status("", false);
                }
            }
            Feed::Done(m, n) => {
                self.keys_pressed += 1;
                if self.check_move_limit() {
                    return;
                }
                let kit = self.kit();
                if !kit_allows(kit, &m) {
                    if !self.demo {
                        self.misses += 1;
                    }
                    let msg = match teacher_of(&m) {
                        Some(t) => format!(
                            "`{}` is taught in skill {} ({}). Finish this skill first!",
                            label(&m, n),
                            t + 1,
                            SKILLS[t].name
                        ),
                        None => format!("`{}` isn't available here.", label(&m, n)),
                    };
                    self.set_status(msg, true);
                    return;
                }
                if !kit_allows_count(kit, &m, n) {
                    if !self.demo {
                        self.misses += 1;
                    }
                    let msg = match counts_teacher() {
                        Some(t) => format!(
                            "Counts are taught in skill {} ({}).",
                            t + 1,
                            SKILLS[t].name
                        ),
                        None => "Counts aren't available here.".to_string(),
                    };
                    self.set_status(msg, true);
                    return;
                }
                let dest = apply(&self.buf, self.cursor, &m, n);
                if dest == self.cursor {
                    if !self.demo {
                        self.misses += 1;
                        self.set_status(format!("`{}` didn't move. Wall or no match.", label(&m, n)), true);
                        self.check_over_par();
                    } else {
                        self.set_status(self.demo_hint(), false);
                    }
                    return;
                }
                self.cursor = dest;
                if self.demo {
                    // Relaxed practice: re-show the keys, never coach or score.
                    if Some(dest) == self.current_goal() {
                        self.finish_goal();
                    } else {
                        self.set_status(self.demo_hint(), false);
                    }
                    return;
                }
                self.push_recent(label(&m, n));
                if self.status_err {
                    self.set_status("", false);
                }
                if Some(dest) == self.current_goal() {
                    self.finish_goal();
                    return;
                }
                self.check_over_par();
                let _ = c;
            }
            Feed::Cancelled => self.set_status("Cleared.", false),
            Feed::Invalid => {
                self.keys_pressed += 1;
                if self.check_move_limit() {
                    return;
                }
                if self.demo {
                    self.set_status(self.demo_hint(), false);
                    return;
                }
                self.misses += 1;
                self.push_recent(c.to_string());
                self.set_status(
                    format!("`{c}` is not a motion here. Esc = menu, ? = help."),
                    true,
                );
            }
        }
    }

    fn done_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        match code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                if self.round_failed {
                    self.begin_round();
                } else {
                    self.advance();
                }
            }
            KeyCode::Esc => self.screen = Screen::Menu,
            KeyCode::Tab => {
                self.difficulty = self.difficulty.next();
                self.save_progress();
                self.set_status(
                    format!(
                        "Difficulty: {} — {}",
                        self.difficulty.name(),
                        self.difficulty.description()
                    ),
                    false,
                );
            }
            KeyCode::Char(c)
                if !mods.contains(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                match c {
                    ':' => {
                        self.cmd_open = true;
                        self.cmd_buf.clear();
                    }
                    'd' | 'D' => {
                        self.difficulty = self.difficulty.next();
                        self.save_progress();
                        self.set_status(
                            format!(
                                "Difficulty: {} — {}",
                                self.difficulty.name(),
                                self.difficulty.description()
                            ),
                            false,
                        );
                    }
                    'n' | 'N' => {
                        if self.round_failed {
                            self.set_status(
                                "Clear this level first before advancing.",
                                true,
                            );
                        } else if self.skill_idx + 1 < SKILL_COUNT {
                            if self.unlocked_skill(self.skill_idx + 1) {
                                self.start_skill(self.skill_idx + 1);
                            } else {
                                let need = WINS_PER_SKILL - self.wins[self.skill_idx].min(WINS_PER_SKILL);
                                self.set_status(
                                    format!("Next skill is locked: win {need} more round(s) here first. Enter replays this one."),
                                    true,
                                );
                            }
                        } else {
                            self.screen = Screen::Menu;
                        }
                    }
                    'r' | 'R' => self.begin_round(),
                    'q' | 'Q' => self.should_quit = true,
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn settings_key(&mut self, code: KeyCode, _mods: KeyModifiers) {
        match code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.screen = self.settings_return;
            }
            KeyCode::Tab
            | KeyCode::Char('d')
            | KeyCode::Char('D')
            | KeyCode::Right
            | KeyCode::Char('l') => {
                self.difficulty = self.difficulty.next();
                self.save_progress();
                self.set_status(
                    format!(
                        "Difficulty: {} ({})",
                        self.difficulty.name(),
                        self.difficulty.description()
                    ),
                    false,
                );
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.difficulty = self.difficulty.prev();
                self.save_progress();
                self.set_status(
                    format!(
                        "Difficulty: {} ({})",
                        self.difficulty.name(),
                        self.difficulty.description()
                    ),
                    false,
                );
            }
            KeyCode::Char('1') => {
                self.difficulty = Difficulty::Casual;
                self.save_progress();
                self.set_status("Difficulty: Casual (Unlimited moves)", false);
            }
            KeyCode::Char('2') => {
                self.difficulty = Difficulty::Normal;
                self.save_progress();
                self.set_status("Difficulty: Normal (Progressive limit)", false);
            }
            KeyCode::Char('3') => {
                self.difficulty = Difficulty::Strict;
                self.save_progress();
                self.set_status("Difficulty: Strict (Exact par only)", false);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.settings_scroll = self.settings_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.settings_scroll = (self.settings_scroll + 1).min(50);
            }
            KeyCode::PageUp => {
                self.settings_scroll = self.settings_scroll.saturating_sub(8);
            }
            KeyCode::PageDown => {
                self.settings_scroll = (self.settings_scroll + 8).min(50);
            }
            KeyCode::Char(':') => {
                self.cmd_open = true;
                self.cmd_buf.clear();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_app() -> App {
        let mut a = App::new();
        a.persist = false; // never touch the real save file in tests
        a.wins = [0; SKILL_COUNT];
        a.buf = Buffer::new(vec!["abc".to_string(), "def".to_string()]);
        a.cursor = Pos::new(0, 0);
        a.goals = vec![Pos::new(0, 2)];
        a.goal_idx = 0;
        a.seg_pars = vec![2];
        a.par = 2;
        a.screen = Screen::Playing;
        a.round_start = Instant::now();
        a
    }

    fn key(c: char) -> (KeyCode, KeyModifiers) {
        (KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn reaches_goal_and_scores_par() {
        let mut a = fixed_app();
        let (c, m) = key('l');
        a.handle_key(c, m);
        assert_eq!(a.cursor, Pos::new(0, 1));
        assert_eq!(a.screen, Screen::Playing);
        let (c, m) = key('l');
        a.handle_key(c, m);
        assert_eq!(a.screen, Screen::RoundDone);
        assert_eq!(a.last_stars, 3); // 2 keys vs par 2
        assert!(a.total_score > 0);
    }

    #[test]
    fn untaught_motion_names_its_skill() {
        let mut a = fixed_app(); // skill 1: only h/l
        let (c, m) = key('w');
        a.handle_key(c, m);
        assert_eq!(a.cursor, Pos::new(0, 0));
        assert_eq!(a.misses, 1);
        assert!(a.status.contains("skill 4"), "got: {}", a.status);
    }

    #[test]
    fn arrows_disabled_but_counted() {
        let mut a = fixed_app();
        a.handle_key(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(a.cursor, Pos::new(0, 0));
        assert_eq!(a.misses, 1);
    }

    #[test]
    fn skill_2_locked_until_mastery() {
        let mut a = fixed_app();
        assert!(a.unlocked_skill(0));
        assert!(!a.unlocked_skill(1));
        a.start_skill(1);
        assert_eq!(a.screen, Screen::Menu); // blocked, stays in menu
        assert!(a.menu_msg.contains("locked"));
        a.wins[0] = WINS_PER_SKILL;
        assert!(a.unlocked_skill(1));
        a.start_skill(1);
        assert_eq!(a.screen, Screen::Intro);
    }

    #[test]
    fn enter_advances_when_next_unlocked() {
        let mut a = fixed_app();
        a.screen = Screen::RoundDone;
        a.wins[0] = WINS_PER_SKILL;
        a.handle_key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(a.screen, Screen::Intro);
        assert_eq!(a.skill_idx, 1);
    }

    #[test]
    fn enter_replays_when_next_locked() {
        let mut a = fixed_app();
        a.screen = Screen::RoundDone;
        a.wins[0] = 1; // not mastered yet
        a.handle_key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(a.screen, Screen::Playing);
        assert_eq!(a.skill_idx, 0);
    }

    #[test]
    fn r_replays_current_skill() {
        let mut a = fixed_app();
        a.screen = Screen::RoundDone;
        let (c, m) = key('r');
        a.handle_key(c, m);
        assert_eq!(a.screen, Screen::Playing);
        assert_eq!(a.skill_idx, 0);
    }

    #[test]
    fn resume_finds_first_unmastered_in_chapter() {
        let mut a = fixed_app();
        assert_eq!(a.resume_skill(0), 0);
        a.wins[0] = WINS_PER_SKILL;
        a.wins[1] = WINS_PER_SKILL;
        assert_eq!(a.resume_skill(0), 2);
    }

    #[test]
    fn demo_comes_first_and_forgives_misses() {
        let mut a = fixed_app();
        a.skill_idx = 3; // Word Forward: demo board ["aaa bbb ccc ddd"]
        a.begin_round();
        assert!(a.demo, "demo room first");
        assert_eq!(a.goals, vec![Pos::new(0, 12)]);
        // Wrong keys cost nothing in the demo room.
        let (c, m) = key('q');
        a.handle_key(c, m);
        assert_eq!(a.misses, 0);
        assert!(a.demo);
        // Copy the shown keys: w w w reaches the goal, then the challenge.
        for _ in 0..3 {
            let (c, m) = key('w');
            a.handle_key(c, m);
        }
        assert!(!a.demo, "demo cleared");
        assert!(a.demo_seen[3]);
        assert_eq!(a.screen, Screen::Playing);
        assert!(!a.goals.is_empty(), "challenge board loaded");
    }

    #[test]
    fn over_par_coach_names_best_and_yours() {
        let mut a = fixed_app();
        a.difficulty = Difficulty::Casual; // allow moves without failure
        a.skill_idx = 2; // full hjkl kit so the goal is reachable
        // Goal off the l-track so presses never finish the hop.
        a.goals = vec![Pos::new(1, 2)];
        a.seg_pars = vec![2];
        for _ in 0..7 {
            let (c, m) = key('l');
            a.handle_key(c, m);
        }
        assert_eq!(a.hint_stage, 1);
        assert!(a.status.contains("Over par (7 vs 2)"), "got: {}", a.status);
        assert!(a.status.contains("Best: j"), "got: {}", a.status);
        assert!(a.status.contains("You did: l l"), "got: {}", a.status);
        // Push to +10 for the full answer.
        for _ in 0..5 {
            let (c, m) = key('l');
            a.handle_key(c, m);
        }
        assert_eq!(a.hint_stage, 2);
        assert!(a.status.contains("Full answer: j"), "got: {}", a.status);
    }

    #[test]
    fn move_limit_fails_and_retries_same_level() {
        let mut a = fixed_app();
        a.difficulty = Difficulty::Strict;
        a.par = 2;
        a.skill_idx = 0;
        let orig_wins = a.wins[0];

        // Move 1: l
        let (c, m) = key('l');
        a.handle_key(c, m);
        assert_eq!(a.screen, Screen::Playing);
        assert!(!a.round_failed);

        // Move 2: wrong direction (h), reaches limit 2
        let (c, m) = key('h');
        a.handle_key(c, m);
        assert_eq!(a.screen, Screen::Playing);
        assert!(!a.round_failed);

        // Move 3: exceeds strict limit of 2!
        let (c, m) = key('h');
        a.handle_key(c, m);
        assert_eq!(a.screen, Screen::RoundDone);
        assert!(a.round_failed);
        assert_eq!(a.wins[0], orig_wins); // win not granted

        // Enter retries the same skill
        a.handle_key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(a.screen, Screen::Playing);
        assert_eq!(a.skill_idx, 0);
        assert!(!a.round_failed);
    }

    #[test]
    fn rapid_completion_and_advance_across_skills() {
        let mut a = App::new();
        a.persist = false;
        a.wins = [0; SKILL_COUNT];
        // Simulate completing skills fast in 0 seconds and advancing to the next level
        for i in 0..SKILL_COUNT {
            a.start_skill(i);
            a.begin_round();
            // Instantly clear round with 0 keys elapsed 0s
            a.keys_pressed = a.par;
            a.finish_round();
            assert_eq!(a.screen, Screen::RoundDone);
            // First win
            a.handle_key(KeyCode::Enter, KeyModifiers::NONE);
            // Second round to master
            if a.screen == Screen::Playing {
                a.keys_pressed = a.par;
                a.finish_round();
            }
            // Mastered: advance should go to next skill Intro
            a.handle_key(KeyCode::Enter, KeyModifiers::NONE);
        }
    }

    #[test]
    fn tab_cycles_difficulty_across_all_screens() {
        let mut a = App::new();
        a.persist = false;
        a.difficulty = Difficulty::Casual;

        // In Menu screen
        a.screen = Screen::Menu;
        a.handle_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(a.difficulty, Difficulty::Normal);
        a.handle_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(a.difficulty, Difficulty::Strict);
        a.handle_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(a.difficulty, Difficulty::Casual);

        // In Intro screen
        a.screen = Screen::Intro;
        a.handle_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(a.difficulty, Difficulty::Normal);

        // In Playing screen
        a.screen = Screen::Playing;
        a.handle_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(a.difficulty, Difficulty::Strict);

        // In RoundDone screen
        a.screen = Screen::RoundDone;
        a.handle_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(a.difficulty, Difficulty::Casual);
    }

    #[test]
    fn mouse_clicks_only_on_topbar_actual_buttons() {
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        let mut a = App::new();
        a.persist = false;
        a.difficulty = Difficulty::Casual;
        a.screen = Screen::Playing;

        // Mouse move events MUST return false (preventing UI flicker)
        let mouse_move = MouseEvent {
            kind: MouseEventKind::Moved,
            column: 20,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        assert!(!a.handle_mouse(mouse_move), "mouse move must return false");

        // Clicking row 1 on "VIMTRAIN" logo (col 4)
        let click_logo = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 4,
            row: 1,
            modifiers: KeyModifiers::NONE,
        };
        assert!(a.handle_mouse(click_logo));
        assert_eq!(a.screen, Screen::Menu);
        assert!(!a.help_open);

        // On 80-col terminal, nav_len = 34, rx = 80 - 36 = 44.
        // [ home ] is at 44..52, [ settings ] at 55..67, [ help ] at 70..78.

        // Clicking [ settings ] button (col 60)
        let click_settings = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 60,
            row: 1,
            modifiers: KeyModifiers::NONE,
        };
        assert!(a.handle_mouse(click_settings));
        assert_eq!(a.screen, Screen::Settings);
        assert!(!a.help_open);

        // Clicking [ help ] button (col 74)
        let click_help = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 74,
            row: 1,
            modifiers: KeyModifiers::NONE,
        };
        assert!(a.handle_mouse(click_help));
        assert!(a.help_open);
        assert_ne!(a.screen, Screen::Settings);

        // Clicking [ home ] button (col 48)
        let click_home = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 48,
            row: 1,
            modifiers: KeyModifiers::NONE,
        };
        assert!(a.handle_mouse(click_home));
        assert_eq!(a.screen, Screen::Menu);
        assert!(!a.help_open);

        // Clicking empty space on row 1 (col 30 or spacers) does NOTHING
        let click_empty_topbar = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 30,
            row: 1,
            modifiers: KeyModifiers::NONE,
        };
        assert!(!a.handle_mouse(click_empty_topbar));

        let click_spacer1 = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 53,
            row: 1,
            modifiers: KeyModifiers::NONE,
        };
        assert!(!a.handle_mouse(click_spacer1));

        let click_spacer2 = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 68,
            row: 1,
            modifiers: KeyModifiers::NONE,
        };
        assert!(!a.handle_mouse(click_spacer2));

        // Clicking row 2 (mode bar) does NOTHING - difficulty does NOT cycle!
        let click_row2 = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 15,
            row: 2,
            modifiers: KeyModifiers::NONE,
        };
        assert!(!a.handle_mouse(click_row2));
        assert_eq!(a.difficulty, Difficulty::Casual, "clicking outside topbar must not cycle difficulty");

        // Clicking body (row 10) does NOTHING
        let click_body = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 20,
            row: 10,
            modifiers: KeyModifiers::NONE,
        };
        assert!(!a.handle_mouse(click_body));
        assert_eq!(a.difficulty, Difficulty::Casual, "clicking body must do nothing");
    }

    #[test]
    fn mutually_exclusive_topbar_nav() {
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        let mut a = App::new();
        a.persist = false;

        let click_home = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 48,
            row: 1,
            modifiers: KeyModifiers::NONE,
        };
        let click_settings = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 60,
            row: 1,
            modifiers: KeyModifiers::NONE,
        };
        let click_help = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 74,
            row: 1,
            modifiers: KeyModifiers::NONE,
        };

        // Initially on Menu:
        assert_eq!(a.screen, Screen::Menu);
        assert!(!a.help_open);

        // Click settings: settings opens, help is false
        assert!(a.handle_mouse(click_settings));
        assert_eq!(a.screen, Screen::Settings);
        assert!(!a.help_open);

        // Click help: help opens, settings exits
        assert!(a.handle_mouse(click_help));
        assert!(a.help_open);
        assert_ne!(a.screen, Screen::Settings);

        // Click home: closes help, goes to Menu
        assert!(a.handle_mouse(click_home));
        assert_eq!(a.screen, Screen::Menu);
        assert!(!a.help_open);

        // Keyboard F(2) opens settings, F(1) switches to help
        a.handle_key(KeyCode::F(2), KeyModifiers::NONE);
        assert_eq!(a.screen, Screen::Settings);
        assert!(!a.help_open);

        a.handle_key(KeyCode::F(1), KeyModifiers::NONE);
        assert!(a.help_open);
        assert_ne!(a.screen, Screen::Settings);

        a.handle_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(!a.help_open);
    }
}

