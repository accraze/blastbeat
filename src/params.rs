//! Host-facing parameters. Param IDs are saved in projects, so they must never change.

use crate::editor;
use crate::feel::Feel;
use crate::mistakes::Mistakes;
use crate::pattern::{Pattern, MAX_LOCKS};
use nih_plug::params::persist::PersistentField;
use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, RwLock};

#[derive(Enum, PartialEq, Clone, Copy)]
pub enum Blast {
    /// Kick and snare alternate
    Traditional,
    /// Kick and snare together
    Hammer,
}

#[derive(Enum, PartialEq, Clone, Copy)]
pub enum Division {
    #[name = "1/8"]
    Eighth,
    #[name = "1/16"]
    Sixteenth,
    #[name = "1/16T"]
    SixteenthTriplet,
    #[name = "1/32"]
    ThirtySecond,
}

impl Division {
    pub fn steps_per_quarter(self) -> i64 {
        match self {
            Division::Eighth => 2,
            Division::Sixteenth => 4,
            Division::SixteenthTriplet => 6,
            Division::ThirtySecond => 8,
        }
    }
}

#[derive(Enum, PartialEq, Clone, Copy)]
pub enum Cymbal {
    Hat,
    Ride,
    China,
}

#[derive(Enum, PartialEq, Clone, Copy)]
pub enum Feet {
    /// Every step, feet alternating
    Rolls,
    /// x.xx on each beat
    Gallop,
}

#[derive(Params)]
pub struct BlastbeatParams {
    // Renamed from "editor-size" so projects saved with the resizable 0.7.0-0.10.0 window open at
    // the faceplate's fixed size
    #[persist = "editor-size-2"]
    pub editor_state: Arc<EguiState>,
    /// Only the GUI and state saving touch this, so the audio thread never waits on the lock
    #[persist = "starred-seeds"]
    pub starred: RwLock<Vec<i32>>,
    #[persist = "section-locks"]
    pub locks: SectionLocks,

    #[id = "intensity"]
    pub intensity: FloatParam,
    #[id = "blast"]
    pub blast: EnumParam<Blast>,
    #[id = "division"]
    pub division: EnumParam<Division>,
    #[id = "cymbal"]
    pub cymbal: EnumParam<Cymbal>,
    #[id = "crash"]
    pub crash_every: IntParam,

    #[id = "sectionbars"]
    pub section_bars: IntParam,
    #[id = "halftime"]
    pub half_time: FloatParam,
    #[id = "breakdown"]
    pub breakdown: FloatParam,
    #[id = "fill"]
    pub fill: FloatParam,

    #[id = "doublebass"]
    pub double_bass: FloatParam,
    #[id = "feet"]
    pub feet: EnumParam<Feet>,

    #[id = "rifffollow"]
    pub riff_follow: BoolParam,

    #[id = "human"]
    pub human: FloatParam,
    #[id = "jitter"]
    pub jitter: FloatParam,
    #[id = "drift"]
    pub drift: FloatParam,
    #[id = "driftrate"]
    pub drift_rate: FloatParam,

    #[id = "amateur"]
    pub amateur: FloatParam,
    #[id = "rush"]
    pub rush: FloatParam,
    #[id = "weakfoot"]
    pub weak_foot: FloatParam,
    #[id = "drops"]
    pub drops: FloatParam,
    #[id = "flams"]
    pub flams: FloatParam,
    #[id = "flatvel"]
    pub flat: FloatParam,

    #[id = "seed"]
    pub seed: IntParam,
    #[id = "lock"]
    pub lock: BoolParam,

    #[id = "kick"]
    pub note_kick: IntParam,
    #[id = "kickleft"]
    pub note_kick_left: IntParam,
    #[id = "snare"]
    pub note_snare: IntParam,
    #[id = "hat"]
    pub note_hat: IntParam,
    #[id = "ride"]
    pub note_ride: IntParam,
    #[id = "china"]
    pub note_china: IntParam,
    #[id = "crashnote"]
    pub note_crash: IntParam,

    /// Bumped by the editor on every star or lock change. Those live outside the params, so without a
    /// param change the host wouldn't see the project as modified and could quit without saving them.
    #[id = "edits"]
    pub edits: IntParam,
}

/// Highest seed. Params are normalized to f32, so a much larger range wouldn't round-trip exactly.
pub const SEED_MAX: i32 = 99_999;
/// `edits` wraps back to 0 after this; only its changing matters.
pub const EDITS_MAX: i32 = 1023;

/// Section locks set from the song map. Atomics, so the audio thread copies them without waiting.
pub struct SectionLocks([AtomicU8; MAX_LOCKS]);

impl Default for SectionLocks {
    fn default() -> Self {
        Self(std::array::from_fn(|_| AtomicU8::new(0)))
    }
}

impl SectionLocks {
    pub fn snapshot(&self) -> [u8; MAX_LOCKS] {
        std::array::from_fn(|i| self.0[i].load(Ordering::Relaxed))
    }

    /// Ignores blocks past the end, which can't be locked.
    pub fn lock(&self, block: i64, code: u8) {
        if let Some(lock) = usize::try_from(block).ok().and_then(|i| self.0.get(i)) {
            lock.store(code, Ordering::Relaxed);
        }
    }
}

/// Saved as a list without its trailing unlocked blocks, so most projects store a few bytes.
impl<'a> PersistentField<'a, Vec<u8>> for SectionLocks {
    fn set(&self, codes: Vec<u8>) {
        for (i, lock) in self.0.iter().enumerate() {
            lock.store(codes.get(i).copied().unwrap_or(0), Ordering::Relaxed);
        }
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&Vec<u8>) -> R,
    {
        let mut codes = self.snapshot().to_vec();
        while codes.last() == Some(&0) {
            codes.pop();
        }
        f(&codes)
    }
}

impl BlastbeatParams {
    /// The current pattern settings, shared by the audio thread and the song map.
    pub fn pattern(&self) -> Pattern {
        Pattern {
            seed: self.seed.value() as u64,
            blast: self.blast.value(),
            division: self.division.value(),
            cymbal: self.cymbal.value(),
            crash_every: self.crash_every.value() as i64,
            intensity: self.intensity.value(),
            section_bars: self.section_bars.value() as i64,
            half_time: self.half_time.value(),
            breakdown: self.breakdown.value(),
            fill: self.fill.value(),
            double_bass: self.double_bass.value(),
            feet: self.feet.value(),
            riff_follow: self.riff_follow.value(),
            locks: self.locks.snapshot(),
        }
    }

    /// The timing and mistakes after the Human and Amateur macros.
    pub fn feel(&self) -> Feel {
        let human = self.human.value();
        let amateur = self.amateur.value();
        Feel {
            jitter: self.jitter.value() * human,
            drift: self.drift.value() * human,
            drift_rate: self.drift_rate.value(),
            mistakes: Mistakes {
                rush: self.rush.value() * amateur,
                weak_foot: self.weak_foot.value() * amateur,
                drops: self.drops.value() * amateur,
                flams: self.flams.value() * amateur,
                flat: self.flat.value() * amateur,
            },
        }
    }
}

fn knob(name: &str, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_unit("%")
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
}

fn note(name: &str, default: i32) -> IntParam {
    IntParam::new(name, default, IntRange::Linear { min: 0, max: 127 })
}

impl Default for BlastbeatParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(editor::WIDTH, editor::HEIGHT),
            starred: RwLock::new(Vec::new()),
            locks: SectionLocks::default(),
            intensity: knob("Intensity", 1.0),
            blast: EnumParam::new("Blast", Blast::Traditional),
            division: EnumParam::new("Division", Division::Sixteenth),
            cymbal: EnumParam::new("Cymbal", Cymbal::Ride),
            crash_every: IntParam::new("Crash Every", 4, IntRange::Linear { min: 0, max: 16 })
                // The unit is part of these strings because "Off bars" wouldn't read or parse
                .with_value_to_string(Arc::new(|v| {
                    if v == 0 {
                        "Off".to_string()
                    } else {
                        format!("{v} bars")
                    }
                }))
                .with_string_to_value(Arc::new(|s| {
                    if s.trim().eq_ignore_ascii_case("off") {
                        Some(0)
                    } else {
                        s.trim().trim_end_matches("bars").trim().parse().ok()
                    }
                })),
            section_bars: IntParam::new("Section Bars", 4, IntRange::Linear { min: 1, max: 32 }),
            // All zero, so projects saved before sections existed keep playing the same
            half_time: knob("Half-Time", 0.0),
            breakdown: knob("Breakdown", 0.0),
            fill: knob("Fill", 0.0),
            double_bass: knob("Double Bass", 0.0),
            feet: EnumParam::new("Double Bass Style", Feet::Rolls),
            riff_follow: BoolParam::new("Riff Follow", false),
            human: knob("Human", 0.5),
            jitter: knob("Jitter", 0.5),
            drift: knob("Drift", 0.3),
            drift_rate: FloatParam::new(
                "Drift Rate",
                0.5,
                FloatRange::Skewed {
                    min: 0.1,
                    max: 8.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_unit(" /bar")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            // Zero, so projects saved before this layer existed keep playing the same
            amateur: knob("Amateur", 0.0),
            rush: knob("Rush", 0.5),
            weak_foot: knob("Weak Foot", 0.5),
            drops: knob("Drops", 0.5),
            flams: knob("Flams", 0.5),
            flat: knob("Flat Velocity", 0.5),
            seed: IntParam::new(
                "Seed",
                1,
                IntRange::Linear {
                    min: 0,
                    max: SEED_MAX,
                },
            ),
            lock: BoolParam::new("Lock Seed", false),
            // General MIDI drum notes
            note_kick: note("Kick Note", 36),
            // The same pad as the right foot, so double bass plays on any kit
            note_kick_left: note("Left Kick Note", 36),
            note_snare: note("Snare Note", 38),
            note_hat: note("Hat Note", 42),
            note_ride: note("Ride Note", 51),
            note_china: note("China Note", 52),
            note_crash: note("Crash Note", 49),
            edits: IntParam::new(
                "Edits",
                0,
                IntRange::Linear {
                    min: 0,
                    max: EDITS_MAX,
                },
            )
            .hide()
            .non_automatable(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starred_seeds_save_with_the_project_and_old_projects_have_none() {
        let saved = BlastbeatParams::default();
        *saved.starred.write().unwrap() = vec![7, 42];
        let state = saved.serialize_fields();

        let loaded = BlastbeatParams::default();
        loaded.deserialize_fields(&state);
        assert_eq!(*loaded.starred.read().unwrap(), [7, 42]);

        let old = BlastbeatParams::default();
        old.deserialize_fields(&Default::default());
        assert!(old.starred.read().unwrap().is_empty());
    }

    #[test]
    fn section_locks_save_with_the_project_and_old_projects_have_none() {
        let saved = BlastbeatParams::default();
        saved.locks.lock(0, 2);
        saved.locks.lock(5, 3);
        saved.locks.lock(-1, 1);
        saved.locks.lock(MAX_LOCKS as i64, 1);
        let state = saved.serialize_fields();
        assert_eq!(state["section-locks"], "[2,0,0,0,0,3]");

        let loaded = BlastbeatParams::default();
        loaded.locks.lock(9, 1);
        loaded.deserialize_fields(&state);
        assert_eq!(loaded.locks.snapshot(), saved.locks.snapshot());

        let old = BlastbeatParams::default();
        old.deserialize_fields(&Default::default());
        assert_eq!(old.locks.snapshot(), [0; MAX_LOCKS]);
    }

    #[test]
    fn the_edits_param_stays_out_of_sight_and_out_of_automation() {
        let flags = BlastbeatParams::default().edits.flags();
        assert!(flags.contains(ParamFlags::HIDDEN | ParamFlags::NON_AUTOMATABLE));
    }

    #[test]
    fn crash_every_text_round_trips() {
        let p = BlastbeatParams::default();
        let c = &p.crash_every;
        for v in 0..=16 {
            let n = c.preview_normalized(v);
            let text = c.normalized_value_to_string(n, true);
            assert_eq!(c.string_to_normalized_value(&text), Some(n), "{text}");
        }
        assert_eq!(c.normalized_value_to_string(0.0, true), "Off");
        assert_eq!(
            c.string_to_normalized_value("8"),
            Some(c.preview_normalized(8))
        );
        assert_eq!(c.string_to_normalized_value("nope"), None);
    }
}
