//! `ROUTE` Center Stage tab: per-source filter send balance + Direct bus selectors,
//! Filter topology, bus destinations, mix, and feedback bus.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use nih_plug::prelude::{EnumParam, FloatParam, GuiContext};
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{
    bold_label, build_enum_dropdown, build_flex_knob_cell_mod, build_flex_knob_cell_mod_dim_tooltip,
    build_module_header,
};
use super::mod_bar::{bottom_tab_tag, ModUiState};
use super::tokens;
use super::{BottomTab, UiData};
use crate::{DirectRouteParam, FilterBusParam, FilterRoutingParam, JSynPluginParams};

pub(super) const FILTER_ROUTING_ITEMS: &[(&str, FilterRoutingParam)] = &[
    ("Serial (F1->F2)", FilterRoutingParam::Serial),
    ("Parallel (F1+F2)", FilterRoutingParam::Parallel),
];

pub(super) const DIRECT_ROUTE_ITEMS: &[(&str, DirectRouteParam)] = &[
    ("Off", DirectRouteParam::Off),
    ("Bus A", DirectRouteParam::BusA),
    ("Bus B", DirectRouteParam::BusB),
    ("Bus C", DirectRouteParam::Sub),
];

pub(super) const FILTER_BUS_ITEMS: &[(&str, FilterBusParam)] = &[
    ("Bus A", FilterBusParam::BusA),
    ("Bus B", FilterBusParam::BusB),
    ("A+B", FilterBusParam::Both),
    ("Bus C", FilterBusParam::Sub),
    ("A+C", FilterBusParam::AC),
    ("B+C", FilterBusParam::BC),
    ("A+B+C", FilterBusParam::All),
];

fn build_source_send_column<FB, FD>(
    cx: &mut Context,
    title: &'static str,
    dest: ModDestination,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
    tab_tag: u8,
    bal_map: FB,
    direct_map: FD,
) where
    FB: Fn(&Arc<JSynPluginParams>) -> &FloatParam + Copy + Send + Sync + 'static,
    FD: Fn(&JSynPluginParams) -> &EnumParam<DirectRouteParam> + Copy + Send + Sync + 'static,
{
    let p_dim = params.clone();
    VStack::new(cx, move |cx| {
        build_flex_knob_cell_mod_dim_tooltip(
            cx,
            UiData::params,
            bal_map,
            title,
            true,
            tokens::KNOB_COMPACT,
            dest,
            mod_ui.clone(),
            tab_tag,
            p_dim,
            move |p| direct_map(p).value() == DirectRouteParam::Off,
            move |p| {
                let target = match direct_map(p).value() {
                    DirectRouteParam::BusA => "Bus A",
                    DirectRouteParam::BusB => "Bus B",
                    DirectRouteParam::Sub => "Bus C",
                    DirectRouteParam::Off => "Filters",
                };
                format!("Direct -> {target} (skips Filters & Inserts)")
            },
        ) ;

        build_enum_dropdown(
            cx,
            params,
            gui_context,
            move |p| direct_map(p),
            DIRECT_ROUTE_ITEMS,
        )
        .width(Stretch(1.0))
        .height(Pixels(tokens::PILL_H))
        .toggle_class(
            "routing-pill-active",
            UiData::params.map(move |p| direct_map(p).value() != DirectRouteParam::Off),
        );
    })
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(tokens::SPACE_XS));
}

pub(super) fn build_routing_tab(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    let tab_tag = bottom_tab_tag(BottomTab::Routing);

    VStack::new(cx, move |cx| {
        // Row 1: Per-source filter send balance (-100% F1 <-> +100% F2) + Direct bus selector
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
                        ModDestination::NoiseFilterSend,
                        params.clone(),
                        gui_context.clone(),
                        mod_ui.clone(),
                        tab_tag,
                        |p| &p.routing.noise_flt_bal,
                        |p| &p.routing.noise_direct,
                    );
                })
                .col_between(Pixels(tokens::SPACE_MD))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .width(Stretch(1.0))
                .height(Stretch(1.0));
            }
        })
        .class("hardware-module")
        .row_between(Pixels(tokens::SPACE_XS))
        .width(Stretch(1.0))
        .height(Stretch(1.0));

        // Row 2: Filter Mode, Filter Out Buses, Filter Mix, Feedback
        VStack::new(cx, move |cx| {
            build_module_header(cx, "FILTER ROUTING, BUS OUT & FEEDBACK", tokens::PILL_H);

            HStack::new(cx, move |cx| {
                VStack::new(cx, {
                    let p_fx = params.clone();
                    let g_fx = gui_context.clone();
                    move |cx| {
                        bold_label(cx, "FILTER MODE", "mini-label")
                            .width(Stretch(1.0))
                            .height(Pixels(11.0))
                            .child_space(Stretch(1.0));
                        build_enum_dropdown(
                            cx,
                            p_fx,
                            g_fx,
                            |p| &p.routing.filter_routing,
                            FILTER_ROUTING_ITEMS,
                        )
                        .width(Stretch(1.0))
                        .height(Pixels(tokens::CONTROL_H));
                    }
                })
                .width(Pixels(136.0))
                .height(Auto)
                .top(Stretch(1.0))
                .bottom(Stretch(1.0))
                .row_between(Pixels(tokens::SPACE_XS));

                VStack::new(cx, {
                    let p_out = params.clone();
                    let g_out = gui_context;
                    move |cx| {
                        bold_label(cx, "F1 OUT BUS", "mini-label")
                            .width(Stretch(1.0))
                            .height(Pixels(11.0))
                            .child_space(Stretch(1.0));
                        build_enum_dropdown(
                            cx,
                            p_out.clone(),
                            g_out.clone(),
                            |p| &p.routing.f1_out_bus,
                            FILTER_BUS_ITEMS,
                        )
                        .width(Stretch(1.0))
                        .height(Pixels(20.0));

                        bold_label(cx, "F2 OUT BUS", "mini-label")
                            .width(Stretch(1.0))
                            .height(Pixels(11.0))
                            .child_space(Stretch(1.0));
                        build_enum_dropdown(
                            cx,
                            p_out,
                            g_out,
                            |p| &p.routing.f2_out_bus,
                            FILTER_BUS_ITEMS,
                        )
                        .width(Stretch(1.0))
                        .height(Pixels(20.0));
                    }
                })
                .width(Pixels(100.0))
                .height(Auto)
                .row_between(Pixels(tokens::SPACE_XS));

                build_flex_knob_cell_mod(
                    cx,
                    UiData::params,
                    |p| &p.routing.filter_mix,
                    "F1/F2 MIX",
                    true,
                    tokens::KNOB_STD,
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
                    tokens::KNOB_STD,
                    ModDestination::FeedbackAmount,
                    mod_ui,
                    tab_tag,
                );
            })
            .col_between(Pixels(tokens::SPACE_LG))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0))
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        })
        .class("hardware-module")
        .row_between(Pixels(tokens::SPACE_XS))
        .width(Stretch(1.0))
        .height(Pixels(106.0));
    })
    .row_between(Pixels(tokens::SPACE_SM))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}
