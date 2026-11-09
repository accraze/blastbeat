//! A plain param list, plus the Reroll button that only an editor can offer: a plugin can only set
//! its own params from the GUI, and Seed has to be a real param to save and automate.

use crate::params::{BlastbeatParams, SEED_MAX};
use crate::pattern::mix;
use nih_plug::prelude::*;
use nih_plug_egui::widgets::generic_ui::{self, GenericSlider};
use nih_plug_egui::{create_egui_editor, egui};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub const WIDTH: u32 = 400;
pub const HEIGHT: u32 = 940;

pub fn create(params: Arc<BlastbeatParams>) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        (),
        |_, _| {},
        move |ctx, setter, _| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let unlocked = !params.lock.value();
                if ui
                    .add_enabled(unlocked, egui::Button::new("Reroll seed"))
                    .clicked()
                {
                    let seed = (now_nanos() % (SEED_MAX as u64 + 1)) as i32;
                    setter.begin_set_parameter(&params.seed);
                    setter.set_parameter(&params.seed, seed);
                    setter.end_set_parameter(&params.seed);
                }
                generic_ui::create(ui, params.clone(), setter, GenericSlider);
            });
        },
    )
}

fn now_nanos() -> u64 {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    mix(d.as_nanos() as u64)
}
