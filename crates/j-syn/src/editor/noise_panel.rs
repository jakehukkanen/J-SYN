//! Per-voice stereo Noise generator strip (`NOISE`) below `OSC 4`.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use nih_plug::prelude::GuiContext;
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{build_enum_dropdown, build_flex_knob_cell_mod_dim};
use super::mod_bar::ModUiState;
use super::tokens;
use super::widgets::LedToggleSwitch;
use super::{AMBER_RGB, CORAL_RGB, GOLD_RGB, UiData};
use crate::{JSynPluginParams, NoiseTypeParam};

const NOISE_TYPE_ITEMS: &[(&str, NoiseTypeParam)] = &[
    ("White", NoiseTypeParam::White),
    ("Pink", NoiseTypeParam::Pink),
    ("Brown", NoiseTypeParam::Brown),
];

#[inline]
fn noise_is_enabled(p: &JSynPluginParams) -> bool {
    p.noise.enabled.value()
}

pub(super) fn build_noise_panel(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    let p_dim = params.clone();
    VStack::new(cx, move |cx| {
        let p_hdr = params.clone();
        let g_hdr = gui_context.clone();
        HStack::new(cx, move |cx| {
            LedToggleSwitch::new(
                cx,
                UiData::params,
                |p| &p.noise.enabled,
                "NOISE",
                "NOISE",
                CORAL_RGB,
            )
            .width(Pixels(58.0))
            .height(Pixels(tokens::HEADER_STRIP_H));

            build_enum_dropdown(
                cx,
                p_hdr,
                g_hdr,
                |p| &p.noise.noise_type,
                NOISE_TYPE_ITEMS,
            )
            .width(Stretch(1.0))
            .height(Pixels(tokens::HEADER_STRIP_H));
        })
        .col_between(Pixels(tokens::SPACE_SM))
        .height(Pixels(tokens::HEADER_STRIP_H))
        .width(Stretch(1.0));

        HStack::new(cx, move |cx| {
            build_flex_knob_cell_mod_dim(
                cx,
                UiData::params,
                |p| &p.noise.color,
                "COLOR",
                true,
                AMBER_RGB,
                tokens::KNOB_OSC,
                ModDestination::NoiseColor,
                mod_ui.clone(),
                0,
                params.clone(),
                noise_is_enabled,
            );

            build_flex_knob_cell_mod_dim(
                cx,
                UiData::params,
                |p| &p.noise.stereo,
                "STEREO",
                false,
                GOLD_RGB,
                tokens::KNOB_OSC,
                ModDestination::NoiseStereo,
                mod_ui.clone(),
                0,
                params.clone(),
                noise_is_enabled,
            );

            build_flex_knob_cell_mod_dim(
                cx,
                UiData::params,
                |p| &p.noise.level,
                "LEVEL",
                false,
                CORAL_RGB,
                tokens::KNOB_OSC,
                ModDestination::NoiseLevel,
                mod_ui,
                0,
                params,
                noise_is_enabled,
            );
        })
        .col_between(Pixels(tokens::SPACE_LG))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })
    .class("hardware-module")
    .toggle_class(
        "module-disabled",
        UiData::mod_ui_rev.map(move |_| !noise_is_enabled(&p_dim)),
    )
    .row_between(Pixels(tokens::SPACE_XS))
    .width(Stretch(1.0))
    .height(Pixels(170.0));
}
