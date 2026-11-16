use std::f32::consts::PI;
use std::sync::Arc;

use j_syn_dsp::mod_matrix::{ModDestination, ModMatrixSource};
use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::util::ModifiersExt;
use nih_plug_vizia::widgets::RawParamEvent;

use crate::editor::mod_bar::ModUiState;
use crate::editor::UiEvent;
use super::{
    emit_widget_tooltip, format_mod_range_tooltip, handle_mod_depth_move,
    update_mod_collar_cursor, DragState, EnabledPredicate, ModDepthDrag, ModTargetBinding,
    ValueFormatter, KNOB_DRAG_PIXELS, KNOB_FINE_MULTIPLIER,
};


/// Skeuomorphic 3D rotary knob widget rendered via `femtovg` (`vizia::vg`).
pub struct RotaryKnob {
    param_base: ParamWidgetBase,
    drag_active: bool,
    drag_state: Option<DragState>,
    mod_depth_drag: Option<ModDepthDrag>,
    collar_cursor_active: bool,
    scrolled_lines: f32,
    hovered: bool,
    bipolar: bool,
    #[allow(dead_code)]
    accent_rgb: (u8, u8, u8),
    mod_binding: Option<ModTargetBinding>,
    value_formatter: Option<ValueFormatter>,
    enabled_state: Option<(Arc<crate::JSynPluginParams>, EnabledPredicate)>,
    snap_steps: Option<u32>,
    default_override: Option<f32>,
}

pub trait KnobDimExt {
    fn enabled_by<F>(self, params: Arc<crate::JSynPluginParams>, check: F) -> Self
    where
        F: Fn(&crate::JSynPluginParams) -> bool + Send + Sync + 'static;

    fn snap_steps(self, steps: u32) -> Self;

    fn default_norm(self, val: Option<f32>) -> Self;
}

impl KnobDimExt for Handle<'_, RotaryKnob> {
    fn enabled_by<F>(self, params: Arc<crate::JSynPluginParams>, check: F) -> Self
    where
        F: Fn(&crate::JSynPluginParams) -> bool + Send + Sync + 'static,
    {
        let pred: EnabledPredicate = Arc::new(check);
        self.modify(move |knob| knob.enabled_state = Some((params, pred)))
    }

    fn snap_steps(self, steps: u32) -> Self {
        self.modify(move |knob| knob.snap_steps = (steps > 0).then_some(steps))
    }

    fn default_norm(self, val: Option<f32>) -> Self {
        self.modify(move |knob| knob.default_override = val.map(|v| v.clamp(0.0, 1.0)))
    }
}

impl RotaryKnob {
    pub fn new<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        bipolar: bool,
        #[allow(dead_code)]
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
        #[allow(dead_code)]
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
        #[allow(dead_code)]
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
        #[allow(dead_code)]
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
            collar_cursor_active: false,
            scrolled_lines: 0.0,
            hovered: false,
            bipolar,
            accent_rgb,
            mod_binding,
            value_formatter,
            enabled_state: None,
            snap_steps: None,
            default_override: None,
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
        (x - center_x).hypot(y - center_y) > (arc_r + 0.4 * dpx)
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
            let on_collar =
                self.hovered && self.is_on_mod_collar(cx, cx.mouse().cursorx, cx.mouse().cursory);
            update_mod_collar_cursor(cx, on_collar, &mut self.collar_cursor_active);
        }
        let target = self
            .default_override
            .unwrap_or_else(|| self.param_base.default_normalized_value())
            .clamp(0.0, 1.0);
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
                let on_collar = !self.drag_active
                    && (self.mod_depth_drag.is_some()
                        || self.is_on_mod_collar(cx, cx.mouse().cursorx, cx.mouse().cursory));
                update_mod_collar_cursor(cx, on_collar, &mut self.collar_cursor_active);
                self.emit_tooltip(cx);
                cx.needs_redraw();
            }
            WindowEvent::MouseOut => {
                self.hovered = false;
                if !self.drag_active && self.mod_depth_drag.is_none() {
                    update_mod_collar_cursor(cx, false, &mut self.collar_cursor_active);
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
                            update_mod_collar_cursor(cx, true, &mut self.collar_cursor_active);
                            cx.emit(UiEvent::ModUiChanged);
                            self.emit_tooltip(cx);
                            cx.needs_redraw();
                            meta.consume();
                            return;
                        }
                    }
                }
                self.drag_active = true;
                update_mod_collar_cursor(cx, false, &mut self.collar_cursor_active);
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
                        let bounds = cx.bounds();
                        let scale = cx.scale_factor().max(0.01);
                        cx.emit(UiEvent::ShowModContextMenu(
                            mb.dest,
                            bounds.x / scale,
                            (bounds.y + bounds.h + 4.0) / scale,
                        ));
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
                    let mx = cx.mouse().cursorx;
                    let my = cx.mouse().cursory;
                    let bounds = cx.bounds();
                    let inside = (bounds.x..=(bounds.x + bounds.w)).contains(&mx)
                        && (bounds.y..=(bounds.y + bounds.h)).contains(&my);
                    let on_collar = self.hovered && inside && self.is_on_mod_collar(cx, mx, my);
                    update_mod_collar_cursor(cx, on_collar, &mut self.collar_cursor_active);
                    if self.hovered {
                        self.emit_tooltip(cx);
                    } else {
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
                        let mx = cx.mouse().cursorx;
                        let my = cx.mouse().cursory;
                        let on_collar = self.is_on_mod_collar(cx, mx, my);
                        update_mod_collar_cursor(cx, on_collar, &mut self.collar_cursor_active);
                        self.emit_tooltip(cx);
                    } else {
                        update_mod_collar_cursor(cx, false, &mut self.collar_cursor_active);
                        cx.emit(UiEvent::HideTooltip);
                    }
                    cx.needs_redraw();
                    meta.consume();
                }
            }
            WindowEvent::MouseMove(x, y) => {
                if handle_mod_depth_move(&mut self.mod_depth_drag, &self.mod_binding, cx, *y) {
                    update_mod_collar_cursor(cx, true, &mut self.collar_cursor_active);
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
                    let raw_val = (state.start_val + dy * sensitivity).clamp(0.0, 1.0);
                    let out_val = match self.snap_steps {
                        Some(steps) if steps > 0 && !cx.modifiers().shift() => {
                            (raw_val * steps as f32).round() / steps as f32
                        }
                        _ => raw_val,
                    };
                    self.param_base.set_normalized_value(cx, out_val);

                    state.start_y = *y;
                    state.start_val = raw_val;

                    self.emit_tooltip(cx);
                    cx.needs_redraw();
                    meta.consume();
                } else if self.hovered {
                    let on_collar = self.is_on_mod_collar(cx, *x, *y);
                    update_mod_collar_cursor(cx, on_collar, &mut self.collar_cursor_active);
                }
            }
            WindowEvent::MouseScroll(_scroll_x, scroll_y) => {
                self.scrolled_lines += scroll_y;
                if self.scrolled_lines.abs() >= 0.5 {
                    let finer = cx.modifiers().shift();
                    if !self.drag_active {
                        self.param_base.begin_set_parameter(cx);
                    }

                    let current = self.param_base.unmodulated_normalized_value();
                    let next = match self.snap_steps {
                        Some(steps) if steps > 0 && !finer => {
                            let cur_step = (current * steps as f32).round();
                            let dir = self.scrolled_lines.signum();
                            ((cur_step + dir) / steps as f32).clamp(0.0, 1.0)
                        }
                        _ => {
                            let step = if finer { 0.004 } else { 0.02 };
                            (current + self.scrolled_lines * step).clamp(0.0, 1.0)
                        }
                    };
                    self.param_base.set_normalized_value(cx, next);
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

        let pal = crate::editor::theme::current_palette(cx);
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
                    vg::Paint::color(crate::editor::theme::vg_color(&pal.text_muted, 200))
                } else {
                    vg::Paint::color(crate::editor::theme::vg_color(&pal.text_dim, 145))
                }
            } else if is_major {
                vg::Paint::color(crate::editor::theme::vg_color(&pal.text_dim, 85))
            } else {
                vg::Paint::color(crate::editor::theme::vg_color(&pal.text_dim, 55))
            };
            paint.set_line_width((if is_major { 1.1 } else { 0.8 }) * dpx);
            canvas.stroke_path(&tick, &paint);
        }

        let mut track_path = vg::Path::new();
        track_path.arc(center_x, center_y, arc_r, start_angle, end_angle, vg::Solidity::Hole);
        let mut track_paint = vg::Paint::color(crate::editor::theme::vg_color(&pal.knob_track, 245));
        track_paint.set_line_width(2.8 * dpx);
        track_paint.set_line_cap(vg::LineCap::Round);
        canvas.stroke_path(&track_path, &track_paint);

        let (ar, ag, ab) = if is_enabled {
            pal.accent_rgb()
        } else {
            crate::editor::theme::hex_to_rgb(&pal.text_dim)
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
                crate::editor::theme::vg_lerp(&pal.border_light, &pal.text_muted, 0.25, 255)
            } else {
                crate::editor::theme::vg_lerp(&pal.panel_header_bg, &pal.border_light, 0.35, 255)
            },
            if is_enabled {
                crate::editor::theme::vg_color(&pal.knob_body, 255)
            } else {
                crate::editor::theme::vg_lerp(&pal.knob_body, &pal.panel_bg, 0.25, 255)
            },
        );
        canvas.fill_path(&body_path, &body_paint);

        let mut rim_stroke = vg::Paint::color(if self.hovered || self.drag_active {
            vg::Color::rgba(ar, ag, ab, if is_enabled { 210 } else { 145 })
        } else {
            crate::editor::theme::vg_color(&pal.border_dark, 235)
        });
        rim_stroke.set_line_width(1.1 * dpx);
        canvas.stroke_path(&body_path, &rim_stroke);

        let (cap_hi, cap_lo) = if is_enabled {
            if self.hovered || self.drag_active {
                (
                    crate::editor::theme::vg_lerp(&pal.border_light, &pal.text_bright, 0.45, 255),
                    crate::editor::theme::vg_lerp(&pal.knob_body, &pal.panel_header_bg, 0.55, 255),
                )
            } else {
                (
                    crate::editor::theme::vg_lerp(&pal.border_light, &pal.text_muted, 0.40, 255),
                    crate::editor::theme::vg_lerp(&pal.knob_body, &pal.panel_header_bg, 0.35, 255),
                )
            }
        } else if self.hovered || self.drag_active {
            (
                crate::editor::theme::vg_lerp(&pal.panel_header_bg, &pal.text_muted, 0.25, 255),
                crate::editor::theme::vg_lerp(&pal.knob_body, &pal.panel_header_bg, 0.20, 255),
            )
        } else {
            (
                crate::editor::theme::vg_lerp(&pal.panel_header_bg, &pal.text_muted, 0.15, 255),
                crate::editor::theme::vg_color(&pal.knob_body, 255),
            )
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
                    let arc_active = is_enabled && !slot.bypassed;
                    let (sr, sg, sb) = if arc_active {
                        crate::editor::theme::mod_source_rgb(&pal, slot.source)
                    } else {
                        crate::editor::theme::hex_to_rgb(&pal.text_dim)
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
                        if arc_active {
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
                            if arc_active {
                                248
                            } else if slot.bypassed {
                                75
                            } else {
                                155
                            },
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
                    let mut tick_paint = vg::Paint::color(if arc_active {
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

