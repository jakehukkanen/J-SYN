//! Oscillator strip (`OSC 1` - `OSC 4`) builder.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::WavetableBank;
use nih_plug::prelude::GuiContext;
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{
    bold_label, build_enum_dropdown, build_flex_knob_cell_mod_dim, build_inline_pill_cell,
    build_inline_pill_cell_mod, build_wavetable_dropdown,
};
use super::mod_bar::ModUiState;
use super::scope::WavetableScope;
use super::widgets::LedToggleSwitch;
use super::UiData;
use crate::{
    JSynPluginParams, ModOrderParam, ModSourceParam, ModTypeParam, OscPluginParams,
    WarpModeParam,
};

fn osc0_enabled(p: &JSynPluginParams) -> bool {
    p.oscs[0].enabled.value()
}
fn osc1_enabled(p: &JSynPluginParams) -> bool {
    p.oscs[1].enabled.value()
}
fn osc2_enabled(p: &JSynPluginParams) -> bool {
    p.oscs[2].enabled.value()
}
fn osc3_enabled(p: &JSynPluginParams) -> bool {
    p.oscs[3].enabled.value()
}

#[inline]
fn osc_enabled_fn(osc_idx: usize) -> fn(&JSynPluginParams) -> bool {
    match osc_idx {
        0 => osc0_enabled,
        1 => osc1_enabled,
        2 => osc2_enabled,
        _ => osc3_enabled,
    }
}

pub(super) fn build_osc_panel(
    cx: &mut Context,
    osc_idx: usize,
    bank: Arc<WavetableBank>,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
    accent: (u8, u8, u8),
) {
    const WARP_ITEMS: &[(&str, WarpModeParam)] = &[
        ("None", WarpModeParam::None),
        ("Bend-", WarpModeParam::BendMinus),
        ("Bend+", WarpModeParam::BendPlus),
        ("Spectrum", WarpModeParam::Spectrum),
    ];
    const MOD_SRC_ITEMS: &[(&str, ModSourceParam)] = &[
        ("Off", ModSourceParam::Off),
        ("Osc 1", ModSourceParam::Osc1),
        ("Osc 2", ModSourceParam::Osc2),
        ("Osc 3", ModSourceParam::Osc3),
        ("Osc 4", ModSourceParam::Osc4),
    ];
    const MOD_TYPE_ITEMS: &[(&str, ModTypeParam)] =
        &[("PM", ModTypeParam::Pm), ("RM", ModTypeParam::Rm)];
    const MOD_ORDER_ITEMS: &[(&str, ModOrderParam)] = &[
        ("PM>W", ModOrderParam::PmThenWarp),
        ("W>PM", ModOrderParam::WarpThenPm),
    ];

    let osc_label = match osc_idx {
        0 => "OSC 1",
        1 => "OSC 2",
        2 => "OSC 3",
        _ => "OSC 4",
    };

    #[inline]
    fn get_osc(p: &Arc<JSynPluginParams>, idx: usize) -> &OscPluginParams {
        &p.oscs[idx]
    }

    let osc_check = osc_enabled_fn(osc_idx);
    let p_dim = params.clone();
    let p_knobs = params.clone();

    HStack::new(cx, move |cx| {
        // Left sub-block: Header (OSC toggle + WT dropdown + RETRIG), Scope, and Warp/Mod dropdowns below
        VStack::new(cx, |cx| {
            HStack::new(cx, |cx| {
                LedToggleSwitch::new(
                    cx,
                    UiData::params,
                    move |p| &get_osc(p, osc_idx).enabled,
                    osc_label,
                    osc_label,
                    accent,
                )
                .width(Pixels(54.0))
                .height(Pixels(20.0));

                build_wavetable_dropdown(
                    cx,
                    osc_idx,
                    bank.clone(),
                    params.clone(),
                    gui_context.clone(),
                )
                .width(Stretch(1.0))
                .height(Pixels(20.0));

                LedToggleSwitch::new(
                    cx,
                    UiData::params,
                    move |p| &get_osc(p, osc_idx).retrig,
                    "RETRIG",
                    "RETRIG",
                    accent,
                )
                .width(Pixels(64.0))
                .height(Pixels(20.0))
                .z_index(10);
            })
            .col_between(Pixels(4.0))
            .height(Pixels(20.0))
            .width(Stretch(1.0));

            WavetableScope::new(cx, osc_idx, bank.clone(), params.clone())
                .width(Stretch(1.0))
                .height(Stretch(1.0));

            HStack::new(cx, |cx| {
                bold_label(cx, "WARP", "mini-label")
                    .width(Pixels(34.0))
                    .height(Stretch(1.0))
                    .child_space(Stretch(1.0));
                build_enum_dropdown(
                    cx,
                    params.clone(),
                    gui_context.clone(),
                    move |p| &get_osc(p, osc_idx).warp_mode,
                    WARP_ITEMS,
                )
                .width(Stretch(1.0))
                .height(Pixels(19.0));
            })
            .col_between(Pixels(4.0))
            .height(Pixels(19.0))
            .width(Stretch(1.0));

            HStack::new(cx, |cx| {
                bold_label(cx, "MOD", "mini-label")
                    .width(Pixels(28.0))
                    .height(Stretch(1.0))
                    .child_space(Stretch(1.0));
                build_enum_dropdown(
                    cx,
                    params.clone(),
                    gui_context.clone(),
                    move |p| &get_osc(p, osc_idx).mod_source,
                    MOD_SRC_ITEMS,
                )
                .width(Stretch(1.0))
                .height(Pixels(19.0));
                build_enum_dropdown(
                    cx,
                    params.clone(),
                    gui_context.clone(),
                    move |p| &get_osc(p, osc_idx).mod_type,
                    MOD_TYPE_ITEMS,
                )
                .width(Pixels(40.0))
                .height(Pixels(19.0));
                build_enum_dropdown(
                    cx,
                    params.clone(),
                    gui_context.clone(),
                    move |p| &get_osc(p, osc_idx).mod_order,
                    MOD_ORDER_ITEMS,
                )
                .width(Pixels(52.0))
                .height(Pixels(19.0));
            })
            .col_between(Pixels(3.0))
            .height(Pixels(19.0))
            .width(Stretch(1.0));
        })
        .width(Pixels(244.0))
        .height(Stretch(1.0))
        .row_between(Pixels(3.0));

        // Right sub-block: 3-tier layout (Pitch/Phase row -> 5 Knobs -> Compact Unison strip)
        let p_det = p_knobs.clone();
        let p_spr = p_knobs.clone();
        VStack::new(cx, move |cx| {
            let ui_pitch = mod_ui.clone();
            HStack::new(cx, move |cx| {
                build_inline_pill_cell(
                    cx,
                    UiData::params,
                    move |p| &get_osc(p, osc_idx).octave,
                    "OCT",
                    9,
                    1.0,
                );
                build_inline_pill_cell_mod(
                    cx,
                    UiData::params,
                    move |p| &get_osc(p, osc_idx).semi,
                    "SEMI",
                    25,
                    1.0,
                    ModDestination::osc_semi(osc_idx),
                    ui_pitch.clone(),
                    0,
                );
                build_inline_pill_cell_mod(
                    cx,
                    UiData::params,
                    move |p| &get_osc(p, osc_idx).fine,
                    "FINE",
                    201,
                    1.0,
                    ModDestination::osc_fine(osc_idx),
                    ui_pitch.clone(),
                    0,
                );
                build_inline_pill_cell_mod(
                    cx,
                    UiData::params,
                    move |p| &get_osc(p, osc_idx).phase,
                    "PH",
                    361,
                    1.0,
                    ModDestination::osc_phase(osc_idx),
                    ui_pitch,
                    0,
                );
            })
            .class("osc-inline-strip")
            .col_between(Pixels(8.0))
            .height(Pixels(18.0))
            .width(Stretch(1.0));

            let ui_knobs = mod_ui.clone();
            let p_k1 = p_knobs.clone();
            let p_k2 = p_knobs.clone();
            let p_k3 = p_knobs.clone();
            let p_k4 = p_knobs.clone();
            let p_k5 = p_knobs;
            HStack::new(cx, move |cx| {
                build_flex_knob_cell_mod_dim(
                    cx,
                    UiData::params,
                    move |p| &get_osc(p, osc_idx).wt_pos,
                    "WT POS",
                    false,
                    accent,
                    42.0,
                    ModDestination::osc_wt_pos(osc_idx),
                    ui_knobs.clone(),
                    0,
                    p_k1,
                    osc_check,
                );
                build_flex_knob_cell_mod_dim(
                    cx,
                    UiData::params,
                    move |p| &get_osc(p, osc_idx).warp_amount,
                    "WARP",
                    false,
                    accent,
                    42.0,
                    ModDestination::osc_warp(osc_idx),
                    ui_knobs.clone(),
                    0,
                    p_k2,
                    osc_check,
                );
                build_flex_knob_cell_mod_dim(
                    cx,
                    UiData::params,
                    move |p| &get_osc(p, osc_idx).mod_depth,
                    "MOD",
                    false,
                    accent,
                    42.0,
                    ModDestination::osc_mod_depth(osc_idx),
                    ui_knobs.clone(),
                    0,
                    p_k3,
                    osc_check,
                );
                build_flex_knob_cell_mod_dim(
                    cx,
                    UiData::params,
                    move |p| &get_osc(p, osc_idx).pan,
                    "PAN",
                    true,
                    accent,
                    42.0,
                    ModDestination::osc_pan(osc_idx),
                    ui_knobs.clone(),
                    0,
                    p_k4,
                    osc_check,
                );
                build_flex_knob_cell_mod_dim(
                    cx,
                    UiData::params,
                    move |p| &get_osc(p, osc_idx).level,
                    "LEVEL",
                    false,
                    accent,
                    42.0,
                    ModDestination::osc_level(osc_idx),
                    ui_knobs,
                    0,
                    p_k5,
                    osc_check,
                );
            })
            .col_between(Pixels(4.0))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0))
            .width(Stretch(1.0))
            .height(Stretch(1.0));

            let ui_uni = mod_ui;
            HStack::new(cx, move |cx| {
                build_inline_pill_cell(
                    cx,
                    UiData::params,
                    move |p| &get_osc(p, osc_idx).unison,
                    "UNI",
                    8,
                    1.0,
                );
                build_inline_pill_cell_mod(
                    cx,
                    UiData::params,
                    move |p| &get_osc(p, osc_idx).detune,
                    "DET",
                    101,
                    1.0,
                    ModDestination::osc_detune(osc_idx),
                    ui_uni.clone(),
                    0,
                )
                .toggle_class(
                    "pill-dimmed",
                    UiData::mod_ui_rev.map(move |_| p_det.oscs[osc_idx].unison.value() <= 1),
                );
                build_inline_pill_cell_mod(
                    cx,
                    UiData::params,
                    move |p| &get_osc(p, osc_idx).spread,
                    "SPR",
                    101,
                    1.0,
                    ModDestination::osc_spread(osc_idx),
                    ui_uni,
                    0,
                )
                .toggle_class(
                    "pill-dimmed",
                    UiData::mod_ui_rev.map(move |_| p_spr.oscs[osc_idx].unison.value() <= 1),
                );
            })
            .class("osc-inline-strip")
            .col_between(Pixels(10.0))
            .height(Pixels(18.0))
            .width(Stretch(1.0));
        })
        .width(Stretch(1.0))
        .height(Stretch(1.0))
        .row_between(Pixels(2.0));
    })
    .class("hardware-module")
    .toggle_class(
        "module-disabled",
        UiData::mod_ui_rev.map(move |_| !osc_check(&p_dim)),
    )
    .col_between(Pixels(8.0))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}
