//! Compact `INSERT SHAPERS` panel (`INS 1` and `INS 2`) below `FILTER 2`.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use nih_plug::prelude::GuiContext;
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{
    bold_label, build_enum_dropdown, build_flex_knob_cell_mod_dim, build_module_header,
};
use super::mod_bar::ModUiState;
use super::{AMBER_RGB, CORAL_RGB, UiData};
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

                HStack::new(cx, move |cx| {
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
            move |cx| {
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
                    p_knob_dim,
                    slot_check,
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
        build_module_header(cx, "INSERT SHAPERS", 18.0);

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
