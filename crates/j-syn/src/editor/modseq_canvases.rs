//! Interactive `femtovg` canvases for `MODSEQ` (`ModSeqCurveCanvas`, `LfoPresetBar`,
//! `ModSeqPaletteGrid`, and `AdsrPreviewScope`).

use std::sync::Arc;

use j_syn_dsp::{
    evaluate_adsr_preview, evaluate_modseq_curve_with_swing, evaluate_mseg_curve,
    evaluate_step_shape, AdsrParams, LfoPreset, ModSeqStep, ModSeqStepShape, MsegNode,
    MAX_MODSEQ_STEPS, MAX_MSEG_NODES,
};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::util::ModifiersExt;

use super::mod_bar::ModUiState;
use super::UiEvent;
use crate::{JSynPluginParams, ModSeqModeParam};

pub(super) struct ModSeqCurveCanvas {
    seq_idx: usize,
    params: Arc<JSynPluginParams>,
    mod_ui: Arc<ModUiState>,
    drag_mode: Option<CanvasDragMode>,
}

#[derive(Debug, Clone, Copy)]
enum CanvasDragMode {
    StampLevel,
    EndLevel { step_idx: usize },
    StepTension { step_idx: usize, start_y: f32, start_tension: f32 },
    MsegNode { node_idx: usize },
    MsegTension { seg_idx: usize, start_y: f32, start_tension: f32 },
}

impl ModSeqCurveCanvas {
    pub(super) fn new(
        cx: &mut Context,
        seq_idx: usize,
        params: Arc<JSynPluginParams>,
        mod_ui: Arc<ModUiState>,
    ) -> Handle<'_, Self> {
        Self {
            seq_idx,
            params,
            mod_ui,
            drag_mode: None,
        }
        .build(cx, |_| {})
    }

    #[inline]
    fn is_mseg(&self) -> bool {
        self.params.modseqs[self.seq_idx].mode.value() == ModSeqModeParam::Mseg
    }

    fn inner_rect(bounds: BoundingBox, scale: f32) -> (f32, f32, f32, f32) {
        let pad = 4.0 * scale.max(1.0);
        let x0 = bounds.x + pad;
        let y0 = bounds.y + pad;
        let w = (bounds.w - 2.0 * pad).max(1.0);
        let h = (bounds.h - 2.0 * pad).max(1.0);
        (x0, y0, w, h)
    }

    fn cursor_step_and_level(&self, bounds: BoundingBox, x: f32, y: f32, num_steps: usize) -> (usize, f32) {
        let (x0, y0, w, h) = Self::inner_rect(bounds, 1.0);
        let nx = ((x - x0) / w).clamp(0.0, 0.999_9);
        let ny = 1.0 - ((y - y0) / h).clamp(0.0, 1.0);
        let step_idx = ((nx * (num_steps as f32)).floor() as usize).min(num_steps - 1);
        (step_idx, ny)
    }

    fn cursor_norm_xy(bounds: BoundingBox, x: f32, y: f32, scale: f32) -> (f32, f32) {
        let (x0, y0, w, h) = Self::inner_rect(bounds, scale);
        let nx = ((x - x0) / w).clamp(0.0, 1.0);
        let ny = (1.0 - (y - y0) / h).clamp(0.0, 1.0);
        (nx, ny)
    }

    fn hit_mseg_node(&self, bounds: BoundingBox, x: f32, y: f32, scale: f32) -> Option<usize> {
        let (x0, y0, w, h) = Self::inner_rect(bounds, scale);
        let bank = self.params.modseq_steps.snapshot();
        let mseg = &bank.msegs[self.seq_idx];
        let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
        let radius = 9.0 * scale.max(1.0);
        let mut best: Option<(usize, f32)> = None;
        for (i, node) in mseg.nodes.iter().enumerate().take(n) {
            let px = x0 + node.time_x * w;
            let py = y0 + (1.0 - node.level_y) * h;
            let d = (x - px).hypot(y - py);
            if d <= radius && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        best.map(|(idx, _)| idx)
    }

    fn hit_mseg_tension_handle(&self, bounds: BoundingBox, x: f32, y: f32, scale: f32) -> Option<(usize, f32)> {
        let (x0, y0, w, h) = Self::inner_rect(bounds, scale);
        let bank = self.params.modseq_steps.snapshot();
        let mseg = &bank.msegs[self.seq_idx];
        let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
        let radius = 8.5 * scale.max(1.0);
        for i in 0..(n - 1) {
            let mid_x = 0.5 * (mseg.nodes[i].time_x + mseg.nodes[i + 1].time_x);
            let mid_y = evaluate_mseg_curve(mseg, mid_x);
            let px = x0 + mid_x * w;
            let py = y0 + (1.0 - mid_y) * h;
            if (x - px).hypot(y - py) <= radius {
                return Some((i, mseg.nodes[i].tension));
            }
        }
        None
    }

    fn apply_drag_edit(
        &self,
        x: f32,
        y: f32,
        bounds: BoundingBox,
        mode: CanvasDragMode,
        scale: f32,
        shift_held: bool,
    ) {
        let mut bank = self.params.modseq_steps.snapshot();
        match mode {
            CanvasDragMode::StampLevel => {
                let num_steps = self.params.modseqs[self.seq_idx]
                    .num_steps
                    .value()
                    .clamp(2, MAX_MODSEQ_STEPS as i32) as usize;
                let (hovered_step, level) = self.cursor_step_and_level(bounds, x, y, num_steps);
                let shape = self.mod_ui.active_stamp();
                let tension = if self.params.modseq_swing.value() {
                    bank.seqs[self.seq_idx][hovered_step].tension
                } else {
                    0.0
                };
                bank.seqs[self.seq_idx][hovered_step] = ModSeqStep::stamped(shape, level, tension);
            }
            CanvasDragMode::EndLevel { step_idx } => {
                let num_steps = self.params.modseqs[self.seq_idx]
                    .num_steps
                    .value()
                    .clamp(2, MAX_MODSEQ_STEPS as i32) as usize;
                let (_hovered, level) = self.cursor_step_and_level(bounds, x, y, num_steps);
                let s = &mut bank.seqs[self.seq_idx][step_idx.min(MAX_MODSEQ_STEPS - 1)];
                s.shape = ModSeqStepShape::Linear;
                s.end_level = level;
            }
            CanvasDragMode::StepTension {
                step_idx,
                start_y,
                start_tension,
            } => {
                let dy = (start_y - y) / (70.0 * scale.max(1.0));
                bank.seqs[self.seq_idx][step_idx.min(MAX_MODSEQ_STEPS - 1)].tension =
                    (start_tension + dy).clamp(-1.0, 1.0);
            }
            CanvasDragMode::MsegNode { node_idx } => {
                let (mut nx, mut ny) = Self::cursor_norm_xy(bounds, x, y, scale);
                let mseg = &mut bank.msegs[self.seq_idx];
                let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
                let idx = node_idx.min(n - 1);
                if self.mod_ui.mseg_snap() || shift_held {
                    ny = (ny * 8.0).round() / 8.0;
                    if idx > 0 && idx + 1 < n {
                        nx = (nx * 16.0).round() / 16.0;
                    }
                }
                mseg.nodes[idx].level_y = ny.clamp(0.0, 1.0);
                if idx == 0 {
                    mseg.nodes[idx].time_x = 0.0;
                } else if idx + 1 == n {
                    mseg.nodes[idx].time_x = 1.0;
                } else {
                    let min_x = (mseg.nodes[idx - 1].time_x + 0.015).min(0.98);
                    let max_x = (mseg.nodes[idx + 1].time_x - 0.015).max(min_x);
                    mseg.nodes[idx].time_x = nx.clamp(min_x, max_x);
                }
            }
            CanvasDragMode::MsegTension {
                seg_idx,
                start_y,
                start_tension,
            } => {
                let mseg = &mut bank.msegs[self.seq_idx];
                let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
                if seg_idx + 1 < n {
                    let sign = if mseg.nodes[seg_idx + 1].level_y >= mseg.nodes[seg_idx].level_y {
                        -1.0
                    } else {
                        1.0
                    };
                    let dy = sign * (start_y - y) / (75.0 * scale.max(1.0));
                    mseg.nodes[seg_idx].tension = (start_tension + dy).clamp(-1.0, 1.0);
                }
            }
        }
        self.params.modseq_steps.store_value(bank);
    }
}

impl View for ModSeqCurveCanvas {
    fn element(&self) -> Option<&'static str> {
        Some("modseq-canvas")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                let bounds = cx.bounds();
                let scale = cx.scale_factor();
                let mx = cx.mouse().cursorx;
                let my = cx.mouse().cursory;

                if self.is_mseg() {
                    if let Some(node_idx) = self.hit_mseg_node(bounds, mx, my, scale) {
                        let mode = CanvasDragMode::MsegNode { node_idx };
                        self.drag_mode = Some(mode);
                        self.apply_drag_edit(
                            mx,
                            my,
                            bounds,
                            mode,
                            scale,
                            cx.modifiers().shift(),
                        );
                        cx.capture();
                        cx.needs_redraw();
                        meta.consume();
                        return;
                    }
                    if let Some((seg_idx, start_tension)) =
                        self.hit_mseg_tension_handle(bounds, mx, my, scale)
                    {
                        self.drag_mode = Some(CanvasDragMode::MsegTension {
                            seg_idx,
                            start_y: my,
                            start_tension,
                        });
                        cx.capture();
                        cx.needs_redraw();
                        meta.consume();
                        return;
                    }
                    // Empty canvas click with Alt bends enclosing segment tension
                    if cx.modifiers().contains(Modifiers::ALT) {
                        let (nx, _) = Self::cursor_norm_xy(bounds, mx, my, scale);
                        let bank = self.params.modseq_steps.snapshot();
                        let mseg = &bank.msegs[self.seq_idx];
                        let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
                        let mut seg_idx = 0usize;
                        for i in 0..(n - 1) {
                            if nx <= mseg.nodes[i + 1].time_x || i + 2 == n {
                                seg_idx = i;
                                break;
                            }
                        }
                        self.drag_mode = Some(CanvasDragMode::MsegTension {
                            seg_idx,
                            start_y: my,
                            start_tension: mseg.nodes[seg_idx].tension,
                        });
                        cx.capture();
                        cx.needs_redraw();
                        meta.consume();
                    }
                    return;
                }

                let num_steps = self.params.modseqs[self.seq_idx]
                    .num_steps
                    .value()
                    .clamp(2, MAX_MODSEQ_STEPS as i32) as usize;
                let (step_idx, _lvl) = self.cursor_step_and_level(bounds, mx, my, num_steps);
                let mode = if cx.modifiers().contains(Modifiers::ALT) {
                    let bank = self.params.modseq_steps.snapshot();
                    CanvasDragMode::StepTension {
                        step_idx,
                        start_y: my,
                        start_tension: bank.seqs[self.seq_idx][step_idx].tension,
                    }
                } else if cx.modifiers().shift() {
                    CanvasDragMode::EndLevel { step_idx }
                } else {
                    CanvasDragMode::StampLevel
                };
                self.drag_mode = Some(mode);
                self.apply_drag_edit(mx, my, bounds, mode, scale, cx.modifiers().shift());
                cx.capture();
                cx.needs_redraw();
                meta.consume();
            }
            WindowEvent::MouseDown(MouseButton::Right) => {
                let bounds = cx.bounds();
                let scale = cx.scale_factor();
                let mx = cx.mouse().cursorx;
                let my = cx.mouse().cursory;
                let bank = self.params.modseq_steps.snapshot();
                if self.is_mseg() {
                    let (nx, _) = Self::cursor_norm_xy(bounds, mx, my, scale);
                    let mseg = &bank.msegs[self.seq_idx];
                    let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
                    let mut seg_idx = 0usize;
                    for i in 0..(n - 1) {
                        if nx <= mseg.nodes[i + 1].time_x || i + 2 == n {
                            seg_idx = i;
                            break;
                        }
                    }
                    self.drag_mode = Some(CanvasDragMode::MsegTension {
                        seg_idx,
                        start_y: my,
                        start_tension: mseg.nodes[seg_idx].tension,
                    });
                } else {
                    let num_steps = self.params.modseqs[self.seq_idx]
                        .num_steps
                        .value()
                        .clamp(2, MAX_MODSEQ_STEPS as i32) as usize;
                    let (step_idx, _) = self.cursor_step_and_level(bounds, mx, my, num_steps);
                    self.drag_mode = Some(CanvasDragMode::StepTension {
                        step_idx,
                        start_y: my,
                        start_tension: bank.seqs[self.seq_idx][step_idx].tension,
                    });
                }
                cx.capture();
                meta.consume();
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                let bounds = cx.bounds();
                let scale = cx.scale_factor();
                let mx = cx.mouse().cursorx;
                let my = cx.mouse().cursory;

                if self.is_mseg() {
                    let mut bank = self.params.modseq_steps.snapshot();
                    let mseg = &mut bank.msegs[self.seq_idx];
                    let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
                    if let Some(hit) = self.hit_mseg_node(bounds, mx, my, scale) {
                        if hit > 0 && hit + 1 < n && n > 2 {
                            for i in hit..(n - 1) {
                                mseg.nodes[i] = mseg.nodes[i + 1];
                            }
                            mseg.num_nodes = n - 1;
                            self.params.modseq_steps.store_value(bank);
                            cx.emit(UiEvent::ModUiChanged);
                            cx.needs_redraw();
                        }
                    } else if n < MAX_MSEG_NODES {
                        let (raw_x, raw_y) = Self::cursor_norm_xy(bounds, mx, my, scale);
                        let snap = self.mod_ui.mseg_snap() || cx.modifiers().shift();
                        let nx = if snap {
                            (raw_x * 16.0).round() / 16.0
                        } else {
                            raw_x
                        };
                        let ny = if snap {
                            (raw_y * 8.0).round() / 8.0
                        } else {
                            raw_y
                        };
                        let tx = nx.clamp(0.02, 0.98);
                        let mut insert_at = n - 1;
                        for (i, node) in mseg.nodes.iter().enumerate().take(n).skip(1) {
                            if tx < node.time_x {
                                insert_at = i;
                                break;
                            }
                        }
                        let left_x = mseg.nodes[insert_at - 1].time_x + 0.015;
                        let right_x = mseg.nodes[insert_at].time_x - 0.015;
                        if left_x <= right_x {
                            for i in (insert_at..n).rev() {
                                mseg.nodes[i + 1] = mseg.nodes[i];
                            }
                            mseg.nodes[insert_at] = MsegNode {
                                time_x: tx.clamp(left_x, right_x),
                                level_y: ny.clamp(0.0, 1.0),
                                tension: 0.0,
                            };
                            mseg.num_nodes = n + 1;
                            self.params.modseq_steps.store_value(bank);
                            cx.emit(UiEvent::ModUiChanged);
                            cx.needs_redraw();
                        }
                    }
                    meta.consume();
                    return;
                }

                let num_steps = self.params.modseqs[self.seq_idx]
                    .num_steps
                    .value()
                    .clamp(2, MAX_MODSEQ_STEPS as i32) as usize;
                let (step_idx, _) = self.cursor_step_and_level(bounds, mx, my, num_steps);
                let mut bank = self.params.modseq_steps.snapshot();
                bank.seqs[self.seq_idx][step_idx] = ModSeqStep::default();
                self.params.modseq_steps.store_value(bank);
                cx.needs_redraw();
                meta.consume();
            }
            WindowEvent::MouseMove(x, y) => {
                if let Some(mode) = self.drag_mode {
                    let bounds = cx.bounds();
                    self.apply_drag_edit(
                        *x,
                        *y,
                        bounds,
                        mode,
                        cx.scale_factor(),
                        cx.modifiers().shift(),
                    );
                    cx.needs_redraw();
                    meta.consume();
                }
            }
            WindowEvent::MouseUp(MouseButton::Left)
            | WindowEvent::MouseUp(MouseButton::Right) => {
                if self.drag_mode.is_some() {
                    self.drag_mode = None;
                    cx.release();
                    cx.needs_redraw();
                    meta.consume();
                }
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 4.0 || bounds.h <= 4.0 {
            return;
        }
        let dpx = cx.scale_factor();
        let pal = super::theme::current_palette(cx);
        let (ar, ag, ab) = pal.mod_seq_rgb();

        let mut bg = vg::Path::new();
        bg.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 4.0 * dpx);
        canvas.fill_path(&bg, &vg::Paint::color(super::theme::vg_color(&pal.scope_bg, 255)));
        let mut border = vg::Paint::color(super::theme::vg_color(&pal.border_mid, 255));
        border.set_line_width(1.0 * dpx);
        canvas.stroke_path(&bg, &border);

        let pad = 4.0 * dpx;
        let x0 = bounds.x + pad;
        let y0 = bounds.y + pad;
        let w = (bounds.w - 2.0 * pad).max(1.0);
        let h = (bounds.h - 2.0 * pad).max(1.0);
        let is_mseg = self.is_mseg();
        let num_steps = self.params.modseqs[self.seq_idx]
            .num_steps
            .value()
            .clamp(2, MAX_MODSEQ_STEPS as i32) as usize;
        let bank = self.params.modseq_steps.snapshot();
        let steps = &bank.seqs[self.seq_idx];
        let mseg = &bank.msegs[self.seq_idx];

        // Subtle grid (in LFO mode with SNAP enabled, show 1/16 x 1/8 snap lines)
        let mut grid_paint = vg::Paint::color(super::theme::vg_color(&pal.scope_grid, 175));
        grid_paint.set_line_width(0.8 * dpx);
        let mut snap_grid_paint = vg::Paint::color(super::theme::vg_color(&pal.scope_grid, 90));
        snap_grid_paint.set_line_width(0.65 * dpx);
        let snap_on = is_mseg && self.mod_ui.mseg_snap();

        let h_divs = if snap_on { 8 } else { 4 };
        for g in 1..h_divs {
            let gy = y0 + (g as f32 / h_divs as f32) * h;
            let mut line = vg::Path::new();
            line.move_to(x0, gy);
            line.line_to(x0 + w, gy);
            let paint = if snap_on && g % 2 != 0 {
                &snap_grid_paint
            } else {
                &grid_paint
            };
            canvas.stroke_path(&line, paint);
        }
        let v_divs = if is_mseg {
            if snap_on { 16 } else { 4 }
        } else {
            num_steps
        };
        for s in 1..v_divs {
            let gx = x0 + (s as f32 / v_divs as f32) * w;
            let mut line = vg::Path::new();
            line.move_to(gx, y0);
            line.line_to(gx, y0 + h);
            let paint = if snap_on && s % 4 != 0 {
                &snap_grid_paint
            } else {
                &grid_paint
            };
            canvas.stroke_path(&line, paint);
        }

        // Subtle per-step depth bars in MODSEQ mode so stamped amplitudes read at a glance
        if !is_mseg {
            let step_w = w / num_steps as f32;
            for (s, step) in steps.iter().enumerate().take(num_steps) {
                let lvl = step.start_level.clamp(0.0, 1.0);
                if lvl > 0.005 {
                    let bx = x0 + s as f32 * step_w + 1.0 * dpx;
                    let bw = (step_w - 2.0 * dpx).max(1.0);
                    let bh = lvl * h;
                    let mut bar = vg::Path::new();
                    bar.rounded_rect(bx, y0 + h - bh, bw, bh, 1.5 * dpx);
                    canvas.fill_path(&bar, &vg::Paint::color(vg::Color::rgba(ar, ag, ab, 20)));
                }
            }
        }

        // Filled + stroked continuous curve
        let groove_swing = self.params.modseq_swing.value();
        let samples = if is_mseg {
            128
        } else {
            (num_steps * 24).max(128)
        };
        let mut fill_path = vg::Path::new();
        let mut stroke_path = vg::Path::new();
        fill_path.move_to(x0, y0 + h);
        for i in 0..=samples {
            let t = (i as f32 / samples as f32).min(0.999_999);
            let val = if is_mseg {
                evaluate_mseg_curve(mseg, t)
            } else {
                evaluate_modseq_curve_with_swing(steps, num_steps, t, groove_swing)
            };
            let px = x0 + (i as f32 / samples as f32) * w;
            let py = y0 + (1.0 - val) * h;
            fill_path.line_to(px, py);
            if i == 0 {
                stroke_path.move_to(px, py);
            } else {
                stroke_path.line_to(px, py);
            }
        }
        fill_path.line_to(x0 + w, y0 + h);
        fill_path.close();

        let fill_paint = vg::Paint::linear_gradient(
            x0,
            y0,
            x0,
            y0 + h,
            vg::Color::rgba(ar, ag, ab, 95),
            vg::Color::rgba(ar, ag, ab, 14),
        );
        canvas.fill_path(&fill_path, &fill_paint);

        let mut stroke_paint = vg::Paint::color(vg::Color::rgb(ar, ag, ab));
        stroke_paint.set_line_width(1.8 * dpx);
        stroke_paint.set_line_cap(vg::LineCap::Round);
        stroke_paint.set_line_join(vg::LineJoin::Round);
        canvas.stroke_path(&stroke_path, &stroke_paint);

        // Node/step control points
        if is_mseg {
            let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
            for i in 0..(n - 1) {
                let mid_x = 0.5 * (mseg.nodes[i].time_x + mseg.nodes[i + 1].time_x);
                let mid_y = evaluate_mseg_curve(mseg, mid_x);
                let mx = x0 + mid_x * w;
                let my = y0 + (1.0 - mid_y) * h;
                let mut hdl = vg::Path::new();
                hdl.circle(mx, my, 2.3 * dpx);
                canvas.fill_path(&hdl, &vg::Paint::color(vg::Color::rgba(255, 211, 130, 175)));
            }
            for node in mseg.nodes.iter().take(n) {
                let nx = x0 + node.time_x * w;
                let ny = y0 + (1.0 - node.level_y) * h;
                let mut ring = vg::Path::new();
                ring.circle(nx, ny, 3.6 * dpx);
                canvas.fill_path(&ring, &vg::Paint::color(vg::Color::rgb(ar, ag, ab)));
                let mut inner = vg::Path::new();
                inner.circle(nx, ny, 2.0 * dpx);
                canvas.fill_path(&inner, &vg::Paint::color(vg::Color::rgb(255, 248, 236)));
            }
        } else {
            let step_w = w / num_steps as f32;
            for (s, step) in steps.iter().enumerate().take(num_steps) {
                let sx0 = x0 + s as f32 * step_w;
                let sx1 = sx0 + step_w;
                let sy0 = y0 + (1.0 - step.start_level) * h;
                let sy1 = y0 + (1.0 - step.end_level) * h;
                let mut dot = vg::Path::new();
                if step.shape == ModSeqStepShape::Linear && (sy1 - sy0).abs() > 2.0 * dpx {
                    dot.circle(sx0 + 1.8 * dpx, sy0, 2.0 * dpx);
                    dot.circle(sx1 - 1.8 * dpx, sy1, 1.8 * dpx);
                } else {
                    dot.circle(sx0 + 0.5 * step_w, sy0, 1.9 * dpx);
                }
                canvas.fill_path(&dot, &vg::Paint::color(vg::Color::rgb(255, 244, 228)));
            }
        }

        // Real-time animated playhead from audio-thread telemetry
        let phase = self.mod_ui.telemetry.modseq_phase(self.seq_idx).clamp(0.0, 1.0);
        let ph_x = x0 + phase * w;
        let mut ph_line = vg::Path::new();
        ph_line.move_to(ph_x, y0);
        ph_line.line_to(ph_x, y0 + h);
        let mut ph_paint = vg::Paint::color(vg::Color::rgba(255, 248, 232, 210));
        ph_paint.set_line_width(1.4 * dpx);
        canvas.stroke_path(&ph_line, &ph_paint);

        let cur_val = if is_mseg {
            evaluate_mseg_curve(mseg, phase.min(0.999_999))
        } else {
            evaluate_modseq_curve_with_swing(steps, num_steps, phase.min(0.999_999), groove_swing)
        };
        let mut ph_dot = vg::Path::new();
        ph_dot.circle(ph_x, y0 + (1.0 - cur_val) * h, 3.0 * dpx);
        canvas.fill_path(&ph_dot, &vg::Paint::color(vg::Color::rgb(ar, ag, ab)));
        let mut ph_ring = vg::Paint::color(vg::Color::rgb(255, 248, 232));
        ph_ring.set_line_width(1.1 * dpx);
        canvas.stroke_path(&ph_dot, &ph_ring);
    }
}

#[inline]
fn grid_tile_rect(
    bounds: BoundingBox,
    idx: usize,
    cols: usize,
    rows: usize,
    pad: f32,
    gap: f32,
) -> (f32, f32, f32, f32) {
    let tile_w =
        ((bounds.w - 2.0 * pad - (cols.saturating_sub(1) as f32) * gap) / cols as f32).max(2.0);
    let tile_h =
        ((bounds.h - 2.0 * pad - (rows.saturating_sub(1) as f32) * gap) / rows as f32).max(2.0);
    let col = idx % cols;
    let row = idx / cols;
    (
        bounds.x + pad + col as f32 * (tile_w + gap),
        bounds.y + pad + row as f32 * (tile_h + gap),
        tile_w,
        tile_h,
    )
}

#[inline]
fn hit_grid_tile(
    bounds: BoundingBox,
    x: f32,
    y: f32,
    count: usize,
    cols: usize,
    rows: usize,
    pad: f32,
    gap: f32,
) -> Option<usize> {
    (0..count).find(|&idx| {
        let (tx, ty, tw, th) = grid_tile_rect(bounds, idx, cols, rows, pad, gap);
        x >= tx && x <= tx + tw && y >= ty && y <= ty + th
    })
}

#[allow(clippy::too_many_arguments)]
fn stroke_tile_curve(
    canvas: &mut Canvas,
    tx: f32,
    ty: f32,
    tw: f32,
    th: f32,
    dpx: f32,
    accent: (u8, u8, u8),
    bright: bool,
    samples: usize,
    eval: impl Fn(f32) -> f32,
) {
    let mx = (tw * 0.14).max(2.0 * dpx);
    let my = (th * 0.18).max(2.0 * dpx);
    let cw = (tw - 2.0 * mx).max(1.0);
    let ch = (th - 2.0 * my).max(1.0);
    let x0 = tx + mx;
    let y0 = ty + my;

    let mut path = vg::Path::new();
    for i in 0..=samples {
        let t = (i as f32 / samples as f32).min(0.999_999);
        let v = eval(t).clamp(0.0, 1.0);
        let px = x0 + (i as f32 / samples as f32) * cw;
        let py = y0 + (1.0 - v) * ch;
        if i == 0 {
            path.move_to(px, py);
        } else {
            path.line_to(px, py);
        }
    }
    let (ar, ag, ab) = accent;
    let mut paint = vg::Paint::color(vg::Color::rgba(ar, ag, ab, if bright { 255 } else { 215 }));
    paint.set_line_width((if bright { 1.5 } else { 1.25 }) * dpx);
    paint.set_line_cap(vg::LineCap::Round);
    paint.set_line_join(vg::LineJoin::Round);
    canvas.stroke_path(&path, &paint);
}

/// Graphical 8-preset vector strip shown in the `LFO` sub-bar next to `RESET`.
pub(super) struct LfoPresetBar {
    seq_idx: usize,
    params: Arc<JSynPluginParams>,
    hovered_idx: Option<usize>,
}

impl LfoPresetBar {
    pub(super) fn new(cx: &mut Context, seq_idx: usize, params: Arc<JSynPluginParams>) -> Handle<'_, Self> {
        Self {
            seq_idx,
            params,
            hovered_idx: None,
        }
        .build(cx, |_| {})
    }
}

impl View for LfoPresetBar {
    fn element(&self) -> Option<&'static str> {
        Some("lfo-preset-bar")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseMove(x, y) => {
                let dpx = cx.scale_factor();
                let next = hit_grid_tile(
                    cx.bounds(),
                    *x,
                    *y,
                    LfoPreset::ALL.len(),
                    LfoPreset::ALL.len(),
                    1,
                    0.0,
                    2.0 * dpx,
                );
                if next != self.hovered_idx {
                    self.hovered_idx = next;
                    cx.needs_redraw();
                }
            }
            WindowEvent::MouseOut => {
                if self.hovered_idx.take().is_some() {
                    cx.needs_redraw();
                }
            }
            WindowEvent::MouseDown(MouseButton::Left) => {
                let dpx = cx.scale_factor();
                if let Some(idx) = hit_grid_tile(
                    cx.bounds(),
                    cx.mouse().cursorx,
                    cx.mouse().cursory,
                    LfoPreset::ALL.len(),
                    LfoPreset::ALL.len(),
                    1,
                    0.0,
                    2.0 * dpx,
                ) {
                    let mut bank = self.params.modseq_steps.snapshot();
                    bank.msegs[self.seq_idx] = LfoPreset::ALL[idx].to_mseg();
                    self.params.modseq_steps.store_value(bank);
                    cx.emit(UiEvent::ModUiChanged);
                    cx.needs_redraw();
                    meta.consume();
                }
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 4.0 || bounds.h <= 4.0 {
            return;
        }
        let dpx = cx.scale_factor();
        let pal = super::theme::current_palette(cx);
        let accent = pal.mod_seq_rgb();
        let gap = 2.0 * dpx;

        for (idx, &preset) in LfoPreset::ALL.iter().enumerate() {
            let (tx, ty, tw, th) =
                grid_tile_rect(bounds, idx, LfoPreset::ALL.len(), 1, 0.0, gap);
            let hovered = self.hovered_idx == Some(idx);

            let mut tile = vg::Path::new();
            tile.rounded_rect(tx, ty, tw, th, 2.5 * dpx);
            canvas.fill_path(
                &tile,
                &vg::Paint::color(if hovered {
                    super::theme::vg_color(&pal.surface_hover, 255)
                } else {
                    super::theme::vg_color(&pal.surface_dark, 255)
                }),
            );
            let mut border = vg::Paint::color(if hovered {
                super::theme::vg_color(&pal.accent_primary, 255)
            } else {
                super::theme::vg_color(&pal.border_mid, 255)
            });
            border.set_line_width(1.0 * dpx);
            canvas.stroke_path(&tile, &border);

            let mseg = preset.to_mseg();
            stroke_tile_curve(canvas, tx, ty, tw, th, dpx, accent, hovered, 20, |t| {
                evaluate_mseg_curve(&mseg, t)
            });
        }
    }
}

/// Interactive `2x9` graphical shape palette for `MODSEQ` step stamping & quick fill.
pub(super) struct ModSeqPaletteGrid {
    seq_idx: usize,
    params: Arc<JSynPluginParams>,
    mod_ui: Arc<ModUiState>,
    hovered_idx: Option<usize>,
}

impl ModSeqPaletteGrid {
    pub(super) fn new(
        cx: &mut Context,
        seq_idx: usize,
        params: Arc<JSynPluginParams>,
        mod_ui: Arc<ModUiState>,
    ) -> Handle<'_, Self> {
        Self {
            seq_idx,
            params,
            mod_ui,
            hovered_idx: None,
        }
        .build(cx, |_| {})
        .class("palette-grid")
    }
}

impl View for ModSeqPaletteGrid {
    fn element(&self) -> Option<&'static str> {
        Some("modseq-palette-grid")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseMove(x, y) => {
                let dpx = cx.scale_factor();
                let next = hit_grid_tile(
                    cx.bounds(),
                    *x,
                    *y,
                    ModSeqStepShape::PALETTE.len(),
                    9,
                    2,
                    2.0 * dpx,
                    2.0 * dpx,
                );
                if next != self.hovered_idx {
                    self.hovered_idx = next;
                    cx.needs_redraw();
                }
            }
            WindowEvent::MouseOut => {
                if self.hovered_idx.take().is_some() {
                    cx.needs_redraw();
                }
            }
            WindowEvent::MouseDown(MouseButton::Left) => {
                let dpx = cx.scale_factor();
                if let Some(idx) = hit_grid_tile(
                    cx.bounds(),
                    cx.mouse().cursorx,
                    cx.mouse().cursory,
                    ModSeqStepShape::PALETTE.len(),
                    9,
                    2,
                    2.0 * dpx,
                    2.0 * dpx,
                ) {
                    self.mod_ui.set_active_stamp(ModSeqStepShape::PALETTE[idx]);
                    cx.emit(UiEvent::ModUiChanged);
                    cx.needs_redraw();
                    meta.consume();
                }
            }
            WindowEvent::MouseDown(MouseButton::Right) => {
                let dpx = cx.scale_factor();
                if let Some(idx) = hit_grid_tile(
                    cx.bounds(),
                    cx.mouse().cursorx,
                    cx.mouse().cursory,
                    ModSeqStepShape::PALETTE.len(),
                    9,
                    2,
                    2.0 * dpx,
                    2.0 * dpx,
                ) {
                    let shape = ModSeqStepShape::PALETTE[idx];
                    self.mod_ui.set_active_stamp(shape);
                    let n = self.params.modseqs[self.seq_idx]
                        .num_steps
                        .value()
                        .clamp(2, MAX_MODSEQ_STEPS as i32) as usize;
                    let mut bank = self.params.modseq_steps.snapshot();
                    for step in bank.seqs[self.seq_idx].iter_mut().take(n) {
                        *step = ModSeqStep::stamped(shape, 1.0, 0.0);
                    }
                    self.params.modseq_steps.store_value(bank);
                    cx.emit(UiEvent::ModUiChanged);
                    cx.needs_redraw();
                    meta.consume();
                }
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let mut frame = cx.build_path();
        cx.draw_background(canvas, &mut frame);
        cx.draw_border(canvas, &mut frame);

        let bounds = cx.bounds();
        if bounds.w <= 8.0 || bounds.h <= 8.0 {
            return;
        }
        let dpx = cx.scale_factor();
        let pad = 2.0 * dpx;
        let gap = 2.0 * dpx;
        let active_shape = self.mod_ui.active_stamp();
        let pal = super::theme::current_palette(cx);
        let accent = pal.mod_seq_rgb();

        for (idx, &shape) in ModSeqStepShape::PALETTE.iter().enumerate() {
            let (tx, ty, tw, th) = grid_tile_rect(bounds, idx, 9, 2, pad, gap);
            let is_active = shape == active_shape;
            let hovered = self.hovered_idx == Some(idx);

            let mut tile = vg::Path::new();
            tile.rounded_rect(tx, ty, tw, th, 2.5 * dpx);
            canvas.fill_path(
                &tile,
                &vg::Paint::color(if is_active {
                    super::theme::vg_color(&pal.surface_active, 255)
                } else if hovered {
                    super::theme::vg_color(&pal.surface_hover, 255)
                } else {
                    super::theme::vg_color(&pal.surface_dark, 255)
                }),
            );
            let mut border = vg::Paint::color(if is_active {
                super::theme::vg_color(&pal.accent_primary, 255)
            } else if hovered {
                super::theme::vg_color(&pal.border_light, 255)
            } else {
                super::theme::vg_color(&pal.border_mid, 255)
            });
            border.set_line_width((if is_active { 1.25 } else { 0.9 }) * dpx);
            canvas.stroke_path(&tile, &border);

            let sub = shape.subdivision();
            if sub > 1 {
                let mut div_paint = vg::Paint::color(super::theme::vg_color(&pal.text_dim, 95));
                div_paint.set_line_width(0.75 * dpx);
                for k in 1..sub {
                    let dx = tx + (k as f32 / sub as f32) * tw;
                    let mut line = vg::Path::new();
                    line.move_to(dx, ty + 2.0 * dpx);
                    line.line_to(dx, ty + th - 2.0 * dpx);
                    canvas.stroke_path(&line, &div_paint);
                }
            }

            let step = ModSeqStep::stamped(shape, 1.0, 0.0);
            stroke_tile_curve(
                canvas,
                tx,
                ty,
                tw,
                th,
                dpx,
                accent,
                is_active || hovered,
                24,
                |t| evaluate_step_shape(step, t),
            );
        }
    }
}

/// Live `femtovg` ADSR shape preview canvas for `ENV 1` – `ENV 4`.
pub(super) struct AdsrPreviewScope {
    env_idx: usize,
    params: Arc<JSynPluginParams>,
    mod_ui: Arc<ModUiState>,
}

impl AdsrPreviewScope {
    pub(super) fn new(
        cx: &mut Context,
        env_idx: usize,
        params: Arc<JSynPluginParams>,
        mod_ui: Arc<ModUiState>,
    ) -> Handle<'_, Self> {
        Self {
            env_idx,
            params,
            mod_ui,
        }
        .build(cx, |_| {})
    }
}

impl View for AdsrPreviewScope {
    fn element(&self) -> Option<&'static str> {
        Some("adsr-scope")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 4.0 || bounds.h <= 4.0 {
            return;
        }
        let dpx = cx.scale_factor();
        let pal = super::theme::current_palette(cx);
        let (ar, ag, ab) = pal.mod_env_rgb();

        let mut bg = vg::Path::new();
        bg.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 4.0 * dpx);
        canvas.fill_path(&bg, &vg::Paint::color(super::theme::vg_color(&pal.scope_bg, 255)));
        let mut border = vg::Paint::color(super::theme::vg_color(&pal.border_mid, 255));
        border.set_line_width(1.0 * dpx);
        canvas.stroke_path(&bg, &border);

        let pad = 6.0 * dpx;
        let x0 = bounds.x + pad;
        let y0 = bounds.y + pad;
        let w = (bounds.w - 2.0 * pad).max(1.0);
        let h = (bounds.h - 2.0 * pad).max(1.0);
        let adsr = if self.env_idx == 0 {
            AdsrParams {
                attack_s: self.params.amp_attack.value(),
                decay_s: self.params.amp_decay.value(),
                sustain: self.params.amp_sustain.value(),
                release_s: self.params.amp_release.value(),
            }
        } else {
            self.params.mod_envs[self.env_idx - 1].to_dsp()
        };

        let samples = 72usize;
        let mut fill_path = vg::Path::new();
        let mut stroke_path = vg::Path::new();
        fill_path.move_to(x0, y0 + h);
        for i in 0..=samples {
            let t = i as f32 / samples as f32;
            let amp = evaluate_adsr_preview(&adsr, t);
            let px = x0 + t * w;
            let py = y0 + (1.0 - amp) * h;
            fill_path.line_to(px, py);
            if i == 0 {
                stroke_path.move_to(px, py);
            } else {
                stroke_path.line_to(px, py);
            }
        }
        fill_path.line_to(x0 + w, y0 + h);
        fill_path.close();

        let fill_paint = vg::Paint::linear_gradient(
            x0,
            y0,
            x0,
            y0 + h,
            vg::Color::rgba(ar, ag, ab, 95),
            vg::Color::rgba(ar, ag, ab, 14),
        );
        canvas.fill_path(&fill_path, &fill_paint);

        let mut stroke_paint = vg::Paint::color(vg::Color::rgb(ar, ag, ab));
        stroke_paint.set_line_width(2.0 * dpx);
        stroke_paint.set_line_cap(vg::LineCap::Round);
        canvas.stroke_path(&stroke_path, &stroke_paint);

        // Live envelope output level bar on right edge
        let live_lvl = self.mod_ui.telemetry.env_level(self.env_idx).clamp(0.0, 1.0);
        if live_lvl > 0.005 {
            let meter_h = live_lvl * h;
            let mut bar = vg::Path::new();
            bar.rounded_rect(x0 + w - 4.0 * dpx, y0 + h - meter_h, 4.0 * dpx, meter_h, 1.5 * dpx);
            canvas.fill_path(&bar, &vg::Paint::color(vg::Color::rgba(ar, ag, ab, 230)));
        }
    }
}

