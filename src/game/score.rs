//! Scoring: stars and points from keystrokes vs par.

/// 3 stars at/under par, 2 within a small margin, else 1.
pub fn stars(keys: usize, par: usize) -> u8 {
    if keys <= par {
        3
    } else if keys <= par + 3 || keys * 2 <= par * 3 {
        2
    } else {
        1
    }
}

/// Points reward efficiency, speed and streaks.
pub fn points(keys: usize, par: usize, secs: u64, streak: u32) -> u32 {
    let base = (par as u32).saturating_mul(20);
    let over = (keys.saturating_sub(par) as u32).saturating_mul(15);
    let eff = base.saturating_sub(over);
    let speed = 60u64.saturating_sub(secs).min(60) as u32 * 2;
    eff + speed + streak.saturating_mul(5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_bands() {
        assert_eq!(stars(4, 4), 3);
        assert_eq!(stars(7, 4), 2);
        assert_eq!(stars(20, 4), 1);
    }
}
