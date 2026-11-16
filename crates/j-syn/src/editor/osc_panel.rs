//! Oscillator strip (`OSC 1` - `OSC 4`) builder in compact 3-column layout.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::WavetableBank;
use nih_plug::prelude::{FloatParam, GuiContext, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{
    bold_label, build_dropdown_trigger_labels, build_enum_dropdown, build_flex_knob_cell_mod_dim,
    build_pitch_pill_cell, build_stacked_pill_cell, build_stacked_pill_cell_mod,
    build_wavetable_dropdown, step_wavetable,
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
    const UNIFIED_MOD_ITEMS: &[(&str, ModSourceParam, ModTypeParam)] = &[
        ("Off", ModSourceParam::Off, ModTypeParam::Pm),
        ("PM Osc 1", ModSourceParam::Osc1, ModTypeParam::Pm),
        ("PM Osc 2", ModSourceParam::Osc2, ModTypeParam::Pm),
        ("PM Osc 3", ModSourceParam::Osc3, ModTypeParam::Pm),
        ("PM Osc 4", ModSourceParam::Osc4, ModTypeParam::Pm),
        ("RM Osc 1", ModSourceParam::Osc1, ModTypeParam::Rm),
        ("RM Osc 2", ModSourceParam::Osc2, ModTypeParam::Rm),
        ("RM Osc 3", ModSourceParam::Osc3, ModTypeParam::Rm),
        ("RM Osc 4", ModSourceParam::Osc4, ModTypeParam::Rm),
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
            LedToggleSwitch::new(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).enabled,
                osc_label,
                osc_label,
                accent,
            )
            .width(Stretch(1.0))
            .height(Pixels(20.0));

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
        .height(Stretch(1.0))
        .row_between(Pixels(4.0));

        // Column 2: [<] [Wavetable v] [>], single-cycle scope, WARP & MOD rows
        let p_center = params.clone();
        let bank_center = bank.clone();
        let ui_scope = mod_ui.clone();
        VStack::new(cx, move |cx| {
            let p_wt = p_center.clone();
            let gui_wt = gui_left.clone();
            let bank_wt = bank_center.clone();
            HStack::new(cx, move |cx| {
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

                build_wavetable_dropdown(cx, osc_idx, bank_wt, p_wt.clone(), gui_wt.clone())
                    .width(Stretch(1.0))
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

            WavetableScope::new(cx, osc_idx, bank_center, p_center, ui_scope)
                .width(Stretch(1.0))
                .height(Stretch(1.0));
        })
        .width(Pixels(184.0))
        .height(Stretch(1.0))
        .row_between(Pixels(4.0));

        // Column 3: Stacked PHASE / UNI / DETUNE / SPREAD pills over 4 knobs + WARP/arrow/MOD row
        let p_right = params.clone();
        let ui_right = mod_ui.clone();
        let gui_right = gui_context.clone();
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

            let p_knobs = p_right.clone();
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
                        44.0,
                        dest,
                        ui_right.clone(),
                        0,
                        p_knobs.clone(),
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

            let p_bot = p_right;
            let g_bot = gui_right;
            HStack::new(cx, move |cx| {
                Element::new(cx).width(Stretch(1.0)).height(Pixels(18.0));

                build_enum_dropdown(
                    cx,
                    p_bot.clone(),
                    g_bot.clone(),
                    move |p| &get_osc(p, osc_idx).warp_mode,
                    WARP_ITEMS,
                )
                .width(Stretch(1.0))
                .height(Pixels(18.0));

                let p_ord = p_bot.clone();
                let g_ord = g_bot.clone();
                let p_ord_lbl = p_bot.clone();
                Button::new(
                    cx,
                    move |_| {
                        let cur = p_ord.oscs[osc_idx].mod_order.value();
                        let next = match cur {
                            ModOrderParam::WarpThenPm => ModOrderParam::PmThenWarp,
                            ModOrderParam::PmThenWarp => ModOrderParam::WarpThenPm,
                        };
                        let setter = ParamSetter::new(g_ord.as_ref());
                        let param = &p_ord.oscs[osc_idx].mod_order;
                        setter.begin_set_parameter(param);
                        setter.set_parameter(param, next);
                        setter.end_set_parameter(param);
                    },
                    move |cx| {
                        let arrow_lens = UiData::params.map(move |_| {
                            match p_ord_lbl.oscs[osc_idx].mod_order.value() {
                                ModOrderParam::WarpThenPm => "->".to_string(),
                                ModOrderParam::PmThenWarp => "<-".to_string(),
                            }
                        });
                        bold_label(cx, arrow_lens, "wt-step-label")
                            .child_space(Stretch(1.0))
                            .hoverable(false)
                    },
                )
                .class("wt-step-btn")
                .width(Pixels(20.0))
                .height(Pixels(18.0));

                let p_mod_trig = p_bot.clone();
                let p_mod_pop = p_bot;
                let g_mod_pop = g_bot;
                Dropdown::new(
                    cx,
                    move |cx| {
                        let p_m = p_mod_trig.clone();
                        HStack::new(cx, move |cx| {
                            build_dropdown_trigger_labels(
                                cx,
                                UiData::params.map(move |_| {
                                    let osc = &p_m.oscs[osc_idx];
                                    let src = osc.mod_source.value();
                                    let ty = osc.mod_type.value();
                                    let prefix = match ty {
                                        ModTypeParam::Pm => "PM",
                                        ModTypeParam::Rm => "RM",
                                    };
                                    match src {
                                        ModSourceParam::Off => "Off".to_string(),
                                        ModSourceParam::Osc1 => format!("{prefix} Osc 1"),
                                        ModSourceParam::Osc2 => format!("{prefix} Osc 2"),
                                        ModSourceParam::Osc3 => format!("{prefix} Osc 3"),
                                        ModSourceParam::Osc4 => format!("{prefix} Osc 4"),
                                    }
                                }),
                            );
                        })
                        .width(Stretch(1.0))
                        .height(Stretch(1.0))
                        .child_top(Pixels(0.0))
                        .child_bottom(Pixels(0.0))
                        .overflow(Overflow::Hidden)
                    },
                    move |cx| {
                        for &(label, src, ty) in UNIFIED_MOD_ITEMS {
                            let p_set = p_mod_pop.clone();
                            let g_set = g_mod_pop.clone();
                            bold_label(cx, label, "dropdown-item").on_press(move |cx| {
                                let setter = ParamSetter::new(g_set.as_ref());
                                let osc = &p_set.oscs[osc_idx];
                                setter.begin_set_parameter(&osc.mod_type);
                                setter.set_parameter(&osc.mod_type, ty);
                                setter.end_set_parameter(&osc.mod_type);
                                setter.begin_set_parameter(&osc.mod_source);
                                setter.set_parameter(&osc.mod_source, src);
                                setter.end_set_parameter(&osc.mod_source);
                                cx.emit(PopupEvent::Close);
                            });
                        }
                    },
                )
                .width(Stretch(1.0))
                .height(Pixels(18.0));

                Element::new(cx).width(Stretch(1.0)).height(Pixels(18.0));
            })
            .col_between(Pixels(3.0))
            .width(Stretch(1.0))
            .height(Pixels(18.0));
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
