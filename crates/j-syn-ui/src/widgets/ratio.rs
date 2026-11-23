//! Split-zone `RatioDragBox` (`0.000..=32.000 ×`): left half steps whole/sub-harmonic
//! ratios (`0, 0.25, 0.5, 1..=32`), right half adjusts fine fractional ratios (`0.10`, or `0.01`/`0.001`).

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::util::ModifiersExt;
use nih_plug_vizia::widgets::RawParamEvent;

use crate::controls::bold_label;
use crate::shell::{ModUiHost, UiKitEvent};
use crate::theme;

use super::pill::draw_pill_with_mod_bar;
use super::{
    emit_widget_tooltip, format_mod_range_tooltip, handle_mod_depth_move,
    update_mod_collar_cursor, ModDepthDrag, ModTargetBinding,
};

const RATIO_MAX: f32 = 32.0;

const RATIO_COARSE_STEPS: [f32; 35] = [
    0.0, 0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0,
    15.0, 16.0, 17.0, 18.0, 19.0, 20.0, 21.0, 22.0, 23.0, 24.0, 25.0, 26.0, 27.0, 28.0, 29.0,
    30.0, 31.0, 32.0,
];

/// Split-zone FM Ratio pill (`1.000 ×`): dragging the left half steps whole harmonic ladder values;
/// dragging the right half steps fractional `0.10` (`Shift`: `0.01`, `Shift+Alt`: `0.001`).
pub struct RatioDragBox {
    param_base: ParamWidgetBase,
    osc_idx: usize,
    hovered: bool,
    drag_active: bool,
    left_half: bool,
    last_y: f32,
    accum_y: f32,
    last_fine_mode: u8,
    scrolled_lines: f32,
    mod_depth_drag: Option<ModDepthDrag>,
    collar_cursor_active: bool,
    mod_binding: Option<ModTargetBinding>,
}

impl RatioDragBox {
    pub fn new_osc_ratio<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        osc_idx: usize,
        params: L,
        params_to_param: FMap,
        dest: ModDestination,
        mod_ui: Arc<dyn ModUiHost>,
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
            osc_idx,
            hovered: false,
            drag_active: false,
            left_half: true,
            last_y: 0.0,
            accum_y: 0.0,
            last_fine_mode: 0,
            scrolled_lines: 0.0,
            mod_depth_drag: None,
            collar_cursor_active: false,
            mod_binding: Some(ModTargetBinding {
                dest,
                mod_ui,
                tab_tag,
            }),
        }
        .build(cx, move |cx| {
            let text_lens = ParamWidgetBase::make_lens(params, params_to_param, |param| {
                let raw = param.normalized_value_to_string(param.modulated_normalized_value(), true);
                raw.replace(" ×", "×")
            });
            HStack::new(cx, move |cx| {
                bold_label(cx, "RATIO", "inline-pill-label")
                    .width(Auto)
                    .height(Auto)
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0))
                    .hoverable(false);
                bold_label(cx, text_lens, "inline-pill-value")
                    .width(Stretch(1.0))
                    .height(Auto)
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0))
                    .hoverable(false);
            })
            .col_between(Pixels(3.0))
            .width(Stretch(1.0))
            .height(Stretch(1.0))
            .child_left(Pixels(4.0))
            .child_right(Pixels(4.0))
            .hoverable(false);
        })
        .class("inline-pill")
        .class("stk-pill")
    }

    #[inline]
    fn frac_mode(cx: &EventContext) -> (u8, f32, f32) {
        if cx.modifiers().shift() && cx.modifiers().contains(Modifiers::ALT) {
            (2, 0.001, 3.0)
        } else if cx.modifiers().shift() {
            (1, 0.01, 4.0)
        } else {
            (0, 0.10, 8.0)
        }
    }

    #[inline]
    fn cursor_in_left_half(cx: &EventContext) -> bool {
        let b = cx.bounds();
        cx.mouse().cursorx < b.x + b.w * 0.5
    }

    fn step_ratio(&self, cx: &mut EventContext, left_half: bool, steps: i32, fine_step: f32) {
        if steps == 0 {
            return;
        }
        let cur = (self.param_base.unmodulated_normalized_value() * RATIO_MAX).clamp(0.0, RATIO_MAX);
        let next = if left_half {
            if steps > 0 {
                let base = RATIO_COARSE_STEPS
                    .iter()
                    .rposition(|&v| v <= cur + 1e-4)
                    .unwrap_or(0);
                RATIO_COARSE_STEPS[(base + steps as usize).min(RATIO_COARSE_STEPS.len() - 1)]
            } else {
                let rev = (-steps) as usize;
                let base = RATIO_COARSE_STEPS
                    .iter()
                    .position(|&v| v >= cur - 1e-4)
                    .unwrap_or(RATIO_COARSE_STEPS.len() - 1);
                RATIO_COARSE_STEPS[base.saturating_sub(rev)]
            }
        } else {
            let delta = (steps as f32) * fine_step;
            (((cur + delta) * 1000.0).round() * 0.001).clamp(0.0, RATIO_MAX)
        };
        self.param_base
            .set_normalized_value(cx, (next / RATIO_MAX).clamp(0.0, 1.0));
    }

    fn format_tooltip_text(&self) -> String {
        if let (Some(md), Some(mb)) = (self.mod_depth_drag, self.mod_binding.as_ref()) {
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
            update_mod_collar_cursor(cx, false, &mut self.collar_cursor_active);
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

impl View for RatioDragBox {
    fn element(&self) -> Option<&'static str> {
        Some("ratio-drag-box")
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
                cx.needs_redraw();
            }
            WindowEvent::MouseOut => {
                self.hovered = false;
                if !self.drag_active && self.mod_depth_drag.is_none() {
                    cx.emit(UiKitEvent::HideTooltip);
                }
                cx.needs_redraw();
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                self.reset_to_default(cx);
                meta.consume();
            }
            WindowEvent::MouseDown(MouseButton::Left)
            | WindowEvent::MouseTripleClick(MouseButton::Left) => {
                if cx.modifiers().contains(Modifiers::ALT) && !cx.modifiers().shift() {
                    if let Some(ref mb) = self.mod_binding {
                        if let Some((slot_idx, slot)) = mb.mod_ui.primary_slot_for_dest(mb.dest) {
                            self.mod_depth_drag = Some(ModDepthDrag {
                                slot_idx,
                                start_y: cx.mouse().cursory,
                                start_depth: slot.depth,
                            });
                            mb.mod_ui.focus_slot(mb.dest, slot_idx);
                            cx.capture();
                            update_mod_collar_cursor(cx, true, &mut self.collar_cursor_active);
                            cx.emit(UiKitEvent::ModUiChanged);
                            self.emit_tooltip(cx);
                            cx.needs_redraw();
                            meta.consume();
                            return;
                        }
                    }
                }
                let cmd_or_ctrl =
                    cx.modifiers().command() || cx.modifiers().contains(Modifiers::CTRL);
                if cmd_or_ctrl {
                    self.reset_to_default(cx);
                } else {
                    self.drag_active = true;
                    self.left_half = Self::cursor_in_left_half(cx);
                    self.last_y = cx.mouse().cursory;
                    self.accum_y = 0.0;
                    self.last_fine_mode = Self::frac_mode(cx).0;
                    cx.capture();
                    self.param_base.begin_set_parameter(cx);
                    self.emit_tooltip(cx);
                    cx.needs_redraw();
                }
                meta.consume();
            }
            WindowEvent::MouseDown(MouseButton::Right)
            | WindowEvent::MouseDoubleClick(MouseButton::Right) => {
                if let Some(ref mb) = self.mod_binding {
                    if cx.modifiers().shift() {
                        if mb.mod_ui.remove_primary_slot_for_dest(mb.dest) {
                            cx.emit(UiKitEvent::ModUiChanged);
                            cx.needs_redraw();
                            meta.consume();
                            return;
                        }
                    }
                    let bounds = cx.bounds();
                    let scale = cx.scale_factor().max(0.01);
                    cx.emit(UiKitEvent::ShowPitchContextMenu(
                        self.osc_idx,
                        mb.dest,
                        bounds.x / scale,
                        (bounds.y + bounds.h + 4.0) / scale,
                    ));
                    cx.needs_redraw();
                    meta.consume();
                    return;
                }
                self.reset_to_default(cx);
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.mod_depth_drag.is_some() {
                    self.mod_depth_drag = None;
                    cx.release();
                    update_mod_collar_cursor(cx, false, &mut self.collar_cursor_active);
                    if self.hovered {
                        self.emit_tooltip(cx);
                    } else {
                        cx.emit(UiKitEvent::HideTooltip);
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
                        cx.emit(UiKitEvent::HideTooltip);
                    }
                    cx.needs_redraw();
                    meta.consume();
                }
            }
            WindowEvent::MouseMove(_x, y) => {
                if handle_mod_depth_move(&mut self.mod_depth_drag, &self.mod_binding, cx, *y) {
                    update_mod_collar_cursor(cx, true, &mut self.collar_cursor_active);
                    self.emit_tooltip(cx);
                    meta.consume();
                    return;
                }
                if self.drag_active {
                    let (mode, fine_step, base_px) = if self.left_half {
                        (0, 1.0, 8.0)
                    } else {
                        Self::frac_mode(cx)
                    };
                    if mode != self.last_fine_mode {
                        self.accum_y = 0.0;
                        self.last_fine_mode = mode;
                    }
                    let px_per_step = base_px * cx.scale_factor();
                    self.accum_y += self.last_y - *y;
                    self.last_y = *y;
                    let steps = (self.accum_y / px_per_step).trunc() as i32;
                    if steps != 0 {
                        self.accum_y -= (steps as f32) * px_per_step;
                        self.step_ratio(cx, self.left_half, steps, fine_step);
                    }
                    self.emit_tooltip(cx);
                    cx.needs_redraw();
                    meta.consume();
                }
            }
            WindowEvent::MouseScroll(_sx, sy) => {
                self.scrolled_lines += *sy;
                if self.scrolled_lines.abs() >= 0.5 {
                    let dir = self.scrolled_lines.signum() as i32;
                    self.scrolled_lines = 0.0;
                    let left_half = Self::cursor_in_left_half(cx);
                    let (_, fine_step, _) = Self::frac_mode(cx);
                    self.param_base.begin_set_parameter(cx);
                    self.step_ratio(cx, left_half, dir, fine_step);
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
        if self.hovered || self.drag_active {
            let b = cx.bounds();
            let dpx = cx.scale_factor();
            let pal = theme::current_palette(cx);
            let (ar, ag, ab) = pal.accent_rgb();
            let cx_mid = (b.x + b.w * 0.5).round();
            let mut sep = vg::Path::new();
            sep.move_to(cx_mid, b.y + 2.0 * dpx);
            sep.line_to(cx_mid, b.y + b.h - 4.0 * dpx);
            let mut paint = vg::Paint::color(vg::Color::rgba(ar, ag, ab, 75));
            paint.set_line_width((1.0 * dpx).max(1.0));
            canvas.stroke_path(&sep, &paint);
        }
    }
}
