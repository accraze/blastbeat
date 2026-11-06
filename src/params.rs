//! Host-facing parameters. Param IDs are saved in projects, so they must never change.

use crate::editor;
use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::Arc;

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

#[derive(Params)]
pub struct BlastbeatParams {
    #[persist = "editor-size"]
    pub editor_state: Arc<EguiState>,

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

    #[id = "human"]
    pub human: FloatParam,
    #[id = "jitter"]
    pub jitter: FloatParam,
    #[id = "drift"]
    pub drift: FloatParam,
    #[id = "driftrate"]
    pub drift_rate: FloatParam,

    #[id = "seed"]
    pub seed: IntParam,
    #[id = "lock"]
    pub lock: BoolParam,

    #[id = "kick"]
    pub note_kick: IntParam,
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
}

/// Highest seed. Params are normalized to f32, so a much larger range wouldn't round-trip exactly.
pub const SEED_MAX: i32 = 99_999;

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
            intensity: knob("Intensity", 1.0),
            blast: EnumParam::new("Blast", Blast::Traditional),
            division: EnumParam::new("Division", Division::Sixteenth),
            cymbal: EnumParam::new("Cymbal", Cymbal::Ride),
            crash_every: IntParam::new("Crash Every", 4, IntRange::Linear { min: 0, max: 16 })
                .with_unit(" bars")
                .with_value_to_string(Arc::new(|v| {
                    if v == 0 {
                        "Off".to_string()
                    } else {
                        v.to_string()
                    }
                })),
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
            note_snare: note("Snare Note", 38),
            note_hat: note("Hat Note", 42),
            note_ride: note("Ride Note", 51),
            note_china: note("China Note", 52),
            note_crash: note("Crash Note", 49),
        }
    }
}
