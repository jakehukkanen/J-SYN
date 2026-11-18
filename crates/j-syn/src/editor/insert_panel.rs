//! Compact `INSERT SHAPERS` panel (`INS 1` and `INS 2`) below `FILTER 2`.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::{format_freq_shift_hz, freq_shift_coarse_hz};
use nih_plug::prelude::{GuiContext, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{
    bold_label, build_enum_dropdown, build_flex_knob_cell_mod_dim, build_module_header,
};
use super::mod_bar::ModUiState;
use super::widgets::{KnobDimExt, RotaryKnob};
use super::{AMBER_RGB, CORAL_RGB, UiData, UiEvent};
use crate::{InsertModeParam, InsertPosParam, JSynPluginParams};

#[inline]
fn insert0_enabled(p: &JSynPluginParams) -> bool {
    p.routing.inserts[0].mode.value() != InsertModeParam::Off
}

#[inline]
fn insert1_enabled(p: &JSynPluginParams) -> bool {
    p.routing.inserts[1].mode.value() != InsertModeParam::Off
}

#[inline]
fn insert_slot_check(slot_idx: usize) -> fn(&JSynPluginParams) -> bool {
    if slot_idx == 0 {
        insert0_enabled
    } else {
        insert1_enabled
    }
}

pub(super) const INSERT_MODE_ITEMS: &[(&str, InsertModeParam)] = &[
    ("Off", InsertModeParam::Off),
    ("Tube", InsertModeParam::Tube),
    ("Mech", InsertModeParam::Mech),
    ("Tube -> Mech", InsertModeParam::TubeToMech),
    ("Mech -> Tube", InsertModeParam::MechToTube),
    ("Sine Shaper", InsertModeParam::SineShaper),
    ("Parabolic", InsertModeParam::Parabolic),
    ("Soft Clip", InsertModeParam::SoftClip),
    ("Bitcrush", InsertModeParam::Bitcrush),
    ("Classic A", InsertModeParam::ClassicA),
    ("Classic B", InsertModeParam::ClassicB),
    ("Classic C", InsertModeParam::ClassicC),
    ("Diode A", InsertModeParam::DiodeA),
    ("Diode B", InsertModeParam::DiodeB),
    ("Linear Fold", InsertModeParam::LinearFold),
    ("Sine Fold", InsertModeParam::SineFold),
    ("Hard Clip", InsertModeParam::HardClip),
    ("Half Rect", InsertModeParam::HalfRectify),
    ("Full Rect", InsertModeParam::FullRectify),
    ("Freq Shift", InsertModeParam::FreqShifter),
];

pub(super) const INSERT_POS_ITEMS: &[(&str, InsertPosParam)] = &[
    ("Pre-F1", InsertPosParam::PreFilter),
    ("Between F1->F2", InsertPosParam::BetweenFilters),
    ("Post-F2", InsertPosParam::PostFilter),
];

fn build_insert_slot_row(
    cx: &mut Context,
    slot_idx: usize,
    slot_label: &'static str,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
    accent: (u8, u8, u8),
) {
    let slot_check = insert_slot_check(slot_idx);
    let p_pos_dim = params.clone();
    let p_drv_dim = params.clone();
    let p_knob_dim = params.clone();
    HStack::new(cx, move |cx| {
        VStack::new(cx, {
            let params = params.clone();
            let gui_context = gui_context.clone();
            move |cx| {
                HStack::new(cx, {
                    let params = params.clone();
                    let gui_context = gui_context.clone();
                    move |cx| {
                        bold_label(cx, slot_label, "mini-label")
                            .width(Pixels(34.0))
                            .height(Stretch(1.0))
                            .child_space(Stretch(1.0));

                        build_enum_dropdown(
                            cx,
                            params.clone(),
                            gui_context.clone(),
                            move |p| &p.routing.inserts[slot_idx].mode,
                            INSERT_MODE_ITEMS,
                        )
                        .width(Stretch(1.0))
                        .height(Pixels(19.0));
                    }
                })
                .col_between(Pixels(4.0))
                .height(Pixels(19.0))
                .width(Stretch(1.0));

                HStack::new(cx, {
                    let params = params.clone();
                    let gui_context = gui_context.clone();
                    let p_kt_vis = params.clone();
                    let p_kt_act = params.clone();
                    let g_kt_act = gui_context.clone();
                    move |cx| {
                        bold_label(cx, "POS", "mini-label")
                            .width(Pixels(34.0))
                            .height(Stretch(1.0))
                            .child_space(Stretch(1.0));

                        build_enum_dropdown(
                            cx,
                            params.clone(),
                            gui_context.clone(),
                            move |p| &p.routing.inserts[slot_idx].position,
                            INSERT_POS_ITEMS,
                        )
                        .width(Stretch(1.0))
                        .height(Pixels(19.0));

                        Binding::new(
                            cx,
                            UiData::mod_ui_rev.map(move |_| {
                                p_kt_vis.routing.inserts[slot_idx].mode.value()
                                    == InsertModeParam::FreqShifter
                            }),
                            move |cx, is_sh_lens| {
                                if !is_sh_lens.get(cx) {
                                    return;
                                }
                                let p_kt_c = p_kt_act.clone();
                                let g_kt_c = g_kt_act.clone();
                                Button::new(
                                    cx,
                                    move |cx| {
                                        let setter = ParamSetter::new(g_kt_c.as_ref());
                                        let p = &p_kt_c.routing.inserts[slot_idx].keytrack;
                                        let next = !p.value();
                                        setter.begin_set_parameter(p);
                                        setter.set_parameter(p, next);
                                        setter.end_set_parameter(p);
                                        cx.emit(UiEvent::ModUiChanged);
                                    },
                                    |cx| bold_label(cx, "KT", "fx-subtab-lbl").hoverable(false),
                                )
                                .class("fx-scope-pill")
                                .class("fx-scope-flt-btn")
                                .toggle_class(
                                    "fx-subtab-active",
                                    UiData::params.map(move |p| {
                                        p.routing.inserts[slot_idx].keytrack.value()
                                    }),
                                )
                                .width(Pixels(24.0))
                                .height(Pixels(19.0));
                            },
                        );
                    }
                })
                .class("insert-pos-row")
                .toggle_class(
                    "slot-disabled",
                    UiData::mod_ui_rev.map(move |_| !slot_check(&p_pos_dim)),
                )
                .col_between(Pixels(4.0))
                .height(Pixels(19.0))
                .width(Stretch(1.0));
            }
        })
        .width(Stretch(1.0))
        .height(Auto)
        .row_between(Pixels(3.0));

        HStack::new(cx, {
            let mod_ui = mod_ui.clone();
            let p_mode = params.clone();
            move |cx| {
                Binding::new(
                    cx,
                    UiData::mod_ui_rev.map(move |_| {
                        (
                            p_mode.routing.inserts[slot_idx].mode.value()
                                == InsertModeParam::FreqShifter,
                            p_mode.routing.inserts[slot_idx].keytrack.value(),
                        )
                    }),
                    move |cx, mode_lens| {
                        let (is_freq_shift, is_kt) = mode_lens.get(cx);
                        if is_freq_shift {
                            let p_cell = p_knob_dim.clone();
                            let mod_cell = mod_ui.clone();
                            let formatter = Arc::new(move |norm: f32, _off: Option<f32>| {
                                let txt = format_freq_shift_hz(freq_shift_coarse_hz(norm));
                                if is_kt {
                                    format!("{txt} (KT)")
                                } else {
                                    txt
                                }
                            });
                            VStack::new(cx, move |cx| {
                                bold_label(cx, "SHIFT", "knob-title")
                                    .width(Stretch(1.0))
                                    .height(Pixels(12.0))
                                    .child_space(Stretch(1.0));

                                RotaryKnob::new_with_mod_formatter(
                                    cx,
                                    UiData::params,
                                    move |p| &p.routing.inserts[slot_idx].drive,
                                    true,
                                    accent,
                                    ModDestination::insert_drive(slot_idx),
                                    mod_cell,
                                    0,
                                    formatter,
                                )
                                .default_norm(Some(0.5))
                                .enabled_by(p_cell, slot_check)
                                .width(Pixels(34.0))
                                .height(Pixels(34.0))
                                .left(Stretch(1.0))
                                .right(Stretch(1.0));
                            })
                            .class("knob-cell")
                            .width(Stretch(1.0))
                            .height(Auto)
                            .row_between(Pixels(1.0));
                        } else {
                            build_flex_knob_cell_mod_dim(
                                cx,
                                UiData::params,
                                move |p| &p.routing.inserts[slot_idx].drive,
                                "DRIVE",
                                false,
                                accent,
                                34.0,
                                ModDestination::insert_drive(slot_idx),
                                mod_ui.clone(),
                                0,
                                p_knob_dim.clone(),
                                slot_check,
                            );
                        }
                    },
                );
            }
        })
        .class("insert-drive-cell")
        .toggle_class(
            "slot-disabled",
            UiData::mod_ui_rev.map(move |_| !slot_check(&p_drv_dim)),
        )
        .width(Pixels(54.0))
        .height(Auto);
    })
    .class("insert-slot-row")
    .col_between(Pixels(6.0))
    .child_top(Stretch(1.0))
    .child_bottom(Stretch(1.0))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}

pub(super) fn build_insert_panel(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    VStack::new(cx, move |cx| {
        build_module_header(cx, "INSERT FX", 18.0);

        build_insert_slot_row(
            cx,
            0,
            "INS 1",
            params.clone(),
            gui_context.clone(),
            mod_ui.clone(),
            AMBER_RGB,
        );
        build_insert_slot_row(cx, 1, "INS 2", params, gui_context, mod_ui, CORAL_RGB);
    })
    .class("hardware-module")
    .row_between(Pixels(4.0))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}
