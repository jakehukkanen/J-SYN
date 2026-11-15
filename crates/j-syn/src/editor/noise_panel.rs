//! Per-voice stereo Noise generator strip (`NOISE`) below `OSC 4`.

use std::sync::Arc;

use nih_plug::prelude::GuiContext;
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{build_enum_dropdown, build_header_knob_cell};
use super::widgets::LedToggleSwitch;
use super::{AMBER_RGB, CORAL_RGB, GOLD_RGB, UiData};
use crate::{JSynPluginParams, NoiseTypeParam};

const NOISE_TYPE_ITEMS: &[(&str, NoiseTypeParam)] = &[
    ("White", NoiseTypeParam::White),
    ("Pink", NoiseTypeParam::Pink),
    ("Brown", NoiseTypeParam::Brown),
];

pub(super) fn build_noise_panel(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
) {
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
        .height(Pixels(22.0));

        build_enum_dropdown(
            cx,
            params.clone(),
            gui_context.clone(),
            |p| &p.noise.noise_type,
            NOISE_TYPE_ITEMS,
        )
        .width(Pixels(74.0))
        .height(Pixels(22.0));

        Element::new(cx).width(Stretch(1.0));

        build_header_knob_cell(
            cx,
            UiData::params,
            |p| &p.noise.color,
            "COLOR",
            true,
            AMBER_RGB,
        );

        build_header_knob_cell(
            cx,
            UiData::params,
            |p| &p.noise.stereo,
            "STEREO",
            false,
            GOLD_RGB,
        );

        build_header_knob_cell(
            cx,
            UiData::params,
            |p| &p.noise.level,
            "LEVEL",
            false,
            CORAL_RGB,
        );
    })
    .class("hardware-module")
    .col_between(Pixels(8.0))
    .child_top(Stretch(1.0))
    .child_bottom(Stretch(1.0))
    .width(Stretch(1.0))
    .height(Pixels(44.0));
}
