//! A MIDI note effect that generates blast-beat drum patterns.

use feel::Feel;
use midi::{Clock, Scheduler};
use mistakes::Mistakes;
use nih_plug::prelude::*;
use params::BlastbeatParams;
use pattern::Pattern;
use std::sync::Arc;

mod editor;
mod feel;
mod midi;
mod mistakes;
mod params;
mod pattern;

struct Blastbeat {
    params: Arc<BlastbeatParams>,
    scheduler: Scheduler,
    sample_rate: f32,
}

impl Default for Blastbeat {
    fn default() -> Self {
        Self {
            params: Arc::new(BlastbeatParams::default()),
            scheduler: Scheduler::default(),
            sample_rate: 48000.0,
        }
    }
}

impl Plugin for Blastbeat {
    const NAME: &'static str = "Blastbeat";
    const VENDOR: &'static str = "accraze";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // Pure note effect, no audio
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[];
    const MIDI_OUTPUT: MidiConfig = MidiConfig::Basic;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(self.params.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        true
    }

    fn reset(&mut self) {
        self.scheduler = Scheduler::default();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let clock = self.read_clock(context.transport());
        let p = &self.params;
        let pattern = Pattern {
            seed: p.seed.value() as u64,
            blast: p.blast.value(),
            division: p.division.value(),
            cymbal: p.cymbal.value(),
            crash_every: p.crash_every.value() as i64,
            intensity: p.intensity.value(),
            section_bars: p.section_bars.value() as i64,
            half_time: p.half_time.value(),
            breakdown: p.breakdown.value(),
            fill: p.fill.value(),
        };
        let human = p.human.value();
        let amateur = p.amateur.value();
        let feel = Feel {
            jitter: p.jitter.value() * human,
            drift: p.drift.value() * human,
            drift_rate: p.drift_rate.value(),
            mistakes: Mistakes {
                rush: p.rush.value() * amateur,
                weak_foot: p.weak_foot.value() * amateur,
                drops: p.drops.value() * amateur,
                flams: p.flams.value() * amateur,
                flat: p.flat.value() * amateur,
            },
        };
        // Same order as the voice indices in `pattern`
        let notes = [
            &p.note_kick,
            &p.note_snare,
            &p.note_hat,
            &p.note_ride,
            &p.note_china,
            &p.note_crash,
        ]
        .map(|n| n.value() as u8);

        self.scheduler
            .process(clock, buffer.samples(), &pattern, &feel, &notes, |e| {
                context.send_event(e)
            });
        ProcessStatus::Normal
    }
}

impl Blastbeat {
    /// `None` when stopped, or when the host doesn't give us enough to find the grid.
    fn read_clock(&self, t: &Transport) -> Option<Clock> {
        if !t.playing {
            return None;
        }
        let tempo = t.tempo.filter(|&bpm| bpm > 0.0)?;
        let numerator = t.time_sig_numerator.filter(|&n| n > 0)?;
        let denominator = t.time_sig_denominator.filter(|&d| d > 0)?;
        Some(Clock {
            pos: t.pos_beats()?,
            q_per_sample: tempo / 60.0 / self.sample_rate as f64,
            q_per_ms: tempo / 60.0 / 1000.0,
            bar: t.bar_number()? as i64,
            bar_start: t.bar_start_pos_beats()?,
            bar_len: numerator as f64 * 4.0 / denominator as f64,
        })
    }
}

impl ClapPlugin for Blastbeat {
    const CLAP_ID: &'static str = "com.accraze.blastbeat";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Stateless blast-beat MIDI generator");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::NoteEffect, ClapFeature::Utility];
}

impl Vst3Plugin for Blastbeat {
    const VST3_CLASS_ID: [u8; 16] = *b"BlastBeatMidiGen";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Tools];
}

nih_export_clap!(Blastbeat);
nih_export_vst3!(Blastbeat);
