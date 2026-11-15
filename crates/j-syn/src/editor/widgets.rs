//! Custom skeuomorphic `femtovg` and `ParamWidgetBase` controls (`RotaryKnob`, `LedToggleSwitch`, `ParamStepPill`).

use std::f32::consts::PI;

use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::util::ModifiersExt;
use nih_plug_vizia::widgets::RawParamEvent;

use super::controls::bold_label;

const KNOB_DRAG_PIXELS: f32 = 150.0;
const KNOB_FINE_MULTIPLIER: f32 = 0.12;

#[derive(Debug, Clone, Copy)]
struct DragState {
    start_y: f32,
    start_val: f32,
}

/// Skeuomorphic 3D rotary knob widget rendered via `femtovg` (`vizia::vg`).
pub struct RotaryKnob {
    param_base: ParamWidgetBase,
    drag_active: bool,
    drag_state: Option<DragState>,
    scrolled_lines: f32,
    bipolar: bool,
    accent_rgb: (u8, u8, u8),
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
        Self {
            param_base: ParamWidgetBase::new(cx, params.clone(), params_to_param),
            drag_active: false,
            drag_state: None,
            scrolled_lines: 0.0,
            bipolar,
            accent_rgb,
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

    fn reset_to_default(&mut self, cx: &mut EventContext) {
        if self.drag_active {
            self.drag_active = false;
            self.drag_state = None;
            cx.release();
            cx.set_active(false);
            self.param_base.end_set_parameter(cx);
        }
        let target = self.param_base.default_normalized_value();
        self.param_base.begin_set_parameter(cx);
        self.param_base.set_normalized_value(cx, target);
        self.param_base.end_set_parameter(cx);
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
                cx.needs_redraw();
            }
        });

        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left)
            | WindowEvent::MouseTripleClick(MouseButton::Left) => {
                if cx.modifiers().command() || cx.modifiers().contains(Modifiers::CTRL) {
                    self.reset_to_default(cx);
                } else {
                    self.drag_active = true;
                    cx.capture();
                    cx.focus();
                    cx.set_active(true);
                    self.param_base.begin_set_parameter(cx);
                    self.drag_state = Some(DragState {
                        start_y: cx.mouse().cursory,
                        start_val: self.param_base.unmodulated_normalized_value(),
                    });
                    cx.needs_redraw();
                }
                meta.consume();
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left)
            | WindowEvent::MouseDown(MouseButton::Right)
            | WindowEvent::MouseDoubleClick(MouseButton::Right)
            | WindowEvent::MouseTripleClick(MouseButton::Right) => {
                self.reset_to_default(cx);
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.drag_active {
                    self.drag_active = false;
                    self.drag_state = None;
                    cx.release();
                    cx.set_active(false);
                    self.param_base.end_set_parameter(cx);
                    cx.needs_redraw();
                    meta.consume();
                }
            }
            WindowEvent::MouseMove(_x, y) => {
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

                    cx.needs_redraw();
                    meta.consume();
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

        let arc_r = max_r * 0.90;
        let body_r = max_r * 0.70;
        let cap_r = max_r * 0.57;

        let norm = self.param_base.modulated_normalized_value().clamp(0.0, 1.0);

        let start_angle = 0.75 * PI;
        let sweep_angle = 1.50 * PI;
        let end_angle = start_angle + sweep_angle;
        let val_angle = start_angle + norm * sweep_angle;

        let num_ticks = 9;
        for i in 0..num_ticks {
            let t = i as f32 / (num_ticks - 1) as f32;
            let a = start_angle + t * sweep_angle;
            let r_in = max_r * 0.94;
            let r_out = max_r * 1.02;
            let mut tick = vg::Path::new();
            tick.move_to(center_x + a.cos() * r_in, center_y + a.sin() * r_in);
            tick.line_to(center_x + a.cos() * r_out, center_y + a.sin() * r_out);
            let is_major = i == 0 || i == num_ticks - 1 || i == num_ticks / 2;
            let mut paint = if is_major {
                vg::Paint::color(vg::Color::rgba(218, 198, 175, 215))
            } else {
                vg::Paint::color(vg::Color::rgba(155, 138, 120, 155))
            };
            paint.set_line_width((if is_major { 1.2 } else { 0.9 }) * dpx);
            canvas.stroke_path(&tick, &paint);
        }

        let mut track_path = vg::Path::new();
        track_path.arc(center_x, center_y, arc_r, start_angle, end_angle, vg::Solidity::Hole);
        let mut track_paint = vg::Paint::color(vg::Color::rgba(30, 25, 21, 245));
        track_paint.set_line_width(2.8 * dpx);
        track_paint.set_line_cap(vg::LineCap::Round);
        canvas.stroke_path(&track_path, &track_paint);

        let (ar, ag, ab) = self.accent_rgb;
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
            let mut glow_path = vg::Path::new();
            glow_path.arc(center_x, center_y, arc_r, arc_a0, arc_a1, vg::Solidity::Hole);
            let mut glow_paint = vg::Paint::color(vg::Color::rgba(ar, ag, ab, 80));
            glow_paint.set_line_width(5.5 * dpx);
            glow_paint.set_line_cap(vg::LineCap::Round);
            canvas.stroke_path(&glow_path, &glow_paint);

            let mut val_path = vg::Path::new();
            val_path.arc(center_x, center_y, arc_r, arc_a0, arc_a1, vg::Solidity::Hole);
            let mut val_paint = vg::Paint::color(vg::Color::rgba(ar, ag, ab, 250));
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
            vg::Color::rgba(14, 11, 8, 200),
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
            vg::Color::rgb(112, 98, 86),
            vg::Color::rgb(50, 42, 36),
        );
        canvas.fill_path(&body_path, &body_paint);

        let mut rim_stroke = vg::Paint::color(vg::Color::rgba(24, 20, 17, 235));
        rim_stroke.set_line_width(1.1 * dpx);
        canvas.stroke_path(&body_path, &rim_stroke);

        let mut cap_path = vg::Path::new();
        cap_path.circle(center_x, center_y, cap_r);
        let cap_paint = vg::Paint::radial_gradient(
            center_x - cap_r * 0.3,
            center_y - cap_r * 0.35,
            cap_r * 0.08,
            cap_r * 1.1,
            vg::Color::rgb(138, 122, 108),
            vg::Color::rgb(62, 53, 46),
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
        let mut ptr_paint = vg::Paint::color(vg::Color::rgb(255, 244, 228));
        ptr_paint.set_line_width(1.8 * dpx);
        ptr_paint.set_line_cap(vg::LineCap::Round);
        canvas.stroke_path(&ptr_path, &ptr_paint);
    }
}

/// Skeuomorphic illuminated hardware push-button / LED switch.
pub struct LedToggleSwitch {
    param_base: ParamWidgetBase,
    accent_rgb: (u8, u8, u8),
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
        Self {
            param_base: ParamWidgetBase::new(cx, params.clone(), params_to_param),
            accent_rgb,
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params.clone(), params_to_param, move |cx, param_data| {
                let is_on_lens =
                    param_data.make_lens(|param| param.modulated_normalized_value() >= 0.5);
                Binding::new(cx, is_on_lens, move |cx, is_on| {
                    cx.needs_redraw();
                    let label_text = if is_on.get(cx) { on_label } else { off_label };
                    HStack::new(cx, move |cx| {
                        Element::new(cx)
                            .width(Pixels(12.0))
                            .height(Stretch(1.0))
                            .hoverable(false);
                        bold_label(cx, label_text, "switch-label").hoverable(false);
                    })
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0))
                    .child_left(Pixels(5.0))
                    .child_right(Pixels(6.0))
                    .col_between(Pixels(3.0))
                    .hoverable(false);
                });
            }),
        )
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
            WindowEvent::MouseDown(MouseButton::Left)
            | WindowEvent::MouseDoubleClick(MouseButton::Left)
            | WindowEvent::MouseTripleClick(MouseButton::Left) => {
                if cx.modifiers().command() || cx.modifiers().contains(Modifiers::CTRL) {
                    let def = self.param_base.default_normalized_value();
                    self.param_base.begin_set_parameter(cx);
                    self.param_base.set_normalized_value(cx, def);
                    self.param_base.end_set_parameter(cx);
                    cx.needs_redraw();
                } else {
                    self.toggle(cx);
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

        let housing_paint = if is_on {
            vg::Paint::linear_gradient(
                bounds.x,
                bounds.y,
                bounds.x,
                bounds.y + bounds.h,
                vg::Color::rgb(82, 70, 60),
                vg::Color::rgb(52, 44, 38),
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
            vg::Color::rgba(ar, ag, ab, 175)
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

/// Clickable / draggable compact pill for discrete Enum or Int parameters.
pub struct ParamStepPill {
    param_base: ParamWidgetBase,
    num_steps: usize,
    drag_active: bool,
    drag_moved: bool,
    start_y: f32,
    start_val: f32,
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
        Self {
            param_base: ParamWidgetBase::new(cx, params.clone(), params_to_param),
            num_steps: num_steps.max(2),
            drag_active: false,
            drag_moved: false,
            start_y: 0.0,
            start_val: 0.0,
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, param_data| {
                let text_lens = param_data.make_lens(|param| {
                    param.normalized_value_to_string(param.modulated_normalized_value(), true)
                });
                bold_label(cx, text_lens, "compact-pill-text")
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_space(Stretch(1.0))
                    .hoverable(false);
            }),
        )
        .class("compact-pill")
    }
}

impl View for ParamStepPill {
    fn element(&self) -> Option<&'static str> {
        Some("param-step-pill")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|raw_param_event, _| {
            if let RawParamEvent::ParametersChanged = raw_param_event {
                cx.needs_redraw();
            }
        });

        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left)
            | WindowEvent::MouseDoubleClick(MouseButton::Left)
            | WindowEvent::MouseTripleClick(MouseButton::Left) => {
                if cx.modifiers().command() || cx.modifiers().contains(Modifiers::CTRL) {
                    let def = self.param_base.default_normalized_value();
                    self.param_base.begin_set_parameter(cx);
                    self.param_base.set_normalized_value(cx, def);
                    self.param_base.end_set_parameter(cx);
                } else {
                    self.drag_active = true;
                    self.drag_moved = false;
                    self.start_y = cx.mouse().cursory;
                    self.start_val = self.param_base.unmodulated_normalized_value();
                    cx.capture();
                    self.param_base.begin_set_parameter(cx);
                }
                meta.consume();
            }
            WindowEvent::MouseDown(MouseButton::Right)
            | WindowEvent::MouseDoubleClick(MouseButton::Right) => {
                let step = 1.0 / (self.num_steps - 1) as f32;
                let cur = self.param_base.unmodulated_normalized_value();
                let cur_idx = (cur * (self.num_steps - 1) as f32).round() as usize;
                let prev_idx = (cur_idx + self.num_steps - 1) % self.num_steps;
                self.param_base.begin_set_parameter(cx);
                self.param_base
                    .set_normalized_value(cx, (prev_idx as f32) * step);
                self.param_base.end_set_parameter(cx);
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.drag_active {
                    if !self.drag_moved {
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
                    meta.consume();
                }
            }
            WindowEvent::MouseMove(_x, y) => {
                if self.drag_active {
                    let moved = (*y - self.start_y).abs();
                    if self.drag_moved || moved >= 6.0 * cx.scale_factor() {
                        self.drag_moved = true;
                        let dy = (self.start_y - *y) / (90.0 * cx.scale_factor());
                        let new_val = (self.start_val + dy).clamp(0.0, 1.0);
                        self.param_base.set_normalized_value(cx, new_val);
                    }
                    meta.consume();
                }
            }
            WindowEvent::MouseScroll(_sx, sy) => {
                self.param_base.begin_set_parameter(cx);
                let step = 1.0 / (self.num_steps - 1) as f32;
                let cur = self.param_base.unmodulated_normalized_value();
                let next = (cur + sy.signum() * step).clamp(0.0, 1.0);
                self.param_base.set_normalized_value(cx, next);
                self.param_base.end_set_parameter(cx);
                meta.consume();
            }
            _ => {}
        });
    }
}
