//! Compact `INSERT SHAPERS` panel (`INS 1` and `INS 2`) below `FILTER 2`.

use std::sync::Arc;

use nih_plug::prelude::GuiContext;
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{
    bold_label, build_enum_dropdown, build_flex_knob_cell, build_module_header,
};
use super::{AMBER_RGB, CORAL_RGB, UiData};
use crate::{InsertModeParam, InsertPosParam, JSynPluginParams};

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
    accent: (u8, u8, u8),
) {
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
                .col_between(Pixels(4.0))
                .height(Pixels(19.0))
                .width(Stretch(1.0));
            }
        })
        .width(Stretch(1.0))
        .height(Auto)
        .row_between(Pixels(3.0));

        HStack::new(cx, move |cx| {
            build_flex_knob_cell(
                cx,
                UiData::params,
                move |p| &p.routing.inserts[slot_idx].drive,
                "DRIVE",
                false,
                accent,
                24.0,
            );
        })
        .width(Pixels(54.0))
        .height(Auto);
    })
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
) {
    VStack::new(cx, move |cx| {
        build_module_header(cx, "INSERT SHAPERS", 18.0);

        build_insert_slot_row(cx, 0, "INS 1", params.clone(), gui_context.clone(), AMBER_RGB);
        build_insert_slot_row(cx, 1, "INS 2", params, gui_context, CORAL_RGB);
    })
    .class("hardware-module")
    .row_between(Pixels(4.0))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}
