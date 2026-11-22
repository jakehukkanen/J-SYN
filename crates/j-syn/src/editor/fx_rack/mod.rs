//! 3-Lane Scrollable Master FX Rack UI (`Bus A`, `Bus B`, `Bus C`, `8` slots per lane,
//! `200 px` variable-height FX Units, `ZStack` translucent floating overlays, and `Cmd/Ctrl + Click` collapse).

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::{FxBusTopology, FxType};
use j_syn_ui::widgets::notify_scroll_lane_wheeled;
use nih_plug::prelude::GuiContext;
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{
    bold_label, build_dropdown_trigger_labels, build_flex_knob_cell_mod, ScrollDropdown,
};
use super::mod_bar::ModUiState;
use super::{tokens, UiData, UiEvent};
use crate::{FxBusTopologyParam, JSynPluginParams};

mod adapters;
mod header;
mod slot_ops;

use adapters::build_fx_unit_body;
use header::build_fx_slot_header;
use slot_ops::{lane_target_label, set_fx_bus_topology, slot_enabled_for_bus};

struct FxLaneScrollGuard;

impl FxLaneScrollGuard {
    fn new<'a, F>(cx: &'a mut Context, content: F) -> Handle<'a, Self>
    where
        F: FnOnce(&mut Context),
    {
        Self.build(cx, content).layout_type(LayoutType::Column)
    }
}

impl View for FxLaneScrollGuard {
    fn element(&self) -> Option<&'static str> {
        Some("fx-lane-scroll-guard")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, _meta| {
            if let WindowEvent::MouseScroll(_, sy) = window_event {
                if *sy != 0.0 {
                    notify_scroll_lane_wheeled(cx.mouse().cursorx, cx.mouse().cursory);
                }
            }
        });
    }
}


fn build_lane_route_dropdown(
    cx: &mut Context,
    bus_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
) {
    const BUS_A_TARGETS: &[(&str, u8)] = &[("-> MST", 0), ("-> B", 1), ("-> C", 2)];
    const BUS_B_TARGETS: &[(&str, u8)] = &[("-> MST", 0), ("-> C", 2)];

    let items: &'static [(&'static str, u8)] = if bus_idx == 0 {
        BUS_A_TARGETS
    } else {
        BUS_B_TARGETS
    };
    let cur_target = move |topo: FxBusTopology| -> u8 {
        if bus_idx == 0 {
            topo.bus_a_target()
        } else {
            topo.bus_b_target()
        }
    };
    let next_topo_for = move |topo: FxBusTopology, target: u8| -> FxBusTopologyParam {
        let next = if bus_idx == 0 {
            FxBusTopology::from_lane_targets(target, topo.bus_b_target())
        } else {
            FxBusTopology::from_lane_targets(topo.bus_a_target(), target)
        };
        FxBusTopologyParam::from(next)
    };

    let p_scroll = params.clone();
    let g_scroll = gui_context.clone();
    let p_trig = params.clone();
    let p_pop = params.clone();
    let g_pop = gui_context;

    ScrollDropdown::new(
        cx,
        move |cx, delta| {
            let cur: FxBusTopology = p_scroll.fx_bus_topology.value().into();
            let target = cur_target(cur);
            let pos = items.iter().position(|&(_, t)| t == target).unwrap_or(0);
            let next = (pos as isize + delta).clamp(0, items.len() as isize - 1) as usize;
            if next != pos {
                set_fx_bus_topology(&p_scroll, &g_scroll, next_topo_for(cur, items[next].1));
                cx.emit(UiEvent::ModUiChanged);
            }
        },
        move |cx| {
            let p_trig = p_trig.clone();
            let p_pop = p_pop.clone();
            let g_pop = g_pop.clone();
            Dropdown::new(
                cx,
                move |cx| {
                    let p_lbl = p_trig.clone();
                    HStack::new(cx, move |cx| {
                        build_dropdown_trigger_labels(
                            cx,
                            UiData::params.map(move |p| {
                                let topo: FxBusTopology = p.fx_bus_topology.value().into();
                                let _ = &p_lbl;
                                lane_target_label(cur_target(topo)).to_string()
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
                    for &(label, target) in items {
                        let p_set = p_pop.clone();
                        let g_set = g_pop.clone();
                        bold_label(cx, label, "dropdown-item").on_press(move |cx| {
                            let cur: FxBusTopology = p_set.fx_bus_topology.value().into();
                            set_fx_bus_topology(&p_set, &g_set, next_topo_for(cur, target));
                            cx.emit(UiEvent::ModUiChanged);
                            cx.emit(PopupEvent::Close);
                        });
                    }
                },
            )
        },
    )
    .toggle_class(
        "routing-pill-active",
        UiData::params.map(move |p| {
            let topo: FxBusTopology = p.fx_bus_topology.value().into();
            cur_target(topo) != 0
        }),
    )
    .width(Stretch(1.0))
    .height(Pixels(tokens::PILL_H));
}

fn build_lane_header(
    cx: &mut Context,
    bus_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    let title = match bus_idx {
        0 => "BUS A",
        1 => "BUS B",
        _ => "BUS C",
    };
    let (lvl_dest, pan_dest) = match bus_idx {
        0 => (ModDestination::BusALevel, ModDestination::BusAPan),
        1 => (ModDestination::BusBLevel, ModDestination::BusBPan),
        _ => (ModDestination::BusCLevel, ModDestination::BusCPan),
    };

    HStack::new(cx, move |cx| {
        let p_hdr = params.clone();
        let g_hdr = gui_context;
        VStack::new(cx, move |cx| {
            bold_label(cx, title, "module-title")
                .width(Stretch(1.0))
                .height(Pixels(tokens::MACRO_HDR_H))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0));

            if bus_idx < 2 {
                build_lane_route_dropdown(cx, bus_idx, p_hdr, g_hdr);
            } else {
                bold_label(cx, "-> MST", "mini-label")
                    .width(Stretch(1.0))
                    .height(Pixels(tokens::PILL_H))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0));
            }
        })
        .row_between(Pixels(tokens::SPACE_SM))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .width(Pixels(tokens::FX_LANE_ROUTE_COL_W))
        .height(Stretch(1.0));

        Element::new(cx).width(Stretch(1.0));

        let m_lvl = mod_ui.clone();
        HStack::new(cx, move |cx| {
            build_flex_knob_cell_mod(
                cx,
                UiData::params,
                move |p| p.fx_bus_level_param(bus_idx),
                "LEVEL",
                false,
                tokens::KNOB_OSC,
                lvl_dest,
                m_lvl,
                0,
            );

            build_flex_knob_cell_mod(
                cx,
                UiData::params,
                move |p| p.fx_bus_pan_param(bus_idx),
                "PAN",
                true,
                tokens::KNOB_OSC,
                pan_dest,
                mod_ui,
                0,
            );
        })
        .col_between(Pixels(tokens::SPACE_SM))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .width(Pixels(tokens::FX_LANE_KNOBS_W))
        .height(Stretch(1.0));
    })
    .class("fx-bus-header")
    .col_between(Pixels(tokens::SPACE_SM))
    .height(Pixels(tokens::FX_LANE_HEADER_H))
    .width(Stretch(1.0));
}

fn build_fx_unit_card(
    cx: &mut Context,
    bus_idx: usize,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    let p_dim = params.clone();
    VStack::new(cx, move |cx| {
        build_fx_slot_header(
            cx,
            bus_idx,
            slot_idx,
            params.clone(),
            gui_context.clone(),
            mod_ui.clone(),
        );

        let p_body = params.clone();
        let g_body = gui_context.clone();
        let m_body = mod_ui.clone();
        let m_col_key = mod_ui.clone();
        Binding::new(
            cx,
            UiData::root.map(move |data| {
                (
                    data.params.fx_bus_slots(bus_idx)[slot_idx].fx_type.value() as usize,
                    m_col_key.fx_collapsed(bus_idx, slot_idx),
                )
            }),
            move |cx, slot_view_lens| {
                let (fx_idx, is_collapsed) = slot_view_lens.get(cx);
                let fx_type = FxType::from_index(fx_idx);
                if fx_type == FxType::Off || is_collapsed {
                    return;
                }
                let p_unit = p_body.clone();
                let g_unit = g_body.clone();
                let m_unit = m_body.clone();
                build_fx_unit_body(
                    cx,
                    fx_type,
                    bus_idx,
                    slot_idx,
                    p_unit,
                    g_unit,
                    m_unit,
                );
            },
        );
    })
    .class("hardware-module")
    .class("fx-full-card")
    .toggle_class(
        "module-disabled",
        UiData::mod_ui_rev.map(move |_| !slot_enabled_for_bus(&p_dim, bus_idx, slot_idx)),
    )
    .width(Pixels(tokens::FX_UNIT_W))
    .height(Auto)
    .row_between(Pixels(tokens::SPACE_SM));
}

/// Right Wing: 3-lane scrollable FX Rack (`Bus A`, `Bus B`, `Bus C`, 8 slots each).
pub(crate) fn build_three_lane_fx_rack(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    HStack::new(cx, move |cx| {
        for bus_idx in 0..3 {
            let p_lane = params.clone();
            let g_lane = gui_context.clone();
            let m_lane = mod_ui.clone();
            VStack::new(cx, move |cx| {
                build_lane_header(
                    cx,
                    bus_idx,
                    p_lane.clone(),
                    g_lane.clone(),
                    m_lane.clone(),
                );

                ScrollView::new(cx, 0.0, 0.0, false, true, move |cx| {
                    FxLaneScrollGuard::new(cx, move |cx| {
                        for slot_idx in 0..8 {
                            build_fx_unit_card(
                                cx,
                                bus_idx,
                                slot_idx,
                                p_lane.clone(),
                                g_lane.clone(),
                                m_lane.clone(),
                            );
                        }
                    })
                    .row_between(Pixels(tokens::SPACE_SM))
                    .child_right(Pixels(tokens::FX_LANE_SCROLL_GUTTER))
                    .width(Stretch(1.0))
                    .height(Auto);
                })
                .width(Stretch(1.0))
                .height(Stretch(1.0));
            })
            .row_between(Pixels(tokens::SPACE_SM))
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        }
    })
    .col_between(Pixels(tokens::SPACE_MD))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}
