use std::sync::Arc;

use j_syn_dsp::mod_matrix::{ModDestination, ModMatrixSource};
use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::util::ModifiersExt;
use nih_plug_vizia::widgets::RawParamEvent;

use crate::editor::controls::bold_label;
use crate::editor::mod_bar::ModUiState;
use crate::editor::UiEvent;
use super::{
    emit_widget_tooltip, format_mod_range_tooltip, handle_mod_depth_move,
    update_mod_collar_cursor, ModDepthDrag, ModTargetBinding,
};

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
    right_click_opens_menu: bool,
    hovered: bool,
    drag_active: bool,
    drag_moved: bool,
    start_y: f32,
    start_val: f32,
    mod_depth_drag: Option<ModDepthDrag>,
    collar_cursor_active: bool,
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
        Self::build_inner(cx, params, params_to_param, num_steps, true, false, None, None)
    }

    pub fn new_with_mod_menu<'a, L, Params, P, FMap>(
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
            false,
            true,
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
        Self::build_inner(cx, params, params_to_param, num_steps, true, false, None, Some(label))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_inline_with_mod<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        num_steps: usize,
        label: &'static str,
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
            true,
            Some(ModTargetBinding {
                dest,
                mod_ui,
                tab_tag,
            }),
            Some(label),
        )
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
            update_mod_collar_cursor(cx, false, &mut self.collar_cursor_active);
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

    #[allow(clippy::too_many_arguments)]
    fn build_inner<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        num_steps: usize,
        click_cycles: bool,
        right_click_opens_menu: bool,
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
            right_click_opens_menu,
            hovered: false,
            drag_active: false,
            drag_moved: false,
            start_y: 0.0,
            start_val: 0.0,
            mod_depth_drag: None,
            collar_cursor_active: false,
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
                            update_mod_collar_cursor(cx, true, &mut self.collar_cursor_active);
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
                    } else if self.right_click_opens_menu {
                        let bounds = cx.bounds();
                        let scale = cx.scale_factor().max(0.01);
                        cx.emit(UiEvent::ShowActionModContextMenu(
                            mb.dest,
                            bounds.x / scale,
                            (bounds.y + bounds.h + 4.0) / scale,
                        ));
                        cx.needs_redraw();
                        meta.consume();
                        return;
                    } else if cx.modifiers().contains(Modifiers::ALT)
                        || cx.modifiers().command()
                        || cx.modifiers().contains(Modifiers::CTRL)
                        || !self.click_cycles
                    {
                        if mb.mod_ui.has_slots_for_dest(mb.dest) {
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
                    update_mod_collar_cursor(cx, false, &mut self.collar_cursor_active);
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
                    update_mod_collar_cursor(cx, true, &mut self.collar_cursor_active);
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

/// Shared pill background + modulation range bar renderer for pills & split-zone ratio drag box.
pub(super) fn draw_pill_with_mod_bar(
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
        let (sr, sg, sb) = if slot.bypassed {
            (110, 100, 92)
        } else {
            slot.source.color_rgb()
        };
        let alpha = if slot.bypassed { 75 } else { 225 };
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
            canvas.fill_path(&seg, &vg::Paint::color(vg::Color::rgba(sr, sg, sb, alpha)));
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
    osc_pitch_idx: Option<usize>,
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
    pub fn new_osc_pitch<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        osc_idx: usize,
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
        let osc_pitch_idx = Some(osc_idx);
        Self {
            param_base: ParamWidgetBase::new(cx, params.clone(), params_to_param),
            osc_pitch_idx,
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
        if cx.modifiers().contains(Modifiers::ALT) {
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
                    } else if let Some(osc_idx) = self.osc_pitch_idx {
                        let bounds = cx.bounds();
                        let scale = cx.scale_factor().max(0.01);
                        cx.emit(UiEvent::ShowPitchContextMenu(
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
                    update_mod_collar_cursor(cx, false, &mut self.collar_cursor_active);
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
                    update_mod_collar_cursor(cx, true, &mut self.collar_cursor_active);
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
