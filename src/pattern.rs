//! The pattern as a pure function of (bar, step, params). There's no running RNG, so looping,
//! scrubbing and starting mid-song all give the same hits.

use crate::params::{Blast, Cymbal, Division};

pub const VOICES: usize = 6;
pub const KICK: usize = 0;
pub const SNARE: usize = 1;
pub const HAT: usize = 2;
pub const RIDE: usize = 3;
pub const CHINA: usize = 4;
pub const CRASH: usize = 5;

/// Independent random streams, so changing how one is used doesn't reshuffle the others.
pub const STREAM_THIN: u64 = 0;
pub const STREAM_TIMING: u64 = 1;
pub const STREAM_VELOCITY: u64 = 2;
pub const STREAM_DRIFT: u64 = 3;
const STREAM_SECTION: u64 = 4;
const STREAM_FILL: u64 = 5;
const STREAM_CHUG: u64 = 6;

const DRUM_VELOCITY: f32 = 0.9;
const CYMBAL_VELOCITY: f32 = 0.7;
const ACCENT_VELOCITY: f32 = 1.0;
/// Cymbals survive thinning longer than kick and snare, which keeps the pulse at low Intensity
const CYMBAL_THIN: f64 = 0.5;
/// Half-time and breakdown snares fall on beat 3 of this many quarter notes
const BACKBEAT_QUARTERS: i64 = 4;
/// Chance of a kick on each eighth note of a breakdown
const CHUG_DENSITY: f64 = 0.5;
/// Chance that a fill step is a kick rather than a snare
const FILL_KICK: f64 = 0.25;

#[derive(Clone, Copy)]
pub struct Pattern {
    pub seed: u64,
    pub blast: Blast,
    pub division: Division,
    pub cymbal: Cymbal,
    /// 0 turns the crash off
    pub crash_every: i64,
    pub intensity: f32,
    /// Bars in each section block, at least 1
    pub section_bars: i64,
    pub half_time: f32,
    pub breakdown: f32,
    pub fill: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Section {
    Blast,
    HalfTime,
    Breakdown,
    Fill,
}

/// The splitmix64 step.
pub fn mix(x: u64) -> u64 {
    let z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A uniform value in 0..1 for this position, limb and stream.
pub fn random(seed: u64, bar: i64, step: i64, voice: usize, stream: u64) -> f64 {
    let h = mix(mix(mix(seed) ^ bar as u64) ^ step as u64) ^ ((voice as u64) << 8 | stream);
    (mix(h) >> 11) as f64 / (1u64 << 53) as f64
}

/// The block of bars this bar belongs to.
fn block(bar: i64, p: &Pattern) -> i64 {
    bar.div_euclid(p.section_bars)
}

/// Each block draws its section on its own. A chain where each block depends on the last would
/// have to be replayed from bar 0, so a Fill replacing the last bar is the only sequencing.
pub fn section_at(bar: i64, p: &Pattern) -> Section {
    let b = block(bar, p);
    let last_bar = bar.rem_euclid(p.section_bars) == p.section_bars - 1;
    if last_bar && random(p.seed, b, 0, 1, STREAM_SECTION) < p.fill as f64 {
        return Section::Fill;
    }
    let (half_time, breakdown) = (p.half_time as f64, p.breakdown as f64);
    // Chances adding up past 100% share it in proportion, leaving no plain blast
    let r = random(p.seed, b, 0, 0, STREAM_SECTION) * (half_time + breakdown).max(1.0);
    if r < half_time {
        Section::HalfTime
    } else if r < half_time + breakdown {
        Section::Breakdown
    } else {
        Section::Blast
    }
}

/// The velocity each voice plays on this step, 0 for silent. Intensity only ever adds hits as it
/// rises: each hit has a fixed threshold, lower on stronger beats.
pub fn step_hits(bar: i64, step: i64, p: &Pattern) -> [f32; VOICES] {
    let spq = p.division.steps_per_quarter();
    let importance = if step == 0 {
        1.0
    } else if step % spq == 0 {
        0.5
    } else {
        0.0
    };
    let keep = |voice: usize, scale: f64| {
        let r = random(p.seed, bar, step, voice, STREAM_THIN);
        p.intensity as f64 >= r * (1.0 - importance) * scale
    };

    let mut hits = [0.0; VOICES];
    let backbeat = step % (BACKBEAT_QUARTERS * spq) == BACKBEAT_QUARTERS * spq / 2;
    let eighth = step % (spq / 2) == 0;
    let (kick, snare, cymbal_on) = match section_at(bar, p) {
        Section::Blast => match p.blast {
            Blast::Traditional => (step % 2 == 0, step % 2 == 1, true),
            Blast::Hammer => (true, true, true),
        },
        Section::HalfTime => (step % (BACKBEAT_QUARTERS * spq) == 0, backbeat, eighth),
        Section::Breakdown => {
            // Hashed by block, not bar, so the chug repeats until the section changes
            let chug = random(p.seed, block(bar, p), step, KICK, STREAM_CHUG) < CHUG_DENSITY;
            (step == 0 || (eighth && chug), backbeat, step % spq == 0)
        }
        Section::Fill => {
            let kick = random(p.seed, bar, step, KICK, STREAM_FILL) < FILL_KICK;
            (kick, !kick, step == 0)
        }
    };
    if kick && keep(KICK, 1.0) {
        hits[KICK] = DRUM_VELOCITY;
    }
    if snare && keep(SNARE, 1.0) {
        hits[SNARE] = DRUM_VELOCITY;
    }

    let cymbal = match p.cymbal {
        Cymbal::Hat => HAT,
        Cymbal::Ride => RIDE,
        Cymbal::China => CHINA,
    };
    if cymbal_on && keep(cymbal, CYMBAL_THIN) {
        hits[cymbal] = if step == 0 {
            ACCENT_VELOCITY
        } else {
            CYMBAL_VELOCITY
        };
    }

    if step == 0 && p.crash_every > 0 && bar.rem_euclid(p.crash_every) == 0 {
        hits[CRASH] = ACCENT_VELOCITY;
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pat() -> Pattern {
        Pattern {
            seed: 7,
            blast: Blast::Traditional,
            division: Division::Sixteenth,
            cymbal: Cymbal::Ride,
            crash_every: 4,
            intensity: 1.0,
            section_bars: 4,
            half_time: 0.0,
            breakdown: 0.0,
            fill: 0.0,
        }
    }

    /// Steps of a voice that play in a 4/4 bar.
    fn steps_with(section: Pattern, bar: i64, voice: usize) -> Vec<i64> {
        (0..16)
            .filter(|&s| step_hits(bar, s, &section)[voice] > 0.0)
            .collect()
    }

    #[test]
    fn repeated_and_out_of_order_calls_match() {
        let p = Pattern {
            intensity: 0.4,
            ..pat()
        };
        let forward: Vec<_> = (0..64).map(|s| step_hits(3, s, &p)).collect();
        for s in (0..64).rev() {
            assert_eq!(step_hits(3, s, &p), forward[s as usize]);
        }
    }

    #[test]
    fn traditional_alternates_under_a_cymbal_on_every_step() {
        for s in 0..16 {
            let h = step_hits(1, s, &pat());
            assert_eq!(h[KICK] > 0.0, s % 2 == 0, "step {s}");
            assert_eq!(h[SNARE] > 0.0, s % 2 == 1, "step {s}");
            assert!(h[RIDE] > 0.0 && h[HAT] == 0.0 && h[CHINA] == 0.0);
        }
    }

    #[test]
    fn hammer_hits_kick_and_snare_together() {
        let p = Pattern {
            blast: Blast::Hammer,
            ..pat()
        };
        for s in 0..16 {
            let h = step_hits(1, s, &p);
            assert!(h[KICK] > 0.0 && h[SNARE] > 0.0);
        }
    }

    #[test]
    fn beat_one_is_accented_and_crash_follows_its_period() {
        assert!(step_hits(1, 0, &pat())[RIDE] > step_hits(1, 1, &pat())[RIDE]);
        for bar in -8..8 {
            assert_eq!(step_hits(bar, 0, &pat())[CRASH] > 0.0, bar % 4 == 0);
            assert_eq!(step_hits(bar, 1, &pat())[CRASH], 0.0);
        }
        let off = Pattern {
            crash_every: 0,
            ..pat()
        };
        assert_eq!(step_hits(0, 0, &off)[CRASH], 0.0);
    }

    #[test]
    fn intensity_goes_from_sparse_to_full_and_only_adds_hits() {
        let count = |intensity: f32, seed: u64| -> usize {
            let p = Pattern {
                intensity,
                seed,
                ..pat()
            };
            (0..16)
                .flat_map(|s| step_hits(0, s, &p))
                .filter(|&v| v > 0.0)
                .count()
        };
        // Bar 0, step 0 keeps kick, ride and crash even at zero
        assert_eq!(count(0.0, 7), 3);
        assert_eq!(count(1.0, 7), 33);
        let mut prev = [0.0; VOICES];
        for i in 0..=10 {
            let p = Pattern {
                intensity: i as f32 / 10.0,
                ..pat()
            };
            let h = step_hits(0, 5, &p);
            for v in 0..VOICES {
                assert!(h[v] >= prev[v]);
            }
            prev = h;
        }
    }

    #[test]
    fn zero_chances_are_always_blast() {
        for bar in -100..100 {
            assert_eq!(section_at(bar, &pat()), Section::Blast);
        }
    }

    #[test]
    fn sections_hold_for_a_block_and_follow_their_chances() {
        let p = Pattern {
            half_time: 0.3,
            breakdown: 0.2,
            ..pat()
        };
        let mut counts = [0; 3];
        for b in -500..500 {
            let first = section_at(b * 4, &p);
            for bar in 1..4 {
                assert_eq!(section_at(b * 4 + bar, &p), first);
            }
            match first {
                Section::HalfTime => counts[0] += 1,
                Section::Breakdown => counts[1] += 1,
                _ => counts[2] += 1,
            }
        }
        for (count, expected) in counts.iter().zip([300, 200, 500]) {
            assert!((count - expected as i32).abs() < 60, "{counts:?}");
        }
        // Past 100% there's no blast left
        let over = Pattern {
            half_time: 1.0,
            breakdown: 1.0,
            ..pat()
        };
        let drawn: Vec<_> = (0..200).map(|b| section_at(b * 4, &over)).collect();
        assert!(drawn.contains(&Section::HalfTime) && drawn.contains(&Section::Breakdown));
        assert!(!drawn.contains(&Section::Blast));
    }

    #[test]
    fn fills_only_replace_the_last_bar_of_a_block() {
        let p = Pattern { fill: 1.0, ..pat() };
        for bar in -40..40 {
            let fill = section_at(bar, &p) == Section::Fill;
            assert_eq!(fill, bar.rem_euclid(4) == 3, "bar {bar}");
        }
        let one_bar = Pattern {
            section_bars: 1,
            ..p
        };
        assert_eq!(section_at(5, &one_bar), Section::Fill);
    }

    #[test]
    fn half_time_has_a_backbeat_on_three() {
        let p = Pattern {
            half_time: 1.0,
            ..pat()
        };
        assert_eq!(steps_with(p, 1, KICK), [0]);
        assert_eq!(steps_with(p, 1, SNARE), [8]);
        assert_eq!(steps_with(p, 1, RIDE), [0, 2, 4, 6, 8, 10, 12, 14]);
    }

    #[test]
    fn breakdown_chugs_the_same_through_its_block() {
        let p = Pattern {
            breakdown: 1.0,
            ..pat()
        };
        assert_eq!(steps_with(p, 1, SNARE), [8]);
        assert_eq!(steps_with(p, 1, RIDE), [0, 4, 8, 12]);
        let kicks = steps_with(p, 1, KICK);
        assert_eq!(kicks, steps_with(p, 2, KICK));
        assert!(kicks.contains(&0) && kicks.iter().all(|s| s % 2 == 0));
        assert_ne!(kicks, steps_with(p, 5, KICK));
    }

    #[test]
    fn fill_plays_one_drum_per_step_with_no_cymbal_after_the_first() {
        let p = Pattern { fill: 1.0, ..pat() };
        assert_eq!(steps_with(p, 3, RIDE), [0]);
        let kicks = steps_with(p, 3, KICK);
        let snares = steps_with(p, 3, SNARE);
        assert_eq!(kicks.len() + snares.len(), 16);
        assert!(!kicks.is_empty() && !snares.is_empty());
        assert!(kicks.iter().all(|s| !snares.contains(s)));
    }

    #[test]
    fn seed_changes_the_thinning() {
        let at = |seed| {
            let p = Pattern {
                seed,
                intensity: 0.5,
                ..pat()
            };
            (0..64).map(|s| step_hits(2, s, &p)).collect::<Vec<_>>()
        };
        assert_ne!(at(1), at(2));
    }
}
