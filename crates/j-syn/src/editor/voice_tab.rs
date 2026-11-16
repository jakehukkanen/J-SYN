//! `VOICE` bottom drawer (`KEYTRACKING`, `VOICING` including `[• MPE]` / `MPE PB`, and `OSCILLATOR PAN`).

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use nih_plug::prelude::GuiContext;
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{
    bold_label, build_bool_dropdown, build_flex_knob_cell_mod, build_flex_knob_cell_mod_dim,
    build_knob_cell, build_mini_pill_cell, build_module_header,
};
use super::mod_bar::{bottom_tab_tag, ModUiState};
use super::osc_panel::osc_enabled_fn;
use super::widgets::LedToggleSwitch;
use super::{BottomTab, UiData, AMBER_RGB, CORAL_RGB, GOLD_RGB};
use crate::JSynPluginParams;

pub(super) fn build_voice_tab(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
            HStack::new(cx, |cx| {
                VStack::new(cx, |cx| {
                    build_module_header(cx, "KEYTRACKING", 18.0);

                    HStack::new(cx, move |cx| {
                        build_knob_cell(
                            cx,
                            UiData::params,
                            |p| &p.filters[0].keytrack,
                            "F1 KEYTRK",
                            false,
                            AMBER_RGB,
                            32.0,
                            62.0,
                        );
                        build_knob_cell(
                            cx,
                            UiData::params,
                            |p| &p.filters[1].keytrack,
                            "F2 KEYTRK",
                            false,
                            CORAL_RGB,
                            32.0,
                            62.0,
                        );

                        HStack::new(cx, |cx| {
                            LedToggleSwitch::new(
                                cx,
                                UiData::params,
                                |p| &p.oscs[0].keytrack,
                                "OSC 1",
                                "OSC 1",
                                AMBER_RGB,
                            )
                            .width(Stretch(1.0))
                            .height(Pixels(24.0));
                            LedToggleSwitch::new(
                                cx,
                                UiData::params,
                                |p| &p.oscs[1].keytrack,
                                "OSC 2",
                                "OSC 2",
                                GOLD_RGB,
                            )
                            .width(Stretch(1.0))
                            .height(Pixels(24.0));
                            LedToggleSwitch::new(
                                cx,
                                UiData::params,
                                |p| &p.oscs[2].keytrack,
                                "OSC 3",
                                "OSC 3",
                                CORAL_RGB,
                            )
                            .width(Stretch(1.0))
                            .height(Pixels(24.0));
                            LedToggleSwitch::new(
                                cx,
                                UiData::params,
                                |p| &p.oscs[3].keytrack,
                                "OSC 4",
                                "OSC 4",
                                AMBER_RGB,
                            )
                            .width(Stretch(1.0))
                            .height(Pixels(24.0));
                        })
                        .col_between(Pixels(6.0))
                        .width(Stretch(1.0))
                        .height(Pixels(24.0))
                        .top(Stretch(1.0))
                        .bottom(Stretch(1.0));
                    })
                    .col_between(Pixels(10.0))
                    .child_left(Pixels(8.0))
                    .child_right(Pixels(8.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0))
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
                })
                .class("hardware-module")
                .class("drawer-module")
                .width(Stretch(1.38))
                .height(Stretch(1.0));

                VStack::new(cx, {
                    let params_v = params.clone();
                    let gui_v = gui_context.clone();
                    let mod_ui_v = mod_ui.clone();
                    move |cx| {
                        build_module_header(cx, "VOICING", 18.0);
                        HStack::new(cx, move |cx| {
                            VStack::new(cx, move |cx| {
                                bold_label(cx, "MODE", "mini-label")
                                    .width(Stretch(1.0))
                                    .height(Pixels(13.0))
                                    .child_space(Stretch(1.0));
                                build_bool_dropdown(
                                    cx,
                                    params_v.clone(),
                                    gui_v.clone(),
                                    |p| &p.mono_legato,
                                    "Poly",
                                    "Mono",
                                )
                                .width(Stretch(1.0))
                                .height(Pixels(24.0));
                            })
                            .row_between(Pixels(2.0))
                            .width(Pixels(62.0))
                            .height(Auto)
                            .top(Stretch(1.0))
                            .bottom(Stretch(1.0));

                            build_mini_pill_cell(
                                cx,
                                UiData::params,
                                |p| &p.pitch_bend_range,
                                "PB RANGE",
                                24,
                                52.0,
                            );

                            LedToggleSwitch::new(
                                cx,
                                UiData::params,
                                |p| &p.mpe_enabled,
                                "MPE",
                                "MPE",
                                AMBER_RGB,
                            )
                            .width(Pixels(54.0))
                            .height(Pixels(22.0))
                            .top(Stretch(1.0))
                            .bottom(Stretch(1.0));

                            build_mini_pill_cell(
                                cx,
                                UiData::params,
                                |p| &p.mpe_pitch_bend_range,
                                "MPE PB",
                                48,
                                50.0,
                            );

                            build_flex_knob_cell_mod(
                                cx,
                                UiData::params,
                                |p| &p.glide_ms,
                                "GLIDE",
                                false,
                                GOLD_RGB,
                                32.0,
                                ModDestination::GlideMs,
                                mod_ui_v.clone(),
                                bottom_tab_tag(BottomTab::VoiceEnv),
                            );
                        })
                        .col_between(Pixels(6.0))
                        .child_left(Pixels(6.0))
                        .child_right(Pixels(6.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0))
                        .width(Stretch(1.0))
                        .height(Stretch(1.0));
                    }
                })
                .class("hardware-module")
                .class("drawer-module")
                .width(Stretch(1.04))
                .height(Stretch(1.0));

                VStack::new(cx, {
                    let params = params.clone();
                    let mod_ui = mod_ui.clone();
                    move |cx| {
                        build_module_header(cx, "OSCILLATOR PAN", 18.0);
                        HStack::new(cx, move |cx| {
                            const PAN_LABELS: [&str; 4] =
                                ["OSC 1 PAN", "OSC 2 PAN", "OSC 3 PAN", "OSC 4 PAN"];
                            const PAN_ACCENTS: [(u8, u8, u8); 4] =
                                [AMBER_RGB, GOLD_RGB, CORAL_RGB, AMBER_RGB];
                            for idx in 0..4 {
                                build_flex_knob_cell_mod_dim(
                                    cx,
                                    UiData::params,
                                    move |p| &p.oscs[idx].pan,
                                    PAN_LABELS[idx],
                                    true,
                                    PAN_ACCENTS[idx],
                                    32.0,
                                    ModDestination::osc_pan(idx),
                                    mod_ui.clone(),
                                    bottom_tab_tag(BottomTab::VoiceEnv),
                                    params.clone(),
                                    osc_enabled_fn(idx),
                                );
                            }
                        })
                        .col_between(Pixels(8.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0))
                        .width(Stretch(1.0))
                        .height(Stretch(1.0));
                    }
                })
                .class("hardware-module")
                .class("drawer-module")
                .width(Stretch(0.88))
                .height(Stretch(1.0));
            })
            .col_between(Pixels(6.0))
            .width(Stretch(1.0))
            .height(Stretch(1.0));
}
