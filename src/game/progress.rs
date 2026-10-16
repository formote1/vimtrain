//! Win-progress persistence: mastered skills survive restarts.
//!
//! Stored as tiny hand-rolled JSON so the game needs no serde dependency:
//! `{"v":2,"wins":[2,2,0,...],"diff":1}` under `~/.local/share/vimtrain/progress.json`.
//! Version-1 files (per-lesson wins) are treated as a fresh start.

use std::fs;
use std::path::PathBuf;

use super::curriculum::SKILL_COUNT;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Difficulty {
    Casual,
    #[default]
    Normal,
    Strict,
}

impl Difficulty {
    pub fn from_u8(val: u8) -> Self {
        match val {
            0 => Difficulty::Casual,
            2 => Difficulty::Strict,
            _ => Difficulty::Normal,
        }
    }

    pub fn to_u8(self) -> u8 {
        match self {
            Difficulty::Casual => 0,
            Difficulty::Normal => 1,
            Difficulty::Strict => 2,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Casual => "Casual",
            Difficulty::Normal => "Normal",
            Difficulty::Strict => "Strict",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Difficulty::Casual => "Unlimited moves, relaxed practice",
            Difficulty::Normal => "Progressive limit (+4 down to +0 over par)",
            Difficulty::Strict => "Exact par only, zero wasted moves",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Difficulty::Casual => Difficulty::Normal,
            Difficulty::Normal => Difficulty::Strict,
            Difficulty::Strict => Difficulty::Casual,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Difficulty::Casual => Difficulty::Strict,
            Difficulty::Normal => Difficulty::Casual,
            Difficulty::Strict => Difficulty::Normal,
        }
    }

    /// Calculate maximum allowed moves for a given skill and its computed par.
    pub fn max_moves(self, skill_idx: usize, par: usize) -> Option<usize> {
        match self {
            Difficulty::Casual => None,
            Difficulty::Strict => Some(par),
            Difficulty::Normal => {
                let buffer = if skill_idx < 4 {
                    4
                } else if skill_idx < 10 {
                    3
                } else if skill_idx < 17 {
                    2
                } else if skill_idx < 22 {
                    1
                } else {
                    0
                };
                Some(par + buffer)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Progress {
    pub wins: [u32; SKILL_COUNT],
    pub difficulty: Difficulty,
}

impl Progress {
    pub fn new() -> Self {
        Self {
            wins: [0; SKILL_COUNT],
            difficulty: Difficulty::Normal,
        }
    }

    pub fn load() -> Self {
        let text = fs::read_to_string(Self::path()).unwrap_or_default();
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Self {
        let mut p = Self::new();
        if !text.contains("\"v\":2") {
            return p; // unknown or legacy format: fresh start
        }
        // Minimal parse: integers inside the first [...] group.
        if let Some(s) = text.split('[').nth(1) {
            let inner = s.split(']').next().unwrap_or("");
            for (i, part) in inner.split(',').enumerate().take(SKILL_COUNT) {
                if let Ok(n) = part.trim().parse::<u32>() {
                    p.wins[i] = n.min(999);
                }
            }
        }
        if let Some(s) = text.split("\"diff\":").nth(1) {
            if let Some(first_char) = s.trim().chars().next() {
                if let Some(d) = first_char.to_digit(10) {
                    p.difficulty = Difficulty::from_u8(d as u8);
                }
            }
        }
        p
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let inner = self
            .wins
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let diff = self.difficulty.to_u8();
        let _ = fs::write(path, format!("{{\"v\":2,\"wins\":[{inner}],\"diff\":{diff}}}"));
    }

    pub fn path() -> PathBuf {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            if !xdg.is_empty() {
                return PathBuf::from(xdg).join("vimtrain").join("progress.json");
            }
        }
        let base = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(base)
            .join(".local")
            .join("share")
            .join("vimtrain")
            .join("progress.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_json_shape() {
        let mut wins = [0; SKILL_COUNT];
        wins[0] = 2;
        wins[1] = 2;
        wins[2] = 1;
        let p = Progress {
            wins,
            difficulty: Difficulty::Strict,
        };
        let inner = p
            .wins
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let text = format!("{{\"v\":2,\"wins\":[{inner}],\"diff\":2}}");
        assert_eq!(Progress::parse(&text), p);
    }

    #[test]
    fn garbage_and_legacy_give_zeros() {
        assert_eq!(Progress::parse("not json at all"), Progress::new());
        assert_eq!(Progress::parse("{\"wins\":[]}"), Progress::new());
        // v1 files (no "v":2 marker) start fresh.
        assert_eq!(Progress::parse("{\"wins\":[3,3,1,0,0,0]}"), Progress::new());
        let mut wins = [0; SKILL_COUNT];
        wins[0] = 2;
        wins[2] = 1;
        let text = format!(
            "{{\"v\":2,\"wins\":[{}]}}",
            wins.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(",")
        );
        let parsed = Progress::parse(&text);
        assert_eq!(parsed.wins[0..3], [2, 0, 1]);
        assert_eq!(parsed.difficulty, Difficulty::Normal);
    }

    #[test]
    fn difficulty_max_moves_calculation() {
        let par = 10;
        assert_eq!(Difficulty::Casual.max_moves(0, par), None);
        assert_eq!(Difficulty::Strict.max_moves(0, par), Some(10));
        // Progressive buffer:
        assert_eq!(Difficulty::Normal.max_moves(2, par), Some(14)); // skill 2 -> +4
        assert_eq!(Difficulty::Normal.max_moves(6, par), Some(13)); // skill 6 -> +3
        assert_eq!(Difficulty::Normal.max_moves(12, par), Some(12)); // skill 12 -> +2
        assert_eq!(Difficulty::Normal.max_moves(18, par), Some(11)); // skill 18 -> +1
        assert_eq!(Difficulty::Normal.max_moves(23, par), Some(10)); // skill 23 -> +0
    }
}
