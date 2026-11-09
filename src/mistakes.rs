//! The amateur layer: things a learning drummer gets wrong. Like feel, these are pure functions of
//! position, so a mistake happens in the same place on every pass.

use crate::pattern::{
    random, KICK, KICK_LEFT, SNARE, STREAM_DROP, STREAM_FLAM, STREAM_FLAM_LEAD, STREAM_SPIKE,
    STREAM_WEAK_TIMING, STREAM_WEAK_VELOCITY,
};

/// How early the last hit of a fill lands at full Rush
const RUSH_MS: f64 = 40.0;
const WEAK_LATE_MS: f64 = 12.0;
/// The most a weak kick loses, as a fraction of its velocity
const WEAK_QUIET: f32 = 0.4;
/// The kick's chance of missing at full Weak Foot, on top of Drops
const WEAK_DROP: f64 = 0.25;
const DROP_CHANCE: f64 = 0.15;
const FLAM_CHANCE: f64 = 0.15;
const FLAM_LEAD_MIN_MS: f64 = 12.0;
const FLAM_LEAD_MAX_MS: f64 = 30.0;
const FLAM_VELOCITY: f32 = 0.5;
/// Where every hit ends up at full Flat Velocity
const FLAT_LEVEL: f32 = 0.72;
const SPIKE_CHANCE: f64 = 0.06;
/// The earliest any of this pushes a hit, on top of `feel::MAX_OFFSET_MS`. The latest is less.
pub const MAX_EARLY_MS: f64 = RUSH_MS + FLAM_LEAD_MAX_MS;

/// Amounts after the Amateur macro has scaled them.
#[derive(Clone, Copy)]
pub struct Mistakes {
    pub rush: f32,
    pub weak_foot: f32,
    pub drops: f32,
    pub flams: f32,
    pub flat: f32,
}

#[cfg(test)]
impl Mistakes {
    pub const NONE: Self = Self {
        rush: 0.0,
        weak_foot: 0.0,
        drops: 0.0,
        flams: 0.0,
        flat: 0.0,
    };
}

pub fn dropped(seed: u64, bar: i64, step: i64, voice: usize, m: &Mistakes) -> bool {
    let weak = if voice == KICK || voice == KICK_LEFT {
        m.weak_foot as f64 * WEAK_DROP
    } else {
        0.0
    };
    random(seed, bar, step, voice, STREAM_DROP) < m.drops as f64 * DROP_CHANCE + weak
}

/// Offset from the grid in milliseconds. `fill_progress` is how far through the bar this step is
/// (0..1) when the bar is a fill: rushing builds up through it, then the next bar snaps back.
pub fn offset_ms(
    seed: u64,
    bar: i64,
    step: i64,
    voice: usize,
    fill_progress: Option<f64>,
    m: &Mistakes,
) -> f64 {
    let rush = fill_progress.unwrap_or(0.0) * m.rush as f64 * RUSH_MS;
    let late = if voice == KICK || voice == KICK_LEFT {
        random(seed, bar, step, voice, STREAM_WEAK_TIMING) * m.weak_foot as f64 * WEAK_LATE_MS
    } else {
        0.0
    };
    late - rush
}

pub fn velocity(seed: u64, bar: i64, step: i64, voice: usize, v: f32, m: &Mistakes) -> f32 {
    if random(seed, bar, step, voice, STREAM_SPIKE) < m.flat as f64 * SPIKE_CHANCE {
        return 1.0;
    }
    let flat = v + (FLAT_LEVEL - v) * m.flat;
    if voice == KICK || voice == KICK_LEFT {
        let r = random(seed, bar, step, voice, STREAM_WEAK_VELOCITY) as f32;
        flat * (1.0 - r * m.weak_foot * WEAK_QUIET)
    } else {
        flat
    }
}

/// How far ahead of the main hit a grace note lands and how loud it is, if this hit is flammed.
pub fn flam(
    seed: u64,
    bar: i64,
    step: i64,
    voice: usize,
    v: f32,
    m: &Mistakes,
) -> Option<(f64, f32)> {
    let drum = voice == KICK || voice == SNARE;
    if !drum || random(seed, bar, step, voice, STREAM_FLAM) >= m.flams as f64 * FLAM_CHANCE {
        return None;
    }
    let r = random(seed, bar, step, voice, STREAM_FLAM_LEAD);
    let lead = FLAM_LEAD_MIN_MS + r * (FLAM_LEAD_MAX_MS - FLAM_LEAD_MIN_MS);
    Some((lead, v * FLAM_VELOCITY))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::RIDE;

    const FULL: Mistakes = Mistakes {
        rush: 1.0,
        weak_foot: 1.0,
        drops: 1.0,
        flams: 1.0,
        flat: 1.0,
    };

    /// How often `f` is true over many positions.
    fn rate(f: impl Fn(i64, i64) -> bool) -> f64 {
        let hits = (0..2000)
            .flat_map(|bar| (0..16).map(move |step| (bar, step)))
            .filter(|&(bar, step)| f(bar, step))
            .count();
        hits as f64 / 32000.0
    }

    #[test]
    fn none_changes_nothing() {
        let m = Mistakes::NONE;
        for step in 0..1000 {
            for voice in 0..6 {
                assert!(!dropped(4, 1, step, voice, &m));
                assert_eq!(offset_ms(4, 1, step, voice, Some(0.9), &m), 0.0);
                assert_eq!(velocity(4, 1, step, voice, 0.83, &m), 0.83);
                assert_eq!(flam(4, 1, step, voice, 0.83, &m), None);
            }
        }
    }

    #[test]
    fn drops_follow_their_chance_and_the_weak_foot_misses_more() {
        let only_drops = Mistakes {
            drops: 1.0,
            ..Mistakes::NONE
        };
        let snare = rate(|b, s| dropped(4, b, s, SNARE, &only_drops));
        assert!((snare - DROP_CHANCE).abs() < 0.01, "{snare}");
        let foot = rate(|b, s| dropped(4, b, s, KICK, &FULL));
        assert!((foot - DROP_CHANCE - WEAK_DROP).abs() < 0.01, "{foot}");
        assert_eq!(
            rate(|b, s| dropped(
                4,
                b,
                s,
                SNARE,
                &Mistakes {
                    weak_foot: 1.0,
                    ..Mistakes::NONE
                }
            )),
            0.0
        );
    }

    #[test]
    fn rushing_builds_through_a_fill_and_only_a_fill() {
        let m = Mistakes {
            rush: 1.0,
            ..Mistakes::NONE
        };
        assert_eq!(offset_ms(4, 1, 8, SNARE, None, &m), 0.0);
        assert_eq!(offset_ms(4, 1, 0, SNARE, Some(0.0), &m), 0.0);
        assert_eq!(offset_ms(4, 1, 8, SNARE, Some(0.5), &m), -RUSH_MS / 2.0);
    }

    #[test]
    fn the_weak_foot_is_late_and_quiet_and_only_the_kick() {
        let m = Mistakes {
            weak_foot: 1.0,
            ..Mistakes::NONE
        };
        for step in 0..500 {
            let late = offset_ms(4, 1, step, KICK, None, &m);
            assert!((0.0..=WEAK_LATE_MS).contains(&late));
            assert_eq!(offset_ms(4, 1, step, SNARE, None, &m), 0.0);
            let v = velocity(4, 1, step, KICK, 0.9, &m);
            assert!((0.9 * (1.0 - WEAK_QUIET)..=0.9).contains(&v));
            assert_eq!(velocity(4, 1, step, SNARE, 0.9, &m), 0.9);
        }
    }

    #[test]
    fn flat_velocity_levels_everything_except_rare_spikes() {
        let m = Mistakes {
            flat: 1.0,
            ..Mistakes::NONE
        };
        let spikes = rate(|b, s| velocity(4, b, s, RIDE, 0.3, &m) == 1.0);
        assert!((spikes - SPIKE_CHANCE).abs() < 0.01, "{spikes}");
        let level = rate(|b, s| velocity(4, b, s, RIDE, 0.3, &m) == FLAT_LEVEL);
        assert!((level + spikes - 1.0).abs() < 1e-9);
    }

    #[test]
    fn flams_are_on_drums_with_a_quiet_grace_note_ahead() {
        let m = Mistakes {
            flams: 1.0,
            ..Mistakes::NONE
        };
        let got = rate(|b, s| flam(4, b, s, SNARE, 0.8, &m).is_some());
        assert!((got - FLAM_CHANCE).abs() < 0.01, "{got}");
        assert_eq!(rate(|b, s| flam(4, b, s, RIDE, 0.8, &m).is_some()), 0.0);
        for step in 0..500 {
            if let Some((lead, v)) = flam(4, 1, step, SNARE, 0.8, &m) {
                assert!((FLAM_LEAD_MIN_MS..=FLAM_LEAD_MAX_MS).contains(&lead));
                assert_eq!(v, 0.8 * FLAM_VELOCITY);
            }
        }
    }
}
