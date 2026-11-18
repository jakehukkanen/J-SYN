//! `VOICE` Center Stage tab: side-by-side per-oscillator Pan / Keytrack columns,
//! stacked global Voicing (`MODE`/`PB RANGE`, `MPE`/`MPE PB`), `GLIDE`, and Filter Keytracking.

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
use super::tokens;
use super::widgets::LedToggleSwitch;
use super::{BottomTab, UiData, AMBER_RGB, CORAL_RGB, GOLD_RGB};
use crate::JSynPluginParams;

pub(super) fn build_voice_tab(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    let tab_tag = bottom_tab_tag(BottomTab::VoiceEnv);

    VStack::new(cx, move |cx| {
        // Top Card: Per-oscillator Pan & Keytrack arranged side-by-side
        let p_osc = params.clone();
        let m_osc = mod_ui.clone();
        VStack::new(cx, move |cx| {
            build_module_header(cx, "OSCILLATOR PAN & KEYTRACK", 18.0);

            const OSC_LABELS: [&str; 4] = ["OSC 1", "OSC 2", "OSC 3", "OSC 4"];
            const OSC_ACCENTS: [(u8, u8, u8); 4] = [AMBER_RGB, GOLD_RGB, CORAL_RGB, AMBER_RGB];

            HStack::new(cx, move |cx| {
                for idx in 0..4 {
                    let p_col = p_osc.clone();
                    let m_col = m_osc.clone();
                    let accent = OSC_ACCENTS[idx];
                    VStack::new(cx, move |cx| {
                        bold_label(cx, OSC_LABELS[idx], "module-title")
                            .width(Stretch(1.0))
                            .height(Pixels(16.0))
                            .child_space(Stretch(1.0));

                        build_flex_knob_cell_mod_dim(
                            cx,
                            UiData::params,
                            move |p| &p.oscs[idx].pan,
                            "PAN",
                            true,
                            accent,
                            tokens::KNOB_STD,
                            ModDestination::osc_pan(idx),
                            m_col,
                            tab_tag,
                            p_col,
                            osc_enabled_fn(idx),
                        );

                        LedToggleSwitch::new(
                            cx,
                            UiData::params,
                            move |p| &p.oscs[idx].keytrack,
                            "KT",
                            "KT",
                            accent,
                        )
                        .width(Pixels(tokens::VOICE_KT_W))
                        .height(Pixels(tokens::CONTROL_H))
                        .left(Stretch(1.0))
                        .right(Stretch(1.0));
                    })
                    .class("knob-cell")
                    .row_between(Pixels(tokens::SPACE_SM))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0))
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
                }
            })
            .col_between(Pixels(tokens::SPACE_MD))
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        })
        .class("hardware-module")
        .class("drawer-module")
        .row_between(Pixels(tokens::SPACE_XS))
        .width(Stretch(1.0))
        .height(Stretch(1.0));

        // Bottom Card: Global Voicing (stacked vertically), MPE, Glide & Filter Keytracking
        VStack::new(cx, move |cx| {
            build_module_header(cx, "VOICING, MPE & FILTER KEYTRACKING", tokens::PILL_H);
            HStack::new(cx, move |cx| {
                // Stacked Voicing column: MODE over PB RANGE
                VStack::new(cx, move |cx| {
                    VStack::new(cx, move |cx| {
                        bold_label(cx, "MODE", "mini-label")
                            .width(Stretch(1.0))
                            .height(Pixels(12.0))
                            .child_space(Stretch(1.0));
                        build_bool_dropdown(
                            cx,
                            params.clone(),
                            gui_context.clone(),
                            |p| &p.mono_legato,
                            "Poly",
                            "Mono",
                        )
                        .width(Stretch(1.0))
                        .height(Pixels(tokens::CONTROL_H));
                    })
                    .row_between(Pixels(tokens::SPACE_XS))
                    .width(Stretch(1.0))
                    .height(Auto);

                    build_mini_pill_cell(
                        cx,
                        UiData::params,
                        |p| &p.pitch_bend_range,
                        "PB RANGE",
                        24,
                        tokens::VOICE_COL_MODE_W,
                    );
                })
                .row_between(Pixels(tokens::SPACE_SM))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .width(Pixels(tokens::VOICE_COL_MODE_W))
                .height(Stretch(1.0));

                // Stacked MPE column: MPE toggle over MPE PB
                VStack::new(cx, move |cx| {
                    LedToggleSwitch::new(
                        cx,
                        UiData::params,
                        |p| &p.mpe_enabled,
                        "MPE",
                        "MPE",
                        AMBER_RGB,
                    )
                    .width(Stretch(1.0))
                    .height(Pixels(tokens::CONTROL_H));

                    build_mini_pill_cell(
                        cx,
                        UiData::params,
                        |p| &p.mpe_pitch_bend_range,
                        "MPE PB",
                        48,
                        tokens::VOICE_COL_MPE_W,
                    );
                })
                .row_between(Pixels(tokens::SPACE_SM))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .width(Pixels(tokens::VOICE_COL_MPE_W))
                .height(Stretch(1.0));

                build_flex_knob_cell_mod(
                    cx,
                    UiData::params,
                    |p| &p.glide_ms,
                    "GLIDE",
                    false,
                    GOLD_RGB,
                    tokens::KNOB_STD,
                    ModDestination::GlideMs,
                    mod_ui,
                    tab_tag,
                );

                build_knob_cell(
                    cx,
                    UiData::params,
                    |p| &p.filters[0].keytrack,
                    "F1 KT",
                    false,
                    AMBER_RGB,
                    tokens::KNOB_STD,
                    tokens::VOICE_FLT_KT_CELL_W,
                );

                build_knob_cell(
                    cx,
                    UiData::params,
                    |p| &p.filters[1].keytrack,
                    "F2 KT",
                    false,
                    CORAL_RGB,
                    tokens::KNOB_STD,
                    tokens::VOICE_FLT_KT_CELL_W,
                );
            })
            .col_between(Pixels(tokens::SPACE_LG))
            .child_left(Pixels(tokens::SPACE_SM))
            .child_right(Pixels(tokens::SPACE_SM))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0))
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        })
        .class("hardware-module")
        .class("drawer-module")
        .row_between(Pixels(tokens::SPACE_XS))
        .width(Stretch(1.0))
        .height(Pixels(tokens::VOICE_VOICING_H));
    })
    .row_between(Pixels(tokens::SPACE_SM))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}
