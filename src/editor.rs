//! A hand-painted faceplate: the song map and step grid in a display window, knobs and switches for
//! the everyday params, and folding sliders for the rest. Reroll lives here because a plugin can
//! only set its own params from the GUI, and Seed has to be a real param to save and automate. The
//! map is drawn from the params alone, since the pattern is a pure function of them. `knob()` and
//! `screw()` come from Vibe.

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
    self, Align2, Color32, FontId, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder,
    Vec2,
};
use nih_plug_egui::widgets::ParamSlider;
use nih_plug_egui::{create_egui_editor, egui::RichText};
use std::f32::consts::{PI, TAU};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const WIDTH: u32 = 560;
pub const HEIGHT: u32 = 900;
/// Room each row needs beside its slider, for the name and the value
const ROW_TEXT_WIDTH: f32 = 230.0;
const MIN_SLIDER_WIDTH: f32 = 100.0;
const MAX_SLIDER_WIDTH: f32 = 200.0;
/// Opacity of a param that has no effect right now. It stays usable, to set up before turning on.
const IDLE_OPACITY: f32 = 0.4;

const KNOB_RADIUS: f32 = 22.0;
/// Knobs turn +-135 degrees from straight up
const SWEEP: f32 = 0.75 * PI;
const SWITCH_SIZE: Vec2 = Vec2::new(92.0, 24.0);
/// Height of each row of knobs and switches, title included
const CONTROL_ROW: f32 = 128.0;
/// From the top of a row to the centre of its controls, leaving clear space under the row's title
const CONTROLS_DOWN: f32 = 76.0;
/// How far above a control's centre its name sits, the same for knobs and switches so names line up
const LABEL_UP: f32 = KNOB_RADIUS + 16.0;
const PLATE: Color32 = Color32::from_gray(58);
const WHITE: Color32 = Color32::from_rgb(236, 234, 228);
const ENGRAVED: Color32 = Color32::from_gray(200);
const LED_RED: Color32 = Color32::from_rgb(255, 50, 35);

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

/// Editor-only state, lost with the plugin instance.
#[derive(Default)]
struct View {
    /// Seeds left behind, newest first
    history: Vec<i32>,
    /// The map's first bar when scrolled by hand, `None` to follow the playhead
    map_start: Option<i64>,
}

pub fn create(params: Arc<BlastbeatParams>, playhead: Arc<Playhead>) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        View::default(),
        |_, _| {},
        move |ctx, setter, view| {
            // The map's marker follows the transport, which moves without any param change
            ctx.request_repaint_after(Duration::from_millis(33));
            // Fixed size and drawn 1:1, since scaled text and hairlines were hard to read
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(Color32::from_gray(20)))
                .show(ctx, |ui| faceplate(ui, setter, &params, &playhead, view));
        },
    )
}

fn faceplate(
    ui: &mut Ui,
    setter: &ParamSetter,
    params: &BlastbeatParams,
    playhead: &Playhead,
    view: &mut View,
) {
    let body = ui.max_rect().shrink(8.0);
    plate(ui.painter(), body);
    let inner = body.shrink2(Vec2::new(22.0, 0.0));

    let logo = Pos2::new(inner.left(), body.top() + 36.0);
    // The default font has no bold weight, so fake one by overdrawing
    for offset in [
        Vec2::ZERO,
        Vec2::new(1.0, 0.0),
        Vec2::new(0.0, 1.0),
        Vec2::splat(1.0),
    ] {
        ui.painter().text(
            logo + offset,
            Align2::LEFT_CENTER,
            "BLASTBEAT",
            FontId::proportional(34.0),
            BLAST.lerp_to_gamma(LED_RED, 0.5),
        );
    }
    engraved(
        ui.painter(),
        Pos2::new(inner.right(), logo.y + 6.0),
        Align2::RIGHT_CENTER,
        "S T A T E L E S S   B L A S T   B E A T S",
        9.0,
    );

    let display = Rect::from_min_size(
        Pos2::new(inner.left(), body.top() + 66.0),
        Vec2::new(inner.width(), 172.0),
    );
    inset(ui.painter(), display);
    ui.scope_builder(UiBuilder::new().max_rect(display.shrink(8.0)), |ui| {
        let (bar, beats, bar_len) = playhead.get();
        let pattern = params.pattern();
        song_map(ui, &pattern, &params.locks, bar, &mut view.map_start);
        ui.add_space(4.0);
        step_grid(ui, &pattern, &params.feel().mistakes, bar, beats, bar_len);
    });

    // The seed buttons are stock egui widgets, so they get a black window to sit in like the display
    let seeds = Rect::from_min_size(
        Pos2::new(inner.left(), display.bottom() + 12.0),
        Vec2::new(inner.width() - SWITCH_SIZE.x - 24.0, 80.0),
    );
    inset(ui.painter(), seeds);
    ui.scope_builder(UiBuilder::new().max_rect(seeds.shrink(8.0)), |ui| {
        seed_rows(ui, setter, params, view)
    });
    let lock_at = Pos2::new(
        inner.right() - SWITCH_SIZE.x / 2.0,
        seeds.center().y + LABEL_UP / 2.0 - 4.0,
    );
    switch(ui, setter, &params.lock, lock_at, true);

    let p = params;
    let riff = p.riff_follow.value();
    let doubled = p.double_bass.value() > 0.0 && !riff;
    let sections = p.half_time.value() + p.breakdown.value() + p.fill.value() > 0.0
        || p.locks.snapshot().iter().any(|&code| code != 0);
    let human = p.human.value() > 0.0;
    let amateur = p.amateur.value() > 0.0;

    // Five columns per row, numbered from the left
    let top = seeds.bottom() + 12.0;
    let column = |i: usize| inner.left() + inner.width() * (i as f32 + 0.5) / 5.0;
    let controls_y = |row: usize| top + row as f32 * CONTROL_ROW + CONTROLS_DOWN;
    let title = |ui: &Ui, row: usize, first: usize, text: &str| {
        let y = top + row as f32 * CONTROL_ROW;
        ui.painter().line_segment(
            [Pos2::new(inner.left(), y), Pos2::new(inner.right(), y)],
            Stroke::new(1.0_f32, Color32::from_black_alpha(90)),
        );
        let x = column(first) - inner.width() / 10.0 + 4.0;
        engraved(
            ui.painter(),
            Pos2::new(x, y + 12.0),
            Align2::LEFT_CENTER,
            text,
            11.0,
        );
    };

    title(ui, 0, 0, "PATTERN");
    let y = controls_y(0);
    knob(ui, setter, &p.intensity, Pos2::new(column(0), y), true);
    knob(ui, setter, &p.crash_every, Pos2::new(column(1), y), true);
    switch(ui, setter, &p.blast, Pos2::new(column(2), y), true);
    switch(ui, setter, &p.division, Pos2::new(column(3), y), true);
    switch(ui, setter, &p.cymbal, Pos2::new(column(4), y), true);

    title(ui, 1, 0, "SECTIONS");
    let y = controls_y(1);
    knob(
        ui,
        setter,
        &p.section_bars,
        Pos2::new(column(0), y),
        sections || doubled,
    );
    knob(ui, setter, &p.half_time, Pos2::new(column(1), y), true);
    knob(ui, setter, &p.breakdown, Pos2::new(column(2), y), true);
    knob(ui, setter, &p.fill, Pos2::new(column(3), y), true);

    title(ui, 2, 0, "KICK");
    title(ui, 2, 3, "FEEL");
    title(ui, 2, 4, "MISTAKES");
    let y = controls_y(2);
    knob(ui, setter, &p.double_bass, Pos2::new(column(0), y), !riff);
    switch(ui, setter, &p.feet, Pos2::new(column(1), y), doubled);
    switch(ui, setter, &p.riff_follow, Pos2::new(column(2), y), true);
    knob(ui, setter, &p.human, Pos2::new(column(3), y), true);
    knob(ui, setter, &p.amateur, Pos2::new(column(4), y), true);

    let rest = Rect::from_min_max(
        Pos2::new(inner.left(), top + 3.0 * CONTROL_ROW),
        Pos2::new(inner.right(), body.bottom() - 20.0),
    );
    inset(ui.painter(), rest);
    let s = Sliders {
        setter,
        width: (rest.width() - ROW_TEXT_WIDTH).clamp(MIN_SLIDER_WIDTH, MAX_SLIDER_WIDTH),
    };
    ui.scope_builder(UiBuilder::new().max_rect(rest.shrink(8.0)), |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                details(ui, "Jitter and drift", |ui| {
                    row_if(ui, &p.jitter, s, human);
                    row_if(ui, &p.drift, s, human);
                    row_if(ui, &p.drift_rate, s, human && p.drift.value() > 0.0);
                });
                details(ui, "Each mistake", |ui| {
                    row_if(ui, &p.rush, s, amateur && p.fill.value() > 0.0);
                    row_if(ui, &p.weak_foot, s, amateur);
                    row_if(ui, &p.drops, s, amateur);
                    row_if(ui, &p.flams, s, amateur);
                    row_if(ui, &p.flat, s, amateur);
                });
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
    });
}

/// Reroll, Star, the Seed slider, and the recent and starred seeds to go back to.
fn seed_rows(ui: &mut Ui, setter: &ParamSetter, params: &BlastbeatParams, view: &mut View) {
    let unlocked = !params.lock.value();
    let current = params.seed.value();
    let starred = params.starred.read().unwrap().clone();
    let mut pick = None;
    ui.horizontal(|ui| {
        if ui
            .add_enabled(unlocked, egui::Button::new("Reroll seed"))
            .clicked()
        {
            pick = Some((now_nanos() % (SEED_MAX as u64 + 1)) as i32);
        }
        let label = if starred.contains(&current) {
            "Unstar seed"
        } else {
            "Star seed"
        };
        if ui.button(label).clicked() {
            toggle_star(&mut params.starred.write().unwrap(), current);
        }
        ui.label("Seed");
        ui.add(ParamSlider::for_param(&params.seed, setter).with_width(90.0));
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Recent:");
        if view.history.is_empty() {
            ui.weak("none yet");
        }
        for &seed in view.history.iter() {
            let button = egui::Button::new(seed.to_string()).small();
            if ui.add_enabled(unlocked, button).clicked() {
                pick = Some(seed);
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Starred:");
        if starred.is_empty() {
            ui.weak("none yet");
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
        remember(&mut view.history, current, seed);
        setter.begin_set_parameter(&params.seed);
        setter.set_parameter(&params.seed, seed);
        setter.end_set_parameter(&params.seed);
    }
}

/// Brushed gunmetal with a bevelled edge and a screw in each corner.
fn plate(p: &Painter, body: Rect) {
    p.rect_filled(body, 10.0, PLATE);
    // Brushing: faint horizontal streaks whose shade comes from a hash of the row, so it holds still
    let mut y = body.top() + 3.0;
    while y < body.bottom() - 3.0 {
        let shade = (mix(y as u64) % 6) as u8;
        let color = if shade < 3 {
            Color32::from_white_alpha(1 + shade)
        } else {
            Color32::from_black_alpha(shade - 1)
        };
        p.line_segment(
            [
                Pos2::new(body.left() + 3.0, y),
                Pos2::new(body.right() - 3.0, y),
            ],
            Stroke::new(1.0_f32, color),
        );
        y += 2.0;
    }
    p.rect_stroke(
        body,
        10.0,
        Stroke::new(2.0_f32, Color32::from_gray(95)),
        StrokeKind::Inside,
    );
    p.rect_stroke(
        body,
        10.0,
        Stroke::new(1.0_f32, Color32::from_black_alpha(200)),
        StrokeKind::Outside,
    );
    for corner in [
        body.left_top(),
        body.right_top(),
        body.left_bottom(),
        body.right_bottom(),
    ] {
        let inward = Vec2::new(
            (body.center().x - corner.x).signum(),
            (body.center().y - corner.y).signum(),
        );
        screw(p, corner + inward * 14.0);
    }
}

/// A recessed black window, darker along the top edge like it's cut into the plate.
fn inset(p: &Painter, rect: Rect) {
    p.rect_filled(rect, 4.0, Color32::from_gray(14));
    p.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0_f32, Color32::from_gray(100)),
        StrokeKind::Outside,
    );
    p.line_segment(
        [
            rect.left_top() + Vec2::new(4.0, 0.5),
            rect.right_top() + Vec2::new(-4.0, 0.5),
        ],
        Stroke::new(1.5_f32, Color32::from_black_alpha(220)),
    );
}

/// Text stamped into the metal: a dark shadow under a light face.
fn engraved(p: &Painter, pos: Pos2, align: Align2, text: &str, size: f32) {
    let font = FontId::proportional(size);
    p.text(
        pos + Vec2::new(0.0, 1.0),
        align,
        text,
        font.clone(),
        Color32::from_black_alpha(180),
    );
    p.text(pos, align, text, font, ENGRAVED);
}

/// A painter that fades everything it draws when the control has no effect right now.
fn faded(ui: &Ui, active: bool) -> Painter {
    let mut p = ui.painter().clone();
    if !active {
        p.multiply_opacity(IDLE_OPACITY);
    }
    p
}

/// Unit vector for a knob angle, measured clockwise from straight up.
fn dir(angle: f32) -> Vec2 {
    Vec2::new(angle.sin(), -angle.cos())
}

/// Drag or mouse wheel to turn, Shift for fine, double-click to reset.
fn knob<P: Param>(ui: &Ui, setter: &ParamSetter, param: &P, center: Pos2, active: bool) {
    let rect = Rect::from_center_size(center, Vec2::splat(2.0 * KNOB_RADIUS + 12.0));
    let response = ui.interact(rect, ui.id().with(param.name()), Sense::click_and_drag());

    // Accumulate drags in egui memory since the param value can lag behind the host round trip
    if response.drag_started() {
        setter.begin_set_parameter(param);
        ui.data_mut(|d| d.insert_temp(response.id, param.unmodulated_normalized_value()));
    }
    if response.dragged() {
        let speed = if ui.input(|i| i.modifiers.shift) {
            0.0005
        } else {
            0.005
        };
        let delta = response.drag_delta();
        let start = ui
            .data(|d| d.get_temp(response.id))
            .unwrap_or(param.unmodulated_normalized_value());
        let value = (start + (delta.x - delta.y) * speed).clamp(0.0, 1.0);
        ui.data_mut(|d| d.insert_temp(response.id, value));
        setter.set_parameter_normalized(param, value);
    }
    if response.drag_stopped() {
        setter.end_set_parameter(param);
    }
    if response.double_clicked() {
        setter.begin_set_parameter(param);
        setter.set_parameter(param, param.default_plain_value());
        setter.end_set_parameter(param);
    }

    if response.hovered() {
        let (notches, shift) = ui.input(|i| {
            let notches: f32 = i
                .events
                .iter()
                .map(|e| match e {
                    // Smooth-scrolling touchpads report points; treat ~50 points as one notch
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta,
                        ..
                    } => delta.y / 50.0,
                    egui::Event::MouseWheel { delta, .. } => delta.y,
                    _ => 0.0,
                })
                .sum();
            (notches, i.modifiers.shift)
        });
        if notches != 0.0 {
            // Stepped params move a whole step per notch
            let step = match param.step_count() {
                Some(steps) => 1.0 / steps as f32,
                None if shift => 0.002,
                None => 0.02,
            };
            let value = (param.unmodulated_normalized_value() + notches * step).clamp(0.0, 1.0);
            setter.begin_set_parameter(param);
            setter.set_parameter_normalized(param, value);
            setter.end_set_parameter(param);
        }
    }

    let value = param.unmodulated_normalized_value();
    let angle = -SWEEP + 2.0 * SWEEP * value;
    let p = faded(ui, active);

    for i in 0..=10 {
        let a = -SWEEP + 2.0 * SWEEP * i as f32 / 10.0;
        let r = if i % 5 == 0 { 1.8 } else { 1.0 };
        p.circle_filled(center + dir(a) * (KNOB_RADIUS + 7.0), r, WHITE);
    }

    // Knurled skirt that turns with the knob
    p.circle_filled(
        center + Vec2::new(0.0, 3.0),
        KNOB_RADIUS,
        Color32::from_black_alpha(160),
    );
    p.circle_filled(center, KNOB_RADIUS, Color32::from_gray(8));
    for i in 0..36 {
        let a = angle + i as f32 * TAU / 36.0;
        p.line_segment(
            [
                center + dir(a) * (KNOB_RADIUS - 5.0),
                center + dir(a) * (KNOB_RADIUS - 0.5),
            ],
            Stroke::new(1.5_f32, Color32::from_gray(42)),
        );
    }
    p.circle_filled(center, KNOB_RADIUS - 6.0, Color32::from_gray(26));
    p.circle_stroke(
        center,
        KNOB_RADIUS - 6.0,
        Stroke::new(1.0_f32, Color32::from_gray(58)),
    );
    p.line_segment(
        [
            center + dir(angle) * 5.0,
            center + dir(angle) * (KNOB_RADIUS - 8.0),
        ],
        Stroke::new(3.0_f32, LED_RED),
    );

    engraved(
        &p,
        center - Vec2::new(0.0, LABEL_UP),
        Align2::CENTER_BOTTOM,
        &param.name().to_uppercase(),
        10.0,
    );
    p.text(
        center + Vec2::new(0.0, KNOB_RADIUS + 12.0),
        Align2::CENTER_TOP,
        param.normalized_value_to_string(value, true),
        FontId::proportional(11.0),
        WHITE,
    );
}

/// A choice or on/off param as a box showing its value, with a light per option underneath.
/// Click for the next option, right-click for the previous one.
fn switch<P: Param>(ui: &Ui, setter: &ParamSetter, param: &P, center: Pos2, active: bool) {
    let rect = Rect::from_center_size(center, SWITCH_SIZE);
    let response = ui.interact(rect, ui.id().with(param.name()), Sense::click());
    let steps = param.step_count().unwrap_or(1).max(1);
    let index = (param.unmodulated_normalized_value() * steps as f32).round() as usize;
    if response.clicked() || response.secondary_clicked() {
        let next = cycle(index, steps + 1, response.clicked());
        setter.begin_set_parameter(param);
        setter.set_parameter_normalized(param, next as f32 / steps as f32);
        setter.end_set_parameter(param);
    }

    let p = faded(ui, active);
    let fill = if response.is_pointer_button_down_on() {
        Color32::from_gray(40)
    } else {
        Color32::from_gray(12)
    };
    p.rect_filled(rect, 4.0, fill);
    p.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0_f32, Color32::from_gray(100)),
        StrokeKind::Outside,
    );
    let value = param.normalized_value_to_string(index as f32 / steps as f32, false);
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        value.to_uppercase(),
        FontId::proportional(11.0),
        WHITE,
    );
    engraved(
        &p,
        center - Vec2::new(0.0, LABEL_UP),
        Align2::CENTER_BOTTOM,
        &param.name().to_uppercase(),
        10.0,
    );
    let spacing = 12.0;
    let first = center.x - spacing * steps as f32 / 2.0;
    for i in 0..=steps {
        let led = Pos2::new(first + i as f32 * spacing, rect.bottom() + 10.0);
        if i == index {
            p.circle_filled(led, 6.0, LED_RED.gamma_multiply(0.16));
            p.circle_filled(led, 3.0, LED_RED);
        } else {
            p.circle_filled(led, 3.0, Color32::from_gray(20));
        }
    }
}

/// The next option, wrapping around, or the previous one.
fn cycle(index: usize, options: usize, forward: bool) -> usize {
    if forward {
        (index + 1) % options
    } else {
        (index + options - 1) % options
    }
}

fn screw(p: &Painter, center: Pos2) {
    p.circle_filled(center, 5.5, Color32::from_gray(90));
    p.circle_stroke(center, 5.5, Stroke::new(1.0_f32, Color32::from_gray(40)));
    p.line_segment(
        [center - dir(0.6) * 4.0, center + dir(0.6) * 4.0],
        Stroke::new(1.5_f32, Color32::from_gray(35)),
    );
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
fn song_map(ui: &mut Ui, p: &Pattern, locks: &SectionLocks, now: i64, start: &mut Option<i64>) {
    let first = start.unwrap_or(now.div_euclid(MAP_PAGE) * MAP_PAGE);
    ui.horizontal(|ui| {
        let section = name(section_at(now, p));
        match start {
            Some(_) => ui.label(format!(
                "Bar {}: {section}, showing {}-{}",
                now + 1,
                first + 1,
                first + MAP_BARS
            )),
            None => ui.label(format!("Bar {}: {section}", now + 1)),
        };
        // Right to left, so these read "< > Now"
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(start.is_some(), egui::Button::new("Now").small())
                .clicked()
            {
                *start = None;
            }
            if ui.small_button(">").clicked() {
                *start = Some(first + MAP_PAGE);
            }
            if ui
                .add_enabled(first > 0, egui::Button::new("<").small())
                .clicked()
            {
                *start = Some((first - MAP_PAGE).max(0));
            }
        });
    });

    let height = LOCK_STRIP + 24.0;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
    let painter = ui.painter();
    let width = rect.width() / MAP_BARS as f32;
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
    fn switches_cycle_both_ways_and_wrap() {
        assert_eq!(cycle(0, 4, true), 1);
        assert_eq!(cycle(3, 4, true), 0);
        assert_eq!(cycle(0, 4, false), 3);
        assert_eq!(cycle(1, 2, true), 0);
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
