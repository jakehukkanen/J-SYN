//! `VOICE` Center Stage tab: per-oscillator Unison / Phase / Retrig / Pan / Keytrack rows,
//! global Voicing (`Poly`/`Mono`, `PB Range`, `[• MPE]`, `MPE PB`, `GLIDE`), and Filter Keytracking.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use nih_plug::prelude::GuiContext;
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{
    bold_label, build_bool_dropdown, build_flex_knob_cell_mod, build_flex_knob_cell_mod_dim,
    build_knob_cell, build_mini_pill_cell, build_module_header, build_stacked_pill_cell,
    build_stacked_pill_cell_mod,
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
        // Top Card: Per-oscillator Retrig, Phase, Unison (Voices/Detune/Spread), Pan & Keytrack
        let p_osc = params.clone();
        let m_osc = mod_ui.clone();
        VStack::new(cx, move |cx| {
            build_module_header(cx, "OSCILLATOR UNISON, PHASE & KEYTRACK", 18.0);

            const OSC_LABELS: [&str; 4] = ["OSC 1", "OSC 2", "OSC 3", "OSC 4"];
            const OSC_ACCENTS: [(u8, u8, u8); 4] = [AMBER_RGB, GOLD_RGB, CORAL_RGB, AMBER_RGB];

            for idx in 0..4 {
                let p_row = p_osc.clone();
                let m_row = m_osc.clone();
                let p_det = p_osc.clone();
                let p_spr = p_osc.clone();
                let accent = OSC_ACCENTS[idx];
                HStack::new(cx, move |cx| {
                    bold_label(cx, OSC_LABELS[idx], "module-title")
                        .width(Pixels(38.0))
                        .height(Stretch(1.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0));

                    LedToggleSwitch::new(
                        cx,
                        UiData::params,
                        move |p| &p.oscs[idx].retrig,
                        "RETRG",
                        "RETRG",
                        accent,
                    )
                    .width(Pixels(46.0))
                    .height(Pixels(20.0))
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));

                    build_stacked_pill_cell_mod(
                        cx,
                        UiData::params,
                        move |p| &p.oscs[idx].phase,
                        "PHASE",
                        360,
                        ModDestination::osc_phase(idx),
                        m_row.clone(),
                        tab_tag,
                    );

                    build_stacked_pill_cell(
                        cx,
                        UiData::params,
                        move |p| &p.oscs[idx].unison,
                        "UNI",
                        8,
                    );

                    build_stacked_pill_cell_mod(
                        cx,
                        UiData::params,
                        move |p| &p.oscs[idx].detune,
                        "DETUNE",
                        100,
                        ModDestination::osc_detune(idx),
                        m_row.clone(),
                        tab_tag,
                    )
                    .toggle_class(
                        "pill-dimmed",
                        UiData::mod_ui_rev.map(move |_| p_det.oscs[idx].unison.value() <= 1),
                    );

                    build_stacked_pill_cell_mod(
                        cx,
                        UiData::params,
                        move |p| &p.oscs[idx].spread,
                        "SPREAD",
                        100,
                        ModDestination::osc_spread(idx),
                        m_row.clone(),
                        tab_tag,
                    )
                    .toggle_class(
                        "pill-dimmed",
                        UiData::mod_ui_rev.map(move |_| p_spr.oscs[idx].unison.value() <= 1),
                    );

                    build_flex_knob_cell_mod_dim(
                        cx,
                        UiData::params,
                        move |p| &p.oscs[idx].pan,
                        "PAN",
                        true,
                        accent,
                        tokens::KNOB_MINI,
                        ModDestination::osc_pan(idx),
                        m_row,
                        tab_tag,
                        p_row,
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
                    .width(Pixels(34.0))
                    .height(Pixels(20.0))
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));
                })
                .col_between(Pixels(tokens::SPACE_SM))
                .height(Stretch(1.0))
                .width(Stretch(1.0));
            }
        })
        .class("hardware-module")
        .class("drawer-module")
        .row_between(Pixels(tokens::SPACE_XS))
        .width(Stretch(1.0))
        .height(Stretch(1.0));

        // Bottom Card: Global Voicing, MPE & Filter Keytracking
        VStack::new(cx, move |cx| {
            build_module_header(cx, "VOICING, MPE & FILTER KEYTRACKING", 18.0);
            HStack::new(cx, move |cx| {
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
                    .height(Pixels(20.0));
                })
                .row_between(Pixels(tokens::SPACE_XS))
                .width(Pixels(58.0))
                .height(Auto)
                .top(Stretch(1.0))
                .bottom(Stretch(1.0));

                build_mini_pill_cell(
                    cx,
                    UiData::params,
                    |p| &p.pitch_bend_range,
                    "PB RANGE",
                    24,
                    48.0,
                );

                LedToggleSwitch::new(
                    cx,
                    UiData::params,
                    |p| &p.mpe_enabled,
                    "MPE",
                    "MPE",
                    AMBER_RGB,
                )
                .width(Pixels(48.0))
                .height(Pixels(20.0))
                .top(Stretch(1.0))
                .bottom(Stretch(1.0));

                build_mini_pill_cell(
                    cx,
                    UiData::params,
                    |p| &p.mpe_pitch_bend_range,
                    "MPE PB",
                    48,
                    46.0,
                );

                build_flex_knob_cell_mod(
                    cx,
                    UiData::params,
                    |p| &p.glide_ms,
                    "GLIDE",
                    false,
                    GOLD_RGB,
                    tokens::KNOB_COMPACT,
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
                    tokens::KNOB_COMPACT,
                    48.0,
                );

                build_knob_cell(
                    cx,
                    UiData::params,
                    |p| &p.filters[1].keytrack,
                    "F2 KT",
                    false,
                    CORAL_RGB,
                    tokens::KNOB_COMPACT,
                    48.0,
                );
            })
            .col_between(Pixels(tokens::SPACE_MD))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0))
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        })
        .class("hardware-module")
        .class("drawer-module")
        .row_between(Pixels(tokens::SPACE_XS))
        .width(Stretch(1.0))
        .height(Pixels(78.0));
    })
    .row_between(Pixels(tokens::SPACE_SM))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}
