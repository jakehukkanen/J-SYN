//! Compact `INSERT SHAPERS` panel (`INS 1` and `INS 2`) below `FILTER 2`.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use nih_plug::prelude::GuiContext;
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{bold_label, build_enum_dropdown, build_flex_knob_cell_mod_dim};
use super::mod_bar::ModUiState;
use super::tokens;
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
];

pub(super) const INSERT_POS_ITEMS: &[(&str, InsertPosParam)] = &[
    ("Pre-F1", InsertPosParam::PreFilter),
    ("Between F1->F2", InsertPosParam::BetweenFilters),
    ("Post-F2", InsertPosParam::PostFilter),
];

fn build_insert_card(
    cx: &mut Context,
    slot_idx: usize,
    slot_label: &'static str,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
    accent: (u8, u8, u8),
) {
    let slot_check = insert_slot_check(slot_idx);
    let p_dim = params.clone();
    let p_pos_dim = params.clone();

    VStack::new(cx, move |cx| {
        // Card header strip: INSERT X + Position dropdown
        HStack::new(cx, {
            let p_hdr = params.clone();
            let g_hdr = gui_context.clone();
            move |cx| {
                bold_label(cx, slot_label, "module-title")
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0));

                build_enum_dropdown(
                    cx,
                    p_hdr,
                    g_hdr,
                    move |p| &p.routing.inserts[slot_idx].position,
                    INSERT_POS_ITEMS,
                )
                .class("insert-pos-row")
                .toggle_class(
                    "slot-disabled",
                    UiData::mod_ui_rev.map(move |_| !slot_check(&p_pos_dim)),
                )
                .width(Pixels(96.0))
                .height(Pixels(tokens::SNAPIN_HEADER_H));
            }
        })
        .col_between(Pixels(tokens::SPACE_SM))
        .height(Pixels(tokens::HEADER_STRIP_H))
        .width(Stretch(1.0));

        // Body: Mode selector on left + DRIVE & MIX full knobs on right
        HStack::new(cx, move |cx| {
            build_enum_dropdown(
                cx,
                params.clone(),
                gui_context,
                move |p| &p.routing.inserts[slot_idx].mode,
                INSERT_MODE_ITEMS,
            )
            .width(Stretch(1.0))
            .height(Pixels(22.0))
            .top(Stretch(1.0))
            .bottom(Stretch(1.0));

            HStack::new(cx, {
                let p_drv = params.clone();
                let m_drv = mod_ui.clone();
                let p_mix = params.clone();
                move |cx| {
                    build_flex_knob_cell_mod_dim(
                        cx,
                        UiData::params,
                        move |p| &p.routing.inserts[slot_idx].drive,
                        "DRIVE",
                        false,
                        accent,
                        tokens::KNOB_STD,
                        ModDestination::insert_drive(slot_idx),
                        m_drv,
                        0,
                        p_drv,
                        slot_check,
                    );

                    build_flex_knob_cell_mod_dim(
                        cx,
                        UiData::params,
                        move |p| &p.routing.inserts[slot_idx].mix,
                        "MIX",
                        false,
                        accent,
                        tokens::KNOB_STD,
                        ModDestination::insert_mix(slot_idx),
                        mod_ui,
                        0,
                        p_mix,
                        slot_check,
                    );
                }
            })
            .class("insert-drive-cell")
            .toggle_class(
                "slot-disabled",
                UiData::mod_ui_rev.map(move |_| !slot_check(&params)),
            )
            .col_between(Pixels(tokens::SPACE_MD))
            .width(Pixels(86.0))
            .height(Auto);
        })
        .col_between(Pixels(tokens::SPACE_MD))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })
    .class("hardware-module")
    .toggle_class(
        "module-disabled",
        UiData::mod_ui_rev.map(move |_| !slot_check(&p_dim)),
    )
    .row_between(Pixels(tokens::SPACE_SM))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}

pub(super) fn build_insert_panel(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    HStack::new(cx, move |cx| {
        build_insert_card(
            cx,
            0,
            "INSERT 1",
            params.clone(),
            gui_context.clone(),
            mod_ui.clone(),
            AMBER_RGB,
        );
        build_insert_card(cx, 1, "INSERT 2", params, gui_context, mod_ui, CORAL_RGB);
    })
    .col_between(Pixels(tokens::SPACE_MD))
    .width(Stretch(1.0))
    .height(Pixels(82.0));
}
