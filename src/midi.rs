//! Turns the pure pattern into note events for one process block.
//!
//! Each block looks for hits whose grid position is within the largest feel offset of the block,
//! and emits those whose offset time lands inside it. Nothing is queued, so a hit pushed early is
//! simply found before its grid position arrives. The state here only tracks sounding notes and
//! transport continuity; it never affects which hits exist.

use crate::feel::{self, Feel, MAX_OFFSET_MS};
use crate::mistakes;
use crate::pattern::{section_at, step_hits, Pattern, Section, VOICES};
use nih_plug::prelude::*;

/// Length of each note. The Drum Machine plays one-shots, so this only has to be short and non-zero.
const GATE_MS: f64 = 30.0;
/// shortcut: hits past this in one block are dropped, which takes about a 1.4 s block at 300 BPM.
/// Note ons and offs together also stay under nih-plug's preallocated 512 output events.
const MAX_HITS: usize = 240;

/// The host transport for one block. Positions are in quarter notes.
/// shortcut: assumes the position runs straight through a block, so a loop wrap inside one plays
/// past the loop end until the next block. Read `Transport::loop_range_beats` if a host does this.
#[derive(Clone, Copy)]
pub struct Clock {
    pub pos: f64,
    pub q_per_sample: f64,
    pub q_per_ms: f64,
    /// A nearby bar line and its number, which anchor the grid
    pub bar: i64,
    pub bar_start: f64,
    pub bar_len: f64,
}

#[derive(Clone, Copy, Default)]
struct Hit {
    sample: u32,
    voice: u8,
    velocity: f32,
}

#[derive(Default)]
pub struct Scheduler {
    /// Where the previous block ended, `None` after a stop
    last_end: Option<f64>,
    /// Note number and the sample of its note off, relative to the current block
    sounding: [Option<(u8, i64)>; VOICES],
}

impl Scheduler {
    /// `clock` is `None` while the transport is stopped, which still lets sounding notes finish.
    pub fn process(
        &mut self,
        clock: Option<Clock>,
        samples: usize,
        pattern: &Pattern,
        feel: &Feel,
        notes: &[u8; VOICES],
        mut emit: impl FnMut(NoteEvent<()>),
    ) {
        if samples == 0 {
            return;
        }
        let mut hits = [Hit::default(); MAX_HITS];
        let mut count = 0;
        let mut gate = 0;
        match clock {
            Some(c) => {
                let end = c.pos + samples as f64 * c.q_per_sample;
                // Starting from the previous end, not the reported position, means a hit on a block
                // boundary can't be played twice or missed through rounding
                let (start, jumped) = match self.last_end {
                    Some(e) if (c.pos - e).abs() < 1.5 * c.q_per_sample => (e, false),
                    _ => (c.pos, true),
                };
                self.last_end = Some(end);
                gate = (GATE_MS * c.q_per_ms / c.q_per_sample) as i64;
                scan(
                    &c,
                    start,
                    end,
                    jumped,
                    pattern,
                    feel,
                    |t, voice, velocity| {
                        if count < MAX_HITS {
                            let sample = ((t - c.pos) / c.q_per_sample).round();
                            hits[count] = Hit {
                                sample: sample.clamp(0.0, samples as f64 - 1.0) as u32,
                                voice: voice as u8,
                                velocity,
                            };
                            count += 1;
                        }
                    },
                );
            }
            None => self.last_end = None,
        }

        // Hosts need output events in time order
        let hits = &mut hits[..count];
        hits.sort_unstable_by_key(|h| (h.sample, h.voice));
        for h in hits.iter() {
            let (at, voice) = (h.sample as i64, h.voice as usize);
            self.note_offs_until(at + 1, &mut emit);
            // Retriggering a voice that's still sounding
            if let Some((note, _)) = self.sounding[voice].take() {
                emit(note_off(at, note));
            }
            let note = notes[voice];
            emit(NoteEvent::NoteOn {
                timing: at as u32,
                voice_id: None,
                channel: 0,
                note,
                velocity: h.velocity,
            });
            self.sounding[voice] = Some((note, at + gate));
        }
        self.note_offs_until(samples as i64, &mut emit);
        for (_, off) in self.sounding.iter_mut().flatten() {
            *off -= samples as i64;
        }
    }

    /// Emits the note offs due before `until`, in time order.
    fn note_offs_until(&mut self, until: i64, emit: &mut impl FnMut(NoteEvent<()>)) {
        while let Some(voice) = (0..VOICES)
            .filter(|&v| self.sounding[v].is_some_and(|(_, off)| off < until))
            .min_by_key(|&v| self.sounding[v].map(|(_, off)| off))
        {
            if let Some((note, off)) = self.sounding[voice].take() {
                emit(note_off(off.max(0), note));
            }
        }
    }
}

fn note_off(at: i64, note: u8) -> NoteEvent<()> {
    NoteEvent::NoteOff {
        timing: at as u32,
        voice_id: None,
        channel: 0,
        note,
        velocity: 0.0,
    }
}

/// Calls `f(time, voice, velocity)` for every hit due in `start..end`.
fn scan(
    c: &Clock,
    start: f64,
    end: f64,
    jumped: bool,
    p: &Pattern,
    feel: &Feel,
    mut f: impl FnMut(f64, usize, f32),
) {
    let margin = (MAX_OFFSET_MS + mistakes::MAX_EARLY_MS) * c.q_per_ms;
    let (lo, hi) = (start - margin, end + margin);
    let spq = p.division.steps_per_quarter();
    // Ignores float noise from an exact fit; an odd remainder gets one short last step
    let steps = (c.bar_len * spq as f64 - 1e-9).ceil() as i64;

    let mut k = ((lo - c.bar_start) / c.bar_len).floor() as i64;
    loop {
        let bar_start = c.bar_start + k as f64 * c.bar_len;
        if bar_start >= hi {
            break;
        }
        let bar = c.bar + k;
        for step in 0..steps {
            let beats = step as f64 / spq as f64;
            let grid = bar_start + beats;
            if grid < lo {
                continue;
            }
            if grid >= hi {
                break;
            }
            let pos_bars = bar as f64 + beats / c.bar_len;
            let fill_progress = (section_at(bar, p) == Section::Fill).then_some(beats / c.bar_len);
            let m = &feel.mistakes;
            for (voice, &base) in step_hits(bar, step, p).iter().enumerate() {
                if base == 0.0 || mistakes::dropped(p.seed, bar, step, voice, m) {
                    continue;
                }
                let offset = feel::offset_ms(p.seed, bar, step, voice, pos_bars, feel)
                    + mistakes::offset_ms(p.seed, bar, step, voice, fill_progress, m);
                let t = grid + offset * c.q_per_ms;
                let velocity = feel::velocity(p.seed, bar, step, voice, base, feel);
                let velocity = mistakes::velocity(p.seed, bar, step, voice, velocity, m);
                // After a jump, a hit pushed from just ahead of the new position to before it would
                // never play, so beat one could vanish on pressing play. Play it right away instead.
                if (t >= start && t < end) || (jumped && grid >= start && t < start) {
                    f(t, voice, velocity);
                }
                if let Some((lead, grace)) = mistakes::flam(p.seed, bar, step, voice, velocity, m) {
                    let t = t - lead * c.q_per_ms;
                    if t >= start && t < end {
                        f(t, voice, grace);
                    }
                }
            }
        }
        k += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mistakes::Mistakes;
    use crate::params::{Blast, Cymbal, Division};

    const FS: f64 = 48000.0;
    const BPM: f64 = 180.0;
    const NOTES: [u8; VOICES] = [36, 38, 42, 51, 52, 49];
    /// 4/4, in quarter notes
    const BAR: f64 = 4.0;

    fn pat() -> Pattern {
        Pattern {
            seed: 11,
            blast: Blast::Traditional,
            division: Division::Sixteenth,
            cymbal: Cymbal::Ride,
            crash_every: 4,
            intensity: 0.8,
            section_bars: 4,
            half_time: 0.0,
            breakdown: 0.0,
            fill: 0.0,
        }
    }

    const HUMAN: Feel = Feel {
        jitter: 1.0,
        drift: 1.0,
        drift_rate: 0.5,
        mistakes: Mistakes::NONE,
    };
    const SLOPPY: Feel = Feel {
        mistakes: Mistakes {
            rush: 1.0,
            weak_foot: 1.0,
            drops: 1.0,
            flams: 1.0,
            flat: 1.0,
        },
        ..HUMAN
    };
    const MACHINE: Feel = Feel {
        jitter: 0.0,
        drift: 0.0,
        ..HUMAN
    };

    fn sections() -> Pattern {
        Pattern {
            half_time: 0.3,
            breakdown: 0.3,
            fill: 0.5,
            ..pat()
        }
    }

    fn clock_at(sample: i64) -> Clock {
        let q_per_sample = BPM / 60.0 / FS;
        let pos = sample as f64 * q_per_sample;
        let bar = (pos / BAR).floor();
        Clock {
            pos,
            q_per_sample,
            q_per_ms: BPM / 60.0 / 1000.0,
            bar: bar as i64,
            bar_start: bar * BAR,
            bar_len: BAR,
        }
    }

    /// (absolute sample, note, velocity) of every note on in `from..to`, played in `block`s.
    /// Also checks that each block's events are in order and in range.
    fn play(
        s: &mut Scheduler,
        from: i64,
        to: i64,
        block: usize,
        p: &Pattern,
        f: &Feel,
    ) -> Vec<(i64, u8, f32)> {
        let mut out = vec![];
        let mut at = from;
        while at < to {
            let mut last = 0;
            s.process(Some(clock_at(at)), block, p, f, &NOTES, |e| {
                let timing = e.timing();
                assert!(timing >= last && (timing as usize) < block);
                last = timing;
                if let NoteEvent::NoteOn { note, velocity, .. } = e {
                    out.push((at + timing as i64, note, velocity));
                }
            });
            at += block as i64;
        }
        out
    }

    /// Note ons match if they play the same note at the same velocity, within a sample of rounding.
    fn assert_same(a: &[(i64, u8, f32)], b: &[(i64, u8, f32)]) {
        assert!(!a.is_empty());
        assert_eq!(a.len(), b.len());
        // Simultaneous hits can come out in a different order around rounding
        let sorted = |x: &[(i64, u8, f32)]| {
            let mut x = x.to_vec();
            x.sort_by_key(|e| (e.1, e.0));
            x
        };
        for (x, y) in sorted(a).iter().zip(&sorted(b)) {
            assert!((x.0 - y.0).abs() <= 1, "{x:?} vs {y:?}");
            assert_eq!((x.1, x.2), (y.1, y.2));
        }
    }

    const ONE_BAR: i64 = (BAR / (BPM / 60.0) * FS) as i64;

    #[test]
    fn block_size_does_not_change_the_output() {
        let end = ONE_BAR * 8;
        for (p, f) in [(pat(), HUMAN), (sections(), SLOPPY)] {
            let a = play(&mut Scheduler::default(), 0, end, 64, &p, &f);
            for block in [128, 441, 1024, 4096] {
                let b = play(&mut Scheduler::default(), 0, end, block, &p, &f);
                // The bigger blocks run slightly past the end
                let b: Vec<_> = b.into_iter().filter(|e| e.0 < end).collect();
                let a: Vec<_> = a.iter().copied().filter(|e| e.0 < end).collect();
                assert_same(&a, &b);
            }
        }
    }

    #[test]
    fn starting_mid_bar_matches_playing_through() {
        let from = ONE_BAR * 5 + ONE_BAR / 2 + 123;
        let to = ONE_BAR * 9;
        for (p, f) in [(pat(), HUMAN), (sections(), HUMAN), (sections(), SLOPPY)] {
            let whole = play(&mut Scheduler::default(), 0, to, 256, &p, &f);
            let mid = play(&mut Scheduler::default(), from, to, 256, &p, &f);
            // Past the first moments, where a hit pushed before the start plays right away instead
            let max_ms = MAX_OFFSET_MS + mistakes::MAX_EARLY_MS;
            let settled = from + (max_ms * FS / 1000.0) as i64 + 1;
            let tail = |x: &[(i64, u8, f32)]| -> Vec<_> {
                x.iter().copied().filter(|e| e.0 >= settled).collect()
            };
            assert_same(&tail(&whole), &tail(&mid));
        }
    }

    #[test]
    fn looping_repeats_exactly_and_holds_up_across_a_jump() {
        let mut s = Scheduler::default();
        let first = play(&mut s, ONE_BAR * 2, ONE_BAR * 4, 256, &pat(), &HUMAN);
        let second = play(&mut s, ONE_BAR * 2, ONE_BAR * 4, 256, &pat(), &HUMAN);
        assert_eq!(first, second);
    }

    #[test]
    fn hits_early_at_the_start_position_still_play() {
        // Half of these seeds push the first kick and crash early; neither may be lost. The window
        // outlasts the latest push, and the next kick is two steps away.
        let window = (MAX_OFFSET_MS * FS / 1000.0) as i64 + 256;
        for seed in 0..50 {
            let p = Pattern { seed, ..pat() };
            let on = play(&mut Scheduler::default(), 0, window, 256, &p, &HUMAN);
            for note in [NOTES[0], NOTES[5]] {
                let n = on.iter().filter(|e| e.1 == note).count();
                assert_eq!(n, 1, "seed {seed}, note {note}");
            }
        }
    }

    #[test]
    fn every_hit_plays_exactly_once() {
        let p = Pattern {
            intensity: 1.0,
            ..pat()
        };
        let on = play(&mut Scheduler::default(), 0, ONE_BAR * 4, 333, &p, &HUMAN);
        // Sixteenths for four bars: one drum and a ride each step, and the crash on bar 0
        // (a last hit pushed past the end, and bar 4's pushed before it, may come and go)
        let expected = 16 * 4 * 2 + 1;
        let near_end = ONE_BAR * 4 - (MAX_OFFSET_MS * FS / 1000.0) as i64;
        let settled = on.iter().filter(|e| e.0 < near_end).count();
        assert!((expected - 6..=expected).contains(&settled), "{settled}");
        // Machine feel has no pushes, so the count is exact
        let on = play(&mut Scheduler::default(), 0, ONE_BAR * 4, 333, &p, &MACHINE);
        assert_eq!(on.iter().filter(|e| e.0 < ONE_BAR * 4).count(), expected);
    }

    #[test]
    fn notes_all_end_after_stopping() {
        let mut s = Scheduler::default();
        let (mut on, mut off) = (0, 0);
        let mut count = |e: NoteEvent<()>| match e {
            NoteEvent::NoteOn { .. } => on += 1,
            NoteEvent::NoteOff { .. } => off += 1,
            _ => {}
        };
        for i in 0..200 {
            s.process(
                Some(clock_at(i * 512)),
                512,
                &pat(),
                &HUMAN,
                &NOTES,
                &mut count,
            );
        }
        // Enough blocks to outlast the gate
        for _ in 0..10 {
            s.process(None, 512, &pat(), &HUMAN, &NOTES, &mut count);
        }
        assert!(on > 0);
        assert_eq!(on, off);
    }

    #[test]
    fn flams_add_a_quiet_hit_ahead_of_the_main_one() {
        let flams = Feel {
            mistakes: Mistakes {
                flams: 1.0,
                ..Mistakes::NONE
            },
            ..MACHINE
        };
        let p = Pattern {
            intensity: 1.0,
            crash_every: 0,
            ..pat()
        };
        let on = play(&mut Scheduler::default(), 0, ONE_BAR * 8, 256, &p, &flams);
        let dead_on = play(&mut Scheduler::default(), 0, ONE_BAR * 8, 256, &p, &MACHINE);
        assert!(on.len() > dead_on.len());
        // Every main hit is still at its machine time, and each extra one is a quieter grace note
        let is_main =
            |e: &&(i64, u8, f32)| dead_on.iter().any(|h| h.1 == e.1 && (e.0 - h.0).abs() <= 1);
        let graces: Vec<_> = on.iter().filter(|e| !is_main(e)).collect();
        assert_eq!(graces.len(), on.len() - dead_on.len());
        assert!(graces.iter().all(|e| e.2 < 0.5));
    }

    #[test]
    fn rushed_fills_land_early_and_the_next_bar_snaps_back() {
        let p = Pattern {
            fill: 1.0,
            intensity: 1.0,
            crash_every: 0,
            ..pat()
        };
        let rushed = Feel {
            mistakes: Mistakes {
                rush: 1.0,
                ..Mistakes::NONE
            },
            ..MACHINE
        };
        let grid = play(&mut Scheduler::default(), 0, ONE_BAR * 5, 256, &p, &MACHINE);
        let on = play(&mut Scheduler::default(), 0, ONE_BAR * 5, 256, &p, &rushed);
        assert_eq!(grid.len(), on.len());
        // Bar 3 is the fill. Past its halfway point every hit is over 20 ms early, about 960 samples
        for (g, r) in grid.iter().zip(&on) {
            if g.0 / ONE_BAR != 3 {
                assert_eq!(r.0, g.0);
            } else if g.0 > ONE_BAR * 3 + ONE_BAR / 2 {
                assert!(r.0 < g.0 - 900, "{g:?} {r:?}");
            }
        }
    }

    #[test]
    fn crossing_bars_keeps_odd_meters_and_triplets_on_the_grid() {
        // 7/8 in triplet sixteenths has exactly 21 steps
        let p = Pattern {
            division: Division::SixteenthTriplet,
            intensity: 1.0,
            crash_every: 0,
            ..pat()
        };
        let c = Clock {
            bar_len: 3.5,
            bar: 0,
            bar_start: 0.0,
            ..clock_at(0)
        };
        let mut grid = vec![];
        scan(&c, 0.0, 3.5, true, &p, &MACHINE, |t, v, _| {
            if v == crate::pattern::RIDE {
                grid.push(t);
            }
        });
        assert_eq!(grid.len(), 21);
        assert!((grid[20] - 20.0 / 6.0).abs() < 1e-9);
    }
}
