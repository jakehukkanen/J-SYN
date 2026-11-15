//! Custom skeuomorphic `femtovg` and `ParamWidgetBase` controls (`RotaryKnob`, `LedToggleSwitch`, `ParamStepPill`).

use std::f32::consts::PI;
use std::sync::Arc;

use j_syn_dsp::mod_matrix::{ModDestination, ModMatrixSource};
use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::util::ModifiersExt;
use nih_plug_vizia::widgets::RawParamEvent;

use super::controls::bold_label;
use super::mod_bar::ModUiState;
use super::{TooltipState, UiEvent};

pub(super) type ValueFormatter = Arc<dyn Fn(f32, Option<f32>) -> String + Send + Sync>;

const KNOB_DRAG_PIXELS: f32 = 150.0;
const KNOB_FINE_MULTIPLIER: f32 = 0.12;
const MOD_DEPTH_DRAG_PIXELS: f32 = 120.0;

#[derive(Debug, Clone, Copy)]
struct DragState {
    start_y: f32,
    start_val: f32,
}

#[derive(Debug, Clone, Copy)]
struct ModDepthDrag {
    slot_idx: usize,
    start_y: f32,
    start_depth: f32,
}

#[derive(Clone)]
struct ModTargetBinding {
    dest: ModDestination,
    mod_ui: Arc<ModUiState>,
    tab_tag: u8,
}

fn mod_slot_offsets(mb: &ModTargetBinding, md: ModDepthDrag) -> (f32, f32, f32) {
    let mat = mb.mod_ui.params.mod_matrix.snapshot();
    let slot = mat
        .slots
        .get(md.slot_idx)
        .copied()
        .filter(|s| s.enabled && s.source != ModMatrixSource::None)
        .or_else(|| mb.mod_ui.primary_slot_for_dest(mb.dest).map(|(_, s)| s));
    let (depth, bipolar) = match slot {
        Some(s) => (s.depth, s.bipolar),
        None => (md.start_depth, false),
    };
    let (lo_offset, hi_offset) = if bipolar {
        let span = depth.abs();
        (-span, span)
    } else if depth >= 0.0 {
        (0.0, depth)
    } else {
        (depth, 0.0)
    };
    (lo_offset, hi_offset, depth)
}

fn format_mod_range_tooltip(
    base_norm: f32,
    mb: &ModTargetBinding,
    md: ModDepthDrag,
    fmt: impl Fn(f32, Option<f32>) -> String,
) -> String {
    let (lo_offset, hi_offset, depth) = mod_slot_offsets(mb, md);
    let lo_norm = (base_norm + lo_offset).clamp(0.0, 1.0);
    let hi_norm = (base_norm + hi_offset).clamp(0.0, 1.0);
    let min_str = fmt(lo_norm, Some(lo_offset));
    let max_str = fmt(hi_norm, Some(hi_offset));
    format!("{min_str} → {max_str} ({:+.0}%)", depth * 100.0)
}

fn emit_widget_tooltip(cx: &mut EventContext, text: String) {
    let bounds = cx.bounds();
    if bounds.w <= 0.0 || bounds.h <= 0.0 {
        return;
    }
    let dpx = cx.scale_factor().max(0.1);
    cx.emit(UiEvent::ShowTooltip(TooltipState {
        visible: true,
        text,
        center_x: (bounds.x + bounds.w * 0.5) / dpx,
        anchor_top: bounds.y / dpx,
        anchor_bottom: (bounds.y + bounds.h) / dpx,
    }));
}

/// Handles Alt+drag modulation depth changes when active, returning true if handled.
fn handle_mod_depth_move(
    mod_depth_drag: &mut Option<ModDepthDrag>,
    mod_binding: &Option<ModTargetBinding>,
    cx: &mut EventContext,
    y: f32,
) -> bool {
    let (Some(md), Some(mb)) = (mod_depth_drag.as_mut(), mod_binding.as_ref()) else {
        return false;
    };
    let sensitivity = if cx.modifiers().shift() { 0.2 } else { 1.0 };
    let dy = (md.start_y - y) / (MOD_DEPTH_DRAG_PIXELS * cx.scale_factor());
    let new_depth = (md.start_depth + dy * sensitivity).clamp(-1.0, 1.0);
    let prev_pct = (md.start_depth * 100.0).round() as i32;
    let next_pct = (new_depth * 100.0).round() as i32;
    mb.mod_ui.adjust_slot_depth(md.slot_idx, new_depth);
    md.start_y = y;
    md.start_depth = new_depth;
    if prev_pct != next_pct {
        cx.emit(UiEvent::ModUiChanged);
    }
    cx.needs_redraw();
    true
}

/// Skeuomorphic 3D rotary knob widget rendered via `femtovg` (`vizia::vg`).
pub struct RotaryKnob {
    param_base: ParamWidgetBase,
    drag_active: bool,
    drag_state: Option<DragState>,
    mod_depth_drag: Option<ModDepthDrag>,
    scrolled_lines: f32,
    hovered: bool,
    bipolar: bool,
    accent_rgb: (u8, u8, u8),
    mod_binding: Option<ModTargetBinding>,
    value_formatter: Option<ValueFormatter>,
    enabled_state: Option<(Arc<crate::JSynPluginParams>, fn(&crate::JSynPluginParams) -> bool)>,
}

pub(super) trait KnobDimExt {
    fn enabled_by(
        self,
        params: Arc<crate::JSynPluginParams>,
        check: fn(&crate::JSynPluginParams) -> bool,
    ) -> Self;
}

impl KnobDimExt for Handle<'_, RotaryKnob> {
    fn enabled_by(
        self,
        params: Arc<crate::JSynPluginParams>,
        check: fn(&crate::JSynPluginParams) -> bool,
    ) -> Self {
        self.modify(move |knob| knob.enabled_state = Some((params, check)))
    }
}

impl RotaryKnob {
    pub fn new<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        bipolar: bool,
        accent_rgb: (u8, u8, u8),
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self::build_inner(cx, params, params_to_param, bipolar, accent_rgb, None, None)
    }

    pub fn new_with_mod<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        bipolar: bool,
        accent_rgb: (u8, u8, u8),
        dest: ModDestination,
        mod_ui: Arc<ModUiState>,
        tab_tag: u8,
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self::build_inner(
            cx,
            params,
            params_to_param,
            bipolar,
            accent_rgb,
            Some(ModTargetBinding {
                dest,
                mod_ui,
                tab_tag,
            }),
            None,
        )
    }

    pub fn new_with_mod_formatter<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        bipolar: bool,
        accent_rgb: (u8, u8, u8),
        dest: ModDestination,
        mod_ui: Arc<ModUiState>,
        tab_tag: u8,
        value_formatter: ValueFormatter,
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self::build_inner(
            cx,
            params,
            params_to_param,
            bipolar,
            accent_rgb,
            Some(ModTargetBinding {
                dest,
                mod_ui,
                tab_tag,
            }),
            Some(value_formatter),
        )
    }

    fn build_inner<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        bipolar: bool,
        accent_rgb: (u8, u8, u8),
        mod_binding: Option<ModTargetBinding>,
        value_formatter: Option<ValueFormatter>,
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self {
            param_base: ParamWidgetBase::new(cx, params.clone(), params_to_param),
            drag_active: false,
            drag_state: None,
            mod_depth_drag: None,
            scrolled_lines: 0.0,
            hovered: false,
            bipolar,
            accent_rgb,
            mod_binding,
            value_formatter,
            enabled_state: None,
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, param_data| {
                let norm_lens =
                    param_data.make_lens(|param| param.modulated_normalized_value());
                Binding::new(cx, norm_lens, |cx, _| {
                    cx.needs_redraw();
                    Element::new(cx).hoverable(false);
                });
            }),
        )
    }

    fn format_norm_value(&self, norm: f32, mod_offset: Option<f32>) -> String {
        let clamped = norm.clamp(0.0, 1.0);
        if let Some(ref fmt) = self.value_formatter {
            fmt(clamped, mod_offset)
        } else {
            self.param_base.normalized_value_to_string(clamped, true)
        }
    }

    fn format_tooltip_text(&self) -> String {
        if let (Some(md), Some(ref mb)) = (self.mod_depth_drag, self.mod_binding.as_ref()) {
            let base_norm = self.param_base.unmodulated_normalized_value().clamp(0.0, 1.0);
            format_mod_range_tooltip(base_norm, mb, md, |n, off| {
                self.format_norm_value(n, off)
            })
        } else if let Some(md) = self.mod_depth_drag {
            format!("Mod: {:+.0}%", md.start_depth * 100.0)
        } else {
            self.format_norm_value(self.param_base.modulated_normalized_value(), None)
        }
    }

    fn emit_tooltip(&self, cx: &mut EventContext) {
        emit_widget_tooltip(cx, self.format_tooltip_text());
    }

    fn is_on_mod_collar(&self, cx: &EventContext, x: f32, y: f32) -> bool {
        let Some(ref mb) = self.mod_binding else {
            return false;
        };
        if mb.mod_ui.primary_slot_for_dest(mb.dest).is_none() {
            return false;
        }
        let bounds = cx.bounds();
        let dpx = cx.scale_factor();
        let center_x = bounds.x + bounds.w * 0.5;
        let center_y = bounds.y + bounds.h * 0.52;
        let max_r = (bounds.w.min(bounds.h) * 0.5) - 2.0 * dpx;
        let mod_ring_r = max_r * 0.99;
        let arc_r = (max_r * 0.81).min(mod_ring_r - 3.6 * dpx);
        (x - center_x).hypot(y - center_y) > (arc_r + 1.4 * dpx)
    }

    fn reset_to_default(&mut self, cx: &mut EventContext) {
        if self.drag_active || self.mod_depth_drag.is_some() {
            let was_param_drag = self.drag_active;
            self.drag_active = false;
            self.drag_state = None;
            self.mod_depth_drag = None;
            cx.release();
            cx.set_active(false);
            if was_param_drag {
                self.param_base.end_set_parameter(cx);
            }
        }
        let target = self.param_base.default_normalized_value();
        self.param_base.begin_set_parameter(cx);
        self.param_base.set_normalized_value(cx, target);
        self.param_base.end_set_parameter(cx);
        if self.hovered {
            self.emit_tooltip(cx);
        }
        cx.needs_redraw();
    }
}

impl View for RotaryKnob {
    fn element(&self) -> Option<&'static str> {
        Some("rotary-knob")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|raw_param_event, _| {
            if let RawParamEvent::ParametersChanged = raw_param_event {
                if self.hovered || self.drag_active {
                    self.emit_tooltip(cx);
                }
                cx.needs_redraw();
            }
        });

        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseOver => {
                self.hovered = true;
                let cursor = if self.is_on_mod_collar(cx, cx.mouse().cursorx, cx.mouse().cursory) {
                    CursorIcon::NsResize
                } else {
                    CursorIcon::Default
                };
                cx.emit(WindowEvent::SetCursor(cursor));
                self.emit_tooltip(cx);
                cx.needs_redraw();
            }
            WindowEvent::MouseOut => {
                self.hovered = false;
                if !self.drag_active && self.mod_depth_drag.is_none() {
                    cx.emit(WindowEvent::SetCursor(CursorIcon::Default));
                    cx.emit(UiEvent::HideTooltip);
                }
                cx.needs_redraw();
            }
            WindowEvent::MouseDown(MouseButton::Left)
            | WindowEvent::MouseTripleClick(MouseButton::Left) => {
                if cx.modifiers().command() || cx.modifiers().contains(Modifiers::CTRL) {
                    self.reset_to_default(cx);
                    meta.consume();
                    return;
                }
                if let Some(ref mb) = self.mod_binding {
                    if let Some((slot_idx, slot)) = mb.mod_ui.primary_slot_for_dest(mb.dest) {
                        if cx.modifiers().contains(Modifiers::ALT)
                            || self.is_on_mod_collar(cx, cx.mouse().cursorx, cx.mouse().cursory)
                        {
                            self.mod_depth_drag = Some(ModDepthDrag {
                                slot_idx,
                                start_y: cx.mouse().cursory,
                                start_depth: slot.depth,
                            });
                            mb.mod_ui.focus_slot(mb.dest, slot_idx);
                            cx.capture();
                            cx.focus();
                            cx.set_active(true);
                            cx.emit(WindowEvent::SetCursor(CursorIcon::Grabbing));
                            cx.emit(UiEvent::ModUiChanged);
                            self.emit_tooltip(cx);
                            cx.needs_redraw();
                            meta.consume();
                            return;
                        }
                    }
                }
                self.drag_active = true;
                cx.capture();
                cx.focus();
                cx.set_active(true);
                self.param_base.begin_set_parameter(cx);
                self.drag_state = Some(DragState {
                    start_y: cx.mouse().cursory,
                    start_val: self.param_base.unmodulated_normalized_value(),
                });
                self.emit_tooltip(cx);
                cx.needs_redraw();
                meta.consume();
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                self.reset_to_default(cx);
                meta.consume();
            }
            WindowEvent::MouseDown(MouseButton::Right)
            | WindowEvent::MouseDoubleClick(MouseButton::Right)
            | WindowEvent::MouseTripleClick(MouseButton::Right) => {
                if let Some(ref mb) = self.mod_binding {
                    if cx.modifiers().shift() {
                        if mb.mod_ui.remove_primary_slot_for_dest(mb.dest) {
                            cx.emit(UiEvent::ModUiChanged);
                            cx.needs_redraw();
                            meta.consume();
                            return;
                        }
                    } else if mb.mod_ui.has_slots_for_dest(mb.dest) {
                        mb.mod_ui.cycle_inspected_slot_for_dest(mb.dest);
                        cx.emit(UiEvent::ModUiChanged);
                        cx.needs_redraw();
                        meta.consume();
                        return;
                    }
                }
                self.reset_to_default(cx);
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.mod_depth_drag.is_some() {
                    self.mod_depth_drag = None;
                    cx.release();
                    cx.set_active(false);
                    if self.hovered {
                        let cursor = if self.is_on_mod_collar(
                            cx,
                            cx.mouse().cursorx,
                            cx.mouse().cursory,
                        ) {
                            CursorIcon::NsResize
                        } else {
                            CursorIcon::Default
                        };
                        cx.emit(WindowEvent::SetCursor(cursor));
                        self.emit_tooltip(cx);
                    } else {
                        cx.emit(WindowEvent::SetCursor(CursorIcon::Default));
                        cx.emit(UiEvent::HideTooltip);
                    }
                    cx.needs_redraw();
                    meta.consume();
                } else if self.drag_active {
                    self.drag_active = false;
                    self.drag_state = None;
                    cx.release();
                    cx.set_active(false);
                    self.param_base.end_set_parameter(cx);
                    if self.hovered {
                        self.emit_tooltip(cx);
                    } else {
                        cx.emit(UiEvent::HideTooltip);
                    }
                    cx.needs_redraw();
                    meta.consume();
                }
            }
            WindowEvent::MouseMove(x, y) => {
                if handle_mod_depth_move(&mut self.mod_depth_drag, &self.mod_binding, cx, *y) {
                    cx.emit(WindowEvent::SetCursor(CursorIcon::Grabbing));
                    self.emit_tooltip(cx);
                    meta.consume();
                    return;
                }
                if self.drag_active {
                    let state = self.drag_state.get_or_insert(DragState {
                        start_y: *y,
                        start_val: self.param_base.unmodulated_normalized_value(),
                    });

                    let sensitivity = if cx.modifiers().shift() {
                        KNOB_FINE_MULTIPLIER
                    } else {
                        1.0
                    };

                    let dy = (state.start_y - *y) / (KNOB_DRAG_PIXELS * cx.scale_factor());
                    let new_val = (state.start_val + dy * sensitivity).clamp(0.0, 1.0);
                    self.param_base.set_normalized_value(cx, new_val);

                    state.start_y = *y;
                    state.start_val = new_val;

                    self.emit_tooltip(cx);
                    cx.needs_redraw();
                    meta.consume();
                } else if self.hovered {
                    let cursor = if self.is_on_mod_collar(cx, *x, *y) {
                        CursorIcon::NsResize
                    } else {
                        CursorIcon::Default
                    };
                    cx.emit(WindowEvent::SetCursor(cursor));
                }
            }
            WindowEvent::MouseScroll(_scroll_x, scroll_y) => {
                self.scrolled_lines += scroll_y;
                if self.scrolled_lines.abs() >= 0.5 {
                    let finer = cx.modifiers().shift();
                    if !self.drag_active {
                        self.param_base.begin_set_parameter(cx);
                    }

                    let mut current = self.param_base.unmodulated_normalized_value();
                    let step = if finer { 0.004 } else { 0.02 };
                    let delta = self.scrolled_lines * step;
                    current = (current + delta).clamp(0.0, 1.0);
                    self.param_base.set_normalized_value(cx, current);
                    self.scrolled_lines = 0.0;

                    if !self.drag_active {
                        self.param_base.end_set_parameter(cx);
                    }
                    if self.hovered || self.drag_active {
                        self.emit_tooltip(cx);
                    }
                    cx.needs_redraw();
                }
                meta.consume();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 0.0 || bounds.h <= 0.0 {
            return;
        }

        let dpx = cx.scale_factor();
        let center_x = bounds.x + bounds.w * 0.5;
        let center_y = bounds.y + bounds.h * 0.52;
        let max_r = (bounds.w.min(bounds.h) * 0.5) - 2.0 * dpx;

        let mod_ring_r = max_r * 0.99;
        let arc_r = (max_r * 0.81).min(mod_ring_r - 3.6 * dpx);
        let body_r = arc_r * 0.78;
        let cap_r = body_r * 0.80;

        let norm = self.param_base.modulated_normalized_value().clamp(0.0, 1.0);

        let start_angle = 0.75 * PI;
        let sweep_angle = 1.50 * PI;
        let end_angle = start_angle + sweep_angle;
        let val_angle = start_angle + norm * sweep_angle;

        if let Some(ref mb) = self.mod_binding {
            mb.mod_ui.register_drop_rect(mb.dest, mb.tab_tag, bounds);
            let drag_src = mb.mod_ui.dragging_source();
            if drag_src != ModMatrixSource::None {
                let (sr, sg, sb) = drag_src.color_rgb();
                let is_hover = mb.mod_ui.is_drop_target(mb.dest);
                let mut ring = vg::Path::new();
                ring.circle(center_x, center_y, mod_ring_r + 1.0 * dpx);
                let mut ring_paint = vg::Paint::color(vg::Color::rgba(
                    sr,
                    sg,
                    sb,
                    if is_hover { 245 } else { 80 },
                ));
                ring_paint.set_line_width((if is_hover { 2.2 } else { 1.1 }) * dpx);
                canvas.stroke_path(&ring, &ring_paint);
            }
        }

        let is_enabled = self
            .enabled_state
            .as_ref()
            .is_none_or(|(p, check)| check(p));

        let num_ticks = 9;
        for i in 0..num_ticks {
            let t = i as f32 / (num_ticks - 1) as f32;
            let a = start_angle + t * sweep_angle;
            let r_in = arc_r - 1.4 * dpx;
            let r_out = arc_r + 1.4 * dpx;
            let mut tick = vg::Path::new();
            tick.move_to(center_x + a.cos() * r_in, center_y + a.sin() * r_in);
            tick.line_to(center_x + a.cos() * r_out, center_y + a.sin() * r_out);
            let is_major = i == 0 || i == num_ticks - 1 || i == num_ticks / 2;
            let mut paint = if is_enabled {
                if is_major {
                    vg::Paint::color(vg::Color::rgba(218, 198, 175, 200))
                } else {
                    vg::Paint::color(vg::Color::rgba(155, 138, 120, 135))
                }
            } else if is_major {
                vg::Paint::color(vg::Color::rgba(115, 105, 95, 85))
            } else {
                vg::Paint::color(vg::Color::rgba(90, 82, 74, 55))
            };
            paint.set_line_width((if is_major { 1.1 } else { 0.8 }) * dpx);
            canvas.stroke_path(&tick, &paint);
        }

        let mut track_path = vg::Path::new();
        track_path.arc(center_x, center_y, arc_r, start_angle, end_angle, vg::Solidity::Hole);
        let mut track_paint = vg::Paint::color(vg::Color::rgba(30, 25, 21, 245));
        track_paint.set_line_width(2.8 * dpx);
        track_paint.set_line_cap(vg::LineCap::Round);
        canvas.stroke_path(&track_path, &track_paint);

        let (ar, ag, ab) = if is_enabled {
            self.accent_rgb
        } else {
            (92, 84, 78)
        };
        let (arc_a0, arc_a1) = if self.bipolar {
            let mid_angle = start_angle + 0.5 * sweep_angle;
            if val_angle >= mid_angle {
                (mid_angle, val_angle)
            } else {
                (val_angle, mid_angle)
            }
        } else {
            (start_angle, val_angle)
        };

        if (arc_a1 - arc_a0).abs() > 0.015 {
            if is_enabled {
                let mut glow_path = vg::Path::new();
                glow_path.arc(center_x, center_y, arc_r, arc_a0, arc_a1, vg::Solidity::Hole);
                let mut glow_paint = vg::Paint::color(vg::Color::rgba(ar, ag, ab, 80));
                glow_paint.set_line_width(5.5 * dpx);
                glow_paint.set_line_cap(vg::LineCap::Round);
                canvas.stroke_path(&glow_path, &glow_paint);
            }

            let mut val_path = vg::Path::new();
            val_path.arc(center_x, center_y, arc_r, arc_a0, arc_a1, vg::Solidity::Hole);
            let mut val_paint =
                vg::Paint::color(vg::Color::rgba(ar, ag, ab, if is_enabled { 250 } else { 150 }));
            val_paint.set_line_width(2.4 * dpx);
            val_paint.set_line_cap(vg::LineCap::Round);
            canvas.stroke_path(&val_path, &val_paint);
        }

        let mut shadow_path = vg::Path::new();
        shadow_path.circle(center_x, center_y + 2.4 * dpx, body_r * 1.03);
        let shadow_paint = vg::Paint::radial_gradient(
            center_x,
            center_y + 2.4 * dpx,
            body_r * 0.5,
            body_r * 1.12,
            vg::Color::rgba(14, 11, 8, if is_enabled { 200 } else { 110 }),
            vg::Color::rgba(14, 11, 8, 0),
        );
        canvas.fill_path(&shadow_path, &shadow_paint);

        let mut body_path = vg::Path::new();
        body_path.circle(center_x, center_y, body_r);
        let body_paint = vg::Paint::linear_gradient(
            center_x,
            center_y - body_r,
            center_x,
            center_y + body_r,
            if is_enabled {
                vg::Color::rgb(112, 98, 86)
            } else {
                vg::Color::rgb(70, 62, 56)
            },
            if is_enabled {
                vg::Color::rgb(50, 42, 36)
            } else {
                vg::Color::rgb(38, 33, 29)
            },
        );
        canvas.fill_path(&body_path, &body_paint);

        let mut rim_stroke = vg::Paint::color(if self.hovered || self.drag_active {
            vg::Color::rgba(ar, ag, ab, if is_enabled { 210 } else { 145 })
        } else {
            vg::Color::rgba(24, 20, 17, 235)
        });
        rim_stroke.set_line_width(1.1 * dpx);
        canvas.stroke_path(&body_path, &rim_stroke);

        let (cap_hi, cap_lo) = if is_enabled {
            if self.hovered || self.drag_active {
                (vg::Color::rgb(156, 138, 122), vg::Color::rgb(74, 63, 55))
            } else {
                (vg::Color::rgb(138, 122, 108), vg::Color::rgb(62, 53, 46))
            }
        } else if self.hovered || self.drag_active {
            (vg::Color::rgb(98, 88, 80), vg::Color::rgb(54, 47, 42))
        } else {
            (vg::Color::rgb(86, 76, 68), vg::Color::rgb(48, 42, 37))
        };
        let mut cap_path = vg::Path::new();
        cap_path.circle(center_x, center_y, cap_r);
        let cap_paint = vg::Paint::radial_gradient(
            center_x - cap_r * 0.3,
            center_y - cap_r * 0.35,
            cap_r * 0.08,
            cap_r * 1.1,
            cap_hi,
            cap_lo,
        );
        canvas.fill_path(&cap_path, &cap_paint);

        let p_in = cap_r * 0.24;
        let p_out = cap_r * 0.88;
        let px0 = center_x + val_angle.cos() * p_in;
        let py0 = center_y + val_angle.sin() * p_in;
        let px1 = center_x + val_angle.cos() * p_out;
        let py1 = center_y + val_angle.sin() * p_out;

        let mut ptr_path = vg::Path::new();
        ptr_path.move_to(px0, py0);
        ptr_path.line_to(px1, py1);
        let mut ptr_paint = vg::Paint::color(if is_enabled {
            vg::Color::rgb(255, 244, 228)
        } else {
            vg::Color::rgba(155, 142, 130, 140)
        });
        ptr_paint.set_line_width(1.8 * dpx);
        ptr_paint.set_line_cap(vg::LineCap::Round);
        canvas.stroke_path(&ptr_path, &ptr_paint);

        // Radial modulation collar on a separated recessed outer track + end-cap tick
        if let Some(ref mb) = self.mod_binding {
            let (slots, _indices, count) = mb.mod_ui.slots_for_dest(mb.dest);
            if count > 0 {
                let mut mod_track = vg::Path::new();
                mod_track.arc(
                    center_x,
                    center_y,
                    mod_ring_r,
                    start_angle,
                    end_angle,
                    vg::Solidity::Hole,
                );
                let mut mod_track_paint = vg::Paint::color(vg::Color::rgba(14, 11, 9, 235));
                mod_track_paint.set_line_width(3.4 * dpx);
                mod_track_paint.set_line_cap(vg::LineCap::Round);
                canvas.stroke_path(&mod_track, &mod_track_paint);

                let base_norm = self.param_base.unmodulated_normalized_value().clamp(0.0, 1.0);
                for slot in slots.iter().take(count) {
                    let (sr, sg, sb) = if is_enabled {
                        slot.source.color_rgb()
                    } else {
                        (110, 100, 92)
                    };
                    let target_norm = (base_norm + slot.depth).clamp(0.0, 1.0);
                    let (lo_norm, hi_norm) = if slot.bipolar {
                        let span = slot.depth.abs();
                        (
                            (base_norm - span).clamp(0.0, 1.0),
                            (base_norm + span).clamp(0.0, 1.0),
                        )
                    } else {
                        (base_norm.min(target_norm), base_norm.max(target_norm))
                    };
                    if (hi_norm - lo_norm) > 0.004 {
                        let a0 = start_angle + lo_norm * sweep_angle;
                        let a1 = start_angle + hi_norm * sweep_angle;
                        if is_enabled {
                            let mut mglow = vg::Path::new();
                            mglow.arc(center_x, center_y, mod_ring_r, a0, a1, vg::Solidity::Hole);
                            let mut mglow_paint = vg::Paint::color(vg::Color::rgba(sr, sg, sb, 75));
                            mglow_paint.set_line_width(4.2 * dpx);
                            mglow_paint.set_line_cap(vg::LineCap::Round);
                            canvas.stroke_path(&mglow, &mglow_paint);
                        }

                        let mut mpath = vg::Path::new();
                        mpath.arc(center_x, center_y, mod_ring_r, a0, a1, vg::Solidity::Hole);
                        let mut mpaint = vg::Paint::color(vg::Color::rgba(
                            sr,
                            sg,
                            sb,
                            if is_enabled { 248 } else { 155 },
                        ));
                        mpaint.set_line_width(2.2 * dpx);
                        mpaint.set_line_cap(vg::LineCap::Round);
                        canvas.stroke_path(&mpath, &mpaint);
                    }

                    // Bright radial end-cap tick marking modulation endpoint
                    let cap_angle = start_angle + target_norm * sweep_angle;
                    let tx0 = center_x + cap_angle.cos() * (mod_ring_r - 2.2 * dpx);
                    let ty0 = center_y + cap_angle.sin() * (mod_ring_r - 2.2 * dpx);
                    let tx1 = center_x + cap_angle.cos() * (mod_ring_r + 2.2 * dpx);
                    let ty1 = center_y + cap_angle.sin() * (mod_ring_r + 2.2 * dpx);
                    let mut end_tick = vg::Path::new();
                    end_tick.move_to(tx0, ty0);
                    end_tick.line_to(tx1, ty1);
                    let mut tick_paint = vg::Paint::color(if is_enabled {
                        vg::Color::rgb(255, 250, 240)
                    } else {
                        vg::Color::rgba(165, 154, 144, 165)
                    });
                    tick_paint.set_line_width(1.7 * dpx);
                    tick_paint.set_line_cap(vg::LineCap::Round);
                    canvas.stroke_path(&end_tick, &tick_paint);
                }

                let live_offset = mb.mod_ui.telemetry.dest_offset(mb.dest);
                let live_norm = (base_norm + live_offset).clamp(0.0, 1.0);
                let live_angle = start_angle + live_norm * sweep_angle;
                let (dr, dg, db) = if is_enabled {
                    slots[0].source.color_rgb()
                } else {
                    (110, 100, 92)
                };
                let lx = center_x + live_angle.cos() * mod_ring_r;
                let ly = center_y + live_angle.sin() * mod_ring_r;
                let mut dot = vg::Path::new();
                dot.circle(lx, ly, 2.3 * dpx);
                canvas.fill_path(
                    &dot,
                    &vg::Paint::color(if is_enabled {
                        vg::Color::rgb(255, 248, 232)
                    } else {
                        vg::Color::rgba(165, 154, 144, 165)
                    }),
                );
                let mut dot_ring = vg::Paint::color(vg::Color::rgb(dr, dg, db));
                dot_ring.set_line_width(1.2 * dpx);
                canvas.stroke_path(&dot, &dot_ring);
            }
        }
    }
}

/// Skeuomorphic illuminated hardware push-button / LED switch.
pub struct LedToggleSwitch {
    param_base: ParamWidgetBase,
    accent_rgb: (u8, u8, u8),
    hovered: bool,
}

impl LedToggleSwitch {
    pub fn new<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        off_label: &'static str,
        on_label: &'static str,
        accent_rgb: (u8, u8, u8),
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        let text_params = params.clone();
        Self {
            param_base: ParamWidgetBase::new(cx, params.clone(), params_to_param),
            accent_rgb,
            hovered: false,
        }
        .build(cx, move |cx| {
            let text_lens = ParamWidgetBase::make_lens(
                text_params,
                params_to_param,
                move |param| {
                    if param.modulated_normalized_value() >= 0.5 {
                        on_label
                    } else {
                        off_label
                    }
                },
            );
            HStack::new(cx, move |cx| {
                Element::new(cx)
                    .width(Pixels(11.0))
                    .height(Stretch(1.0))
                    .hoverable(false);
                bold_label(cx, text_lens, "switch-label")
                    .width(Auto)
                    .height(Stretch(1.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0))
                    .hoverable(false);
            })
            .width(Stretch(1.0))
            .height(Stretch(1.0))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0))
            .child_left(Pixels(4.0))
            .child_right(Pixels(4.0))
            .col_between(Pixels(3.0))
            .hoverable(false);
        })
        .checked(ParamWidgetBase::make_lens(
            params,
            params_to_param,
            |param| param.modulated_normalized_value() >= 0.5,
        ))
    }

    fn toggle(&self, cx: &mut EventContext) {
        let current = self.param_base.unmodulated_normalized_value();
        let next = if current >= 0.5 { 0.0 } else { 1.0 };
        self.param_base.begin_set_parameter(cx);
        self.param_base.set_normalized_value(cx, next);
        self.param_base.end_set_parameter(cx);
        cx.needs_redraw();
    }
}

impl View for LedToggleSwitch {
    fn element(&self) -> Option<&'static str> {
        Some("led-toggle")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|raw_param_event, _| {
            if let RawParamEvent::ParametersChanged = raw_param_event {
                cx.set_checked(self.param_base.modulated_normalized_value() >= 0.5);
                cx.needs_redraw();
            }
        });

        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseOver => {
                self.hovered = true;
                cx.needs_redraw();
            }
            WindowEvent::MouseOut => {
                self.hovered = false;
                cx.needs_redraw();
            }
            WindowEvent::MouseDown(MouseButton::Left)
            | WindowEvent::MouseDoubleClick(MouseButton::Left)
            | WindowEvent::MouseTripleClick(MouseButton::Left) => {
                if cx.modifiers().command() || cx.modifiers().contains(Modifiers::CTRL) {
                    let def = self.param_base.default_normalized_value();
                    self.param_base.begin_set_parameter(cx);
                    self.param_base.set_normalized_value(cx, def);
                    self.param_base.end_set_parameter(cx);
                    cx.emit(super::UiEvent::ModUiChanged);
                    cx.needs_redraw();
                } else {
                    self.toggle(cx);
                    cx.emit(super::UiEvent::ModUiChanged);
                }
                meta.consume();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 0.0 || bounds.h <= 0.0 {
            return;
        }

        let dpx = cx.scale_factor();
        let is_on = self.param_base.modulated_normalized_value() >= 0.5;
        let (ar, ag, ab) = self.accent_rgb;

        let r = 3.5 * dpx;
        let mut bg = vg::Path::new();
        bg.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, r);

        let housing_paint = if is_on && self.hovered {
            vg::Paint::linear_gradient(
                bounds.x,
                bounds.y,
                bounds.x,
                bounds.y + bounds.h,
                vg::Color::rgb(98, 84, 72),
                vg::Color::rgb(64, 54, 46),
            )
        } else if is_on {
            vg::Paint::linear_gradient(
                bounds.x,
                bounds.y,
                bounds.x,
                bounds.y + bounds.h,
                vg::Color::rgb(82, 70, 60),
                vg::Color::rgb(52, 44, 38),
            )
        } else if self.hovered {
            vg::Paint::linear_gradient(
                bounds.x,
                bounds.y,
                bounds.x,
                bounds.y + bounds.h,
                vg::Color::rgb(68, 58, 50),
                vg::Color::rgb(48, 40, 35),
            )
        } else {
            vg::Paint::linear_gradient(
                bounds.x,
                bounds.y,
                bounds.x,
                bounds.y + bounds.h,
                vg::Color::rgb(54, 46, 40),
                vg::Color::rgb(38, 32, 28),
            )
        };
        canvas.fill_path(&bg, &housing_paint);

        let mut border = vg::Paint::color(if is_on {
            vg::Color::rgba(ar, ag, ab, if self.hovered { 235 } else { 175 })
        } else if self.hovered {
            vg::Color::rgba(ar, ag, ab, 120)
        } else {
            vg::Color::rgba(24, 20, 17, 235)
        });
        border.set_line_width(1.0 * dpx);
        canvas.stroke_path(&bg, &border);

        let led_x = bounds.x + 10.0 * dpx;
        let led_y = bounds.y + bounds.h * 0.5;
        let led_r = 3.2 * dpx;

        if is_on {
            let mut glow = vg::Path::new();
            glow.circle(led_x, led_y, led_r * 2.0);
            let glow_paint = vg::Paint::radial_gradient(
                led_x,
                led_y,
                led_r * 0.2,
                led_r * 2.0,
                vg::Color::rgba(ar, ag, ab, 165),
                vg::Color::rgba(ar, ag, ab, 0),
            );
            canvas.fill_path(&glow, &glow_paint);
        }

        let mut lamp = vg::Path::new();
        lamp.circle(led_x, led_y, led_r);
        let lamp_paint = if is_on {
            vg::Paint::radial_gradient(
                led_x - led_r * 0.25,
                led_y - led_r * 0.25,
                led_r * 0.1,
                led_r,
                vg::Color::rgb(255, 250, 240),
                vg::Color::rgb(ar, ag, ab),
            )
        } else {
            vg::Paint::radial_gradient(
                led_x - led_r * 0.2,
                led_y - led_r * 0.2,
                led_r * 0.1,
                led_r,
                vg::Color::rgb(92, 80, 70),
                vg::Color::rgb(44, 38, 33),
            )
        };
        canvas.fill_path(&lamp, &lamp_paint);
    }
}

#[inline]
fn format_inline_value(raw: String) -> String {
    if let Some(v) = raw.strip_suffix(" %") {
        format!("{v}%")
    } else if let Some(v) = raw.strip_suffix(" deg") {
        format!("{v}°")
    } else if let Some(v) = raw.strip_suffix(" ct") {
        format!("{v}c")
    } else if let Some(v) = raw.strip_suffix(" st") {
        v.to_string()
    } else {
        raw
    }
}

/// Clickable / draggable compact pill for discrete Enum/Int or continuous parameters (with optional modulation bar).
pub struct ParamStepPill {
    param_base: ParamWidgetBase,
    num_steps: usize,
    click_cycles: bool,
    hovered: bool,
    drag_active: bool,
    drag_moved: bool,
    start_y: f32,
    start_val: f32,
    mod_depth_drag: Option<ModDepthDrag>,
    mod_binding: Option<ModTargetBinding>,
}

impl ParamStepPill {
    pub fn new<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        num_steps: usize,
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self::build_inner(cx, params, params_to_param, num_steps, true, None, None)
    }

    pub fn new_with_mod<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        num_steps: usize,
        dest: ModDestination,
        mod_ui: Arc<ModUiState>,
        tab_tag: u8,
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self::build_inner(
            cx,
            params,
            params_to_param,
            num_steps,
            num_steps <= 16,
            Some(ModTargetBinding {
                dest,
                mod_ui,
                tab_tag,
            }),
            None,
        )
    }

    pub fn new_inline<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        num_steps: usize,
        label: &'static str,
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self::build_inner(cx, params, params_to_param, num_steps, true, None, Some(label))
    }

    fn format_tooltip_text(&self) -> Option<String> {
        if let (Some(md), Some(ref mb)) = (self.mod_depth_drag, self.mod_binding.as_ref()) {
            let base_norm = self.param_base.unmodulated_normalized_value().clamp(0.0, 1.0);
            Some(format_mod_range_tooltip(base_norm, mb, md, |n, _| {
                self.param_base.normalized_value_to_string(n, true)
            }))
        } else if self.mod_binding.is_some() && (self.hovered || self.drag_active) {
            Some(
                self.param_base
                    .normalized_value_to_string(self.param_base.modulated_normalized_value(), true),
            )
        } else {
            None
        }
    }

    fn emit_tooltip(&self, cx: &mut EventContext) {
        if let Some(text) = self.format_tooltip_text() {
            emit_widget_tooltip(cx, text);
        }
    }

    fn reset_to_default(&mut self, cx: &mut EventContext) {
        if self.drag_active || self.mod_depth_drag.is_some() {
            let was_drag = self.drag_active;
            self.drag_active = false;
            self.drag_moved = false;
            self.mod_depth_drag = None;
            cx.release();
            if was_drag {
                self.param_base.end_set_parameter(cx);
            }
        }
        let def = self.param_base.default_normalized_value();
        self.param_base.begin_set_parameter(cx);
        self.param_base.set_normalized_value(cx, def);
        self.param_base.end_set_parameter(cx);
        cx.emit(UiEvent::ModUiChanged);
        if self.hovered {
            self.emit_tooltip(cx);
        }
        cx.needs_redraw();
    }

    fn build_inner<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        num_steps: usize,
        click_cycles: bool,
        mod_binding: Option<ModTargetBinding>,
        inline_label: Option<&'static str>,
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        let class_name = if inline_label.is_some() {
            "inline-pill"
        } else {
            "compact-pill"
        };
        Self {
            param_base: ParamWidgetBase::new(cx, params.clone(), params_to_param),
            num_steps: num_steps.max(2),
            click_cycles,
            hovered: false,
            drag_active: false,
            drag_moved: false,
            start_y: 0.0,
            start_val: 0.0,
            mod_depth_drag: None,
            mod_binding,
        }
        .build(cx, move |cx| {
            if let Some(title) = inline_label {
                let text_lens = ParamWidgetBase::make_lens(params, params_to_param, |param| {
                    format_inline_value(
                        param.normalized_value_to_string(param.modulated_normalized_value(), true),
                    )
                });
                HStack::new(cx, move |cx| {
                    bold_label(cx, title, "inline-pill-label")
                        .width(Auto)
                        .height(Stretch(1.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0))
                        .hoverable(false);
                    bold_label(cx, text_lens, "inline-pill-value")
                        .width(Stretch(1.0))
                        .height(Stretch(1.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0))
                        .overflow(Overflow::Hidden)
                        .hoverable(false);
                })
                .col_between(Pixels(3.0))
                .width(Stretch(1.0))
                .height(Stretch(1.0))
                .child_left(Pixels(3.0))
                .child_right(Pixels(3.0))
                .hoverable(false);
            } else {
                ParamWidgetBase::build_view(params, params_to_param, move |cx, param_data| {
                    let text_lens = param_data.make_lens(|param| {
                        param.normalized_value_to_string(param.modulated_normalized_value(), true)
                    });
                    bold_label(cx, text_lens, "compact-pill-text")
                        .width(Stretch(1.0))
                        .height(Stretch(1.0))
                        .child_space(Stretch(1.0))
                        .hoverable(false);
                })(cx);
            }
        })
        .class(class_name)
    }
}

impl View for ParamStepPill {
    fn element(&self) -> Option<&'static str> {
        Some("param-step-pill")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|raw_param_event, _| {
            if let RawParamEvent::ParametersChanged = raw_param_event {
                if self.hovered || self.drag_active {
                    self.emit_tooltip(cx);
                }
                cx.needs_redraw();
            }
        });

        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseOver => {
                self.hovered = true;
                self.emit_tooltip(cx);
            }
            WindowEvent::MouseOut => {
                self.hovered = false;
                if !self.drag_active && self.mod_depth_drag.is_none() {
                    cx.emit(UiEvent::HideTooltip);
                }
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left) if !self.click_cycles => {
                self.reset_to_default(cx);
                meta.consume();
            }
            WindowEvent::MouseDown(MouseButton::Left)
            | WindowEvent::MouseDoubleClick(MouseButton::Left)
            | WindowEvent::MouseTripleClick(MouseButton::Left) => {
                if cx.modifiers().contains(Modifiers::ALT) {
                    if let Some(ref mb) = self.mod_binding {
                        if let Some((slot_idx, slot)) = mb.mod_ui.primary_slot_for_dest(mb.dest) {
                            self.mod_depth_drag = Some(ModDepthDrag {
                                slot_idx,
                                start_y: cx.mouse().cursory,
                                start_depth: slot.depth,
                            });
                            mb.mod_ui.focus_slot(mb.dest, slot_idx);
                            cx.capture();
                            cx.emit(UiEvent::ModUiChanged);
                            self.emit_tooltip(cx);
                            cx.needs_redraw();
                            meta.consume();
                            return;
                        }
                    }
                }
                if cx.modifiers().command() || cx.modifiers().contains(Modifiers::CTRL) {
                    self.reset_to_default(cx);
                } else {
                    self.drag_active = true;
                    self.drag_moved = false;
                    self.start_y = cx.mouse().cursory;
                    self.start_val = self.param_base.unmodulated_normalized_value();
                    cx.capture();
                    self.param_base.begin_set_parameter(cx);
                    self.emit_tooltip(cx);
                }
                meta.consume();
            }
            WindowEvent::MouseDown(MouseButton::Right)
            | WindowEvent::MouseDoubleClick(MouseButton::Right) => {
                if let Some(ref mb) = self.mod_binding {
                    if cx.modifiers().shift() {
                        if mb.mod_ui.remove_primary_slot_for_dest(mb.dest) {
                            cx.emit(UiEvent::ModUiChanged);
                            cx.needs_redraw();
                            meta.consume();
                            return;
                        }
                    } else if cx.modifiers().contains(Modifiers::ALT)
                        || cx.modifiers().command()
                        || cx.modifiers().contains(Modifiers::CTRL)
                        || !self.click_cycles
                    {
                        if mb.mod_ui.has_slots_for_dest(mb.dest) {
                            mb.mod_ui.cycle_inspected_slot_for_dest(mb.dest);
                            cx.emit(UiEvent::ModUiChanged);
                            cx.needs_redraw();
                            meta.consume();
                            return;
                        }
                    }
                }
                if !self.click_cycles {
                    self.reset_to_default(cx);
                    meta.consume();
                    return;
                }
                let step = 1.0 / (self.num_steps - 1) as f32;
                let cur = self.param_base.unmodulated_normalized_value();
                let cur_idx = (cur * (self.num_steps - 1) as f32).round() as usize;
                let prev_idx = (cur_idx + self.num_steps - 1) % self.num_steps;
                self.param_base.begin_set_parameter(cx);
                self.param_base
                    .set_normalized_value(cx, (prev_idx as f32) * step);
                self.param_base.end_set_parameter(cx);
                cx.emit(UiEvent::ModUiChanged);
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.mod_depth_drag.is_some() {
                    self.mod_depth_drag = None;
                    cx.release();
                    if self.hovered {
                        self.emit_tooltip(cx);
                    } else {
                        cx.emit(UiEvent::HideTooltip);
                    }
                    cx.needs_redraw();
                    meta.consume();
                } else if self.drag_active {
                    if self.click_cycles && !self.drag_moved {
                        // Treat as click: cycle cleanly to next discrete step
                        let step = 1.0 / (self.num_steps - 1) as f32;
                        let cur_idx =
                            (self.start_val * (self.num_steps - 1) as f32).round() as usize;
                        let next_idx = (cur_idx + 1) % self.num_steps;
                        self.param_base
                            .set_normalized_value(cx, (next_idx as f32) * step);
                    }
                    self.drag_active = false;
                    self.drag_moved = false;
                    cx.release();
                    self.param_base.end_set_parameter(cx);
                    if self.hovered {
                        self.emit_tooltip(cx);
                    } else {
                        cx.emit(UiEvent::HideTooltip);
                    }
                    cx.emit(UiEvent::ModUiChanged);
                    meta.consume();
                }
            }
            WindowEvent::MouseMove(_x, y) => {
                if handle_mod_depth_move(&mut self.mod_depth_drag, &self.mod_binding, cx, *y) {
                    self.emit_tooltip(cx);
                    meta.consume();
                    return;
                }
                if self.drag_active {
                    let threshold = if self.click_cycles { 6.0 } else { 2.0 };
                    let moved = (*y - self.start_y).abs();
                    if self.drag_moved || moved >= threshold * cx.scale_factor() {
                        self.drag_moved = true;
                        let fine = if cx.modifiers().shift() { 0.2 } else { 1.0 };
                        let drag_px = if self.click_cycles { 90.0 } else { 130.0 };
                        let dy = (self.start_y - *y) / (drag_px * cx.scale_factor());
                        let new_val = (self.start_val + dy * fine).clamp(0.0, 1.0);
                        self.param_base.set_normalized_value(cx, new_val);
                        if !self.click_cycles {
                            self.start_y = *y;
                            self.start_val = new_val;
                        }
                        self.emit_tooltip(cx);
                        cx.emit(UiEvent::ModUiChanged);
                    }
                    meta.consume();
                }
            }
            WindowEvent::MouseScroll(_sx, sy) => {
                self.param_base.begin_set_parameter(cx);
                let base_step = 1.0 / (self.num_steps - 1) as f32;
                let step = if !self.click_cycles && cx.modifiers().shift() {
                    base_step * 0.25
                } else {
                    base_step
                };
                let cur = self.param_base.unmodulated_normalized_value();
                let next = (cur + sy.signum() * step).clamp(0.0, 1.0);
                self.param_base.set_normalized_value(cx, next);
                self.param_base.end_set_parameter(cx);
                if self.hovered || self.drag_active {
                    self.emit_tooltip(cx);
                }
                cx.emit(UiEvent::ModUiChanged);
                meta.consume();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        draw_pill_with_mod_bar(cx, canvas, &self.param_base, &self.mod_binding);
    }
}

fn draw_pill_with_mod_bar(
    cx: &mut DrawContext,
    canvas: &mut Canvas,
    param_base: &ParamWidgetBase,
    mod_binding: &Option<ModTargetBinding>,
) {
    let mut path = cx.build_path();
    cx.draw_shadows(canvas, &mut path);
    cx.draw_backdrop_filter(canvas, &mut path);
    cx.draw_background(canvas, &mut path);
    cx.draw_border(canvas, &mut path);
    cx.draw_inset_box_shadows(canvas, &mut path);
    cx.draw_outline(canvas);

    let Some(mb) = mod_binding else {
        return;
    };
    let bounds = cx.bounds();
    if bounds.w <= 2.0 || bounds.h <= 2.0 {
        return;
    }
    mb.mod_ui.register_drop_rect(mb.dest, mb.tab_tag, bounds);
    let dpx = cx.scale_factor();

    let drag_src = mb.mod_ui.dragging_source();
    if drag_src != ModMatrixSource::None {
        let (sr, sg, sb) = drag_src.color_rgb();
        let is_hover = mb.mod_ui.is_drop_target(mb.dest);
        let mut hl = vg::Path::new();
        hl.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 3.0 * dpx);
        let mut p = vg::Paint::color(vg::Color::rgba(
            sr,
            sg,
            sb,
            if is_hover { 235 } else { 70 },
        ));
        p.set_line_width((if is_hover { 1.8 } else { 1.0 }) * dpx);
        canvas.stroke_path(&hl, &p);
    }

    let (slots, _indices, count) = mb.mod_ui.slots_for_dest(mb.dest);
    if count == 0 {
        return;
    }
    let base_norm = param_base.unmodulated_normalized_value().clamp(0.0, 1.0);
    let bar_x = bounds.x + 2.5 * dpx;
    let bar_w = (bounds.w - 5.0 * dpx).max(1.0);
    let bar_y = bounds.y + bounds.h - 2.6 * dpx;
    let bar_h = 2.0 * dpx;

    for slot in slots.iter().take(count) {
        let (sr, sg, sb) = slot.source.color_rgb();
        let (lo, hi) = if slot.bipolar {
            let span = slot.depth.abs();
            ((base_norm - span).clamp(0.0, 1.0), (base_norm + span).clamp(0.0, 1.0))
        } else {
            let tgt = (base_norm + slot.depth).clamp(0.0, 1.0);
            (base_norm.min(tgt), base_norm.max(tgt))
        };
        if hi - lo > 0.005 {
            let mut seg = vg::Path::new();
            seg.rounded_rect(bar_x + lo * bar_w, bar_y, (hi - lo) * bar_w, bar_h, 1.0 * dpx);
            canvas.fill_path(&seg, &vg::Paint::color(vg::Color::rgba(sr, sg, sb, 225)));
        }
    }

    let live_norm = (base_norm + mb.mod_ui.telemetry.dest_offset(mb.dest)).clamp(0.0, 1.0);
    let mut tick = vg::Path::new();
    tick.circle(bar_x + live_norm * bar_w, bar_y + bar_h * 0.5, 1.8 * dpx);
    canvas.fill_path(&tick, &vg::Paint::color(vg::Color::rgb(255, 248, 232)));
}

/// Pitch drag pill (`-48.00..=+48.00 st`) with semitone, fine (`Shift`), and octave (`Cmd+Opt` / `Ctrl+Alt`) drag/scroll steps.
pub struct PitchDragBox {
    param_base: ParamWidgetBase,
    hovered: bool,
    drag_active: bool,
    last_y: f32,
    accum_y: f32,
    last_mode: u8,
    scrolled_lines: f32,
    mod_depth_drag: Option<ModDepthDrag>,
    mod_binding: Option<ModTargetBinding>,
}

impl PitchDragBox {
    pub fn new_with_mod<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        dest: ModDestination,
        mod_ui: Arc<ModUiState>,
        tab_tag: u8,
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self {
            param_base: ParamWidgetBase::new(cx, params.clone(), params_to_param),
            hovered: false,
            drag_active: false,
            last_y: 0.0,
            accum_y: 0.0,
            last_mode: 0,
            scrolled_lines: 0.0,
            mod_depth_drag: None,
            mod_binding: Some(ModTargetBinding {
                dest,
                mod_ui,
                tab_tag,
            }),
        }
        .build(cx, move |cx| {
            ParamWidgetBase::build_view(params, params_to_param, move |cx, param_data| {
                let text_lens = param_data.make_lens(|param| {
                    param.normalized_value_to_string(param.modulated_normalized_value(), false)
                });
                bold_label(cx, text_lens, "compact-pill-text")
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_space(Stretch(1.0))
                    .hoverable(false);
            })(cx);
        })
        .class("compact-pill")
    }

    #[inline]
    fn pitch_mode(cx: &EventContext) -> (u8, f32, f32) {
        let cmd_or_ctrl = cx.modifiers().command() || cx.modifiers().contains(Modifiers::CTRL);
        let alt = cx.modifiers().contains(Modifiers::ALT);
        if cmd_or_ctrl && alt {
            (2, 12.0, 14.0)
        } else if cx.modifiers().shift() {
            (1, 0.01, 2.5)
        } else {
            (0, 1.0, 4.0)
        }
    }

    fn step_semitones(&self, cx: &mut EventContext, delta_st: f32) {
        let cur_norm = self.param_base.unmodulated_normalized_value();
        let cur_st = cur_norm * 96.0 - 48.0;
        let next_st = (((cur_st + delta_st) * 100.0).round() * 0.01).clamp(-48.0, 48.0);
        let next_norm = ((next_st + 48.0) / 96.0).clamp(0.0, 1.0);
        self.param_base.set_normalized_value(cx, next_norm);
    }

    fn format_tooltip_text(&self) -> String {
        if let (Some(md), Some(ref mb)) = (self.mod_depth_drag, self.mod_binding.as_ref()) {
            let base_norm = self.param_base.unmodulated_normalized_value().clamp(0.0, 1.0);
            format_mod_range_tooltip(base_norm, mb, md, |n, _| {
                self.param_base.normalized_value_to_string(n, true)
            })
        } else {
            self.param_base
                .normalized_value_to_string(self.param_base.modulated_normalized_value(), true)
        }
    }

    fn emit_tooltip(&self, cx: &mut EventContext) {
        emit_widget_tooltip(cx, self.format_tooltip_text());
    }

    fn reset_to_default(&mut self, cx: &mut EventContext) {
        if self.drag_active || self.mod_depth_drag.is_some() {
            let was_drag = self.drag_active;
            self.drag_active = false;
            self.mod_depth_drag = None;
            cx.release();
            if was_drag {
                self.param_base.end_set_parameter(cx);
            }
        }
        let def = self.param_base.default_normalized_value();
        self.param_base.begin_set_parameter(cx);
        self.param_base.set_normalized_value(cx, def);
        self.param_base.end_set_parameter(cx);
        if self.hovered {
            self.emit_tooltip(cx);
        }
        cx.needs_redraw();
    }
}

impl View for PitchDragBox {
    fn element(&self) -> Option<&'static str> {
        Some("pitch-drag-box")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|raw_param_event, _| {
            if let RawParamEvent::ParametersChanged = raw_param_event {
                if self.hovered || self.drag_active {
                    self.emit_tooltip(cx);
                }
                cx.needs_redraw();
            }
        });

        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseOver => {
                self.hovered = true;
                self.emit_tooltip(cx);
            }
            WindowEvent::MouseOut => {
                self.hovered = false;
                if !self.drag_active && self.mod_depth_drag.is_none() {
                    cx.emit(UiEvent::HideTooltip);
                }
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                self.reset_to_default(cx);
                meta.consume();
            }
            WindowEvent::MouseDown(MouseButton::Left)
            | WindowEvent::MouseTripleClick(MouseButton::Left) => {
                let cmd_or_ctrl =
                    cx.modifiers().command() || cx.modifiers().contains(Modifiers::CTRL);
                let alt = cx.modifiers().contains(Modifiers::ALT);
                if cmd_or_ctrl && alt {
                    self.drag_active = true;
                    self.last_y = cx.mouse().cursory;
                    self.accum_y = 0.0;
                    self.last_mode = 2;
                    cx.capture();
                    self.param_base.begin_set_parameter(cx);
                    self.emit_tooltip(cx);
                    meta.consume();
                    return;
                }
                if alt {
                    if let Some(ref mb) = self.mod_binding {
                        if let Some((slot_idx, slot)) = mb.mod_ui.primary_slot_for_dest(mb.dest) {
                            self.mod_depth_drag = Some(ModDepthDrag {
                                slot_idx,
                                start_y: cx.mouse().cursory,
                                start_depth: slot.depth,
                            });
                            mb.mod_ui.focus_slot(mb.dest, slot_idx);
                            cx.capture();
                            cx.emit(UiEvent::ModUiChanged);
                            self.emit_tooltip(cx);
                            cx.needs_redraw();
                            meta.consume();
                            return;
                        }
                    }
                }
                if cmd_or_ctrl {
                    self.reset_to_default(cx);
                } else {
                    self.drag_active = true;
                    self.last_y = cx.mouse().cursory;
                    self.accum_y = 0.0;
                    self.last_mode = Self::pitch_mode(cx).0;
                    cx.capture();
                    self.param_base.begin_set_parameter(cx);
                    self.emit_tooltip(cx);
                }
                meta.consume();
            }
            WindowEvent::MouseDown(MouseButton::Right)
            | WindowEvent::MouseDoubleClick(MouseButton::Right) => {
                if let Some(ref mb) = self.mod_binding {
                    if cx.modifiers().shift() {
                        if mb.mod_ui.remove_primary_slot_for_dest(mb.dest) {
                            cx.emit(UiEvent::ModUiChanged);
                            cx.needs_redraw();
                            meta.consume();
                            return;
                        }
                    } else if mb.mod_ui.has_slots_for_dest(mb.dest) {
                        mb.mod_ui.cycle_inspected_slot_for_dest(mb.dest);
                        cx.emit(UiEvent::ModUiChanged);
                        cx.needs_redraw();
                        meta.consume();
                        return;
                    }
                }
                self.reset_to_default(cx);
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.mod_depth_drag.is_some() {
                    self.mod_depth_drag = None;
                    cx.release();
                    if self.hovered {
                        self.emit_tooltip(cx);
                    } else {
                        cx.emit(UiEvent::HideTooltip);
                    }
                    cx.needs_redraw();
                    meta.consume();
                } else if self.drag_active {
                    self.drag_active = false;
                    cx.release();
                    self.param_base.end_set_parameter(cx);
                    if self.hovered {
                        self.emit_tooltip(cx);
                    } else {
                        cx.emit(UiEvent::HideTooltip);
                    }
                    cx.needs_redraw();
                    meta.consume();
                }
            }
            WindowEvent::MouseMove(_x, y) => {
                if handle_mod_depth_move(&mut self.mod_depth_drag, &self.mod_binding, cx, *y) {
                    self.emit_tooltip(cx);
                    meta.consume();
                    return;
                }
                if self.drag_active {
                    let (mode, step_st, base_px) = Self::pitch_mode(cx);
                    if mode != self.last_mode {
                        self.accum_y = 0.0;
                        self.last_mode = mode;
                    }
                    let px_per_step = base_px * cx.scale_factor();
                    self.accum_y += self.last_y - *y;
                    self.last_y = *y;
                    let steps = (self.accum_y / px_per_step).trunc() as i32;
                    if steps != 0 {
                        self.accum_y -= (steps as f32) * px_per_step;
                        self.step_semitones(cx, (steps as f32) * step_st);
                    }
                    self.emit_tooltip(cx);
                    cx.needs_redraw();
                    meta.consume();
                }
            }
            WindowEvent::MouseScroll(_sx, sy) => {
                self.scrolled_lines += *sy;
                if self.scrolled_lines.abs() >= 0.5 {
                    let dir = self.scrolled_lines.signum();
                    self.scrolled_lines = 0.0;
                    let (_, step_st, _) = Self::pitch_mode(cx);
                    self.param_base.begin_set_parameter(cx);
                    self.step_semitones(cx, dir * step_st);
                    self.param_base.end_set_parameter(cx);
                    if self.hovered || self.drag_active {
                        self.emit_tooltip(cx);
                    }
                    cx.needs_redraw();
                }
                meta.consume();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        draw_pill_with_mod_bar(cx, canvas, &self.param_base, &self.mod_binding);
    }
}
