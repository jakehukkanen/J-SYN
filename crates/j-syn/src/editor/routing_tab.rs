//! `ROUTING` bottom drawer: per-source filter send balance + Direct bus selectors,
//! Serial/Parallel filter routing + Bus output selectors, feedback bus, and Insert slot mix.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::freq_shift_split_hz;
use nih_plug::prelude::{EnumParam, FloatParam, GuiContext, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{
    bold_label, build_enum_dropdown, build_flex_knob_cell_mod, build_module_header,
};
use super::fx_rack::{format_hz, FX_FREQ_SHIFT_MODE_ITEMS, FX_FREQ_SPLIT_ITEMS};
use super::insert_panel::INSERT_MODE_ITEMS;
use super::mod_bar::{bottom_tab_tag, ModUiState};
use super::widgets::{KnobDimExt, RotaryKnob};
use super::{BottomTab, AMBER_RGB, CORAL_RGB, GOLD_RGB, UiData, UiEvent};
use crate::{
    DirectRouteParam, FilterBusParam, FilterRoutingParam, FreqSplitBandParam, InsertModeParam,
    JSynPluginParams,
};

const FILTER_ROUTING_ITEMS: &[(&str, FilterRoutingParam)] = &[
    ("Serial (F1->F2)", FilterRoutingParam::Serial),
    ("Parallel (F1+F2)", FilterRoutingParam::Parallel),
];

const DIRECT_ROUTE_ITEMS: &[(&str, DirectRouteParam)] = &[
    ("Off", DirectRouteParam::Off),
    ("Bus A", DirectRouteParam::BusA),
    ("Bus B", DirectRouteParam::BusB),
    ("Sub", DirectRouteParam::Sub),
];

const FILTER_BUS_ITEMS: &[(&str, FilterBusParam)] = &[
    ("Bus A", FilterBusParam::BusA),
    ("Bus B", FilterBusParam::BusB),
    ("A+B", FilterBusParam::Both),
    ("Sub", FilterBusParam::Sub),
];

fn build_source_send_column<FB, FD>(
    cx: &mut Context,
    title: &'static str,
    accent: (u8, u8, u8),
    dest: ModDestination,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
    tab_tag: u8,
    bal_map: FB,
    direct_map: FD,
) where
    FB: Fn(&Arc<JSynPluginParams>) -> &FloatParam + Copy + Send + Sync + 'static,
    FD: Fn(&Arc<JSynPluginParams>) -> &EnumParam<DirectRouteParam> + Copy + Send + Sync + 'static,
{
    VStack::new(cx, move |cx| {
        build_flex_knob_cell_mod(
            cx,
            UiData::params,
            bal_map,
            title,
            true,
            accent,
            26.0,
            dest,
            mod_ui.clone(),
            tab_tag,
        );

        build_enum_dropdown(cx, params, gui_context, direct_map, DIRECT_ROUTE_ITEMS)
            .width(Stretch(1.0))
            .height(Pixels(18.0))
            .toggle_class(
                "routing-pill-active",
                UiData::params.map(move |p| direct_map(p).value() != DirectRouteParam::Off),
            );
    })
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(2.0));
}

pub(super) fn build_routing_tab(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    let tab_tag = bottom_tab_tag(BottomTab::Routing);

    HStack::new(cx, move |cx| {
        // Card 1: Per-source filter send balance (-100% F1 <-> +100% F2) + Direct bus selector
        VStack::new(cx, {
            let params = params.clone();
            let gui_context = gui_context.clone();
            let mod_ui = mod_ui.clone();
            move |cx| {
                build_module_header(cx, "SOURCE FILTER SEND (F1 <-> F2) & DIRECT BUS", 18.0);

                HStack::new(cx, move |cx| {
                    build_source_send_column(
                        cx,
                        "OSC 1",
                        AMBER_RGB,
                        ModDestination::Osc1FilterSend,
                        params.clone(),
                        gui_context.clone(),
                        mod_ui.clone(),
                        tab_tag,
                        |p| &p.routing.osc1_flt_bal,
                        |p| &p.routing.osc1_direct,
                    );
                    build_source_send_column(
                        cx,
                        "OSC 2",
                        GOLD_RGB,
                        ModDestination::Osc2FilterSend,
                        params.clone(),
                        gui_context.clone(),
                        mod_ui.clone(),
                        tab_tag,
                        |p| &p.routing.osc2_flt_bal,
                        |p| &p.routing.osc2_direct,
                    );
                    build_source_send_column(
                        cx,
                        "OSC 3",
                        CORAL_RGB,
                        ModDestination::Osc3FilterSend,
                        params.clone(),
                        gui_context.clone(),
                        mod_ui.clone(),
                        tab_tag,
                        |p| &p.routing.osc3_flt_bal,
                        |p| &p.routing.osc3_direct,
                    );
                    build_source_send_column(
                        cx,
                        "OSC 4",
                        AMBER_RGB,
                        ModDestination::Osc4FilterSend,
                        params.clone(),
                        gui_context.clone(),
                        mod_ui.clone(),
                        tab_tag,
                        |p| &p.routing.osc4_flt_bal,
                        |p| &p.routing.osc4_direct,
                    );
                    build_source_send_column(
                        cx,
                        "NOISE",
                        CORAL_RGB,
                        ModDestination::NoiseFilterSend,
                        params.clone(),
                        gui_context.clone(),
                        mod_ui.clone(),
                        tab_tag,
                        |p| &p.routing.noise_flt_bal,
                        |p| &p.routing.noise_direct,
                    );
                })
                .col_between(Pixels(6.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .width(Stretch(1.0))
                .height(Stretch(1.0));
            }
        })
        .class("hardware-module")
        .row_between(Pixels(3.0))
        .width(Stretch(1.65))
        .height(Stretch(1.0));

        // Card 2: Serial / Parallel routing, Filter Out Buses, Filter Mix, Feedback
        VStack::new(cx, {
            let params = params.clone();
            let gui_context = gui_context.clone();
            let mod_ui = mod_ui.clone();
            move |cx| {
                build_module_header(cx, "FILTER BUS & FEEDBACK", 18.0);

                HStack::new(cx, move |cx| {
                    VStack::new(cx, {
                        let params = params.clone();
                        let gui_context = gui_context.clone();
                        move |cx| {
                            bold_label(cx, "TOPOLOGY", "mini-label")
                                .width(Stretch(1.0))
                                .height(Pixels(11.0))
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

                            HStack::new(cx, move |cx| {
                                VStack::new(cx, {
                                    let params = params.clone();
                                    let gui_context = gui_context.clone();
                                    move |cx| {
                                        bold_label(cx, "F1 OUT", "mini-label")
                                            .width(Stretch(1.0))
                                            .height(Pixels(11.0))
                                            .child_space(Stretch(1.0));
                                        build_enum_dropdown(
                                            cx,
                                            params,
                                            gui_context,
                                            |p| &p.routing.f1_out_bus,
                                            FILTER_BUS_ITEMS,
                                        )
                                        .width(Stretch(1.0))
                                        .height(Pixels(18.0));
                                    }
                                })
                                .width(Stretch(1.0))
                                .height(Auto)
                                .row_between(Pixels(2.0));

                                VStack::new(cx, move |cx| {
                                    bold_label(cx, "F2 OUT", "mini-label")
                                        .width(Stretch(1.0))
                                        .height(Pixels(11.0))
                                        .child_space(Stretch(1.0));
                                    build_enum_dropdown(
                                        cx,
                                        params,
                                        gui_context,
                                        |p| &p.routing.f2_out_bus,
                                        FILTER_BUS_ITEMS,
                                    )
                                    .width(Stretch(1.0))
                                    .height(Pixels(18.0));
                                })
                                .width(Stretch(1.0))
                                .height(Auto)
                                .row_between(Pixels(2.0));
                            })
                            .col_between(Pixels(4.0))
                            .width(Stretch(1.0))
                            .height(Auto);
                        }
                    })
                    .width(Pixels(156.0))
                    .height(Auto)
                    .row_between(Pixels(4.0));

                    build_flex_knob_cell_mod(
                        cx,
                        UiData::params,
                        |p| &p.routing.filter_mix,
                        "F1/F2 MIX",
                        true,
                        GOLD_RGB,
                        36.0,
                        ModDestination::FilterMix,
                        mod_ui.clone(),
                        tab_tag,
                    );

                    build_flex_knob_cell_mod(
                        cx,
                        UiData::params,
                        |p| &p.routing.feedback_amount,
                        "FEEDBACK",
                        false,
                        CORAL_RGB,
                        36.0,
                        ModDestination::FeedbackAmount,
                        mod_ui.clone(),
                        tab_tag,
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

        // Card 3: Insert slot mix, quick mode view, and Insert Freq Shifter split/mode controls
        VStack::new(cx, {
            let params = params.clone();
            let gui_context = gui_context.clone();
            let mod_ui = mod_ui.clone();
            move |cx| {
                build_module_header(cx, "INSERT MIX", 18.0);

                HStack::new(cx, {
                    let params = params.clone();
                    let gui_context = gui_context.clone();
                    let mod_ui = mod_ui.clone();
                    move |cx| {
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
                        .width(Pixels(136.0))
                        .height(Auto)
                        .row_between(Pixels(4.0));

                        build_flex_knob_cell_mod(
                            cx,
                            UiData::params,
                            |p| &p.routing.inserts[0].mix,
                            "INS 1 MIX",
                            false,
                            AMBER_RGB,
                            32.0,
                            ModDestination::Insert1Mix,
                            mod_ui.clone(),
                            tab_tag,
                        );

                        build_flex_knob_cell_mod(
                            cx,
                            UiData::params,
                            |p| &p.routing.inserts[1].mix,
                            "INS 2 MIX",
                            false,
                            CORAL_RGB,
                            32.0,
                            ModDestination::Insert2Mix,
                            mod_ui.clone(),
                            tab_tag,
                        );

                        let p_shf_vis = params.clone();
                        let p_shf_act = params.clone();
                        let g_shf_act = gui_context.clone();
                        Binding::new(
                            cx,
                            UiData::mod_ui_rev.map(move |_| {
                                (
                                    p_shf_vis.routing.inserts[0].mode.value()
                                        == InsertModeParam::FreqShifter,
                                    p_shf_vis.routing.inserts[1].mode.value()
                                        == InsertModeParam::FreqShifter,
                                )
                            }),
                            move |cx, shf_lens| {
                                let (has0, has1) = shf_lens.get(cx);
                                if !has0 && !has1 {
                                    return;
                                }
                                for (idx, has) in [(0usize, has0), (1usize, has1)] {
                                    if !has {
                                        continue;
                                    }
                                    let p_col = p_shf_act.clone();
                                    let g_col = g_shf_act.clone();
                                    let p_dim = p_shf_act.clone();
                                    let p_kt = p_shf_act.clone();
                                    let g_kt = g_shf_act.clone();
                                    let accent = if idx == 0 { AMBER_RGB } else { CORAL_RGB };
                                    let slot_lbl = if idx == 0 { "I1" } else { "I2" };
                                    HStack::new(cx, move |cx| {
                                        VStack::new(cx, move |cx| {
                                            HStack::new(cx, {
                                                let p_kt_b = p_kt.clone();
                                                let g_kt_b = g_kt.clone();
                                                move |cx| {
                                                    bold_label(cx, slot_lbl, "mini-label")
                                                        .width(Pixels(14.0))
                                                        .height(Stretch(1.0))
                                                        .child_space(Stretch(1.0));
                                                    Button::new(
                                                        cx,
                                                        move |cx| {
                                                            let setter =
                                                                ParamSetter::new(g_kt_b.as_ref());
                                                            let param = &p_kt_b.routing.inserts[idx]
                                                                .keytrack;
                                                            let next = !param.value();
                                                            setter.begin_set_parameter(param);
                                                            setter.set_parameter(param, next);
                                                            setter.end_set_parameter(param);
                                                            cx.emit(UiEvent::ModUiChanged);
                                                        },
                                                        |cx| {
                                                            bold_label(cx, "KT", "fx-subtab-lbl")
                                                                .hoverable(false)
                                                        },
                                                    )
                                                    .class("fx-scope-pill")
                                                    .class("fx-scope-flt-btn")
                                                    .toggle_class(
                                                        "fx-subtab-active",
                                                        UiData::params.map(move |p| {
                                                            p.routing.inserts[idx].keytrack.value()
                                                        }),
                                                    )
                                                    .width(Pixels(24.0))
                                                    .height(Pixels(18.0));
                                                }
                                            })
                                            .col_between(Pixels(3.0))
                                            .height(Pixels(18.0))
                                            .width(Auto);

                                            build_enum_dropdown(
                                                cx,
                                                p_col.clone(),
                                                g_col.clone(),
                                                move |p| &p.routing.inserts[idx].shift_mode,
                                                FX_FREQ_SHIFT_MODE_ITEMS,
                                            )
                                            .width(Pixels(52.0))
                                            .height(Pixels(18.0));

                                            build_enum_dropdown(
                                                cx,
                                                p_col.clone(),
                                                g_col.clone(),
                                                move |p| &p.routing.inserts[idx].shift_split,
                                                FX_FREQ_SPLIT_ITEMS,
                                            )
                                            .width(Pixels(52.0))
                                            .height(Pixels(18.0));
                                        })
                                        .row_between(Pixels(3.0))
                                        .width(Auto)
                                        .height(Auto);

                                        let formatter = Arc::new(|norm: f32, _off: Option<f32>| {
                                            format_hz(freq_shift_split_hz(norm))
                                        });
                                        VStack::new(cx, move |cx| {
                                            bold_label(cx, "SPLIT", "knob-title")
                                                .width(Stretch(1.0))
                                                .height(Pixels(12.0))
                                                .child_space(Stretch(1.0));
                                            RotaryKnob::new_with_formatter(
                                                cx,
                                                UiData::params,
                                                move |p| &p.routing.inserts[idx].split,
                                                false,
                                                accent,
                                                formatter,
                                            )
                                            .default_norm(Some(0.45))
                                            .enabled_by(p_dim, move |p| {
                                                p.routing.inserts[idx].shift_split.value()
                                                    != FreqSplitBandParam::Full
                                            })
                                            .width(Pixels(30.0))
                                            .height(Pixels(30.0))
                                            .left(Stretch(1.0))
                                            .right(Stretch(1.0));
                                        })
                                        .class("knob-cell")
                                        .width(Pixels(44.0))
                                        .height(Auto)
                                        .row_between(Pixels(1.0));
                                    })
                                    .col_between(Pixels(4.0))
                                    .child_top(Stretch(1.0))
                                    .child_bottom(Stretch(1.0))
                                    .width(Auto)
                                    .height(Auto);
                                }
                            },
                        );
                    }
                })
                .col_between(Pixels(6.0))
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
