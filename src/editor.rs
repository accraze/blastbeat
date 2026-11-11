//! Grouped params under a song map, plus the Reroll button that only an editor can offer: a
//! plugin can only set its own params from the GUI, and Seed has to be a real param to save and
//! automate. The map is drawn from the params alone, since the pattern is a pure function of them.

use crate::midi::{steps_in_bar, Playhead};
use crate::mistakes::{self, Mistakes};
use crate::params::{BlastbeatParams, SectionLocks, SEED_MAX};
use crate::pattern::{
    block, block_section, double_bass_at, mix, section_at, step_hits, Pattern, Section, CHINA,
    CRASH, HAT, KICK, KICK_LEFT, LOCK_BLAST, LOCK_BREAKDOWN, LOCK_HALF_TIME, MAX_LOCKS, RIDE,
    SNARE,
};
use nih_plug::prelude::*;
use nih_plug_egui::egui::{
    self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
};
use nih_plug_egui::resizable_window::ResizableWindow;
use nih_plug_egui::widgets::ParamSlider;
use nih_plug_egui::{create_egui_editor, egui::RichText};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const WIDTH: u32 = 400;
pub const HEIGHT: u32 = 860;
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
/// Recent seeds only last while the plugin is loaded; starring is what saves one with the project.
const HISTORY: usize = 6;
/// Extra space before the first bar of each section block
const BLOCK_GAP: f32 = 3.0;
/// Room above the map's cells for the lock marker
const LOCK_STRIP: f32 = 4.0;

/// Top to bottom. The cymbal row shows whichever cymbal is selected.
const GRID_ROWS: [(&str, &[usize]); 5] = [
    ("Crash", &[CRASH]),
    ("Cymbal", &[HAT, RIDE, CHINA]),
    ("Snare", &[SNARE]),
    ("R kick", &[KICK]),
    ("L kick", &[KICK_LEFT]),
];
const GRID_LABEL_WIDTH: f32 = 46.0;
const GRID_ROW_HEIGHT: f32 = 13.0;
const BEAT_GAP: f32 = 2.0;
const EMPTY_STEP: Color32 = Color32::from_gray(50);

const BLAST: Color32 = Color32::from_rgb(150, 30, 30);
const HALF_TIME: Color32 = Color32::from_rgb(200, 120, 40);
const BREAKDOWN: Color32 = Color32::from_rgb(110, 60, 160);
const FILL: Color32 = Color32::from_rgb(220, 200, 60);
const DOUBLE_BASS: Color32 = Color32::from_gray(230);

pub fn create(params: Arc<BlastbeatParams>, playhead: Arc<Playhead>) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        Vec::<i32>::new(),
        |_, _| {},
        move |ctx, setter, history| {
            // The map's marker follows the transport, which moves without any param change
            ctx.request_repaint_after(Duration::from_millis(33));
            ResizableWindow::new("resize").min_size(MIN_SIZE).show(
                ctx,
                &params.editor_state,
                |ui| {
                    egui::Frame::new().inner_margin(MARGIN).show(ui, |ui| {
                        let (bar, beats, bar_len) = playhead.get();
                        let pattern = params.pattern();
                        song_map(ui, &pattern, &params.locks, bar);
                        ui.add_space(4.0);
                        step_grid(ui, &pattern, &params.feel().mistakes, bar, beats, bar_len);
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
                        let current = params.seed.value();
                        let starred = params.starred.read().unwrap().clone();
                        ui.horizontal_wrapped(|ui| {
                            let label = if starred.contains(&current) {
                                "Unstar seed"
                            } else {
                                "Star seed"
                            };
                            if ui.button(label).clicked() {
                                toggle_star(&mut params.starred.write().unwrap(), current);
                            }
                            if !starred.is_empty() {
                                ui.label("Starred:");
                            }
                            for &seed in &starred {
                                let button = egui::Button::new(seed.to_string()).small();
                                // The seed playing now is greyed out, which also shows where you are
                                if ui
                                    .add_enabled(unlocked && seed != current, button)
                                    .clicked()
                                {
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
    let sections = p.half_time.value() + p.breakdown.value() + p.fill.value() > 0.0
        || p.locks.snapshot().iter().any(|&code| code != 0);
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

fn toggle_star(starred: &mut Vec<i32>, seed: i32) {
    match starred.iter().position(|&s| s == seed) {
        Some(i) => {
            starred.remove(i);
        }
        None => starred.push(seed),
    }
}

/// One cell per bar, coloured by section, with a white line under double-bass bars and a marker
/// above locked blocks. Left-click pins or cycles a block's section, right-click unlocks it.
fn song_map(ui: &mut Ui, p: &Pattern, locks: &SectionLocks, now: i64) {
    let section = section_at(now, p);
    ui.label(format!("Bar {}: {}", now + 1, name(section)));

    let height = LOCK_STRIP + 24.0;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
    let painter = ui.painter();
    let width = rect.width() / MAP_BARS as f32;
    let first = now.div_euclid(MAP_PAGE) * MAP_PAGE;
    let doubled = |bar| section_at(bar, p) != Section::Fill && double_bass_at(bar, p);
    let locked = |bar| lock_at(block(bar, p), p) != 0;
    for i in 0..MAP_BARS {
        let bar = first + i;
        let gap = if bar.rem_euclid(p.section_bars) == 0 {
            BLOCK_GAP
        } else {
            0.0
        };
        let left = rect.left() + i as f32 * width + gap;
        let size = Vec2::new(width - 1.0 - gap, 18.0);
        let cell = Rect::from_min_size(egui::pos2(left, rect.top() + LOCK_STRIP), size);
        painter.rect_filled(cell, 1.0, color(section_at(bar, p)));
        if locked(bar) {
            let marker = Rect::from_min_size(
                egui::pos2(cell.left(), rect.top()),
                Vec2::new(width - gap, 2.0),
            );
            painter.rect_filled(marker, 0.0, Color32::WHITE);
        }
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

    let bar_at = |x: f32| first + (((x - rect.left()) / width) as i64).clamp(0, MAP_BARS - 1);
    if let Some(pos) = response.interact_pointer_pos() {
        let b = block(bar_at(pos.x), p);
        if response.clicked() {
            locks.lock(b, next_lock(lock_at(b, p), block_section(b, p)));
        } else if response.secondary_clicked() {
            locks.lock(b, 0);
        }
    }
    if let Some(pos) = response.hover_pos() {
        let bar = bar_at(pos.x);
        let extra = if doubled(bar) { " + double bass" } else { "" };
        let tip = if locked(bar) {
            "locked. Click to change, right-click to unlock"
        } else if (0..MAX_LOCKS as i64).contains(&block(bar, p)) {
            "click to lock this block"
        } else {
            "too far in to lock"
        };
        response.on_hover_text_at_pointer(format!(
            "Bar {}: {}{extra}\n{tip}",
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
        ui.label("Top line: locked");
    });
}

/// One row per drum for the bar playing now, with a box around the current step. Hits take the
/// section's colour, fainter when quieter.
fn step_grid(ui: &mut Ui, p: &Pattern, m: &Mistakes, bar: i64, beats: f64, bar_len: f64) {
    let spq = p.division.steps_per_quarter();
    let steps = steps_in_bar(bar_len, spq);
    let height = GRID_ROWS.len() as f32 * GRID_ROW_HEIGHT;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    let painter = ui.painter();
    let left = rect.left() + GRID_LABEL_WIDTH;
    let width = (rect.width() - GRID_LABEL_WIDTH) / steps as f32;
    let hit = color(section_at(bar, p));
    for (row, (label, voices)) in GRID_ROWS.iter().enumerate() {
        let top = rect.top() + row as f32 * GRID_ROW_HEIGHT;
        painter.text(
            egui::pos2(rect.left(), top + GRID_ROW_HEIGHT / 2.0),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(11.0),
            ui.visuals().text_color(),
        );
        for step in 0..steps {
            let gap = if step % spq == 0 { BEAT_GAP } else { 0.0 };
            let size = Vec2::new((width - 1.0 - gap).max(1.0), GRID_ROW_HEIGHT - 2.0);
            let cell = Rect::from_min_size(egui::pos2(left + step as f32 * width + gap, top), size);
            let v = heard(bar, step, voices, p, m);
            let fill = if v > 0.0 {
                hit.gamma_multiply(v)
            } else {
                EMPTY_STEP
            };
            painter.rect_filled(cell, 1.0, fill);
        }
    }
    let step = ((beats * spq as f64) as i64).clamp(0, steps - 1);
    let column = Rect::from_min_size(
        egui::pos2(left + step as f32 * width, rect.top()),
        Vec2::new(width - 1.0, height - 2.0),
    );
    painter.rect_stroke(
        column,
        1.0,
        Stroke::new(1.0_f32, Color32::WHITE),
        StrokeKind::Outside,
    );
}

/// The loudest of these voices on this step, 0 if none play or a mistake drops them.
fn heard(bar: i64, step: i64, voices: &[usize], p: &Pattern, m: &Mistakes) -> f32 {
    let hits = step_hits(bar, step, p);
    voices
        .iter()
        .filter(|&&v| !mistakes::dropped(p.seed, bar, step, v, m))
        .fold(0.0, |loudest, &v| hits[v].max(loudest))
}

fn lock_at(b: i64, p: &Pattern) -> u8 {
    usize::try_from(b)
        .ok()
        .and_then(|i| p.locks.get(i))
        .copied()
        .unwrap_or(0)
}

/// An unlocked block pins the section it's showing; a locked one moves on to the next section.
fn next_lock(code: u8, showing: Section) -> u8 {
    match (code, showing) {
        (LOCK_BLAST, _) => LOCK_HALF_TIME,
        (LOCK_HALF_TIME, _) => LOCK_BREAKDOWN,
        (LOCK_BREAKDOWN, _) => LOCK_BLAST,
        (_, Section::HalfTime) => LOCK_HALF_TIME,
        (_, Section::Breakdown) => LOCK_BREAKDOWN,
        _ => LOCK_BLAST,
    }
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

    #[test]
    fn the_grid_shows_the_selected_cymbal_without_the_dropped_hits() {
        let p = BlastbeatParams::default().pattern();
        let m = Mistakes {
            rush: 0.0,
            weak_foot: 0.0,
            drops: 1.0,
            flams: 0.0,
            flat: 0.0,
        };
        let cymbals = GRID_ROWS[1].1;
        let mut dropped = 0;
        for bar in 0..50 {
            for step in 0..16 {
                let shown = heard(bar, step, cymbals, &p, &m);
                if mistakes::dropped(p.seed, bar, step, RIDE, &m) {
                    assert_eq!(shown, 0.0);
                    dropped += 1;
                } else {
                    assert_eq!(shown, step_hits(bar, step, &p)[RIDE]);
                    assert!(shown > 0.0);
                }
            }
        }
        assert!(dropped > 0);
    }

    #[test]
    fn clicking_pins_what_shows_then_cycles_every_section() {
        assert_eq!(next_lock(0, Section::Breakdown), LOCK_BREAKDOWN);
        assert_eq!(next_lock(0, Section::Blast), LOCK_BLAST);
        let mut code = next_lock(0, Section::HalfTime);
        let mut seen = vec![code];
        for _ in 0..2 {
            code = next_lock(code, Section::HalfTime);
            seen.push(code);
        }
        seen.sort();
        assert_eq!(seen, [LOCK_BLAST, LOCK_HALF_TIME, LOCK_BREAKDOWN]);
        assert_eq!(next_lock(code, Section::HalfTime), LOCK_HALF_TIME);
    }

    #[test]
    fn starring_a_seed_twice_removes_it_and_keeps_the_rest_in_order() {
        let mut s = vec![];
        toggle_star(&mut s, 5);
        toggle_star(&mut s, 9);
        toggle_star(&mut s, 2);
        assert_eq!(s, [5, 9, 2]);
        toggle_star(&mut s, 9);
        assert_eq!(s, [5, 2]);
    }
}
