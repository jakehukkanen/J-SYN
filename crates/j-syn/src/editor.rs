//! Warm dark-mode skeuomorphic Vizia GUI for `J-SYN` (Milestone 1).

use std::f32::consts::PI;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use j_syn_dsp::WavetableBank;
use nih_plug::prelude::{Editor, GuiContext, Param, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::util::ModifiersExt;
use nih_plug_vizia::widgets::RawParamEvent;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};

use crate::{JSynPluginParams, OscPluginParams};

const BASE_WIDTH: u32 = 1040;
const BASE_HEIGHT: u32 = 720;
const DEFAULT_SCALE: f64 = 1.1;

const KNOB_DRAG_PIXELS: f32 = 150.0;
const KNOB_FINE_MULTIPLIER: f32 = 0.12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BottomTab {
    VoiceEnv,
    PerformerMod,
    Routing,
    FxRack,
}

impl Data for BottomTab {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

pub(crate) enum UiEvent {
    SelectTab(BottomTab),
}

#[derive(Lens)]
pub(crate) struct UiData {
    pub params: Arc<JSynPluginParams>,
    pub active_tab: BottomTab,
}

impl Model for UiData {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|ui_event, meta| match ui_event {
            UiEvent::SelectTab(tab) => {
                self.active_tab = *tab;
                meta.consume();
            }
        });
    }
}

pub(crate) fn default_state() -> Arc<ViziaState> {
    ViziaState::new_with_default_scale_factor(|| (BASE_WIDTH, BASE_HEIGHT), DEFAULT_SCALE)
}

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
                    HStack::new(cx, |cx| {
                        Element::new(cx)
                            .width(Pixels(12.0))
                            .height(Stretch(1.0))
                            .hoverable(false);
                        Label::new(cx, label_text)
                            .class("switch-label")
                            .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                            .font_weight(FontWeightKeyword::Bold)
                            .hoverable(false);
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
                Label::new(cx, text_lens)
                    .class("compact-pill-text")
                    .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                    .font_weight(FontWeightKeyword::Bold)
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

/// Live wavetable waveform preview canvas per oscillator.
pub struct WavetableScope {
    osc_idx: usize,
    bank: Arc<WavetableBank>,
    params: Arc<JSynPluginParams>,
}

impl WavetableScope {
    pub fn new(
        cx: &mut Context,
        osc_idx: usize,
        bank: Arc<WavetableBank>,
        params: Arc<JSynPluginParams>,
    ) -> Handle<'_, Self> {
        Self {
            osc_idx,
            bank,
            params,
        }
        .build(cx, move |cx| {
            let scope_lens = UiData::params.map(move |p| {
                let osc = &p.oscs[osc_idx];
                (
                    osc.wt_pos.modulated_normalized_value().to_bits(),
                    osc.enabled.value(),
                    osc.wt_select.value() as u8,
                )
            });
            Binding::new(cx, scope_lens, |cx, _| {
                cx.needs_redraw();
                Element::new(cx).hoverable(false);
            });
        })
    }
}

impl View for WavetableScope {
    fn element(&self) -> Option<&'static str> {
        Some("wavetable-scope")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|raw_param_event, _| {
            if let RawParamEvent::ParametersChanged = raw_param_event {
                cx.needs_redraw();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 4.0 || bounds.h <= 4.0 {
            return;
        }

        let dpx = cx.scale_factor();
        let radius = 4.0 * dpx;

        let mut bg = vg::Path::new();
        bg.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, radius);
        canvas.fill_path(&bg, &vg::Paint::color(vg::Color::rgb(22, 19, 16)));

        // Center zero-crossing line
        let mid_y = bounds.y + bounds.h * 0.5;
        let mut zero_line = vg::Path::new();
        zero_line.move_to(bounds.x + 3.0 * dpx, mid_y);
        zero_line.line_to(bounds.x + bounds.w - 3.0 * dpx, mid_y);
        let mut zero_paint = vg::Paint::color(vg::Color::rgba(95, 82, 68, 100));
        zero_paint.set_line_width(1.0 * dpx);
        canvas.stroke_path(&zero_line, &zero_paint);

        let choice = self.params.oscs[self.osc_idx].wt_select.value();
        let custom_guard;
        let wt: &j_syn_dsp::Wavetable = match choice.factory_preset() {
            Some(preset) => self.bank.factory_table(preset),
            None => {
                custom_guard = self.bank.osc_tables[self.osc_idx].load();
                &custom_guard
            }
        };
        let wt_pos = self.params.oscs[self.osc_idx].wt_pos.value();
        let enabled = self.params.oscs[self.osc_idx].enabled.value();

        const PREVIEW_PTS: usize = 128;
        let mut pts = [0.0_f32; PREVIEW_PTS];
        wt.fill_preview(wt_pos, &mut pts);

        let pad_x = 4.0 * dpx;
        let pad_y = 4.0 * dpx;
        let inner_w = (bounds.w - 2.0 * pad_x).max(1.0);
        let half_h = ((bounds.h - 2.0 * pad_y) * 0.45).max(1.0);

        let mut wave = vg::Path::new();
        for (i, &s) in pts.iter().enumerate() {
            let x = bounds.x + pad_x + (i as f32 / (PREVIEW_PTS - 1) as f32) * inner_w;
            let y = mid_y - s.clamp(-1.05, 1.05) * half_h;
            if i == 0 {
                wave.move_to(x, y);
            } else {
                wave.line_to(x, y);
            }
        }

        let mut wave_paint = vg::Paint::color(if enabled {
            vg::Color::rgba(245, 179, 73, 240)
        } else {
            vg::Color::rgba(140, 120, 98, 140)
        });
        wave_paint.set_line_width(1.6 * dpx);
        canvas.stroke_path(&wave, &wave_paint);

        let mut border = vg::Path::new();
        border.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, radius);
        let mut border_paint = vg::Paint::color(vg::Color::rgb(68, 58, 50));
        border_paint.set_line_width(1.0 * dpx);
        canvas.stroke_path(&border, &border_paint);
    }
}

/// Non-blocking `.wav` file loader button that spawns `rfd` on a background thread
/// and swaps the parsed `Arc<Wavetable>` via `ArcSwap`.
pub struct LoadWavButton {
    osc_idx: usize,
    bank: Arc<WavetableBank>,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    dialog_open: Arc<AtomicBool>,
}

impl LoadWavButton {
    pub fn new(
        cx: &mut Context,
        osc_idx: usize,
        bank: Arc<WavetableBank>,
        params: Arc<JSynPluginParams>,
        gui_context: Arc<dyn GuiContext>,
    ) -> Handle<'_, Self> {
        Self {
            osc_idx,
            bank,
            params,
            gui_context,
            dialog_open: Arc::new(AtomicBool::new(false)),
        }
        .build(cx, |cx| {
            Label::new(cx, "LOAD .WAV")
                .class("load-wav-text")
                .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                .font_weight(FontWeightKeyword::Bold)
                .width(Stretch(1.0))
                .height(Stretch(1.0))
                .child_space(Stretch(1.0))
                .hoverable(false);
        })
        .class("load-wav-btn")
    }
}

impl View for LoadWavButton {
    fn element(&self) -> Option<&'static str> {
        Some("load-wav-button")
    }

    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                if !self.dialog_open.swap(true, Ordering::AcqRel) {
                    let osc_idx = self.osc_idx;
                    let bank = self.bank.clone();
                    let params = self.params.clone();
                    let gui_context = self.gui_context.clone();
                    let dialog_flag = self.dialog_open.clone();

                    std::thread::spawn(move || {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("WAV Audio", &["wav", "WAV"])
                            .pick_file()
                        {
                            if let Ok(wt) = j_syn_dsp::Wavetable::load_from_wav(&path) {
                                bank.swap_custom(osc_idx, Arc::new(wt));
                                if let Ok(mut guard) = params.wavetable_paths.lock() {
                                    guard[osc_idx] = path.to_string_lossy().into_owned();
                                }
                                let setter = ParamSetter::new(gui_context.as_ref());
                                setter.begin_set_parameter(&params.oscs[osc_idx].wt_select);
                                setter.set_parameter(
                                    &params.oscs[osc_idx].wt_select,
                                    crate::WavetableChoice::Custom,
                                );
                                setter.end_set_parameter(&params.oscs[osc_idx].wt_select);
                            }
                        }
                        dialog_flag.store(false, Ordering::Release);
                    });
                }
                meta.consume();
            }
        });
    }
}

fn build_knob_cell<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
    accent_rgb: (u8, u8, u8),
    knob_sz: f32,
    cell_w: f32,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        Label::new(cx, title)
            .class("knob-title")
            .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
            .font_weight(FontWeightKeyword::Bold)
            .width(Stretch(1.0))
            .height(Pixels(13.0))
            .child_space(Stretch(1.0));

        RotaryKnob::new(cx, params.clone(), params_to_param, bipolar, accent_rgb)
            .width(Pixels(knob_sz))
            .height(Pixels(knob_sz))
            .left(Stretch(1.0))
            .right(Stretch(1.0));

        HStack::new(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, param_data| {
                let readout_lens = param_data.make_lens(|param| {
                    param.normalized_value_to_string(param.modulated_normalized_value(), true)
                });
                Label::new(cx, readout_lens)
                    .class("knob-readout-text")
                    .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                    .font_weight(FontWeightKeyword::Bold)
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_space(Stretch(1.0));
            }),
        )
        .class("knob-readout-box")
        .width(Pixels(cell_w - 4.0))
        .height(Pixels(17.0))
        .left(Stretch(1.0))
        .right(Stretch(1.0));
    })
    .class("knob-cell")
    .width(Pixels(cell_w))
    .height(Auto)
    .row_between(Pixels(2.0));
}

fn build_flex_knob_cell<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
    accent_rgb: (u8, u8, u8),
    knob_sz: f32,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        Label::new(cx, title)
            .class("knob-title")
            .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
            .font_weight(FontWeightKeyword::Bold)
            .width(Stretch(1.0))
            .height(Pixels(12.0))
            .child_space(Stretch(1.0));

        RotaryKnob::new(cx, params.clone(), params_to_param, bipolar, accent_rgb)
            .width(Pixels(knob_sz))
            .height(Pixels(knob_sz))
            .left(Stretch(1.0))
            .right(Stretch(1.0));

        HStack::new(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, param_data| {
                let readout_lens = param_data.make_lens(|param| {
                    param.normalized_value_to_string(param.modulated_normalized_value(), true)
                });
                Label::new(cx, readout_lens)
                    .class("knob-readout-text")
                    .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                    .font_weight(FontWeightKeyword::Bold)
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_space(Stretch(1.0));
            }),
        )
        .class("knob-readout-box")
        .width(Stretch(1.0))
        .height(Pixels(15.0));
    })
    .class("knob-cell")
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(1.0));
}

fn build_mini_pill_cell<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    num_steps: usize,
    pill_w: f32,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        Label::new(cx, title)
            .class("mini-label")
            .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
            .font_weight(FontWeightKeyword::Bold)
            .width(Stretch(1.0))
            .height(Pixels(11.0))
            .child_space(Stretch(1.0));

        ParamStepPill::new(cx, params, params_to_param, num_steps)
            .width(Pixels(pill_w))
            .height(Pixels(19.0));
    })
    .width(Pixels(pill_w))
    .height(Auto)
    .row_between(Pixels(2.0));
}

fn build_flex_pill_cell<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    num_steps: usize,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        Label::new(cx, title)
            .class("mini-label")
            .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
            .font_weight(FontWeightKeyword::Bold)
            .width(Stretch(1.0))
            .height(Pixels(10.0))
            .child_space(Stretch(1.0));

        ParamStepPill::new(cx, params, params_to_param, num_steps)
            .width(Stretch(1.0))
            .height(Pixels(17.0));
    })
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(1.0));
}

fn build_osc_panel(
    cx: &mut Context,
    osc_idx: usize,
    bank: Arc<WavetableBank>,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    accent: (u8, u8, u8),
) {
    let osc_label = match osc_idx {
        0 => "OSC 1",
        1 => "OSC 2",
        2 => "OSC 3",
        _ => "OSC 4",
    };

    #[inline]
    fn get_osc(p: &Arc<JSynPluginParams>, idx: usize) -> &OscPluginParams {
        &p.oscs[idx]
    }

    VStack::new(cx, move |cx| {
        // Row 1: Enable toggle, WT selector, LOAD .WAV, Retrig toggle
        HStack::new(cx, |cx| {
            LedToggleSwitch::new(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).enabled,
                osc_label,
                osc_label,
                accent,
            )
            .width(Pixels(58.0))
            .height(Pixels(20.0));

            ParamStepPill::new(cx, UiData::params, move |p| &get_osc(p, osc_idx).wt_select, 5)
                .width(Stretch(1.0))
                .height(Pixels(20.0));

            LoadWavButton::new(cx, osc_idx, bank.clone(), params.clone(), gui_context.clone())
                .width(Pixels(62.0))
                .height(Pixels(20.0));

            LedToggleSwitch::new(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).retrig,
                "RETRIG",
                "RETRIG",
                accent,
            )
            .width(Pixels(58.0))
            .height(Pixels(20.0));
        })
        .class("module-header")
        .height(Pixels(24.0))
        .width(Stretch(1.0))
        .col_between(Pixels(5.0));

        // Row 2: Compact Pitch & Unison 6-column flex strip inside card bounds
        HStack::new(cx, |cx| {
            build_flex_pill_cell(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).octave,
                "OCT",
                7,
            );
            build_flex_pill_cell(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).semi,
                "SEMI",
                25,
            );
            build_flex_pill_cell(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).fine,
                "FINE",
                201,
            );
            build_flex_pill_cell(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).unison,
                "UNI",
                8,
            );
            build_flex_pill_cell(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).detune,
                "DETUNE",
                101,
            );
            build_flex_pill_cell(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).spread,
                "SPREAD",
                101,
            );
        })
        .col_between(Pixels(4.0))
        .height(Pixels(28.0))
        .width(Stretch(1.0));

        // Row 3: Live wavetable waveform preview + Warp/Mod selectors
        HStack::new(cx, |cx| {
            WavetableScope::new(cx, osc_idx, bank.clone(), params.clone())
                .width(Stretch(1.0))
                .height(Stretch(1.0));

            VStack::new(cx, |cx| {
                HStack::new(cx, |cx| {
                    Label::new(cx, "WARP")
                        .class("mini-label")
                        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                        .font_weight(FontWeightKeyword::Bold)
                        .width(Pixels(32.0))
                        .height(Stretch(1.0))
                        .child_space(Stretch(1.0));
                    ParamStepPill::new(
                        cx,
                        UiData::params,
                        move |p| &get_osc(p, osc_idx).warp_mode,
                        4,
                    )
                    .width(Stretch(1.0))
                    .height(Pixels(18.0));
                })
                .col_between(Pixels(4.0))
                .height(Pixels(18.0))
                .width(Stretch(1.0));

                HStack::new(cx, |cx| {
                    Label::new(cx, "MOD")
                        .class("mini-label")
                        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                        .font_weight(FontWeightKeyword::Bold)
                        .width(Pixels(28.0))
                        .height(Stretch(1.0))
                        .child_space(Stretch(1.0));
                    ParamStepPill::new(
                        cx,
                        UiData::params,
                        move |p| &get_osc(p, osc_idx).mod_source,
                        5,
                    )
                    .width(Stretch(1.0))
                    .height(Pixels(18.0));
                    ParamStepPill::new(
                        cx,
                        UiData::params,
                        move |p| &get_osc(p, osc_idx).mod_type,
                        2,
                    )
                    .width(Pixels(34.0))
                    .height(Pixels(18.0));
                })
                .col_between(Pixels(3.0))
                .height(Pixels(18.0))
                .width(Stretch(1.0));
            })
            .width(Pixels(134.0))
            .height(Pixels(40.0))
            .row_between(Pixels(4.0));
        })
        .col_between(Pixels(8.0))
        .height(Pixels(40.0))
        .width(Stretch(1.0));

        // Row 4: Primary 6-knob flex row cleanly fit inside card width
        HStack::new(cx, |cx| {
            build_flex_knob_cell(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).wt_pos,
                "WT POS",
                false,
                accent,
                32.0,
            );
            build_flex_knob_cell(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).phase,
                "PHASE",
                false,
                accent,
                32.0,
            );
            build_flex_knob_cell(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).warp_amount,
                "WARP",
                false,
                accent,
                32.0,
            );
            build_flex_knob_cell(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).mod_depth,
                "MOD",
                false,
                accent,
                32.0,
            );
            build_flex_knob_cell(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).pan,
                "PAN",
                true,
                accent,
                32.0,
            );
            build_flex_knob_cell(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).level,
                "LEVEL",
                false,
                accent,
                32.0,
            );
        })
        .col_between(Pixels(3.0))
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })
    .class("hardware-module")
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}

fn build_tab_button(cx: &mut Context, tab: BottomTab, title: &'static str) {
    Button::new(
        cx,
        move |cx| cx.emit(UiEvent::SelectTab(tab)),
        move |cx| {
            Label::new(cx, title)
                .class("tab-btn-label")
                .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                .font_weight(FontWeightKeyword::Bold)
                .hoverable(false)
        },
    )
    .class("tab-btn")
    .checked(UiData::active_tab.map(move |t| *t == tab))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}

fn build_placeholder_bay(cx: &mut Context, title: &'static str, subtitle: &'static str) {
    VStack::new(cx, move |cx| {
        Label::new(cx, title)
            .class("placeholder-title")
            .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
            .font_weight(FontWeightKeyword::Bold)
            .width(Stretch(1.0))
            .child_left(Stretch(1.0))
            .child_right(Stretch(1.0));
        Label::new(cx, subtitle)
            .class("placeholder-sub")
            .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
            .font_weight(FontWeightKeyword::Bold)
            .width(Stretch(1.0))
            .child_left(Stretch(1.0))
            .child_right(Stretch(1.0));
    })
    .class("placeholder-bay")
    .width(Stretch(1.0))
    .height(Stretch(1.0))
    .child_top(Stretch(1.0))
    .child_bottom(Stretch(1.0));
}

pub(crate) fn create(
    params: Arc<JSynPluginParams>,
    editor_state: Arc<ViziaState>,
    bank: Arc<WavetableBank>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Builtin, move |cx, gui_context| {
        assets::register_noto_sans_bold(cx);
        cx.set_default_font(&[assets::NOTO_SANS]);

        let _ = cx.add_stylesheet(include_style!("src/theme.css"));

        UiData {
            params: params.clone(),
            active_tab: BottomTab::VoiceEnv,
        }
        .build(cx);

        let amber = (245, 179, 73);
        let gold = (242, 203, 97);
        let coral = (235, 140, 85);

        let bank_for_ui = bank.clone();
        let params_for_ui = params.clone();
        let gui_ctx_for_ui = gui_context.clone();

        VStack::new(cx, move |cx| {
            // Header bar
            HStack::new(cx, |cx| {
                Element::new(cx).class("rack-screw");
                HStack::new(cx, |cx| {
                    Element::new(cx).class("brand-pill");
                    Label::new(cx, "J-SYN")
                        .class("header-title")
                        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                        .font_weight(FontWeightKeyword::Bold);
                    Label::new(cx, "16-VOICE WAVETABLE SYNTHESIZER")
                        .class("header-subtitle")
                        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                        .font_weight(FontWeightKeyword::Bold);
                })
                .col_between(Pixels(10.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .width(Auto);

                Element::new(cx).width(Stretch(1.0));
                Element::new(cx).class("rack-screw");
            })
            .class("header-bar")
            .width(Stretch(1.0))
            .height(Pixels(34.0))
            .child_left(Pixels(12.0))
            .child_right(Pixels(12.0))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0));

            // Main chassis: Split Layout
            VStack::new(cx, |cx| {
                VStack::new(cx, |cx| {
                    // Top Section (Always Visible): 2x2 Oscillator Grid + Right Filter/Master Strip
                    HStack::new(cx, |cx| {
                        // Left: 2x2 Oscillator Grid
                        VStack::new(cx, |cx| {
                            HStack::new(cx, |cx| {
                                build_osc_panel(
                                    cx,
                                    0,
                                    bank_for_ui.clone(),
                                    params_for_ui.clone(),
                                    gui_ctx_for_ui.clone(),
                                    amber,
                                );
                                build_osc_panel(
                                    cx,
                                    1,
                                    bank_for_ui.clone(),
                                    params_for_ui.clone(),
                                    gui_ctx_for_ui.clone(),
                                    gold,
                                );
                            })
                            .col_between(Pixels(8.0))
                            .width(Stretch(1.0))
                            .height(Stretch(1.0));

                            HStack::new(cx, |cx| {
                                build_osc_panel(
                                    cx,
                                    2,
                                    bank_for_ui.clone(),
                                    params_for_ui.clone(),
                                    gui_ctx_for_ui.clone(),
                                    coral,
                                );
                                build_osc_panel(
                                    cx,
                                    3,
                                    bank_for_ui.clone(),
                                    params_for_ui.clone(),
                                    gui_ctx_for_ui.clone(),
                                    amber,
                                );
                            })
                            .col_between(Pixels(8.0))
                            .width(Stretch(1.0))
                            .height(Stretch(1.0));
                        })
                        .row_between(Pixels(8.0))
                        .width(Stretch(3.2))
                        .height(Stretch(1.0));

                        // Right: Reserved Filter / Master Strip
                        VStack::new(cx, |cx| {
                            VStack::new(cx, |cx| {
                                HStack::new(cx, |cx| {
                                    Label::new(cx, "FILTER & ROUTING (M2)")
                                        .class("module-title")
                                        .font_family(vec![FamilyOwned::Name(String::from(
                                            assets::NOTO_SANS,
                                        ))])
                                        .font_weight(FontWeightKeyword::Bold);
                                })
                                .class("module-header")
                                .height(Pixels(22.0))
                                .width(Stretch(1.0));

                                build_placeholder_bay(
                                    cx,
                                    "FILTER 1 (SER / PAR)",
                                    "LP / BP / HP / DAFT / SCREAM",
                                );
                                build_placeholder_bay(
                                    cx,
                                    "FILTER 2",
                                    "RESERVED FOR MILESTONE 2",
                                );
                            })
                            .class("hardware-module")
                            .width(Stretch(1.0))
                            .height(Stretch(1.0));

                            VStack::new(cx, |cx| {
                                HStack::new(cx, |cx| {
                                    Label::new(cx, "MASTER OUTPUT")
                                        .class("module-title")
                                        .font_family(vec![FamilyOwned::Name(String::from(
                                            assets::NOTO_SANS,
                                        ))])
                                        .font_weight(FontWeightKeyword::Bold);
                                })
                                .class("module-header")
                                .height(Pixels(22.0))
                                .width(Stretch(1.0));

                                HStack::new(cx, |cx| {
                                    build_knob_cell(
                                        cx,
                                        UiData::params,
                                        |p| &p.master_volume_db,
                                        "MASTER",
                                        false,
                                        amber,
                                        40.0,
                                        58.0,
                                    );

                                    VStack::new(cx, |cx| {
                                        LedToggleSwitch::new(
                                            cx,
                                            UiData::params,
                                            |p| &p.hq_mode,
                                            "2007 MODE",
                                            "HQ MODE",
                                            amber,
                                        )
                                        .width(Stretch(1.0))
                                        .height(Pixels(22.0));

                                        LedToggleSwitch::new(
                                            cx,
                                            UiData::params,
                                            |p| &p.mono_legato,
                                            "POLY 16",
                                            "MONO LEG",
                                            gold,
                                        )
                                        .width(Stretch(1.0))
                                        .height(Pixels(22.0));
                                    })
                                    .row_between(Pixels(6.0))
                                    .top(Stretch(1.0))
                                    .bottom(Stretch(1.0))
                                    .width(Stretch(1.0))
                                    .height(Auto);
                                })
                                .col_between(Pixels(8.0))
                                .child_top(Stretch(1.0))
                                .child_bottom(Stretch(1.0))
                                .width(Stretch(1.0))
                                .height(Stretch(1.0));
                            })
                            .class("hardware-module")
                            .width(Stretch(1.0))
                            .height(Pixels(126.0));
                        })
                        .row_between(Pixels(8.0))
                        .width(Stretch(1.0))
                        .height(Stretch(1.0));
                    })
                    .col_between(Pixels(8.0))
                    .width(Stretch(1.0))
                    .height(Stretch(1.75));

                    // Bottom Section: Tab Bar + Swappable Content Pane
                    VStack::new(cx, |cx| {
                        HStack::new(cx, |cx| {
                            build_tab_button(cx, BottomTab::VoiceEnv, "VOICE & ENV");
                            build_tab_button(cx, BottomTab::PerformerMod, "PERFORMER / MOD");
                            build_tab_button(cx, BottomTab::Routing, "ROUTING");
                            build_tab_button(cx, BottomTab::FxRack, "FX RACK");
                        })
                        .class("tab-bar")
                        .col_between(Pixels(6.0))
                        .height(Pixels(28.0))
                        .width(Stretch(1.0));

                        Binding::new(cx, UiData::active_tab, move |cx, active| {
                            match active.get(cx) {
                                BottomTab::VoiceEnv => {
                                    HStack::new(cx, |cx| {
                                        // Master & Voice
                                        VStack::new(cx, |cx| {
                                            HStack::new(cx, |cx| {
                                                Label::new(cx, "MASTER & VOICE")
                                                    .class("module-title")
                                                    .font_family(vec![FamilyOwned::Name(
                                                        String::from(assets::NOTO_SANS),
                                                    )])
                                                    .font_weight(FontWeightKeyword::Bold);
                                            })
                                            .class("module-header")
                                            .height(Pixels(22.0))
                                            .width(Stretch(1.0));

                                            HStack::new(cx, |cx| {
                                                build_knob_cell(
                                                    cx,
                                                    UiData::params,
                                                    |p| &p.master_volume_db,
                                                    "MASTER",
                                                    false,
                                                    amber,
                                                    44.0,
                                                    64.0,
                                                );

                                                VStack::new(cx, |cx| {
                                                    LedToggleSwitch::new(
                                                        cx,
                                                        UiData::params,
                                                        |p| &p.hq_mode,
                                                        "2007 MODE",
                                                        "HQ MODE",
                                                        amber,
                                                    )
                                                    .width(Pixels(102.0))
                                                    .height(Pixels(24.0));

                                                    LedToggleSwitch::new(
                                                        cx,
                                                        UiData::params,
                                                        |p| &p.mono_legato,
                                                        "POLY 16",
                                                        "MONO LEG",
                                                        gold,
                                                    )
                                                    .width(Pixels(102.0))
                                                    .height(Pixels(24.0));
                                                })
                                                .row_between(Pixels(6.0))
                                                .top(Stretch(1.0))
                                                .bottom(Stretch(1.0))
                                                .width(Auto)
                                                .height(Auto);

                                                build_knob_cell(
                                                    cx,
                                                    UiData::params,
                                                    |p| &p.glide_ms,
                                                    "GLIDE",
                                                    false,
                                                    gold,
                                                    44.0,
                                                    64.0,
                                                );

                                                build_mini_pill_cell(
                                                    cx,
                                                    UiData::params,
                                                    |p| &p.pitch_bend_range,
                                                    "PB RANGE",
                                                    24,
                                                    58.0,
                                                );
                                            })
                                            .col_between(Pixels(14.0))
                                            .child_left(Stretch(1.0))
                                            .child_right(Stretch(1.0))
                                            .child_top(Stretch(1.0))
                                            .child_bottom(Stretch(1.0))
                                            .width(Stretch(1.0))
                                            .height(Stretch(1.0));
                                        })
                                        .class("hardware-module")
                                        .width(Stretch(1.1))
                                        .height(Stretch(1.0));

                                        // Amp Envelope
                                        VStack::new(cx, |cx| {
                                            HStack::new(cx, |cx| {
                                                Label::new(cx, "AMP ENVELOPE (ADSR)")
                                                    .class("module-title")
                                                    .font_family(vec![FamilyOwned::Name(
                                                        String::from(assets::NOTO_SANS),
                                                    )])
                                                    .font_weight(FontWeightKeyword::Bold);
                                            })
                                            .class("module-header")
                                            .height(Pixels(22.0))
                                            .width(Stretch(1.0));

                                            HStack::new(cx, |cx| {
                                                build_knob_cell(
                                                    cx,
                                                    UiData::params,
                                                    |p| &p.amp_attack,
                                                    "ATTACK",
                                                    false,
                                                    amber,
                                                    44.0,
                                                    68.0,
                                                );
                                                build_knob_cell(
                                                    cx,
                                                    UiData::params,
                                                    |p| &p.amp_decay,
                                                    "DECAY",
                                                    false,
                                                    amber,
                                                    44.0,
                                                    68.0,
                                                );
                                                build_knob_cell(
                                                    cx,
                                                    UiData::params,
                                                    |p| &p.amp_sustain,
                                                    "SUSTAIN",
                                                    false,
                                                    gold,
                                                    44.0,
                                                    68.0,
                                                );
                                                build_knob_cell(
                                                    cx,
                                                    UiData::params,
                                                    |p| &p.amp_release,
                                                    "RELEASE",
                                                    false,
                                                    coral,
                                                    44.0,
                                                    68.0,
                                                );
                                            })
                                            .col_between(Pixels(14.0))
                                            .child_left(Stretch(1.0))
                                            .child_right(Stretch(1.0))
                                            .child_top(Stretch(1.0))
                                            .child_bottom(Stretch(1.0))
                                            .width(Stretch(1.0))
                                            .height(Stretch(1.0));
                                        })
                                        .class("hardware-module")
                                        .width(Stretch(1.0))
                                        .height(Stretch(1.0));
                                    })
                                    .col_between(Pixels(8.0))
                                    .width(Stretch(1.0))
                                    .height(Stretch(1.0));
                                }
                                BottomTab::PerformerMod => {
                                    VStack::new(cx, |cx| {
                                        HStack::new(cx, |cx| {
                                            Label::new(
                                                cx,
                                                "PERFORMER / MSEG & LFO MODULATION (MILESTONE 3)",
                                            )
                                            .class("module-title")
                                            .font_family(vec![FamilyOwned::Name(String::from(
                                                assets::NOTO_SANS,
                                            ))])
                                            .font_weight(FontWeightKeyword::Bold);
                                        })
                                        .class("module-header")
                                        .height(Pixels(22.0))
                                        .width(Stretch(1.0));

                                        HStack::new(cx, |cx| {
                                            build_placeholder_bay(
                                                cx,
                                                "16-STEP PERFORMER & MSEG CURVES",
                                                "BIPOLAR STEP SEQUENCER / GLIDE / CURVE SHAPING (M3)",
                                            );
                                            build_placeholder_bay(
                                                cx,
                                                "MODULATION MATRIX & RADIAL RINGS",
                                                "ENV 1-4 / LFO 1-2 / MACRO ASSIGNMENTS (M3)",
                                            );
                                        })
                                        .col_between(Pixels(8.0))
                                        .width(Stretch(1.0))
                                        .height(Stretch(1.0));
                                    })
                                    .class("hardware-module")
                                    .width(Stretch(1.0))
                                    .height(Stretch(1.0));
                                }
                                BottomTab::Routing => {
                                    VStack::new(cx, |cx| {
                                        HStack::new(cx, |cx| {
                                            Label::new(
                                                cx,
                                                "SIGNAL FLOW & FILTER ROUTING MATRIX (MILESTONES 2-3)",
                                            )
                                            .class("module-title")
                                            .font_family(vec![FamilyOwned::Name(String::from(
                                                assets::NOTO_SANS,
                                            ))])
                                            .font_weight(FontWeightKeyword::Bold);
                                        })
                                        .class("module-header")
                                        .height(Pixels(22.0))
                                        .width(Stretch(1.0));

                                        HStack::new(cx, |cx| {
                                            build_placeholder_bay(
                                                cx,
                                                "OSC 1-4 -> FILTER 1 / FILTER 2",
                                                "SERIAL / PARALLEL FILTER BALANCE & FEEDBACK LOOP (M2)",
                                            );
                                            build_placeholder_bay(
                                                cx,
                                                "INSERT FX -> EQ -> MASTER BUS",
                                                "PRE / POST FILTER INSERT ROUTING & VOICING (M2-3)",
                                            );
                                        })
                                        .col_between(Pixels(8.0))
                                        .width(Stretch(1.0))
                                        .height(Stretch(1.0));
                                    })
                                    .class("hardware-module")
                                    .width(Stretch(1.0))
                                    .height(Stretch(1.0));
                                }
                                BottomTab::FxRack => {
                                    VStack::new(cx, |cx| {
                                        HStack::new(cx, |cx| {
                                            Label::new(cx, "MASTER FX RACK (MILESTONE 4)")
                                                .class("module-title")
                                                .font_family(vec![FamilyOwned::Name(
                                                    String::from(assets::NOTO_SANS),
                                                )])
                                                .font_weight(FontWeightKeyword::Bold);
                                        })
                                        .class("module-header")
                                        .height(Pixels(22.0))
                                        .width(Stretch(1.0));

                                        HStack::new(cx, |cx| {
                                            build_placeholder_bay(
                                                cx,
                                                "SLOT 1: TUBE DRIVE",
                                                "SATURATION / WAVESHAPER (M4)",
                                            );
                                            build_placeholder_bay(
                                                cx,
                                                "SLOT 2: DIMENSION CHORUS",
                                                "STEREO ENSEMBLE / PHASER (M4)",
                                            );
                                            build_placeholder_bay(
                                                cx,
                                                "SLOT 3: SYNC DELAY",
                                                "PING-PONG TAPE DELAY (M4)",
                                            );
                                            build_placeholder_bay(
                                                cx,
                                                "SLOT 4: HALL REVERB",
                                                "DIFFUSION / DAMPING (M4)",
                                            );
                                        })
                                        .col_between(Pixels(8.0))
                                        .width(Stretch(1.0))
                                        .height(Stretch(1.0));
                                    })
                                    .class("hardware-module")
                                    .width(Stretch(1.0))
                                    .height(Stretch(1.0));
                                }
                            }
                        });
                    })
                    .row_between(Pixels(6.0))
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
                })
                .class("section-panel")
                .row_between(Pixels(8.0))
                .width(Stretch(1.0))
                .height(Stretch(1.0));
            })
            .class("main-chassis")
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        })
        .class("plugin-root")
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })
}
