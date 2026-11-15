//! `ROUTING` bottom drawer: per-source filter send balance + Direct Out toggles,
//! Serial/Parallel filter routing, feedback bus, and Insert slot mix controls.

use std::sync::Arc;

use nih_plug::prelude::{BoolParam, FloatParam, GuiContext};
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{
    bold_label, build_enum_dropdown, build_flex_knob_cell, build_module_header,
};
use super::insert_panel::INSERT_MODE_ITEMS;
use super::widgets::LedToggleSwitch;
use super::{AMBER_RGB, CORAL_RGB, GOLD_RGB, UiData};
use crate::{FilterRoutingParam, JSynPluginParams};

const FILTER_ROUTING_ITEMS: &[(&str, FilterRoutingParam)] = &[
    ("Serial (F1->F2)", FilterRoutingParam::Serial),
    ("Parallel (F1+F2)", FilterRoutingParam::Parallel),
];

fn build_source_send_column<FB, FD>(
    cx: &mut Context,
    title: &'static str,
    accent: (u8, u8, u8),
    bal_map: FB,
    direct_map: FD,
) where
    FB: Fn(&Arc<JSynPluginParams>) -> &FloatParam + Copy + Send + Sync + 'static,
    FD: Fn(&Arc<JSynPluginParams>) -> &BoolParam + Copy + Send + Sync + 'static,
{
    VStack::new(cx, move |cx| {
        build_flex_knob_cell(cx, UiData::params, bal_map, title, true, accent, 26.0);

        LedToggleSwitch::new(cx, UiData::params, direct_map, "DIRECT", "DIRECT", accent)
            .width(Stretch(1.0))
            .height(Pixels(16.0));
    })
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(2.0));
}

pub(super) fn build_routing_tab(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
) {
    HStack::new(cx, move |cx| {
        // Card 1: Per-source filter send balance (-100% F1 <-> +100% F2) + DIRECT bypass
        VStack::new(cx, |cx| {
            build_module_header(cx, "SOURCE FILTER SEND (F1 <-> F2) & DIRECT OUT", 18.0);

            HStack::new(cx, |cx| {
                build_source_send_column(
                    cx,
                    "OSC 1",
                    AMBER_RGB,
                    |p| &p.routing.osc1_flt_bal,
                    |p| &p.routing.osc1_direct,
                );
                build_source_send_column(
                    cx,
                    "OSC 2",
                    GOLD_RGB,
                    |p| &p.routing.osc2_flt_bal,
                    |p| &p.routing.osc2_direct,
                );
                build_source_send_column(
                    cx,
                    "OSC 3",
                    CORAL_RGB,
                    |p| &p.routing.osc3_flt_bal,
                    |p| &p.routing.osc3_direct,
                );
                build_source_send_column(
                    cx,
                    "OSC 4",
                    AMBER_RGB,
                    |p| &p.routing.osc4_flt_bal,
                    |p| &p.routing.osc4_direct,
                );
                build_source_send_column(
                    cx,
                    "NOISE",
                    CORAL_RGB,
                    |p| &p.routing.noise_flt_bal,
                    |p| &p.routing.noise_direct,
                );
            })
            .col_between(Pixels(6.0))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0))
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        })
        .class("hardware-module")
        .row_between(Pixels(3.0))
        .width(Stretch(1.7))
        .height(Stretch(1.0));

        // Card 2: Serial / Parallel routing, Filter Mix, Feedback
        VStack::new(cx, {
            let params = params.clone();
            let gui_context = gui_context.clone();
            move |cx| {
                build_module_header(cx, "FILTER BUS & FEEDBACK", 18.0);

                HStack::new(cx, move |cx| {
                    VStack::new(cx, {
                        let params = params.clone();
                        let gui_context = gui_context.clone();
                        move |cx| {
                            bold_label(cx, "TOPOLOGY", "mini-label")
                                .width(Stretch(1.0))
                                .height(Pixels(12.0))
                                .child_space(Stretch(1.0));

                            build_enum_dropdown(
                                cx,
                                params.clone(),
                                gui_context.clone(),
                                |p| &p.routing.filter_routing,
                                FILTER_ROUTING_ITEMS,
                            )
                            .width(Stretch(1.0))
                            .height(Pixels(20.0));
                        }
                    })
                    .width(Pixels(132.0))
                    .height(Auto)
                    .row_between(Pixels(3.0));

                    build_flex_knob_cell(
                        cx,
                        UiData::params,
                        |p| &p.routing.filter_mix,
                        "F1/F2 MIX",
                        true,
                        GOLD_RGB,
                        32.0,
                    );

                    build_flex_knob_cell(
                        cx,
                        UiData::params,
                        |p| &p.routing.feedback_amount,
                        "FEEDBACK",
                        false,
                        CORAL_RGB,
                        32.0,
                    );
                })
                .col_between(Pixels(8.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .width(Stretch(1.0))
                .height(Stretch(1.0));
            }
        })
        .class("hardware-module")
        .row_between(Pixels(3.0))
        .width(Stretch(1.1))
        .height(Stretch(1.0));

        // Card 3: Insert slot mix & quick mode view
        VStack::new(cx, {
            let params = params.clone();
            let gui_context = gui_context.clone();
            move |cx| {
                build_module_header(cx, "INSERT MIX", 18.0);

                HStack::new(cx, move |cx| {
                    VStack::new(cx, {
                        let params = params.clone();
                        let gui_context = gui_context.clone();
                        move |cx| {
                            HStack::new(cx, {
                                let params = params.clone();
                                let gui_context = gui_context.clone();
                                move |cx| {
                                    bold_label(cx, "INS 1", "mini-label")
                                        .width(Pixels(38.0))
                                        .height(Stretch(1.0))
                                        .child_space(Stretch(1.0));
                                    build_enum_dropdown(
                                        cx,
                                        params.clone(),
                                        gui_context.clone(),
                                        |p| &p.routing.inserts[0].mode,
                                        INSERT_MODE_ITEMS,
                                    )
                                    .width(Stretch(1.0))
                                    .height(Pixels(20.0));
                                }
                            })
                            .col_between(Pixels(4.0))
                            .height(Pixels(20.0))
                            .width(Stretch(1.0));

                            HStack::new(cx, move |cx| {
                                bold_label(cx, "INS 2", "mini-label")
                                    .width(Pixels(38.0))
                                    .height(Stretch(1.0))
                                    .child_space(Stretch(1.0));
                                build_enum_dropdown(
                                    cx,
                                    params.clone(),
                                    gui_context.clone(),
                                    |p| &p.routing.inserts[1].mode,
                                    INSERT_MODE_ITEMS,
                                )
                                .width(Stretch(1.0))
                                .height(Pixels(20.0));
                            })
                            .col_between(Pixels(4.0))
                            .height(Pixels(20.0))
                            .width(Stretch(1.0));
                        }
                    })
                    .width(Pixels(146.0))
                    .height(Auto)
                    .row_between(Pixels(4.0));

                    build_flex_knob_cell(
                        cx,
                        UiData::params,
                        |p| &p.routing.inserts[0].mix,
                        "INS 1 MIX",
                        false,
                        AMBER_RGB,
                        32.0,
                    );

                    build_flex_knob_cell(
                        cx,
                        UiData::params,
                        |p| &p.routing.inserts[1].mix,
                        "INS 2 MIX",
                        false,
                        CORAL_RGB,
                        32.0,
                    );
                })
                .col_between(Pixels(8.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .width(Stretch(1.0))
                .height(Stretch(1.0));
            }
        })
        .class("hardware-module")
        .row_between(Pixels(3.0))
        .width(Stretch(1.2))
        .height(Stretch(1.0));
    })
    .col_between(Pixels(6.0))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}
