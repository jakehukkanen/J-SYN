//! Multi-part oscillator pitch tuning drag boxes (`OCT`, `SEMI`, `FINE`) and spanning modulation overlay.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::RawParamEvent;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::util::ModifiersExt;

use crate::controls::bold_label;
use crate::shell::{ModUiHost, UiKitEvent};

use super::pill::{draw_mod_overlay, draw_pill_with_mod_bar};
use super::{
    ModDepthDrag, ModTargetBinding, emit_widget_tooltip, format_mod_range_tooltip,
    handle_mod_depth_move, update_mod_collar_cursor,
};

/// Which tuning sub-field a `PitchDragBox` edits within an oscillator's `[-48..=+48 st]` pitch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PitchPart {
    Octave,
    Semitone,
    Fine,
}

impl PitchPart {
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Octave => "OCT",
            Self::Semitone => "SEMI",
            Self::Fine => "FINE",
        }
    }

    #[inline]
    fn value_from_st(self, pitch_st: f32) -> i32 {
        let (oct, semi, fine) = decompose_pitch_st(pitch_st);
        match self {
            Self::Octave => oct,
            Self::Semitone => semi,
            Self::Fine => fine,
        }
    }
}

/// Decomposes a continuous `[-48.0, +48.0]` semitone pitch into signed `(oct, semi, fine_cents)`
/// using truncation toward zero so e.g. `-0.05 st -> (0, 0, -5)` and `-13.25 st -> (-1, -1, -25)`.
#[inline]
pub fn decompose_pitch_st(pitch_st: f32) -> (i32, i32, i32) {
    let total_cents = (pitch_st.clamp(-48.0, 48.0) * 100.0).round() as i32;
    let oct = total_cents / 1200;
    let rem = total_cents % 1200;
    let semi = rem / 100;
    let fine = rem % 100;
    (oct, semi, fine)
}

/// Hover-passthrough `ZStack` overlay over `[OCT | SEMI | FINE]` that owns the unified
/// `ModDestination::osc_pitch` drop rect and draws the spanning modulation range bar.
pub struct PitchGroupModOverlay {
    param_base: ParamWidgetBase,
    mod_binding: Option<ModTargetBinding>,
}

impl PitchGroupModOverlay {
    pub fn new<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
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
            param_base: ParamWidgetBase::new(cx, params, params_to_param),
            mod_binding: Some(ModTargetBinding {
                dest,
                mod_ui,
                tab_tag,
            }),
        }
        .build(cx, |_| {})
        .hoverable(false)
    }
}

impl View for PitchGroupModOverlay {
    fn element(&self) -> Option<&'static str> {
        Some("pitch-group-mod-overlay")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|raw_param_event, _| {
            if let RawParamEvent::ParametersChanged = raw_param_event {
                cx.needs_redraw();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        draw_mod_overlay(cx, canvas, cx.bounds(), &self.param_base, &self.mod_binding);
    }
}

/// Inline tuning pill (`OCT`, `SEMI`, `FINE`) over the oscillator's `[-48.00..=+48.00 st]` pitch param.
pub struct PitchDragBox {
    param_base: ParamWidgetBase,
    osc_pitch_idx: Option<usize>,
    part: PitchPart,
    hovered: bool,
    drag_active: bool,
    last_y: f32,
    accum_y: f32,
    last_mode: u8,
    scrolled_lines: f32,
    mod_depth_drag: Option<ModDepthDrag>,
    collar_cursor_active: bool,
    mod_binding: Option<ModTargetBinding>,
}

impl PitchDragBox {
    #[allow(clippy::too_many_arguments)]
    pub fn new_osc_pitch_part<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        osc_idx: usize,
        part: PitchPart,
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
        let osc_pitch_idx = Some(osc_idx);
        Self {
            param_base: ParamWidgetBase::new(cx, params.clone(), params_to_param),
            osc_pitch_idx,
            part,
            hovered: false,
            drag_active: false,
            last_y: 0.0,
            accum_y: 0.0,
            last_mode: 0,
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
            let text_lens = ParamWidgetBase::make_lens(params, params_to_param, move |param| {
                let st = param.modulated_normalized_value() * 96.0 - 48.0;
                format!("{}", part.value_from_st(st))
            });
            HStack::new(cx, move |cx| {
                bold_label(cx, part.label(), "inline-pill-label")
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
    fn pitch_mode(&self, cx: &EventContext) -> (u8, f32, f32) {
        match self.part {
            PitchPart::Octave => (2, 12.0, 14.0),
            PitchPart::Semitone => {
                if cx.modifiers().shift() {
                    (1, 0.01, 2.5)
                } else {
                    (0, 1.0, 5.0)
                }
            }
            PitchPart::Fine => (1, 0.01, if cx.modifiers().shift() { 5.0 } else { 2.0 }),
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
        if let (Some(md), Some(mb)) = (self.mod_depth_drag, self.mod_binding.as_ref()) {
            let base_norm = self
                .param_base
                .unmodulated_normalized_value()
                .clamp(0.0, 1.0);
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

    fn reset_part_or_all(&mut self, cx: &mut EventContext, reset_all: bool) {
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
        let next_norm = if reset_all {
            self.param_base.default_normalized_value()
        } else {
            let cur_st = self.param_base.unmodulated_normalized_value() * 96.0 - 48.0;
            let (oct, semi, fine) = decompose_pitch_st(cur_st);
            let new_cents = match self.part {
                PitchPart::Octave => semi * 100 + fine,
                PitchPart::Semitone => oct * 1200 + fine,
                PitchPart::Fine => oct * 1200 + semi * 100,
            };
            ((new_cents as f32 * 0.01 + 48.0) / 96.0).clamp(0.0, 1.0)
        };
        self.param_base.begin_set_parameter(cx);
        self.param_base.set_normalized_value(cx, next_norm);
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
                    cx.emit(UiKitEvent::HideTooltip);
                }
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                self.reset_part_or_all(cx, false);
                meta.consume();
            }
            WindowEvent::MouseDown(MouseButton::Left)
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
                    self.reset_part_or_all(cx, true);
                } else {
                    self.drag_active = true;
                    self.last_y = cx.mouse().cursory;
                    self.accum_y = 0.0;
                    self.last_mode = self.pitch_mode(cx).0;
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
                            cx.emit(UiKitEvent::ModUiChanged);
                            cx.needs_redraw();
                            meta.consume();
                            return;
                        }
                    } else if let Some(osc_idx) = self.osc_pitch_idx {
                        let bounds = cx.bounds();
                        let scale = cx.scale_factor().max(0.01);
                        cx.emit(UiKitEvent::ShowPitchContextMenu(
                            osc_idx,
                            mb.dest,
                            bounds.x / scale,
                            (bounds.y + bounds.h + 4.0) / scale,
                        ));
                        cx.needs_redraw();
                        meta.consume();
                        return;
                    } else if mb.mod_ui.has_slots_for_dest(mb.dest) {
                        let bounds = cx.bounds();
                        let scale = cx.scale_factor().max(0.01);
                        cx.emit(UiKitEvent::ShowModContextMenu(
                            mb.dest,
                            bounds.x / scale,
                            (bounds.y + bounds.h + 4.0) / scale,
                        ));
                        cx.needs_redraw();
                        meta.consume();
                        return;
                    }
                }
                self.reset_part_or_all(cx, false);
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
                    let (mode, step_st, base_px) = self.pitch_mode(cx);
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
                    let (_, step_st, _) = self.pitch_mode(cx);
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
        draw_pill_with_mod_bar(cx, canvas, &self.param_base, &self.mod_binding_none());
    }
}

impl PitchDragBox {
    #[inline]
    fn mod_binding_none(&self) -> Option<ModTargetBinding> {
        // Modulation drop rect and spanning bar are rendered by enclosing `PitchGroupModOverlay`.
        None
    }
}

#[cfg(test)]
mod tests {
    use super::decompose_pitch_st;

    #[test]
    fn pitch_decomposition_is_sign_symmetric_and_bijective() {
        assert_eq!(decompose_pitch_st(0.0), (0, 0, 0));
        assert_eq!(decompose_pitch_st(12.25), (1, 0, 25));
        assert_eq!(decompose_pitch_st(-0.05), (0, 0, -5));
        assert_eq!(decompose_pitch_st(-1.50), (0, -1, -50));
        assert_eq!(decompose_pitch_st(-13.25), (-1, -1, -25));
        assert_eq!(decompose_pitch_st(48.0), (4, 0, 0));
        assert_eq!(decompose_pitch_st(-48.0), (-4, 0, 0));
    }
}
