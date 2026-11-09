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

#[derive(Enum, PartialEq, Clone, Copy)]
pub enum Feet {
    /// Every step, feet alternating
    Rolls,
    /// x.xx on each beat
    Gallop,
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
