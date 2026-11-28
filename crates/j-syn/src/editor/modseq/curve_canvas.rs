//! Interactive `femtovg` curve editor for `MODSEQ` (`2×16` Performer rows with `4 px` separator
//! and continuous `MSEG` / `LFO` breakpoint mode).

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::{
    MAX_MODSEQ_STEPS, MODSEQ_ROW_STEPS, ModSeqRowMode, ModSeqRowStart, ModSeqStep,
    ModSeqStepShape, ModSeqStepSpan, apply_groove_swing_tension, evaluate_step_shape,
};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::util::ModifiersExt;

use super::super::mod_bar::ModUiState;
use super::super::{UiEvent, theme, tokens};
use super::mseg_canvas::{self, MsegHover};
use crate::{JSynPluginParams, ModSeqModeParam};

pub(super) struct ModSeqCurveCanvas {
    seq_idx: usize,
    params: Arc<JSynPluginParams>,
    mod_ui: Arc<ModUiState>,
    drag_mode: Option<CanvasDragMode>,
    mseg_hover: Option<MsegHover>,
    bold_font: std::cell::Cell<Option<vg::FontId>>,
}

#[derive(Debug, Clone, Copy)]
enum CanvasDragMode {
    StampLevel {
        row: usize,
        anchor_col: usize,
    },
    EndLevel {
        step_idx: usize,
    },
    StepTension {
        step_idx: usize,
        start_y: f32,
        start_tension: f32,
    },
    MsegNode {
        node_idx: usize,
    },
    MsegTension {
        seg_idx: usize,
    },
    MsegTensionRelative {
        seg_idx: usize,
        start_y: f32,
        start_tension: f32,
    },
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
            mseg_hover: None,
            bold_font: std::cell::Cell::new(None),
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

    /// Computes `(row1_rect, row2_rect)` for the locked `2 × 16` Performer grid separated by `MODSEQ_ROW_SEP_H`.
    /// Note: if testing dynamic width-stretching for `STEPS <= 16` later, Row 1 column count can be `num_steps.min(16)`.
    fn row_rects(bounds: BoundingBox, scale: f32) -> ((f32, f32, f32, f32), (f32, f32, f32, f32)) {
        let s = scale.max(1.0);
        let (x0, y0, w, total_h) = Self::inner_rect(bounds, s);
        let sep = tokens::MODSEQ_ROW_SEP_H * s;
        let row_h = ((total_h - sep) * 0.5).max(1.0);
        ((x0, y0, w, row_h), (x0, y0 + row_h + sep, w, row_h))
    }

    fn level_for_row(bounds: BoundingBox, row: usize, y: f32, scale: f32) -> f32 {
        let (r1, r2) = Self::row_rects(bounds, scale);
        let (_, ry, _, rh) = if row == 0 { r1 } else { r2 };
        1.0 - ((y - ry) / rh).clamp(0.0, 1.0)
    }

    fn col_for_x(bounds: BoundingBox, x: f32, scale: f32) -> usize {
        let (x0, _, w, _) = Self::inner_rect(bounds, scale);
        let nx = ((x - x0) / w).clamp(0.0, 0.999_9);
        ((nx * (MODSEQ_ROW_STEPS as f32)).floor() as usize).min(MODSEQ_ROW_STEPS - 1)
    }

    fn cursor_row_col_level(
        bounds: BoundingBox,
        x: f32,
        y: f32,
        scale: f32,
    ) -> (usize, usize, f32) {
        let s = scale.max(1.0);
        let (r1, r2) = Self::row_rects(bounds, s);
        let sep = tokens::MODSEQ_ROW_SEP_H * s;
        let row = if y >= r2.1 - sep * 0.5 { 1 } else { 0 };
        let (rx, ry, rw, rh) = if row == 0 { r1 } else { r2 };
        let nx = ((x - rx) / rw).clamp(0.0, 0.999_9);
        let ny = 1.0 - ((y - ry) / rh).clamp(0.0, 1.0);
        let col = ((nx * (MODSEQ_ROW_STEPS as f32)).floor() as usize).min(MODSEQ_ROW_STEPS - 1);
        (row, col, ny)
    }

    fn cursor_norm_xy(bounds: BoundingBox, x: f32, y: f32, scale: f32) -> (f32, f32) {
        let (x0, y0, w, h) = Self::inner_rect(bounds, scale);
        let nx = ((x - x0) / w).clamp(0.0, 1.0);
        let ny = (1.0 - (y - y0) / h).clamp(0.0, 1.0);
        (nx, ny)
    }

    fn apply_drag_edit(
        &self,
        x: f32,
        y: f32,
        bounds: BoundingBox,
        mode: CanvasDragMode,
        scale: f32,
        shift_held: bool,
        alt_held: bool,
    ) {
        let mut bank = self.params.modseq_steps.snapshot();
        match mode {
            CanvasDragMode::StampLevel { row, anchor_col } => {
                let col = Self::col_for_x(bounds, x, scale);
                let level = Self::level_for_row(bounds, row, y, scale);
                self.mod_ui.set_active_modseq_row(self.seq_idx, row);
                let tile = self.mod_ui.active_tile();
                let swing = self.params.modseq_swing.value();
                let spans: &[ModSeqStepSpan] = match tile.stamp_steps {
                    4 => &[
                        ModSeqStepSpan::Quarter1,
                        ModSeqStepSpan::Quarter2,
                        ModSeqStepSpan::Quarter3,
                        ModSeqStepSpan::Quarter4,
                    ],
                    2 => &[ModSeqStepSpan::HalfFirst, ModSeqStepSpan::HalfSecond],
                    _ => &[tile.span],
                };
                let span_n = spans.len();
                let stride = span_n as isize;
                let delta_groups = (col as isize - anchor_col as isize).div_euclid(stride);
                let c0 = (anchor_col as isize + delta_groups * stride)
                    .clamp(0, (MODSEQ_ROW_STEPS - span_n) as isize)
                    as usize;
                let s0 = row * MODSEQ_ROW_STEPS + c0;
                for (offset, &span) in spans.iter().enumerate() {
                    let s = s0 + offset;
                    let tension = if swing {
                        bank.seqs[self.seq_idx][s].tension
                    } else {
                        0.0
                    };
                    bank.seqs[self.seq_idx][s] =
                        ModSeqStep::stamped_with_span(tile.shape, level, tension, span);
                }
            }
            CanvasDragMode::EndLevel { step_idx } => {
                let row = (step_idx / MODSEQ_ROW_STEPS).min(1);
                let level = Self::level_for_row(bounds, row, y, scale);
                let s = &mut bank.seqs[self.seq_idx][step_idx.min(MAX_MODSEQ_STEPS - 1)];
                s.shape = ModSeqStepShape::Linear;
                s.span = ModSeqStepSpan::Full;
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
                let (nx, ny) = Self::cursor_norm_xy(bounds, x, y, scale);
                let snap_on = self.mod_ui.mseg_snap();
                let snap_active = (snap_on && !alt_held) || (!snap_on && shift_held);
                mseg_canvas::apply_mseg_node_drag(
                    &mut bank.msegs[self.seq_idx],
                    node_idx,
                    nx,
                    ny,
                    snap_active,
                );
            }
            CanvasDragMode::MsegTension { seg_idx } => {
                let (_, ny) = Self::cursor_norm_xy(bounds, x, y, scale);
                mseg_canvas::apply_mseg_tension_direct(&mut bank.msegs[self.seq_idx], seg_idx, ny);
            }
            CanvasDragMode::MsegTensionRelative {
                seg_idx,
                start_y,
                start_tension,
            } => {
                mseg_canvas::apply_mseg_tension_relative(
                    &mut bank.msegs[self.seq_idx],
                    seg_idx,
                    start_y,
                    y,
                    scale,
                    start_tension,
                );
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
                    let rect = Self::inner_rect(bounds, scale);
                    let bank = self.params.modseq_steps.snapshot();
                    let mseg = &bank.msegs[self.seq_idx];
                    if let Some(node_idx) = mseg_canvas::hit_mseg_node(mseg, rect, mx, my, scale) {
                        self.drag_mode = Some(CanvasDragMode::MsegNode { node_idx });
                        cx.capture();
                        cx.needs_redraw();
                        meta.consume();
                        return;
                    }
                    if let Some(seg_idx) =
                        mseg_canvas::hit_mseg_tension_handle(mseg, rect, mx, my, scale)
                    {
                        self.drag_mode = Some(CanvasDragMode::MsegTension { seg_idx });
                        cx.capture();
                        cx.needs_redraw();
                        meta.consume();
                        return;
                    }
                    if cx.modifiers().contains(Modifiers::ALT) {
                        let (nx, _) = Self::cursor_norm_xy(bounds, mx, my, scale);
                        if let Some(seg_idx) = mseg_canvas::segment_index_for_x(mseg, nx) {
                            self.drag_mode = Some(CanvasDragMode::MsegTensionRelative {
                                seg_idx,
                                start_y: my,
                                start_tension: mseg.nodes[seg_idx].tension,
                            });
                            cx.capture();
                            cx.needs_redraw();
                            meta.consume();
                        }
                    }
                    return;
                }

                let (row, col, _lvl) = Self::cursor_row_col_level(bounds, mx, my, scale);
                self.mod_ui.set_active_modseq_row(self.seq_idx, row);
                let step_idx = row * MODSEQ_ROW_STEPS + col;
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
                    let span_n =
                        (self.mod_ui.active_tile().stamp_steps as usize).clamp(1, MODSEQ_ROW_STEPS);
                    CanvasDragMode::StampLevel {
                        row,
                        anchor_col: col.min(MODSEQ_ROW_STEPS - span_n),
                    }
                };
                self.drag_mode = Some(mode);
                self.apply_drag_edit(
                    mx,
                    my,
                    bounds,
                    mode,
                    scale,
                    cx.modifiers().shift(),
                    cx.modifiers().contains(Modifiers::ALT),
                );
                cx.capture();
                cx.needs_redraw();
                meta.consume();
            }
            WindowEvent::MouseDown(MouseButton::Right) => {
                let bounds = cx.bounds();
                let scale = cx.scale_factor();
                let mx = cx.mouse().cursorx;
                let my = cx.mouse().cursory;
                let mut bank = self.params.modseq_steps.snapshot();
                if self.is_mseg() {
                    let rect = Self::inner_rect(bounds, scale);
                    let mseg = &mut bank.msegs[self.seq_idx];
                    if let Some(hit) = mseg_canvas::hit_mseg_node(mseg, rect, mx, my, scale) {
                        if mseg_canvas::delete_mseg_node(mseg, hit) {
                            self.mseg_hover = None;
                            self.params.modseq_steps.store_value(bank);
                            cx.emit(UiEvent::ModUiChanged);
                            cx.needs_redraw();
                        }
                        meta.consume();
                        return;
                    }
                    if let Some(seg_idx) =
                        mseg_canvas::hit_mseg_tension_handle(mseg, rect, mx, my, scale)
                    {
                        mseg.nodes[seg_idx].tension = 0.0;
                        self.params.modseq_steps.store_value(bank);
                        cx.emit(UiEvent::ModUiChanged);
                        cx.needs_redraw();
                        meta.consume();
                        return;
                    }
                    let (nx, _) = Self::cursor_norm_xy(bounds, mx, my, scale);
                    if let Some(seg_idx) = mseg_canvas::segment_index_for_x(mseg, nx) {
                        self.drag_mode = Some(CanvasDragMode::MsegTensionRelative {
                            seg_idx,
                            start_y: my,
                            start_tension: mseg.nodes[seg_idx].tension,
                        });
                        cx.capture();
                    }
                    meta.consume();
                    return;
                } else {
                    let (row, col, _) = Self::cursor_row_col_level(bounds, mx, my, scale);
                    self.mod_ui.set_active_modseq_row(self.seq_idx, row);
                    let step_idx = row * MODSEQ_ROW_STEPS + col;
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
                    let rect = Self::inner_rect(bounds, scale);
                    let mut bank = self.params.modseq_steps.snapshot();
                    let mseg = &mut bank.msegs[self.seq_idx];
                    if let Some(hit) = mseg_canvas::hit_mseg_node(mseg, rect, mx, my, scale) {
                        if mseg_canvas::delete_mseg_node(mseg, hit) {
                            self.mseg_hover = None;
                            self.params.modseq_steps.store_value(bank);
                            cx.emit(UiEvent::ModUiChanged);
                            cx.needs_redraw();
                        }
                    } else if let Some(seg_idx) =
                        mseg_canvas::hit_mseg_tension_handle(mseg, rect, mx, my, scale)
                    {
                        mseg.nodes[seg_idx].tension = 0.0;
                        self.params.modseq_steps.store_value(bank);
                        cx.emit(UiEvent::ModUiChanged);
                        cx.needs_redraw();
                    } else {
                        let (raw_x, raw_y) = Self::cursor_norm_xy(bounds, mx, my, scale);
                        let snap_on = self.mod_ui.mseg_snap();
                        let snap_active = (snap_on && !cx.modifiers().contains(Modifiers::ALT))
                            || (!snap_on && cx.modifiers().shift());
                        if mseg_canvas::insert_mseg_node(mseg, raw_x, raw_y, snap_active) {
                            self.params.modseq_steps.store_value(bank);
                            cx.emit(UiEvent::ModUiChanged);
                            cx.needs_redraw();
                        }
                    }
                    meta.consume();
                    return;
                }

                let (row, col, _) = Self::cursor_row_col_level(bounds, mx, my, scale);
                self.mod_ui.set_active_modseq_row(self.seq_idx, row);
                let step_idx = row * MODSEQ_ROW_STEPS + col;
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
                        cx.modifiers().contains(Modifiers::ALT),
                    );
                    cx.needs_redraw();
                    meta.consume();
                } else if self.is_mseg() {
                    let bounds = cx.bounds();
                    let scale = cx.scale_factor();
                    let rect = Self::inner_rect(bounds, scale);
                    let bank = self.params.modseq_steps.snapshot();
                    let mseg = &bank.msegs[self.seq_idx];
                    let next_hover = mseg_canvas::hit_mseg_node(mseg, rect, *x, *y, scale)
                        .map(MsegHover::Node)
                        .or_else(|| {
                            mseg_canvas::hit_mseg_tension_handle(mseg, rect, *x, *y, scale)
                                .map(MsegHover::Tension)
                        });
                    if next_hover != self.mseg_hover {
                        self.mseg_hover = next_hover;
                        cx.needs_redraw();
                    }
                }
            }
            WindowEvent::MouseOut => {
                if self.mseg_hover.take().is_some() {
                    cx.needs_redraw();
                }
            }
            WindowEvent::MouseUp(MouseButton::Left) | WindowEvent::MouseUp(MouseButton::Right) => {
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
        let pal = theme::current_palette(cx);
        let (ar, ag, ab) = pal.mod_seq_rgb();

        let mut bg = vg::Path::new();
        bg.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 4.0 * dpx);
        canvas.fill_path(&bg, &vg::Paint::color(theme::vg_color(&pal.scope_bg, 255)));
        let mut border = vg::Paint::color(theme::vg_color(&pal.border_mid, 255));
        border.set_line_width(1.0 * dpx);
        canvas.stroke_path(&bg, &border);

        let bank = self.params.modseq_steps.snapshot();
        if self.is_mseg() {
            let active_hover = match self.drag_mode {
                Some(CanvasDragMode::MsegNode { node_idx }) => Some(MsegHover::Node(node_idx)),
                Some(CanvasDragMode::MsegTension { seg_idx })
                | Some(CanvasDragMode::MsegTensionRelative { seg_idx, .. }) => {
                    Some(MsegHover::Tension(seg_idx))
                }
                _ => self.mseg_hover,
            };
            mseg_canvas::draw_mseg(
                canvas,
                Self::inner_rect(bounds, dpx),
                dpx,
                &pal,
                (ar, ag, ab),
                &bank.msegs[self.seq_idx],
                self.mod_ui.mseg_snap(),
                self.mod_ui.telemetry.modseq_phase(self.seq_idx),
                active_hover,
            );
            return;
        }

        let num_steps = self.params.modseqs[self.seq_idx]
            .num_steps
            .value()
            .clamp(2, MAX_MODSEQ_STEPS as i32) as usize;
        let steps = &bank.seqs[self.seq_idx];
        let groove_swing = self.params.modseq_swing.value();

        let (r1, r2) = Self::row_rects(bounds, dpx);
        let sep_y = r1.1 + r1.3;
        let sep_h = (r2.1 - sep_y).max(1.0);
        let sep_x0 = bounds.x + 0.5 * dpx;
        let sep_w = (bounds.w - 1.0 * dpx).max(1.0);
        let mut sep_rect = vg::Path::new();
        sep_rect.rect(sep_x0, sep_y, sep_w, sep_h);
        canvas.fill_path(
            &sep_rect,
            &vg::Paint::color(theme::vg_color(&pal.panel_bg, 255)),
        );
        let mut sep_stroke = vg::Paint::color(theme::vg_color(&pal.border_light, 255));
        sep_stroke.set_line_width(1.0 * dpx);
        let mut sep_edges = vg::Path::new();
        sep_edges.move_to(sep_x0, sep_y);
        sep_edges.line_to(sep_x0 + sep_w, sep_y);
        sep_edges.move_to(sep_x0, sep_y + sep_h);
        sep_edges.line_to(sep_x0 + sep_w, sep_y + sep_h);
        canvas.stroke_path(&sep_edges, &sep_stroke);

        let start_row: ModSeqRowStart = self.params.modseqs[self.seq_idx].start_row.value().into();
        let row_mode: ModSeqRowMode = self.params.modseqs[self.seq_idx].row_mode.value().into();
        let raw_row_mod = self
            .mod_ui
            .telemetry
            .dest_offset(ModDestination::modseq_row(self.seq_idx));
        let eff_row_mod = if raw_row_mod.is_finite() {
            raw_row_mod
        } else {
            0.0
        };
        let base_row = if start_row == ModSeqRowStart::Row2 {
            1.0
        } else {
            0.0
        };
        let target_row = usize::from((base_row + eff_row_mod) >= 0.5);
        let active_row = self.mod_ui.active_modseq_row(self.seq_idx);
        let is_step_active = |s: usize| -> bool {
            let row = s / MODSEQ_ROW_STEPS;
            let col = s % MODSEQ_ROW_STEPS;
            match row_mode {
                ModSeqRowMode::LoopRow => {
                    row == target_row && col < num_steps.min(MODSEQ_ROW_STEPS)
                }
                ModSeqRowMode::Continue => {
                    if num_steps <= MODSEQ_ROW_STEPS {
                        row == target_row && col < num_steps
                    } else {
                        s < num_steps
                    }
                }
            }
        };

        let mut grid_paint = vg::Paint::color(theme::vg_color(&pal.scope_grid, 175));
        grid_paint.set_line_width(0.8 * dpx);
        let font_id = j_syn_ui::shell::ensure_canvas_bold_font(canvas, &self.bold_font);
        let mut stroke_paint = vg::Paint::color(vg::Color::rgb(ar, ag, ab));
        stroke_paint.set_line_width(1.7 * dpx);
        stroke_paint.set_line_cap(vg::LineCap::Round);
        stroke_paint.set_line_join(vg::LineJoin::Round);

        for row in 0..2 {
            let (rx, ry, rw, rh) = if row == 0 { r1 } else { r2 };
            let step_w = rw / MODSEQ_ROW_STEPS as f32;

            for g in 1..4 {
                let gy = ry + (g as f32 / 4.0) * rh;
                let mut line = vg::Path::new();
                line.move_to(rx, gy);
                line.line_to(rx + rw, gy);
                canvas.stroke_path(&line, &grid_paint);
            }
            for c in 1..MODSEQ_ROW_STEPS {
                let gx = rx + (c as f32) * step_w;
                let mut line = vg::Path::new();
                line.move_to(gx, ry);
                line.line_to(gx, ry + rh);
                canvas.stroke_path(&line, &grid_paint);
            }

            for col in 0..MODSEQ_ROW_STEPS {
                let s = row * MODSEQ_ROW_STEPS + col;
                let lvl = steps[s].start_level.clamp(0.0, 1.0);
                if lvl > 0.005 {
                    let bx = rx + col as f32 * step_w + 1.0 * dpx;
                    let bw = (step_w - 2.0 * dpx).max(1.0);
                    let bh = lvl * rh;
                    let mut bar = vg::Path::new();
                    bar.rounded_rect(bx, ry + rh - bh, bw, bh, 1.5 * dpx);
                    canvas.fill_path(&bar, &vg::Paint::color(vg::Color::rgba(ar, ag, ab, 20)));
                }
            }

            const SUB_SAMPLES: usize = 20;
            let mut fill_path = vg::Path::new();
            let mut stroke_path = vg::Path::new();
            fill_path.move_to(rx, ry + rh);
            let mut started = false;
            for col in 0..MODSEQ_ROW_STEPS {
                let s = row * MODSEQ_ROW_STEPS + col;
                let mut step = steps[s];
                step.tension = apply_groove_swing_tension(s, step.tension, groove_swing);
                let sx0 = rx + col as f32 * step_w;
                for k in 0..=SUB_SAMPLES {
                    let local_t = k as f32 / SUB_SAMPLES as f32;
                    let val = evaluate_step_shape(step, local_t);
                    let px = sx0 + local_t * step_w;
                    let py = ry + (1.0 - val) * rh;
                    fill_path.line_to(px, py);
                    if !started {
                        stroke_path.move_to(px, py);
                        started = true;
                    } else {
                        stroke_path.line_to(px, py);
                    }
                }
            }
            fill_path.line_to(rx + rw, ry + rh);
            fill_path.close();

            let fill_paint = vg::Paint::linear_gradient(
                rx,
                ry,
                rx,
                ry + rh,
                vg::Color::rgba(ar, ag, ab, 90),
                vg::Color::rgba(ar, ag, ab, 14),
            );
            canvas.fill_path(&fill_path, &fill_paint);
            canvas.stroke_path(&stroke_path, &stroke_paint);

            let dot_paint = vg::Paint::color(theme::vg_color(&pal.text_primary, 245));
            for col in 0..MODSEQ_ROW_STEPS {
                let s = row * MODSEQ_ROW_STEPS + col;
                let mut step = steps[s];
                step.tension = apply_groove_swing_tension(s, step.tension, groove_swing);
                let sx0 = rx + col as f32 * step_w;
                let sx1 = sx0 + step_w;
                let sy0 = ry + (1.0 - evaluate_step_shape(step, 0.0)) * rh;
                let sy1 = ry + (1.0 - evaluate_step_shape(step, 1.0)) * rh;
                let mut dot = vg::Path::new();
                if step.shape == ModSeqStepShape::Linear && (sy1 - sy0).abs() > 2.0 * dpx {
                    dot.circle(sx0 + 1.8 * dpx, sy0, 1.9 * dpx);
                    dot.circle(sx1 - 1.8 * dpx, sy1, 1.7 * dpx);
                } else {
                    let sy_mid = ry + (1.0 - evaluate_step_shape(step, 0.5)) * rh;
                    dot.circle(sx0 + 0.5 * step_w, sy_mid, 1.8 * dpx);
                }
                canvas.fill_path(&dot, &dot_paint);
            }

            for col in 0..MODSEQ_ROW_STEPS {
                let s = row * MODSEQ_ROW_STEPS + col;
                if !is_step_active(s) {
                    let mut dim = vg::Path::new();
                    dim.rect(rx + col as f32 * step_w, ry, step_w, rh);
                    canvas.fill_path(&dim, &vg::Paint::color(theme::vg_color(&pal.scope_bg, 150)));
                }
            }

            if active_row == row {
                let mut row_ring = vg::Path::new();
                row_ring.rounded_rect(rx, ry, rw, rh, 2.0 * dpx);
                let mut rp = vg::Paint::color(vg::Color::rgba(ar, ag, ab, 120));
                rp.set_line_width(1.1 * dpx);
                canvas.stroke_path(&row_ring, &rp);
            }
            if let Some(fid) = font_id {
                let mut tp = vg::Paint::color(if active_row == row {
                    vg::Color::rgba(ar, ag, ab, 220)
                } else {
                    theme::vg_color(&pal.text_dim, 160)
                });
                tp.set_font(&[fid]);
                tp.set_font_size(8.5 * dpx);
                tp.set_text_align(vg::Align::Left);
                tp.set_text_baseline(vg::Baseline::Top);
                let _ = canvas.fill_text(
                    rx + 3.0 * dpx,
                    ry + 2.0 * dpx,
                    if row == 0 { "R1" } else { "R2" },
                    &tp,
                );
            }
        }

        let disp = self
            .mod_ui
            .telemetry
            .modseq_display_phase(self.seq_idx)
            .clamp(0.0, 0.999_999);
        let ph_step_f = disp * MAX_MODSEQ_STEPS as f32;
        let ph_step = (ph_step_f.floor() as usize).min(MAX_MODSEQ_STEPS - 1);
        let ph_row = ph_step / MODSEQ_ROW_STEPS;
        let ph_col_norm = ((ph_step_f - (ph_row * MODSEQ_ROW_STEPS) as f32)
            / MODSEQ_ROW_STEPS as f32)
            .clamp(0.0, 1.0);
        let (rx, ry, rw, rh) = if ph_row == 0 { r1 } else { r2 };
        let ph_x = rx + ph_col_norm * rw;
        let mut ph_line = vg::Path::new();
        ph_line.move_to(ph_x, ry);
        ph_line.line_to(ph_x, ry + rh);
        let mut ph_paint = vg::Paint::color(theme::vg_color(&pal.text_primary, 210));
        ph_paint.set_line_width(1.4 * dpx);
        canvas.stroke_path(&ph_line, &ph_paint);

        let mut ph_step_val = steps[ph_step];
        ph_step_val.tension =
            apply_groove_swing_tension(ph_step, ph_step_val.tension, groove_swing);
        let cur_val = evaluate_step_shape(ph_step_val, ph_step_f.fract());
        let mut ph_dot = vg::Path::new();
        ph_dot.circle(ph_x, ry + (1.0 - cur_val) * rh, 2.8 * dpx);
        canvas.fill_path(&ph_dot, &vg::Paint::color(vg::Color::rgb(ar, ag, ab)));
        let mut ph_ring = vg::Paint::color(theme::vg_color(&pal.text_primary, 255));
        ph_ring.set_line_width(1.1 * dpx);
        canvas.stroke_path(&ph_dot, &ph_ring);
    }
}
