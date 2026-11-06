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

const DRUM_VELOCITY: f32 = 0.9;
const CYMBAL_VELOCITY: f32 = 0.7;
const ACCENT_VELOCITY: f32 = 1.0;
/// Cymbals survive thinning longer than kick and snare, which keeps the pulse at low Intensity
const CYMBAL_THIN: f64 = 0.5;

#[derive(Clone, Copy)]
pub struct Pattern {
    pub seed: u64,
    pub blast: Blast,
    pub division: Division,
    pub cymbal: Cymbal,
    /// 0 turns the crash off
    pub crash_every: i64,
    pub intensity: f32,
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

/// The velocity each voice plays on this step, 0 for silent. Intensity only ever adds hits as it
/// rises: each hit has a fixed threshold, lower on stronger beats.
pub fn step_hits(bar: i64, step: i64, p: &Pattern) -> [f32; VOICES] {
    let importance = if step == 0 {
        1.0
    } else if step % p.division.steps_per_quarter() == 0 {
        0.5
    } else {
        0.0
    };
    let keep = |voice: usize, scale: f64| {
        let r = random(p.seed, bar, step, voice, STREAM_THIN);
        p.intensity as f64 >= r * (1.0 - importance) * scale
    };

    let mut hits = [0.0; VOICES];
    let (kick, snare) = match p.blast {
        Blast::Traditional => (step % 2 == 0, step % 2 == 1),
        Blast::Hammer => (true, true),
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
    if keep(cymbal, CYMBAL_THIN) {
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
        }
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
