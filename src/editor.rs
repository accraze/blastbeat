//! Grouped params under a song map, plus the Reroll button that only an editor can offer: a
//! plugin can only set its own params from the GUI, and Seed has to be a real param to save and
//! automate. The map is drawn from the params alone, since the pattern is a pure function of them.

use crate::params::{BlastbeatParams, SEED_MAX};
use crate::pattern::{double_bass_at, mix, section_at, Pattern, Section};
use nih_plug::prelude::*;
use nih_plug_egui::egui::{self, Color32, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use nih_plug_egui::resizable_window::ResizableWindow;
use nih_plug_egui::widgets::ParamSlider;
use nih_plug_egui::{create_egui_editor, egui::RichText};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const WIDTH: u32 = 400;
pub const HEIGHT: u32 = 760;
const MIN_SIZE: Vec2 = Vec2::new(360.0, 320.0);
const MARGIN: f32 = 8.0;
/// Room each row needs beside its slider, for the name and the value
const ROW_TEXT_WIDTH: f32 = 230.0;
const MIN_SLIDER_WIDTH: f32 = 100.0;
/// Opacity of a param that has no effect right now. It stays usable, to set up before turning on.
const IDLE_OPACITY: f32 = 0.4;

const MAP_BARS: i64 = 32;
/// The map scrolls in steps of this many bars, so the marker moves rather than the bars
const MAP_PAGE: i64 = 8;
/// shortcut: recent seeds live in the open editor only and are lost with the plugin instance,
/// persist them with the project if that turns out to matter.
const HISTORY: usize = 6;
/// Extra space before the first bar of each section block
const BLOCK_GAP: f32 = 3.0;

const BLAST: Color32 = Color32::from_rgb(150, 30, 30);
const HALF_TIME: Color32 = Color32::from_rgb(200, 120, 40);
const BREAKDOWN: Color32 = Color32::from_rgb(110, 60, 160);
const FILL: Color32 = Color32::from_rgb(220, 200, 60);
const DOUBLE_BASS: Color32 = Color32::from_gray(230);

pub fn create(params: Arc<BlastbeatParams>, bar: Arc<AtomicI64>) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        Vec::<i32>::new(),
        |_, _| {},
        move |ctx, setter, history| {
            // The map's marker follows the transport, which moves without any param change
            ctx.request_repaint_after(Duration::from_millis(50));
            ResizableWindow::new("resize").min_size(MIN_SIZE).show(
                ctx,
                &params.editor_state,
                |ui| {
                    egui::Frame::new().inner_margin(MARGIN).show(ui, |ui| {
                        song_map(ui, &params.pattern(), bar.load(Ordering::Relaxed));
                        ui.add_space(6.0);

                        let unlocked = !params.lock.value();
                        let mut pick = None;
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .add_enabled(unlocked, egui::Button::new("Reroll seed"))
                                .clicked()
                            {
                                pick = Some((now_nanos() % (SEED_MAX as u64 + 1)) as i32);
                            }
                            if !history.is_empty() {
                                ui.label("Recent:");
                            }
                            for &seed in history.iter() {
                                let button = egui::Button::new(seed.to_string()).small();
                                if ui.add_enabled(unlocked, button).clicked() {
                                    pick = Some(seed);
                                }
                            }
                        });
                        if let Some(seed) = pick {
                            remember(history, params.seed.value(), seed);
                            setter.begin_set_parameter(&params.seed);
                            setter.set_parameter(&params.seed, seed);
                            setter.end_set_parameter(&params.seed);
                        }
                        param_panel(ui, &params, setter);
                    });
                },
            );
        },
    )
}

/// Everyday controls stay visible; the detail behind each macro and the drum map fold away.
fn param_panel(ui: &mut Ui, p: &BlastbeatParams, setter: &ParamSetter) {
    let s = Sliders {
        setter,
        width: (ui.available_width() - ROW_TEXT_WIDTH).max(MIN_SLIDER_WIDTH),
    };
    let riff = p.riff_follow.value();
    let doubled = p.double_bass.value() > 0.0 && !riff;
    let sections = p.half_time.value() + p.breakdown.value() + p.fill.value() > 0.0;
    let human = p.human.value() > 0.0;
    let amateur = p.amateur.value() > 0.0;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            group(ui, "Seed", |ui| {
                row(ui, &p.seed, s);
                row(ui, &p.lock, s);
            });
            group(ui, "Pattern", |ui| {
                row(ui, &p.intensity, s);
                row(ui, &p.blast, s);
                row(ui, &p.division, s);
                row(ui, &p.cymbal, s);
                row(ui, &p.crash_every, s);
            });
            group(ui, "Sections", |ui| {
                row_if(ui, &p.section_bars, s, sections || doubled);
                row(ui, &p.half_time, s);
                row(ui, &p.breakdown, s);
                row(ui, &p.fill, s);
            });
            group(ui, "Kick", |ui| {
                row_if(ui, &p.double_bass, s, !riff);
                row_if(ui, &p.feet, s, doubled);
                row(ui, &p.riff_follow, s);
            });
            group(ui, "Feel", |ui| row(ui, &p.human, s));
            details(ui, "Jitter and drift", |ui| {
                row_if(ui, &p.jitter, s, human);
                row_if(ui, &p.drift, s, human);
                row_if(ui, &p.drift_rate, s, human && p.drift.value() > 0.0);
            });
            group(ui, "Mistakes", |ui| row(ui, &p.amateur, s));
            details(ui, "Each mistake", |ui| {
                row_if(ui, &p.rush, s, amateur && p.fill.value() > 0.0);
                row_if(ui, &p.weak_foot, s, amateur);
                row_if(ui, &p.drops, s, amateur);
                row_if(ui, &p.flams, s, amateur);
                row_if(ui, &p.flat, s, amateur);
            });
            ui.add_space(6.0);
            details(ui, "Drum map", |ui| {
                row(ui, &p.note_kick, s);
                row_if(ui, &p.note_kick_left, s, doubled);
                row(ui, &p.note_snare, s);
                row(ui, &p.note_hat, s);
                row(ui, &p.note_ride, s);
                row(ui, &p.note_china, s);
                row(ui, &p.note_crash, s);
            });
        });
}

fn group(ui: &mut Ui, title: &str, rows: impl FnOnce(&mut Ui)) {
    ui.add_space(6.0);
    ui.strong(title);
    egui::Grid::new(title).num_columns(2).show(ui, rows);
}

fn details(ui: &mut Ui, title: &str, rows: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(title)
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new(title).num_columns(2).show(ui, rows);
        });
}

#[derive(Clone, Copy)]
struct Sliders<'a> {
    setter: &'a ParamSetter<'a>,
    width: f32,
}

fn row<P: Param>(ui: &mut Ui, param: &P, s: Sliders) {
    row_if(ui, param, s, true);
}

fn row_if<P: Param>(ui: &mut Ui, param: &P, s: Sliders, active: bool) {
    let opacity = if active { 1.0 } else { IDLE_OPACITY };
    ui.scope(|ui| {
        ui.multiply_opacity(opacity);
        ui.label(param.name());
    });
    ui.scope(|ui| {
        ui.multiply_opacity(opacity);
        ui.add(ParamSlider::for_param(param, s.setter).with_width(s.width));
    });
    ui.end_row();
}

/// Keeps the seed being left at the front of the recent list, and leaves out the one now playing.
fn remember(history: &mut Vec<i32>, from: i32, to: i32) {
    history.retain(|&s| s != from && s != to);
    if from != to {
        history.insert(0, from);
    }
    history.truncate(HISTORY);
}

/// One cell per bar, coloured by section, with a white line under double-bass bars.
fn song_map(ui: &mut Ui, p: &Pattern, now: i64) {
    let section = section_at(now, p);
    ui.label(format!("Bar {}: {}", now + 1, name(section)));

    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::hover());
    let painter = ui.painter();
    let width = rect.width() / MAP_BARS as f32;
    let first = now.div_euclid(MAP_PAGE) * MAP_PAGE;
    let doubled = |bar| section_at(bar, p) != Section::Fill && double_bass_at(bar, p);
    for i in 0..MAP_BARS {
        let bar = first + i;
        let gap = if bar.rem_euclid(p.section_bars) == 0 {
            BLOCK_GAP
        } else {
            0.0
        };
        let left = rect.left() + i as f32 * width + gap;
        let size = Vec2::new(width - 1.0 - gap, 18.0);
        let cell = Rect::from_min_size(egui::pos2(left, rect.top()), size);
        painter.rect_filled(cell, 1.0, color(section_at(bar, p)));
        if doubled(bar) {
            let line = Rect::from_min_size(
                cell.left_bottom() + Vec2::new(0.0, 2.0),
                Vec2::new(cell.width(), 3.0),
            );
            painter.rect_filled(line, 0.0, DOUBLE_BASS);
        }
        if bar == now {
            painter.rect_stroke(
                cell,
                1.0,
                Stroke::new(2.0_f32, Color32::WHITE),
                StrokeKind::Outside,
            );
        }
    }
    if let Some(pos) = response.hover_pos() {
        let i = (((pos.x - rect.left()) / width) as i64).clamp(0, MAP_BARS - 1);
        let bar = first + i;
        let extra = if doubled(bar) { " + double bass" } else { "" };
        response.on_hover_text_at_pointer(format!(
            "Bar {}: {}{extra}",
            bar + 1,
            name(section_at(bar, p))
        ));
    }

    ui.horizontal_wrapped(|ui| {
        for s in [
            Section::Blast,
            Section::HalfTime,
            Section::Breakdown,
            Section::Fill,
        ] {
            ui.label(RichText::new(format!("■ {}", name(s))).color(color(s)));
        }
        ui.label(RichText::new("▁ Double bass").color(DOUBLE_BASS));
    });
}

fn name(s: Section) -> &'static str {
    match s {
        Section::Blast => "Blast",
        Section::HalfTime => "Half-Time",
        Section::Breakdown => "Breakdown",
        Section::Fill => "Fill",
    }
}

fn color(s: Section) -> Color32 {
    match s {
        Section::Blast => BLAST,
        Section::HalfTime => HALF_TIME,
        Section::Breakdown => BREAKDOWN,
        Section::Fill => FILL,
    }
}

fn now_nanos() -> u64 {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    mix(d.as_nanos() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_seeds_keep_the_last_left_first_without_repeats() {
        let mut h = vec![];
        remember(&mut h, 1, 2);
        remember(&mut h, 2, 3);
        assert_eq!(h, [2, 1]);
        // Going back to 1 moves 3 to the front and drops 1, which is playing again
        remember(&mut h, 3, 1);
        assert_eq!(h, [3, 2]);
        remember(&mut h, 1, 1);
        assert_eq!(h, [3, 2]);
        for s in 10..20 {
            remember(&mut h, s, s + 1);
        }
        assert_eq!(h.len(), HISTORY);
        assert_eq!(h[0], 19);
    }
}
