//! Timing and velocity humanization, also pure functions of position. Offsets are in milliseconds
//! because that's how a drummer's looseness behaves, whatever the tempo.

use crate::pattern::{random, STREAM_DRIFT, STREAM_TIMING, STREAM_VELOCITY};

const JITTER_MS: f64 = 15.0;
const JITTER_VELOCITY: f32 = 0.2;
const DRIFT_MS: f64 = 30.0;
/// The largest offset either way, which sets how far the scheduler has to look ahead
pub const MAX_OFFSET_MS: f64 = JITTER_MS + DRIFT_MS;
/// Keeps a jittered hit from reaching velocity 0, which MIDI treats as a note off
const MIN_VELOCITY: f32 = 0.01;

/// Amounts after the Human macro has scaled them.
#[derive(Clone, Copy)]
pub struct Feel {
    pub jitter: f32,
    pub drift: f32,
    /// In cycles per bar
    pub drift_rate: f32,
}

fn bipolar(r: f64) -> f64 {
    r * 2.0 - 1.0
}

/// The wobble shared by all limbs, -1..1: smooth noise, so it wanders like a warped tape but can
/// be read at any position without having walked there.
fn drift(seed: u64, pos_bars: f64, rate: f32) -> f64 {
    let x = pos_bars * rate as f64;
    let i = x.floor();
    let t = x - i;
    let at = |i: i64| bipolar(random(seed, i, 0, 0, STREAM_DRIFT));
    let (a, b) = (at(i as i64), at(i as i64 + 1));
    a + (b - a) * t * t * (3.0 - 2.0 * t)
}

/// Offset from the grid in milliseconds. `pos_bars` is the fractional position of the step.
pub fn offset_ms(seed: u64, bar: i64, step: i64, voice: usize, pos_bars: f64, f: &Feel) -> f64 {
    let jitter = bipolar(random(seed, bar, step, voice, STREAM_TIMING));
    jitter * f.jitter as f64 * JITTER_MS
        + drift(seed, pos_bars, f.drift_rate) * f.drift as f64 * DRIFT_MS
}

pub fn velocity(seed: u64, bar: i64, step: i64, voice: usize, base: f32, f: &Feel) -> f32 {
    let r = bipolar(random(seed, bar, step, voice, STREAM_VELOCITY)) as f32;
    (base + r * f.jitter * JITTER_VELOCITY).clamp(MIN_VELOCITY, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: Feel = Feel {
        jitter: 1.0,
        drift: 1.0,
        drift_rate: 1.0,
    };

    #[test]
    fn machine_is_dead_on() {
        let machine = Feel {
            jitter: 0.0,
            drift: 0.0,
            drift_rate: 1.0,
        };
        assert_eq!(offset_ms(5, 3, 2, 1, 3.1, &machine), 0.0);
        assert_eq!(velocity(5, 3, 2, 1, 0.7, &machine), 0.7);
    }

    #[test]
    fn offsets_stay_within_the_lookahead_and_velocity_in_range() {
        for step in 0..1000 {
            let pos = step as f64 / 16.0;
            let o = offset_ms(9, step / 16, step % 16, 1, pos, &FULL);
            assert!(o.abs() <= MAX_OFFSET_MS, "{o}");
            let v = velocity(9, step / 16, step % 16, 1, 1.0, &FULL);
            assert!((MIN_VELOCITY..=1.0).contains(&v));
        }
    }

    #[test]
    fn jitter_is_per_limb_and_drift_is_shared() {
        let jitter = Feel { drift: 0.0, ..FULL };
        assert_ne!(
            offset_ms(9, 1, 1, 0, 1.1, &jitter),
            offset_ms(9, 1, 1, 1, 1.1, &jitter)
        );
        let drift = Feel {
            jitter: 0.0,
            ..FULL
        };
        assert_eq!(
            offset_ms(9, 1, 1, 0, 1.1, &drift),
            offset_ms(9, 1, 1, 1, 1.1, &drift)
        );
    }

    #[test]
    fn drift_wanders_smoothly() {
        let mut seen = (0.0f64, 0.0f64);
        let mut prev = drift(3, 0.0, 1.0);
        for i in 1..1600 {
            let d = drift(3, i as f64 / 16.0 / 10.0, 1.0);
            // The steepest slope of the interpolation over 1/160 bar, between extremes
            assert!((d - prev).abs() < 2.0 * 1.5 / 160.0 + 1e-9);
            seen = (seen.0.min(d), seen.1.max(d));
            prev = d;
        }
        assert!(seen.0 < -0.3 && seen.1 > 0.3, "{seen:?}");
    }
}
