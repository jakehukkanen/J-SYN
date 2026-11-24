//! Minimalist FX Unit title bar:
//! `[Bypass LED] [Title / Drag Handle] [STK Pill] [Collapse Chevron]`.

use std::sync::Arc;

use j_syn_dsp::FxType;
use j_syn_ui::widgets::{LedToggleSwitch, LedToggleSwitchExt};
use nih_plug::prelude::GuiContext;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::util::ModifiersExt;

use super::super::controls::{bold_label, build_inline_pill_cell, CollapseButton};
use super::super::fx_drag_overlay::FxDragDropTarget;
use super::super::mod_bar::ModUiState;
use super::super::{theme, tokens, UiData, UiEvent};
use super::slot_ops::{
    duplicate_fx_unit, move_fx_across_lanes, remove_fx_slot, reorder_fx_within_lane,
};
use crate::{FxTypeParam, JSynPluginParams};

#[inline]
fn toggle_slot_collapse_if_active(
    cx: &mut EventContext,
    params: &JSynPluginParams,
    mod_ui: &ModUiState,
    bus_idx: usize,
    slot_idx: usize,
) {
    if params.fx_bus_slots(bus_idx)[slot_idx].fx_type.value() != FxTypeParam::Off {
        mod_ui.toggle_fx_collapsed(bus_idx, slot_idx);
        cx.emit(UiEvent::ModUiChanged);
    }
}

/// Interactive FX card title bar handle supporting:
/// - Left-drag (`>= 4 px`) to reorder within lane, move across lanes, `Alt`-duplicate, or drop outside to remove
/// - Double-click or `Cmd/Ctrl + Click` to collapse/expand card
/// - Right-click to open the FX Unit context menu
struct FxTitleDragHandle {
    bus_idx: usize,
    visual_pos: usize,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
    pressed: bool,
    dragging: bool,
    hovered: bool,
    start_x: f32,
    start_y: f32,
}

impl FxTitleDragHandle {
    fn new<'a>(
        cx: &'a mut Context,
        bus_idx: usize,
        visual_pos: usize,
        slot_idx: usize,
        params: Arc<JSynPluginParams>,
        gui_context: Arc<dyn GuiContext>,
        mod_ui: Arc<ModUiState>,
    ) -> Handle<'a, Self> {
        let p_lbl = params.clone();
        Self {
            bus_idx,
            visual_pos,
            slot_idx,
            params,
            gui_context,
            mod_ui,
            pressed: false,
            dragging: false,
            hovered: false,
            start_x: 0.0,
            start_y: 0.0,
        }
        .build(cx, move |cx| {
            let label_lens = UiData::mod_ui_rev.map(move |_| {
                let ty = FxType::from(p_lbl.fx_bus_slots(bus_idx)[slot_idx].fx_type.value());
                ty.label().to_string()
            });
            HStack::new(cx, move |cx| {
                // Subtle grip indicator space (painted in `draw`) + bold effect name
                Element::new(cx)
                    .width(Pixels(6.0))
                    .height(Stretch(1.0))
                    .hoverable(false);
                bold_label(cx, label_lens, "fx-title-text")
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0))
                    .hoverable(false);
            })
            .col_between(Pixels(2.0))
            .width(Stretch(1.0))
            .height(Stretch(1.0))
            .hoverable(false);
        })
        .class("fx-title-handle")
    }
}

impl View for FxTitleDragHandle {
    fn element(&self) -> Option<&'static str> {
        Some("fx-title-handle")
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
            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                self.pressed = false;
                if self.dragging {
                    self.dragging = false;
                    self.mod_ui.fx_drag.cancel_drag();
                    cx.release();
                }
                toggle_slot_collapse_if_active(
                    cx,
                    &self.params,
                    &self.mod_ui,
                    self.bus_idx,
                    self.slot_idx,
                );
                meta.consume();
            }
            WindowEvent::MouseDown(MouseButton::Left) => {
                let cmd_or_ctrl =
                    cx.modifiers().command() || cx.modifiers().contains(Modifiers::CTRL);
                if cmd_or_ctrl {
                    toggle_slot_collapse_if_active(
                        cx,
                        &self.params,
                        &self.mod_ui,
                        self.bus_idx,
                        self.slot_idx,
                    );
                    meta.consume();
                    return;
                }
                self.pressed = true;
                self.dragging = false;
                self.start_x = cx.mouse().cursorx;
                self.start_y = cx.mouse().cursory;
                cx.capture();
                meta.consume();
            }
            WindowEvent::MouseMove(x, y) => {
                if self.pressed {
                    let dpx = cx.scale_factor().max(1.0);
                    let alt = cx.modifiers().contains(Modifiers::ALT);
                    if !self.dragging {
                        let dist = (*x - self.start_x).hypot(*y - self.start_y);
                        if dist >= 4.0 * dpx {
                            self.dragging = true;
                            let fx_type = FxType::from(
                                self.params.fx_bus_slots(self.bus_idx)[self.slot_idx]
                                    .fx_type
                                    .value(),
                            );
                            self.mod_ui.fx_drag.start_drag(
                                self.bus_idx,
                                self.visual_pos,
                                self.slot_idx,
                                fx_type,
                                *x,
                                *y,
                                alt,
                                dpx,
                            );
                            cx.emit(UiEvent::HideFxOverlayMenu);
                            cx.emit(UiEvent::ModValChanged);
                        }
                    } else {
                        self.mod_ui.fx_drag.update_drag(*x, *y, alt, dpx);
                    }
                    cx.needs_redraw();
                    meta.consume();
                }
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.pressed {
                    self.pressed = false;
                    let was_dragging = self.dragging;
                    self.dragging = false;
                    cx.release();
                    if was_dragging {
                        if let Some(drag) = self.mod_ui.fx_drag.finish_drag() {
                            match drag.drop_target {
                                FxDragDropTarget::Insert { bus: to_bus, pos: to_pos } => {
                                    if drag.alt_duplicate {
                                        let _ = duplicate_fx_unit(
                                            &self.params,
                                            self.gui_context.as_ref(),
                                            &self.mod_ui,
                                            drag.from_bus,
                                            drag.from_slot,
                                            to_bus,
                                            to_pos,
                                        );
                                    } else if drag.from_bus == to_bus {
                                        let _ = reorder_fx_within_lane(
                                            &self.params,
                                            drag.from_bus,
                                            drag.from_pos,
                                            to_pos,
                                        );
                                    } else {
                                        let _ = move_fx_across_lanes(
                                            &self.params,
                                            self.gui_context.as_ref(),
                                            &self.mod_ui,
                                            drag.from_bus,
                                            drag.from_slot,
                                            to_bus,
                                            to_pos,
                                        );
                                    }
                                }
                                FxDragDropTarget::Remove => {
                                    remove_fx_slot(
                                        &self.params,
                                        self.gui_context.as_ref(),
                                        &self.mod_ui,
                                        drag.from_bus,
                                        drag.from_slot,
                                    );
                                }
                                FxDragDropTarget::None => {}
                            }
                            cx.emit(UiEvent::ModUiChanged);
                        }
                    }
                    cx.needs_redraw();
                    meta.consume();
                }
            }
            WindowEvent::KeyDown(Code::Escape, _) => {
                if self.dragging || self.pressed {
                    self.pressed = false;
                    self.dragging = false;
                    self.mod_ui.fx_drag.cancel_drag();
                    cx.release();
                    cx.emit(UiEvent::ModValChanged);
                    cx.needs_redraw();
                    meta.consume();
                }
            }
            WindowEvent::MouseDown(MouseButton::Right) => {
                let scale = cx.scale_factor().max(0.01);
                cx.emit(UiEvent::ShowFxUnitMenu(
                    self.bus_idx,
                    self.visual_pos,
                    self.slot_idx,
                    cx.mouse().cursorx / scale,
                    cx.mouse().cursory / scale,
                ));
                meta.consume();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        if b.w <= 2.0 || b.h <= 2.0 {
            return;
        }
        let dpx = cx.scale_factor();
        let pal = theme::current_palette(cx);
        let enabled = self.params.fx_bus_slots(self.bus_idx)[self.slot_idx]
            .enabled
            .value();

        if self.hovered || self.pressed {
            let mut bg = vg::Path::new();
            bg.rounded_rect(b.x, b.y, b.w, b.h, tokens::RADIUS_PILL * dpx);
            canvas.fill_path(&bg, &vg::Paint::color(theme::vg_color(&pal.surface_hover, 165)));
            let mut border = vg::Paint::color(theme::vg_color(&pal.border_light, 210));
            border.set_line_width(1.0 * dpx);
            canvas.stroke_path(&bg, &border);
        }

        // Draw subtle 2x3 dot grip on the left side of the title handle
        let grip_col = if self.hovered {
            theme::vg_color(&pal.accent_primary, 220)
        } else if enabled {
            theme::vg_color(&pal.text_dim, 175)
        } else {
            theme::vg_color(&pal.text_dim, 85)
        };
        let gx = b.x + 5.0 * dpx;
        let cy = b.y + b.h * 0.5;
        let step_x = 2.8 * dpx;
        let step_y = 3.2 * dpx;
        let r = 0.85 * dpx;
        for row in [-1.0_f32, 0.0, 1.0] {
            for col in [0.0_f32, 1.0] {
                let mut dot = vg::Path::new();
                dot.circle(gx + col * step_x, cy + row * step_y, r);
                canvas.fill_path(&dot, &vg::Paint::color(grip_col));
            }
        }
    }
}

pub(super) fn build_fx_slot_header(
    cx: &mut Context,
    bus_idx: usize,
    visual_pos: usize,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    HStack::new(cx, move |cx| {
        let p_led_cmd = params.clone();
        let m_led_cmd = mod_ui.clone();
        LedToggleSwitch::new(
            cx,
            UiData::params,
            move |p| &p.fx_bus_slots(bus_idx)[slot_idx].enabled,
            "",
            "",
        )
        .on_cmd_click(move |cx| {
            toggle_slot_collapse_if_active(cx, &p_led_cmd, &m_led_cmd, bus_idx, slot_idx);
        })
        .width(Pixels(tokens::FX_UNIT_LED_W))
        .height(Pixels(tokens::FX_UNIT_ARROW_H))
        .top(Stretch(1.0))
        .bottom(Stretch(1.0));

        FxTitleDragHandle::new(
            cx,
            bus_idx,
            visual_pos,
            slot_idx,
            params.clone(),
            gui_context.clone(),
            mod_ui.clone(),
        )
        .width(Stretch(1.0))
        .height(Pixels(tokens::FX_UNIT_HEADER_H));

        HStack::new(cx, move |cx| {
            build_inline_pill_cell(
                cx,
                UiData::params,
                move |p| &p.fx_bus_slots(bus_idx)[slot_idx].stack,
                "STK",
                4,
                1.0,
            )
            .class("stk-pill");
        })
        .width(Pixels(tokens::FX_UNIT_STACK_W))
        .height(Pixels(tokens::FX_UNIT_HEADER_H));

        let p_col_press = params.clone();
        let m_col_press = mod_ui.clone();
        let m_col_state = mod_ui;
        CollapseButton::reactive(
            cx,
            move |cx| {
                if cx.modifiers().contains(Modifiers::ALT) {
                    let target_collapsed = !m_col_press.fx_collapsed(bus_idx, slot_idx);
                    m_col_press.set_lane_collapsed(bus_idx, target_collapsed);
                    cx.emit(UiEvent::ModUiChanged);
                } else {
                    toggle_slot_collapse_if_active(
                        cx,
                        &p_col_press,
                        &m_col_press,
                        bus_idx,
                        slot_idx,
                    );
                }
            },
            UiData::mod_ui_rev,
            move |_| m_col_state.fx_collapsed(bus_idx, slot_idx),
        )
        .class("fx-move-btn")
        .class("fx-move-label")
        .width(Pixels(tokens::FX_UNIT_COLLAPSE_W))
        .height(Pixels(tokens::FX_UNIT_ARROW_H))
        .top(Stretch(1.0))
        .bottom(Stretch(1.0));
    })
    .col_between(Pixels(tokens::SPACE_XS))
    .height(Pixels(tokens::FX_UNIT_HEADER_H))
    .width(Stretch(1.0));
}
