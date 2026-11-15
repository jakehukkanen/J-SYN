//! `MODSEQ` bottom drawer: 4 hybrid 16-step Performer / MSEG curve sequencers (`MSEQ 1`–`MSEQ 4`)
//! and 4 modulation envelopes (`ENV 1`–`ENV 4`) with interactive `femtovg` curves and live playheads.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModMatrixSource;
use j_syn_dsp::{
    evaluate_adsr_preview, evaluate_modseq_curve, evaluate_mseg_curve, AdsrParams, ModSeqStep,
    ModSeqStepShape, MsegNode, MAX_MODSEQ_STEPS, MAX_MSEG_NODES,
};
use nih_plug::prelude::{FloatParam, GuiContext, Param};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::util::ModifiersExt;

use super::controls::{
    bold_label, build_enum_dropdown, build_flex_pill_cell, build_knob_cell,
    build_knob_cell_with_readout,
};
use super::mod_bar::ModUiState;
use super::{UiData, UiEvent, CORAL_RGB, GOLD_RGB};
use crate::{
    JSynPluginParams, ModSeqModeParam, ModSeqPlayParam, ModSeqRateParam, ModSeqRetrigParam,
    ModSeqSyncParam,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModSeqSubTab {
    MSeq(usize),
    Env(usize),
}

impl Data for ModSeqSubTab {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

const MODE_ITEMS: &[(&str, ModSeqModeParam)] = &[
    ("Performer", ModSeqModeParam::Performer),
    ("MSEG", ModSeqModeParam::Mseg),
];

const SYNC_ITEMS: &[(&str, ModSeqSyncParam)] = &[
    ("Sync", ModSeqSyncParam::TempoSync),
    ("Hz", ModSeqSyncParam::FreeHz),
];

const RATE_ITEMS: &[(&str, ModSeqRateParam)] = &[
    ("1/32", ModSeqRateParam::Div1_32),
    ("1/16", ModSeqRateParam::Div1_16),
    ("1/8", ModSeqRateParam::Div1_8),
    ("1/4", ModSeqRateParam::Div1_4),
    ("1/2", ModSeqRateParam::Div1_2),
    ("1 Bar", ModSeqRateParam::Bars1),
    ("2 Bars", ModSeqRateParam::Bars2),
    ("4 Bars", ModSeqRateParam::Bars4),
];

const PLAY_ITEMS: &[(&str, ModSeqPlayParam)] = &[
    ("Loop", ModSeqPlayParam::Loop),
    ("1-Shot", ModSeqPlayParam::OneShot),
    ("PingPong", ModSeqPlayParam::PingPong),
];

const RETRIG_ITEMS: &[(&str, ModSeqRetrigParam)] = &[
    ("Poly", ModSeqRetrigParam::PolyVoice),
    ("Global", ModSeqRetrigParam::MonoGlobal),
];

fn subtab_accent(sub: ModSeqSubTab) -> (u8, u8, u8) {
    let src = match sub {
        ModSeqSubTab::MSeq(0) => ModMatrixSource::ModSeq1,
        ModSeqSubTab::MSeq(1) => ModMatrixSource::ModSeq2,
        ModSeqSubTab::MSeq(2) => ModMatrixSource::ModSeq3,
        ModSeqSubTab::MSeq(_) => ModMatrixSource::ModSeq4,
        ModSeqSubTab::Env(0) => ModMatrixSource::Env1,
        ModSeqSubTab::Env(1) => ModMatrixSource::Env2,
        ModSeqSubTab::Env(2) => ModMatrixSource::Env3,
        ModSeqSubTab::Env(_) => ModMatrixSource::Env4,
    };
    src.color_rgb()
}

/// Interactive `femtovg` Performer/MSEG curve canvas with real-time playhead.
struct ModSeqCurveCanvas {
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
    fn new(
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
        for i in 0..n {
            let px = x0 + mseg.nodes[i].time_x * w;
            let py = y0 + (1.0 - mseg.nodes[i].level_y) * h;
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

    fn apply_drag_edit(&self, x: f32, y: f32, bounds: BoundingBox, mode: CanvasDragMode, scale: f32) {
        let mut bank = self.params.modseq_steps.snapshot();
        match mode {
            CanvasDragMode::StampLevel => {
                let num_steps = self.params.modseqs[self.seq_idx]
                    .num_steps
                    .value()
                    .clamp(2, MAX_MODSEQ_STEPS as i32) as usize;
                let (hovered_step, level) = self.cursor_step_and_level(bounds, x, y, num_steps);
                let shape = self.mod_ui.active_stamp();
                let keep_tension = bank.seqs[self.seq_idx][hovered_step].tension;
                bank.seqs[self.seq_idx][hovered_step] =
                    ModSeqStep::stamped(shape, level, keep_tension);
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
                let (nx, ny) = Self::cursor_norm_xy(bounds, x, y, scale);
                let mseg = &mut bank.msegs[self.seq_idx];
                let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
                let idx = node_idx.min(n - 1);
                mseg.nodes[idx].level_y = ny;
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
                        self.apply_drag_edit(mx, my, bounds, mode, scale);
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
                self.apply_drag_edit(mx, my, bounds, mode, scale);
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
                        let (nx, ny) = Self::cursor_norm_xy(bounds, mx, my, scale);
                        let tx = nx.clamp(0.02, 0.98);
                        let mut insert_at = n - 1;
                        for i in 1..n {
                            if tx < mseg.nodes[i].time_x {
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
                                level_y: ny,
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
                    self.apply_drag_edit(*x, *y, bounds, mode, cx.scale_factor());
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
        let (ar, ag, ab) = subtab_accent(ModSeqSubTab::MSeq(self.seq_idx));

        let mut bg = vg::Path::new();
        bg.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 4.0 * dpx);
        canvas.fill_path(&bg, &vg::Paint::color(vg::Color::rgb(26, 21, 18)));
        let mut border = vg::Paint::color(vg::Color::rgb(73, 62, 54));
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

        // Subtle grid
        let mut grid_paint = vg::Paint::color(vg::Color::rgba(90, 77, 66, 95));
        grid_paint.set_line_width(0.8 * dpx);
        for g in 1..4 {
            let gy = y0 + (g as f32 / 4.0) * h;
            let mut line = vg::Path::new();
            line.move_to(x0, gy);
            line.line_to(x0 + w, gy);
            canvas.stroke_path(&line, &grid_paint);
        }
        let v_divs = if is_mseg { 4 } else { num_steps };
        for s in 1..v_divs {
            let gx = x0 + (s as f32 / v_divs as f32) * w;
            let mut line = vg::Path::new();
            line.move_to(gx, y0);
            line.line_to(gx, y0 + h);
            canvas.stroke_path(&line, &grid_paint);
        }

        // Filled + stroked continuous curve
        let samples = if is_mseg {
            128
        } else {
            (num_steps * 12).max(64)
        };
        let mut fill_path = vg::Path::new();
        let mut stroke_path = vg::Path::new();
        fill_path.move_to(x0, y0 + h);
        for i in 0..=samples {
            let t = (i as f32 / samples as f32).min(0.999_999);
            let val = if is_mseg {
                evaluate_mseg_curve(mseg, t)
            } else {
                evaluate_modseq_curve(steps, num_steps, t)
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
            for i in 0..n {
                let nx = x0 + mseg.nodes[i].time_x * w;
                let ny = y0 + (1.0 - mseg.nodes[i].level_y) * h;
                let mut ring = vg::Path::new();
                ring.circle(nx, ny, 3.6 * dpx);
                canvas.fill_path(&ring, &vg::Paint::color(vg::Color::rgb(ar, ag, ab)));
                let mut inner = vg::Path::new();
                inner.circle(nx, ny, 2.0 * dpx);
                canvas.fill_path(&inner, &vg::Paint::color(vg::Color::rgb(255, 248, 236)));
            }
        } else {
            for s in 0..num_steps {
                let sx0 = x0 + (s as f32 / num_steps as f32) * w;
                let sx1 = x0 + ((s + 1) as f32 / num_steps as f32) * w;
                let sy0 = y0 + (1.0 - steps[s].start_level) * h;
                let sy1 = y0 + (1.0 - steps[s].end_level) * h;
                let mut dot = vg::Path::new();
                dot.circle(sx0 + 1.5 * dpx, sy0, 2.0 * dpx);
                if steps[s].shape == ModSeqStepShape::Linear && (sy1 - sy0).abs() > 2.0 * dpx {
                    dot.circle(sx1 - 1.5 * dpx, sy1, 1.8 * dpx);
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
            evaluate_modseq_curve(steps, num_steps, phase.min(0.999_999))
        };
        let mut ph_dot = vg::Path::new();
        ph_dot.circle(ph_x, y0 + (1.0 - cur_val) * h, 3.0 * dpx);
        canvas.fill_path(&ph_dot, &vg::Paint::color(vg::Color::rgb(ar, ag, ab)));
        let mut ph_ring = vg::Paint::color(vg::Color::rgb(255, 248, 232));
        ph_ring.set_line_width(1.1 * dpx);
        canvas.stroke_path(&ph_dot, &ph_ring);
    }
}

/// Live `femtovg` ADSR shape preview canvas for `ENV 1` – `ENV 4`.
struct AdsrPreviewScope {
    env_idx: usize,
    params: Arc<JSynPluginParams>,
    mod_ui: Arc<ModUiState>,
}

impl AdsrPreviewScope {
    fn new(
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
        let (ar, ag, ab) = subtab_accent(ModSeqSubTab::Env(self.env_idx));

        let mut bg = vg::Path::new();
        bg.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 4.0 * dpx);
        canvas.fill_path(&bg, &vg::Paint::color(vg::Color::rgb(26, 21, 18)));
        let mut border = vg::Paint::color(vg::Color::rgb(73, 62, 54));
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

fn build_subtab_button(cx: &mut Context, sub: ModSeqSubTab, title: &'static str) {
    Button::new(
        cx,
        move |cx| cx.emit(UiEvent::SelectModSeqSubTab(sub)),
        move |cx| bold_label(cx, title, "subtab-btn-label").hoverable(false),
    )
    .class("subtab-btn")
    .checked(UiData::modseq_subtab.map(move |cur| *cur == sub))
    .width(Pixels(45.0))
    .height(Pixels(18.0));
}

fn build_labeled_dropdown<E, P, FMap>(
    cx: &mut Context,
    title: &'static str,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    map_fn: FMap,
    items: &'static [(&'static str, E)],
) where
    E: nih_plug::prelude::Enum + Copy + PartialEq + Send + Sync + 'static,
    P: nih_plug::prelude::Param<Plain = E> + 'static,
    FMap: Fn(&Arc<JSynPluginParams>) -> &P + Copy + Send + Sync + 'static,
{
    VStack::new(cx, move |cx| {
        bold_label(cx, title, "mini-label")
            .width(Stretch(1.0))
            .height(Pixels(9.0))
            .child_space(Stretch(1.0));
        build_enum_dropdown(cx, params.clone(), gui_context.clone(), map_fn, items)
            .width(Stretch(1.0))
            .height(Pixels(18.0));
    })
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(0.0));
}

fn env_param_fns(
    env_idx: usize,
) -> (
    fn(&Arc<JSynPluginParams>) -> &FloatParam,
    fn(&Arc<JSynPluginParams>) -> &FloatParam,
    fn(&Arc<JSynPluginParams>) -> &FloatParam,
    fn(&Arc<JSynPluginParams>) -> &FloatParam,
) {
    match env_idx {
        0 => (
            |p| &p.amp_attack,
            |p| &p.amp_decay,
            |p| &p.amp_sustain,
            |p| &p.amp_release,
        ),
        1 => (
            |p| &p.mod_envs[0].attack,
            |p| &p.mod_envs[0].decay,
            |p| &p.mod_envs[0].sustain,
            |p| &p.mod_envs[0].release,
        ),
        2 => (
            |p| &p.mod_envs[1].attack,
            |p| &p.mod_envs[1].decay,
            |p| &p.mod_envs[1].sustain,
            |p| &p.mod_envs[1].release,
        ),
        _ => (
            |p| &p.mod_envs[2].attack,
            |p| &p.mod_envs[2].decay,
            |p| &p.mod_envs[2].sustain,
            |p| &p.mod_envs[2].release,
        ),
    }
}

pub(super) fn build_modseq_tab(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    VStack::new(cx, move |cx| {
        // Top sub-selector bar (MSEQ 1..4 / ENV 1..4 + Performer stamps or MSEG actions)
        HStack::new(cx, {
            let params_top = params.clone();
            let mod_ui_top = mod_ui.clone();
            move |cx| {
                build_subtab_button(cx, ModSeqSubTab::MSeq(0), "MSEQ 1");
                build_subtab_button(cx, ModSeqSubTab::MSeq(1), "MSEQ 2");
                build_subtab_button(cx, ModSeqSubTab::MSeq(2), "MSEQ 3");
                build_subtab_button(cx, ModSeqSubTab::MSeq(3), "MSEQ 4");
                build_subtab_button(cx, ModSeqSubTab::Env(0), "ENV 1");
                build_subtab_button(cx, ModSeqSubTab::Env(1), "ENV 2");
                build_subtab_button(cx, ModSeqSubTab::Env(2), "ENV 3");
                build_subtab_button(cx, ModSeqSubTab::Env(3), "ENV 4");

                Element::new(cx).width(Stretch(1.0));

                let params_hdr = params_top.clone();
                let ui_hdr = mod_ui_top.clone();
                Binding::new(cx, UiData::modseq_subtab, move |cx, sub| {
                    let p_inner = params_hdr.clone();
                    let u_inner = ui_hdr.clone();
                    match sub.get(cx) {
                        ModSeqSubTab::MSeq(seq_idx) => {
                            HStack::new(
                                cx,
                                ParamWidgetBase::build_view(
                                    UiData::params,
                                    move |pr| &pr.modseqs[seq_idx].mode,
                                    move |cx, param_data| {
                                        let is_mseg = param_data.make_lens(|p| {
                                            p.unmodulated_plain_value() == ModSeqModeParam::Mseg
                                        });
                                        let p_m = p_inner.clone();
                                        let u_m = u_inner.clone();
                                        Binding::new(cx, is_mseg, move |cx, m| {
                                            let p_mode = p_m.clone();
                                            let u_mode = u_m.clone();
                                            if m.get(cx) {
                                                HStack::new(cx, move |cx| {
                                                    bold_label(
                                                        cx,
                                                        "MSEG: DBL-CLICK ADD/DEL NODE  |  DRAG MID/R-CLICK CURVE",
                                                        "mini-label",
                                                    )
                                                    .width(Auto)
                                                    .height(Stretch(1.0))
                                                    .child_top(Stretch(1.0))
                                                    .child_bottom(Stretch(1.0));

                                                    let p_rst = p_mode.clone();
                                                    Button::new(
                                                        cx,
                                                        move |cx| {
                                                            let mut bank =
                                                                p_rst.modseq_steps.snapshot();
                                                            bank.msegs[seq_idx] =
                                                                j_syn_dsp::MsegCurve::default();
                                                            p_rst.modseq_steps.store_value(bank);
                                                            cx.emit(UiEvent::ModUiChanged);
                                                            cx.needs_redraw();
                                                        },
                                                        |cx| {
                                                            bold_label(cx, "RESET", "preset-btn-label")
                                                                .child_space(Stretch(1.0))
                                                                .hoverable(false)
                                                        },
                                                    )
                                                    .class("preset-btn")
                                                    .width(Pixels(48.0))
                                                    .height(Pixels(18.0));
                                                })
                                                .col_between(Pixels(6.0))
                                                .width(Auto)
                                                .height(Pixels(18.0));
                                            } else {
                                                HStack::new(cx, move |cx| {
                                                    bold_label(cx, "STAMP", "mini-label")
                                                        .width(Pixels(38.0))
                                                        .height(Stretch(1.0))
                                                        .child_space(Stretch(1.0));

                                                    for &shape in &ModSeqStepShape::STAMPS {
                                                        let u_sel = u_mode.clone();
                                                        Button::new(
                                                            cx,
                                                            move |cx| {
                                                                u_sel.set_active_stamp(shape);
                                                                cx.emit(UiEvent::ModUiChanged);
                                                                cx.needs_redraw();
                                                            },
                                                            move |cx| {
                                                                bold_label(
                                                                    cx,
                                                                    shape.label(),
                                                                    "preset-btn-label",
                                                                )
                                                                .child_space(Stretch(1.0))
                                                                .hoverable(false)
                                                            },
                                                        )
                                                        .class("preset-btn")
                                                        .checked(UiData::mod_ui_rev.map({
                                                            let u_chk = u_mode.clone();
                                                            move |_| u_chk.active_stamp() == shape
                                                        }))
                                                        .width(Pixels(46.0))
                                                        .height(Pixels(18.0));
                                                    }

                                                    let p_fill = p_mode.clone();
                                                    let u_fill = u_mode.clone();
                                                    Button::new(
                                                        cx,
                                                        move |cx| {
                                                            let shape = u_fill.active_stamp();
                                                            let n = p_fill.modseqs[seq_idx]
                                                                .num_steps
                                                                .value()
                                                                .clamp(2, MAX_MODSEQ_STEPS as i32)
                                                                as usize;
                                                            let mut bank =
                                                                p_fill.modseq_steps.snapshot();
                                                            for i in 0..n {
                                                                bank.seqs[seq_idx][i] =
                                                                    ModSeqStep::stamped(
                                                                        shape, 1.0, 0.0,
                                                                    );
                                                            }
                                                            p_fill.modseq_steps.store_value(bank);
                                                            cx.needs_redraw();
                                                        },
                                                        |cx| {
                                                            bold_label(cx, "FILL", "preset-btn-label")
                                                                .child_space(Stretch(1.0))
                                                                .hoverable(false)
                                                        },
                                                    )
                                                    .class("preset-btn")
                                                    .width(Pixels(36.0))
                                                    .height(Pixels(18.0));
                                                })
                                                .col_between(Pixels(3.0))
                                                .width(Auto)
                                                .height(Pixels(18.0));
                                            }
                                        });
                                    },
                                ),
                            )
                            .width(Auto)
                            .height(Pixels(18.0));
                        }
                        ModSeqSubTab::Env(env_idx) => {
                            let env_num = env_idx + 1;
                            let hint = if env_idx == 0 {
                                String::from("ENV 1 (AMP) ADSR  |  DRAG E1 PILL TO MODULATE")
                            } else {
                                format!("ENV {env_num} ADSR  |  DRAG E{env_num} PILL TO MODULATE")
                            };
                            bold_label(cx, &hint, "mini-label")
                                .width(Auto)
                                .height(Pixels(18.0))
                                .child_top(Stretch(1.0))
                                .child_bottom(Stretch(1.0));
                        }
                    }
                });
            }
        })
        .col_between(Pixels(3.0))
        .width(Stretch(1.0))
        .height(Pixels(18.0));

        // Main subtab body
        let params_body = params.clone();
        let gui_ctx_body = gui_context.clone();
        let mod_ui_body = mod_ui.clone();
        Binding::new(cx, UiData::modseq_subtab, move |cx, sub| {
            let p_sub = params_body.clone();
            let g_sub = gui_ctx_body.clone();
            let m_sub = mod_ui_body.clone();
            match sub.get(cx) {
                ModSeqSubTab::MSeq(seq_idx) => {
                    HStack::new(cx, move |cx| {
                        ModSeqCurveCanvas::new(cx, seq_idx, p_sub.clone(), m_sub.clone())
                            .width(Stretch(1.0))
                            .height(Stretch(1.0));

                        HStack::new(cx, {
                            let p_ctrl = p_sub.clone();
                            let g_ctrl = g_sub.clone();
                            let accent = subtab_accent(ModSeqSubTab::MSeq(seq_idx));
                            move |cx| {
                                VStack::new(cx, {
                                    let p = p_ctrl.clone();
                                    let g = g_ctrl.clone();
                                    move |cx| {
                                        build_labeled_dropdown(
                                            cx,
                                            "MODE",
                                            p.clone(),
                                            g.clone(),
                                            move |pr| &pr.modseqs[seq_idx].mode,
                                            MODE_ITEMS,
                                        );
                                        build_labeled_dropdown(
                                            cx,
                                            "PLAY",
                                            p,
                                            g,
                                            move |pr| &pr.modseqs[seq_idx].play_mode,
                                            PLAY_ITEMS,
                                        );
                                    }
                                })
                                .row_between(Pixels(2.0))
                                .width(Pixels(78.0))
                                .height(Auto)
                                .top(Stretch(1.0))
                                .bottom(Stretch(1.0));

                                VStack::new(cx, {
                                    let p = p_ctrl.clone();
                                    let g = g_ctrl.clone();
                                    move |cx| {
                                        build_labeled_dropdown(
                                            cx,
                                            "SYNC",
                                            p.clone(),
                                            g.clone(),
                                            move |pr| &pr.modseqs[seq_idx].sync_mode,
                                            SYNC_ITEMS,
                                        );
                                        build_labeled_dropdown(
                                            cx,
                                            "RETRIG",
                                            p,
                                            g,
                                            move |pr| &pr.modseqs[seq_idx].retrig,
                                            RETRIG_ITEMS,
                                        );
                                    }
                                })
                                .row_between(Pixels(2.0))
                                .width(Pixels(70.0))
                                .height(Auto)
                                .top(Stretch(1.0))
                                .bottom(Stretch(1.0));

                                VStack::new(cx, {
                                    let p = p_ctrl.clone();
                                    let g = g_ctrl.clone();
                                    move |cx| {
                                        build_labeled_dropdown(
                                            cx,
                                            "RATE",
                                            p.clone(),
                                            g.clone(),
                                            move |pr| &pr.modseqs[seq_idx].sync_rate,
                                            RATE_ITEMS,
                                        );
                                        let p_steps = p.clone();
                                        HStack::new(
                                            cx,
                                            ParamWidgetBase::build_view(
                                                UiData::params,
                                                move |pr| &pr.modseqs[seq_idx].mode,
                                                move |cx, param_data| {
                                                    let is_mseg = param_data.make_lens(|pr| {
                                                        pr.unmodulated_plain_value()
                                                            == ModSeqModeParam::Mseg
                                                    });
                                                    let p_s = p_steps.clone();
                                                    Binding::new(cx, is_mseg, move |cx, m| {
                                                        if m.get(cx) {
                                                            let p_n = p_s.clone();
                                                            Binding::new(
                                                                cx,
                                                                UiData::mod_ui_rev,
                                                                move |cx, _| {
                                                                    let n = p_n
                                                                        .modseq_steps
                                                                        .snapshot()
                                                                        .msegs[seq_idx]
                                                                        .num_nodes;
                                                                    let txt =
                                                                        format!("NODES: {n}/16");
                                                                    bold_label(
                                                                        cx,
                                                                        &txt,
                                                                        "mini-label",
                                                                    )
                                                                    .width(Stretch(1.0))
                                                                    .height(Pixels(24.0))
                                                                    .child_space(Stretch(1.0));
                                                                },
                                                            );
                                                        } else {
                                                            build_flex_pill_cell(
                                                                cx,
                                                                UiData::params,
                                                                move |pr| {
                                                                    &pr.modseqs[seq_idx].num_steps
                                                                },
                                                                "STEPS",
                                                                14,
                                                            );
                                                        }
                                                    });
                                                },
                                            ),
                                        )
                                        .width(Stretch(1.0))
                                        .height(Pixels(28.0));
                                    }
                                })
                                .row_between(Pixels(2.0))
                                .width(Pixels(70.0))
                                .height(Auto)
                                .top(Stretch(1.0))
                                .bottom(Stretch(1.0));

                                build_knob_cell(
                                    cx,
                                    UiData::params,
                                    move |pr| &pr.modseqs[seq_idx].free_hz,
                                    "FREQ",
                                    false,
                                    accent,
                                    32.0,
                                    52.0,
                                );

                                build_knob_cell(
                                    cx,
                                    UiData::params,
                                    move |pr| &pr.modseqs[seq_idx].smooth,
                                    "SMOOTH",
                                    false,
                                    CORAL_RGB,
                                    32.0,
                                    52.0,
                                );
                            }
                        })
                        .col_between(Pixels(5.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0))
                        .width(Pixels(342.0))
                        .height(Stretch(1.0));
                    })
                    .col_between(Pixels(8.0))
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
                }
                ModSeqSubTab::Env(env_idx) => {
                    let (atk_fn, dec_fn, sus_fn, rel_fn) = env_param_fns(env_idx);
                    HStack::new(cx, move |cx| {
                        AdsrPreviewScope::new(cx, env_idx, p_sub.clone(), m_sub.clone())
                            .width(Stretch(1.35))
                            .height(Stretch(1.0));

                        HStack::new(cx, move |cx| {
                            let accent = subtab_accent(ModSeqSubTab::Env(env_idx));
                            build_knob_cell_with_readout(
                                cx,
                                UiData::params,
                                atk_fn,
                                "ATTACK",
                                false,
                                accent,
                                30.0,
                                68.0,
                            );
                            build_knob_cell_with_readout(
                                cx,
                                UiData::params,
                                dec_fn,
                                "DECAY",
                                false,
                                accent,
                                30.0,
                                68.0,
                            );
                            build_knob_cell_with_readout(
                                cx,
                                UiData::params,
                                sus_fn,
                                "SUSTAIN",
                                false,
                                GOLD_RGB,
                                30.0,
                                68.0,
                            );
                            build_knob_cell_with_readout(
                                cx,
                                UiData::params,
                                rel_fn,
                                "RELEASE",
                                false,
                                CORAL_RGB,
                                30.0,
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
                    .col_between(Pixels(10.0))
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
                }
            }
        });
    })
    .class("hardware-module")
    .class("drawer-module")
    .row_between(Pixels(2.0))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}
