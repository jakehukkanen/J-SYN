//! 3-Lane Scrollable Phase Plant-style Master FX Rack (`Bus A`, `Bus B`, `Bus C`,
//! up to `30` active FX Units per lane = `90` total, drag-and-drop reorder/move/duplicate/delete,
//! categorized FX Picker, per-lane Bypass & Mute, and mid-chain `+` insertion seams).

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::{FxBusTopology, FxType, NUM_FX_SLOTS};
use j_syn_ui::widgets::{notify_scroll_lane_wheeled, LedToggleSwitch};
use nih_plug::prelude::{GuiContext, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use super::controls::{
    bold_label, build_dropdown_trigger_labels, build_flex_knob_cell_mod, ScrollDropdown,
};
use super::mod_bar::ModUiState;
use super::{theme, tokens, UiData, UiEvent};
use crate::{FxBusTopologyParam, JSynPluginParams};

mod adapters;
mod header;
pub(super) mod slot_ops;

use adapters::build_fx_unit_body;
use super::fx_drag_overlay::FxDragState;
use super::fx_picker_menu::FxPickerTarget;
use header::build_fx_slot_header;
use slot_ops::{lane_name, lane_target_label, set_fx_bus_topology, slot_enabled_for_bus};

#[derive(Clone, Copy, PartialEq, Eq)]
struct LaneOrderKey {
    /// Each entry packs `(slot_idx as u16) | ((fx_type as u16) << 8)`.
    entries: [u16; NUM_FX_SLOTS],
    len: u8,
}

impl Data for LaneOrderKey {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl LaneOrderKey {
    fn from_params(params: &JSynPluginParams, bus_idx: usize) -> Self {
        let order = params.reconciled_fx_rack_order();
        let active = order.active_slots(bus_idx);
        let slots = params.fx_bus_slots(bus_idx);
        let mut entries = [0u16; NUM_FX_SLOTS];
        let mut len = 0usize;
        for &slot_u8 in active {
            let s = slot_u8 as usize;
            if s < NUM_FX_SLOTS && len < NUM_FX_SLOTS {
                let ty = FxType::from(slots[s].fx_type.value()) as u16;
                if ty != 0 {
                    entries[len] = (s as u16) | (ty << 8);
                    len += 1;
                }
            }
        }
        Self {
            entries,
            len: len as u8,
        }
    }
}

/// Transparent layout container that records the full 3-lane FX Wing bounds for drag-outside-to-delete.
struct FxWingBoundsTracker {
    drag_state: Arc<FxDragState>,
}

impl FxWingBoundsTracker {
    fn new<'a, F>(cx: &'a mut Context, drag_state: Arc<FxDragState>, content: F) -> Handle<'a, Self>
    where
        F: FnOnce(&mut Context),
    {
        Self { drag_state }
            .build(cx, content)
            .layout_type(LayoutType::Row)
    }
}

impl View for FxWingBoundsTracker {
    fn element(&self) -> Option<&'static str> {
        Some("fx-wing-bounds-tracker")
    }

    fn draw(&self, cx: &mut DrawContext, _canvas: &mut Canvas) {
        self.drag_state.register_wing_bounds(cx.bounds());
    }
}

/// Transparent wrapper recording each lane's `ScrollView` viewport bounds and `Entity` for hit-testing & edge auto-scroll.
struct FxLaneViewportTracker {
    bus_idx: usize,
    drag_state: Arc<FxDragState>,
    scroll_entity: Entity,
}

impl FxLaneViewportTracker {
    fn new<'a, F>(
        cx: &'a mut Context,
        bus_idx: usize,
        drag_state: Arc<FxDragState>,
        content: F,
    ) -> Handle<'a, Self>
    where
        F: FnOnce(&mut Context) -> Entity,
    {
        let mut captured = Entity::null();
        let handle = Self {
            bus_idx,
            drag_state,
            scroll_entity: Entity::null(),
        }
        .build(cx, |cx| {
            captured = content(cx);
        })
        .layout_type(LayoutType::Column);
        handle.modify(|v| v.scroll_entity = captured)
    }
}

impl View for FxLaneViewportTracker {
    fn element(&self) -> Option<&'static str> {
        Some("fx-lane-viewport-tracker")
    }

    fn draw(&self, cx: &mut DrawContext, _canvas: &mut Canvas) {
        self.drag_state
            .register_lane_bounds(self.bus_idx, cx.bounds(), self.scroll_entity);
    }
}

/// Transparent wrapper reporting each mounted FX card's live bounding box to `FxDragState`.
struct FxCardBoundsTracker {
    bus_idx: usize,
    visual_pos: usize,
    slot_idx: usize,
    drag_state: Arc<FxDragState>,
}

impl FxCardBoundsTracker {
    fn new<'a, F>(
        cx: &'a mut Context,
        bus_idx: usize,
        visual_pos: usize,
        slot_idx: usize,
        drag_state: Arc<FxDragState>,
        content: F,
    ) -> Handle<'a, Self>
    where
        F: FnOnce(&mut Context),
    {
        Self {
            bus_idx,
            visual_pos,
            slot_idx,
            drag_state,
        }
        .build(cx, content)
        .layout_type(LayoutType::Column)
    }
}

impl View for FxCardBoundsTracker {
    fn element(&self) -> Option<&'static str> {
        Some("fx-card-bounds-tracker")
    }

    fn draw(&self, cx: &mut DrawContext, _canvas: &mut Canvas) {
        self.drag_state
            .register_card_bounds(self.bus_idx, self.visual_pos, self.slot_idx, cx.bounds());
    }
}

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

/// Subtle hoverable `+` mid-chain insertion seam placed between consecutive FX cards.
struct FxInsertSeam {
    bus_idx: usize,
    insert_pos: usize,
    hovered: bool,
}

impl FxInsertSeam {
    fn new(cx: &mut Context, bus_idx: usize, insert_pos: usize) -> Handle<'_, Self> {
        Self {
            bus_idx,
            insert_pos,
            hovered: false,
        }
        .build(cx, |_| {})
        .width(Pixels(tokens::FX_UNIT_W))
        .height(Pixels(tokens::FX_UNIT_SEAM_H))
    }
}

impl View for FxInsertSeam {
    fn element(&self) -> Option<&'static str> {
        Some("fx-insert-seam")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseOver => {
                self.hovered = true;
                cx.needs_redraw();
            }
            WindowEvent::MouseOut => {
                self.hovered = false;
                cx.needs_redraw();
            }
            WindowEvent::MouseDown(MouseButton::Left) => {
                let scale = cx.scale_factor().max(0.01);
                cx.emit(UiEvent::ShowFxPicker(
                    self.bus_idx,
                    FxPickerTarget::InsertAt(self.insert_pos),
                    cx.mouse().cursorx / scale,
                    cx.mouse().cursory / scale,
                ));
                meta.consume();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        if !self.hovered {
            return;
        }
        let b = cx.bounds();
        if b.w <= 4.0 || b.h <= 1.0 {
            return;
        }
        let dpx = cx.scale_factor();
        let pal = theme::current_palette(cx);
        let (ar, ag, ab) = pal.accent_rgb();
        let mid_y = b.y + b.h * 0.5;
        let mid_x = b.x + b.w * 0.5;

        let mut line = vg::Path::new();
        line.move_to(b.x + 8.0 * dpx, mid_y);
        line.line_to(b.x + b.w - 8.0 * dpx, mid_y);
        let mut lp = vg::Paint::color(vg::Color::rgba(ar, ag, ab, 175));
        lp.set_line_width(1.25 * dpx);
        canvas.stroke_path(&line, &lp);

        let badge_r = 5.5 * dpx;
        let mut dot = vg::Path::new();
        dot.circle(mid_x, mid_y, badge_r);
        canvas.fill_path(&dot, &vg::Paint::color(theme::vg_color(&pal.surface_dark, 245)));
        let mut border = vg::Paint::color(vg::Color::rgba(ar, ag, ab, 235));
        border.set_line_width(1.1 * dpx);
        canvas.stroke_path(&dot, &border);

        let arm = 2.8 * dpx;
        let mut plus = vg::Path::new();
        plus.move_to(mid_x - arm, mid_y);
        plus.line_to(mid_x + arm, mid_y);
        plus.move_to(mid_x, mid_y - arm);
        plus.line_to(mid_x, mid_y + arm);
        let mut pp = vg::Paint::color(vg::Color::rgb(ar, ag, ab));
        pp.set_line_width(1.25 * dpx);
        canvas.stroke_path(&plus, &pp);
    }
}

/// Hover-only empty lane area below the last FX card:
/// - Invisible at rest (zero visual clutter)
/// - Hovering the empty lane shows a darker rounded highlight box in the next slot position
/// - Hovering directly over that box brightens the highlight + translucent `"Insert FX..."` label
/// - Left-click anywhere in the empty lane opens the categorized FX Picker; right-click opens the Lane Menu.
struct FxEmptyLaneZone {
    bus_idx: usize,
    insert_pos: usize,
    can_add: bool,
    is_lane_empty: bool,
    hovered_zone: bool,
    hovered_box: bool,
}

impl FxEmptyLaneZone {
    fn new(
        cx: &mut Context,
        bus_idx: usize,
        insert_pos: usize,
        can_add: bool,
        is_lane_empty: bool,
    ) -> Handle<'_, Self> {
        let min_h = if is_lane_empty {
            tokens::FX_UNIT_EMPTY_SLOT_H + 8.0
        } else {
            tokens::FX_UNIT_TRAILING_SLOT_H + 12.0
        };
        Self {
            bus_idx,
            insert_pos,
            can_add,
            is_lane_empty,
            hovered_zone: false,
            hovered_box: false,
        }
        .build(cx, |_| {})
        .width(Pixels(tokens::FX_UNIT_W))
        .height(Stretch(1.0))
        .min_height(Pixels(min_h))
    }

    #[inline]
    fn highlight_box_rect(&self, bounds: BoundingBox, dpx: f32) -> (f32, f32, f32, f32) {
        let top_pad = if self.is_lane_empty {
            2.0 * dpx
        } else {
            6.0 * dpx
        };
        let target_h = if self.is_lane_empty {
            tokens::FX_UNIT_EMPTY_SLOT_H
        } else {
            tokens::FX_UNIT_TRAILING_SLOT_H
        } * dpx;
        let max_h = (bounds.h - top_pad - 4.0 * dpx).max(32.0 * dpx);
        (bounds.x, bounds.y + top_pad, bounds.w, target_h.min(max_h))
    }

    #[inline]
    fn point_in_box(&self, bounds: BoundingBox, dpx: f32, x: f32, y: f32) -> bool {
        let (bx, by, bw, bh) = self.highlight_box_rect(bounds, dpx);
        x >= bx && x <= bx + bw && y >= by && y <= by + bh
    }
}

impl View for FxEmptyLaneZone {
    fn element(&self) -> Option<&'static str> {
        Some("fx-empty-lane-zone")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseOver => {
                self.hovered_zone = true;
                self.hovered_box = self.point_in_box(
                    cx.bounds(),
                    cx.scale_factor(),
                    cx.mouse().cursorx,
                    cx.mouse().cursory,
                );
                cx.needs_redraw();
            }
            WindowEvent::MouseMove(x, y) if self.hovered_zone => {
                let next_box = self.point_in_box(cx.bounds(), cx.scale_factor(), *x, *y);
                if next_box != self.hovered_box {
                    self.hovered_box = next_box;
                    cx.needs_redraw();
                }
            }
            WindowEvent::MouseOut => {
                self.hovered_zone = false;
                self.hovered_box = false;
                cx.needs_redraw();
            }
            WindowEvent::MouseDown(MouseButton::Left) if self.can_add => {
                let scale = cx.scale_factor().max(0.01);
                cx.emit(UiEvent::ShowFxPicker(
                    self.bus_idx,
                    FxPickerTarget::InsertAt(self.insert_pos),
                    cx.mouse().cursorx / scale,
                    cx.mouse().cursory / scale,
                ));
                meta.consume();
            }
            WindowEvent::MouseDown(MouseButton::Right) => {
                let scale = cx.scale_factor().max(0.01);
                cx.emit(UiEvent::ShowFxLaneMenu(
                    self.bus_idx,
                    cx.mouse().cursorx / scale,
                    cx.mouse().cursory / scale,
                ));
                meta.consume();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        if !self.can_add || !self.hovered_zone {
            return;
        }
        let bounds = cx.bounds();
        if bounds.w <= 4.0 || bounds.h <= 8.0 {
            return;
        }
        let dpx = cx.scale_factor();
        let (bx, by, bw, bh) = self.highlight_box_rect(bounds, dpx);
        let pal = theme::current_palette(cx);
        let base_rgb = theme::hex_to_rgb(&pal.surface_hover);
        let (ar, ag, ab) = pal.accent_rgb();

        let (fill_col, border_col, border_w, text_col) = if self.hovered_box {
            let (fr, fg, fb) = theme::lerp_rgb(base_rgb, (ar, ag, ab), 0.22);
            (
                vg::Color::rgba(fr, fg, fb, 155),
                vg::Color::rgba(ar, ag, ab, 215),
                1.35 * dpx,
                theme::vg_color(&pal.text_bright, 195),
            )
        } else {
            let (fr, fg, fb) = theme::lerp_rgb(base_rgb, (ar, ag, ab), 0.08);
            (
                vg::Color::rgba(fr, fg, fb, 72),
                vg::Color::rgba(ar, ag, ab, 92),
                1.0 * dpx,
                theme::vg_color(&pal.text_muted, 110),
            )
        };

        let mut rect = vg::Path::new();
        rect.rounded_rect(bx, by, bw, bh, 5.0 * dpx);
        canvas.fill_path(&rect, &vg::Paint::color(fill_col));

        let mut stroke = vg::Paint::color(border_col);
        stroke.set_line_width(border_w);
        canvas.stroke_path(&rect, &stroke);

        let mut text_paint = vg::Paint::color(text_col);
        text_paint.set_font_size(11.0 * dpx);
        text_paint.set_text_align(vg::Align::Center);
        text_paint.set_text_baseline(vg::Baseline::Middle);
        let _ = canvas.fill_text(bx + bw * 0.5, by + bh * 0.5, "Insert FX...", &text_paint);
    }
}

/// Lane header row container that listens for right-click anywhere inside the header to open the Lane Context Menu.
struct FxLaneHeaderRow {
    bus_idx: usize,
}

impl FxLaneHeaderRow {
    fn new<'a, F>(cx: &'a mut Context, bus_idx: usize, content: F) -> Handle<'a, Self>
    where
        F: FnOnce(&mut Context),
    {
        Self { bus_idx }
            .build(cx, content)
            .layout_type(LayoutType::Row)
    }
}

impl View for FxLaneHeaderRow {
    fn element(&self) -> Option<&'static str> {
        Some("fx-lane-header-row")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Right) = window_event {
                let scale = cx.scale_factor().max(0.01);
                cx.emit(UiEvent::ShowFxLaneMenu(
                    self.bus_idx,
                    cx.mouse().cursorx / scale,
                    cx.mouse().cursory / scale,
                ));
                meta.consume();
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
    let title = lane_name(bus_idx);
    let (lvl_dest, pan_dest) = match bus_idx {
        0 => (ModDestination::BusALevel, ModDestination::BusAPan),
        1 => (ModDestination::BusBLevel, ModDestination::BusBPan),
        _ => (ModDestination::BusCLevel, ModDestination::BusCPan),
    };

    FxLaneHeaderRow::new(cx, bus_idx, move |cx| {
        let p_hdr = params.clone();
        let g_hdr = gui_context.clone();
        let p_mute = params.clone();
        let g_mute = gui_context;
        let p_cnt = params.clone();
        VStack::new(cx, move |cx| {
            // Top row: BUS A/B/C + Bypass LED + Mute [M] + active count
            HStack::new(cx, move |cx| {
                bold_label(cx, title, "module-title")
                    .width(Auto)
                    .height(Stretch(1.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0));

                LedToggleSwitch::new(
                    cx,
                    UiData::params,
                    move |p| p.fx_bus_enabled_param(bus_idx),
                    "",
                    "",
                )
                .width(Pixels(14.0))
                .height(Pixels(14.0))
                .top(Stretch(1.0))
                .bottom(Stretch(1.0));

                let p_m_press = p_mute.clone();
                let g_m_press = g_mute.clone();
                let p_m_chk = p_mute;
                Button::new(
                    cx,
                    move |cx| {
                        let setter = ParamSetter::new(g_m_press.as_ref());
                        let param = p_m_press.fx_bus_muted_param(bus_idx);
                        setter.begin_set_parameter(param);
                        setter.set_parameter(param, !param.value());
                        setter.end_set_parameter(param);
                        cx.emit(UiEvent::ModUiChanged);
                    },
                    |cx| {
                        bold_label(cx, "M", "fx-lane-mute-text")
                            .child_space(Stretch(1.0))
                            .hoverable(false)
                    },
                )
                .class("fx-lane-mute-btn")
                .checked(UiData::mod_ui_rev.map(move |_| p_m_chk.fx_bus_muted_param(bus_idx).value()))
                .width(Pixels(tokens::FX_LANE_MUTE_W))
                .height(Pixels(tokens::FX_LANE_MUTE_H))
                .top(Stretch(1.0))
                .bottom(Stretch(1.0));

                let cnt_lens = UiData::mod_ui_rev.map(move |_| {
                    let count = p_cnt.reconciled_fx_rack_order().len(bus_idx);
                    if count > 0 {
                        format!("{count}/{NUM_FX_SLOTS}")
                    } else {
                        String::new()
                    }
                });
                bold_label(cx, cnt_lens, "fx-lane-count")
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_left(Stretch(1.0))
                    .child_right(Pixels(2.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0));
            })
            .col_between(Pixels(3.0))
            .width(Stretch(1.0))
            .height(Pixels(tokens::MACRO_HDR_H + 2.0));

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
        .width(Stretch(1.0))
        .height(Stretch(1.0));

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
    visual_pos: usize,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    let drag_ref = mod_ui.fx_drag.clone();
    let drag_dim = mod_ui.fx_drag.clone();
    let p_dim = params.clone();
    FxCardBoundsTracker::new(cx, bus_idx, visual_pos, slot_idx, drag_ref, move |cx| {
        VStack::new(cx, move |cx| {
            build_fx_slot_header(
                cx,
                bus_idx,
                visual_pos,
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
                    build_fx_unit_body(
                        cx,
                        fx_type,
                        bus_idx,
                        slot_idx,
                        p_body.clone(),
                        g_body.clone(),
                        m_body.clone(),
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
        .opacity(UiData::mod_val_rev.map(move |_| {
            if drag_dim.is_dragging_source_move(bus_idx, slot_idx) {
                0.35
            } else {
                1.0
            }
        }))
        .width(Pixels(tokens::FX_UNIT_W))
        .height(Auto)
        .row_between(Pixels(tokens::SPACE_SM));
    })
    .width(Pixels(tokens::FX_UNIT_W))
    .height(Auto);
}

/// Right Wing: 3-lane Phase Plant-style scrollable FX Rack (`Bus A`, `Bus B`, `Bus C`, up to 30 active FX Units each).
pub(crate) fn build_three_lane_fx_rack(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    let drag_wing = mod_ui.fx_drag.clone();
    FxWingBoundsTracker::new(cx, drag_wing, move |cx| {
        for bus_idx in 0..3 {
            let p_lane = params.clone();
            let g_lane = gui_context.clone();
            let m_lane = mod_ui.clone();
            let p_byp = params.clone();
            let p_mute = params.clone();
            let p_opac = params.clone();
            VStack::new(cx, move |cx| {
                build_lane_header(
                    cx,
                    bus_idx,
                    p_lane.clone(),
                    g_lane.clone(),
                    m_lane.clone(),
                );

                let drag_vp = m_lane.fx_drag.clone();
                FxLaneViewportTracker::new(cx, bus_idx, drag_vp, move |cx| {
                    let p_sv = p_lane.clone();
                    let g_sv = g_lane.clone();
                    let m_sv = m_lane.clone();
                    ScrollView::new(cx, 0.0, 0.0, false, true, move |cx| {
                        let p_guard = p_sv.clone();
                        let g_guard = g_sv.clone();
                        let m_guard = m_sv.clone();
                        FxLaneScrollGuard::new(cx, move |cx| {
                            let p_bind = p_guard.clone();
                            let g_bind = g_guard.clone();
                            let m_bind = m_guard.clone();
                            Binding::new(
                                cx,
                                UiData::root.map(move |data| {
                                    LaneOrderKey::from_params(&data.params, bus_idx)
                                }),
                                move |cx, key_lens| {
                                    let key = key_lens.get(cx);
                                    let count = key.len as usize;
                                    m_bind.fx_drag.set_lane_card_count(bus_idx, count);

                                    for (visual_pos, &entry) in
                                        key.entries.iter().enumerate().take(count)
                                    {
                                        if visual_pos > 0 {
                                            FxInsertSeam::new(cx, bus_idx, visual_pos);
                                        }
                                        let slot_idx = (entry & 0xFF) as usize;
                                        build_fx_unit_card(
                                            cx,
                                            bus_idx,
                                            visual_pos,
                                            slot_idx,
                                            p_bind.clone(),
                                            g_bind.clone(),
                                            m_bind.clone(),
                                        );
                                    }

                                    let can_add = count < NUM_FX_SLOTS;
                                    FxEmptyLaneZone::new(
                                        cx,
                                        bus_idx,
                                        count,
                                        can_add,
                                        count == 0,
                                    );
                                },
                            );
                        })
                        .child_right(Pixels(tokens::FX_LANE_SCROLL_GUTTER))
                        .width(Stretch(1.0))
                        .height(Auto)
                        .min_height(Percentage(100.0));
                    })
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .entity()
                })
                .class("fx-lane-viewport")
                .opacity(UiData::mod_ui_rev.map(move |_| {
                    if p_opac.fx_bus_muted_param(bus_idx).value() {
                        0.42
                    } else if !p_opac.fx_bus_enabled_param(bus_idx).value() {
                        0.58
                    } else {
                        1.0
                    }
                }))
                .width(Stretch(1.0))
                .height(Stretch(1.0));
            })
            .class("fx-lane")
            .toggle_class(
                "fx-lane-bypassed",
                UiData::mod_ui_rev.map(move |_| !p_byp.fx_bus_enabled_param(bus_idx).value()),
            )
            .toggle_class(
                "fx-lane-muted",
                UiData::mod_ui_rev.map(move |_| p_mute.fx_bus_muted_param(bus_idx).value()),
            )
            .row_between(Pixels(tokens::SPACE_SM))
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        }
    })
    .col_between(Pixels(tokens::SPACE_MD))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}
