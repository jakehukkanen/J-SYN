//! Oscillator strip (`OSC 1` - `OSC 4`) builder in compact 3-column layout.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::WavetableBank;
use nih_plug::prelude::{FloatParam, GuiContext};
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{
    bold_label, build_enum_dropdown, build_flex_knob_cell_mod_dim, build_pitch_pill_cell,
    build_stacked_pill_cell, build_stacked_pill_cell_mod, build_wavetable_dropdown,
    step_wavetable,
};
use super::mod_bar::ModUiState;
use super::scope::WavetableScope;
use super::widgets::LedToggleSwitch;
use super::UiData;
use crate::{
    JSynPluginParams, ModOrderParam, ModSourceParam, ModTypeParam, OscPluginParams,
    WarpModeParam,
};

#[inline]
fn mod_source_idx(src: ModSourceParam) -> Option<usize> {
    match src {
        ModSourceParam::Off => None,
        ModSourceParam::Osc1 => Some(0),
        ModSourceParam::Osc2 => Some(1),
        ModSourceParam::Osc3 => Some(2),
        ModSourceParam::Osc4 => Some(3),
    }
}

/// Returns `true` when oscillator `osc_idx` has base output above zero or is routed
/// (directly or transitively) as a cross-modulation source by another oscillator.
pub(super) fn is_osc_awake(p: &JSynPluginParams, osc_idx: usize) -> bool {
    let mut awake = [false; 4];
    for i in 0..4 {
        if p.oscs[i].level.value() > 0.0001 {
            awake[i] = true;
        }
        if let Some(src) = mod_source_idx(p.oscs[i].mod_source.value()) {
            if src != i {
                awake[src] = true;
            }
        }
    }
    for _ in 0..3 {
        let prev = awake;
        for i in 0..4 {
            if prev[i] {
                if let Some(src) = mod_source_idx(p.oscs[i].mod_source.value()) {
                    awake[src] = true;
                }
            }
        }
    }
    awake[osc_idx]
}

fn osc0_enabled(p: &JSynPluginParams) -> bool {
    is_osc_awake(p, 0)
}
fn osc1_enabled(p: &JSynPluginParams) -> bool {
    is_osc_awake(p, 1)
}
fn osc2_enabled(p: &JSynPluginParams) -> bool {
    is_osc_awake(p, 2)
}
fn osc3_enabled(p: &JSynPluginParams) -> bool {
    is_osc_awake(p, 3)
}

#[inline]
pub(super) fn osc_enabled_fn(osc_idx: usize) -> fn(&JSynPluginParams) -> bool {
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

    HStack::new(cx, move |cx| {
        // Column 1: OSC title, PITCH drag pill, RETRIG toggle
        let p_pitch = mod_ui.clone();
        let gui_left = gui_context.clone();
        VStack::new(cx, move |cx| {
            bold_label(cx, osc_label, "module-title")
                .width(Stretch(1.0))
                .height(Pixels(16.0));

            build_pitch_pill_cell(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).pitch,
                "PITCH",
                ModDestination::osc_pitch(osc_idx),
                p_pitch,
                0,
            )
            .top(Stretch(1.0))
            .bottom(Stretch(1.0));

            LedToggleSwitch::new(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).retrig,
                "RETRIG",
                "RETRIG",
                accent,
            )
            .width(Stretch(1.0))
            .height(Pixels(20.0));
        })
        .width(Pixels(64.0))
        .height(Stretch(1.0));

        // Column 2: Wavetable dropdown + < > step arrows, single-cycle scope, WARP & MOD rows
        let p_center = params.clone();
        let bank_center = bank.clone();
        VStack::new(cx, move |cx| {
            let p_wt = p_center.clone();
            let gui_wt = gui_left.clone();
            let bank_wt = bank_center.clone();
            HStack::new(cx, move |cx| {
                build_wavetable_dropdown(cx, osc_idx, bank_wt, p_wt.clone(), gui_wt.clone())
                    .width(Stretch(1.0))
                    .height(Pixels(20.0));

                let p_prev = p_wt.clone();
                let g_prev = gui_wt.clone();
                Button::new(
                    cx,
                    move |_| step_wavetable(&p_prev, &g_prev, osc_idx, -1),
                    move |cx| bold_label(cx, "<", "wt-step-label").hoverable(false),
                )
                .class("wt-step-btn")
                .width(Pixels(18.0))
                .height(Pixels(20.0));

                let p_next = p_wt;
                let g_next = gui_wt;
                Button::new(
                    cx,
                    move |_| step_wavetable(&p_next, &g_next, osc_idx, 1),
                    move |cx| bold_label(cx, ">", "wt-step-label").hoverable(false),
                )
                .class("wt-step-btn")
                .width(Pixels(18.0))
                .height(Pixels(20.0));
            })
            .col_between(Pixels(2.0))
            .width(Stretch(1.0))
            .height(Pixels(20.0));

            WavetableScope::new(cx, osc_idx, bank_center, p_center.clone())
                .width(Stretch(1.0))
                .height(Stretch(1.0));

            let p_warp = p_center.clone();
            let g_warp = gui_left.clone();
            HStack::new(cx, move |cx| {
                bold_label(cx, "WARP", "mini-label")
                    .width(Pixels(32.0))
                    .height(Stretch(1.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0));

                build_enum_dropdown(
                    cx,
                    p_warp,
                    g_warp,
                    move |p| &get_osc(p, osc_idx).warp_mode,
                    WARP_ITEMS,
                )
                .width(Stretch(1.0))
                .height(Pixels(20.0));
            })
            .col_between(Pixels(4.0))
            .width(Stretch(1.0))
            .height(Pixels(20.0));

            HStack::new(cx, move |cx| {
                bold_label(cx, "MOD", "mini-label")
                    .width(Pixels(24.0))
                    .height(Stretch(1.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0));

                build_enum_dropdown(
                    cx,
                    p_center.clone(),
                    gui_left.clone(),
                    move |p| &get_osc(p, osc_idx).mod_source,
                    MOD_SRC_ITEMS,
                )
                .width(Stretch(1.0))
                .height(Pixels(20.0));

                build_enum_dropdown(
                    cx,
                    p_center.clone(),
                    gui_left.clone(),
                    move |p| &get_osc(p, osc_idx).mod_type,
                    MOD_TYPE_ITEMS,
                )
                .width(Pixels(40.0))
                .height(Pixels(20.0));

                build_enum_dropdown(
                    cx,
                    p_center,
                    gui_left,
                    move |p| &get_osc(p, osc_idx).mod_order,
                    MOD_ORDER_ITEMS,
                )
                .width(Pixels(54.0))
                .height(Pixels(20.0));
            })
            .col_between(Pixels(2.0))
            .width(Stretch(1.0))
            .height(Pixels(20.0));
        })
        .width(Pixels(184.0))
        .height(Stretch(1.0))
        .row_between(Pixels(3.0));

        // Column 3: Stacked PHASE / UNI / DETUNE / SPREAD pills over 4 large 46px knobs
        let p_right = params.clone();
        let ui_right = mod_ui.clone();
        VStack::new(cx, move |cx| {
            let p_pills = p_right.clone();
            let ui_pills = ui_right.clone();
            HStack::new(cx, move |cx| {
                build_stacked_pill_cell_mod(
                    cx,
                    UiData::params,
                    move |p| &get_osc(p, osc_idx).phase,
                    "PHASE",
                    360,
                    ModDestination::osc_phase(osc_idx),
                    ui_pills.clone(),
                    0,
                );

                build_stacked_pill_cell(
                    cx,
                    UiData::params,
                    move |p| &get_osc(p, osc_idx).unison,
                    "UNI",
                    8,
                );

                let p_det = p_pills.clone();
                build_stacked_pill_cell_mod(
                    cx,
                    UiData::params,
                    move |p| &get_osc(p, osc_idx).detune,
                    "DETUNE",
                    100,
                    ModDestination::osc_detune(osc_idx),
                    ui_pills.clone(),
                    0,
                )
                .toggle_class(
                    "pill-dimmed",
                    UiData::mod_ui_rev.map(move |_| get_osc(&p_det, osc_idx).unison.value() <= 1),
                );

                let p_spr = p_pills;
                build_stacked_pill_cell_mod(
                    cx,
                    UiData::params,
                    move |p| &get_osc(p, osc_idx).spread,
                    "SPREAD",
                    100,
                    ModDestination::osc_spread(osc_idx),
                    ui_pills,
                    0,
                )
                .toggle_class(
                    "pill-dimmed",
                    UiData::mod_ui_rev.map(move |_| get_osc(&p_spr, osc_idx).unison.value() <= 1),
                );
            })
            .col_between(Pixels(6.0))
            .width(Stretch(1.0))
            .height(Pixels(33.0));

            HStack::new(cx, move |cx| {
                let add_knob = |cx: &mut Context,
                                field: fn(&OscPluginParams) -> &FloatParam,
                                title: &'static str,
                                dest: ModDestination| {
                    build_flex_knob_cell_mod_dim(
                        cx,
                        UiData::params,
                        move |p| field(&p.oscs[osc_idx]),
                        title,
                        false,
                        accent,
                        46.0,
                        dest,
                        ui_right.clone(),
                        0,
                        p_right.clone(),
                        osc_check,
                    );
                };

                add_knob(cx, |o| &o.wt_pos, "WT POS", ModDestination::osc_wt_pos(osc_idx));
                add_knob(cx, |o| &o.warp_amount, "WARP", ModDestination::osc_warp(osc_idx));
                add_knob(cx, |o| &o.mod_depth, "MOD", ModDestination::osc_mod_depth(osc_idx));
                add_knob(cx, |o| &o.level, "LEVEL", ModDestination::osc_level(osc_idx));
            })
            .col_between(Pixels(6.0))
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        })
        .width(Stretch(1.0))
        .height(Stretch(1.0))
        .row_between(Pixels(4.0));
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
