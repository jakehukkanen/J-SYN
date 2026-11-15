//! `MODSEQ` bottom drawer: 4 hybrid 16-step Performer / MSEG curve sequencers (`MSEQ 1`–`MSEQ 4`)
//! and 3 modulation envelopes (`ENV 2`–`ENV 4`) with interactive `femtovg` curves and live playheads.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModMatrixSource;
use j_syn_dsp::{
    evaluate_adsr_preview, evaluate_modseq_curve, ModSeqShapePreset, MAX_MODSEQ_STEPS,
};
use nih_plug::prelude::GuiContext;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::util::ModifiersExt;

use super::controls::{
    bold_label, build_enum_dropdown, build_flex_pill_cell, build_knob_cell,
};
use super::mod_bar::ModUiState;
use super::{UiData, UiEvent, CORAL_RGB, GOLD_RGB};
use crate::{
    JSynPluginParams, ModSeqPlayParam, ModSeqRateParam, ModSeqRetrigParam, ModSeqSyncParam,
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
        ModSeqSubTab::Env(0) => ModMatrixSource::Env2,
        ModSeqSubTab::Env(1) => ModMatrixSource::Env3,
        ModSeqSubTab::Env(_) => ModMatrixSource::Env4,
    };
    src.color_rgb()
}

/// Interactive `femtovg` Performer/MSEG 2–16 step curve canvas with real-time playhead.
struct ModSeqCurveCanvas {
    seq_idx: usize,
    params: Arc<JSynPluginParams>,
    mod_ui: Arc<ModUiState>,
    drag_mode: Option<StepDragMode>,
}

#[derive(Debug, Clone, Copy)]
enum StepDragMode {
    Level,
    EndLevel { step_idx: usize },
    Tension { step_idx: usize, start_y: f32, start_tension: f32 },
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

    fn cursor_step_and_level(&self, bounds: BoundingBox, x: f32, y: f32, num_steps: usize) -> (usize, f32) {
        let pad = 4.0;
        let inner_w = (bounds.w - 2.0 * pad).max(1.0);
        let inner_h = (bounds.h - 2.0 * pad).max(1.0);
        let nx = ((x - (bounds.x + pad)) / inner_w).clamp(0.0, 0.999_9);
        let ny = 1.0 - ((y - (bounds.y + pad)) / inner_h).clamp(0.0, 1.0);
        let step_idx = ((nx * (num_steps as f32)).floor() as usize).min(num_steps - 1);
        (step_idx, ny)
    }

    fn apply_step_edit(&self, x: f32, y: f32, bounds: BoundingBox, mode: StepDragMode, scale: f32) {
        let num_steps = self.params.modseqs[self.seq_idx]
            .num_steps
            .value()
            .clamp(2, MAX_MODSEQ_STEPS as i32) as usize;
        let (hovered_step, level) = self.cursor_step_and_level(bounds, x, y, num_steps);
        let mut bank = self.params.modseq_steps.snapshot();
        let steps = &mut bank.seqs[self.seq_idx];

        match mode {
            StepDragMode::Level => {
                steps[hovered_step].start_level = level;
                steps[hovered_step].end_level = level;
            }
            StepDragMode::EndLevel { step_idx } => {
                steps[step_idx.min(MAX_MODSEQ_STEPS - 1)].end_level = level;
            }
            StepDragMode::Tension {
                step_idx,
                start_y,
                start_tension,
            } => {
                let dy = (start_y - y) / (70.0 * scale.max(1.0));
                steps[step_idx.min(MAX_MODSEQ_STEPS - 1)].tension =
                    (start_tension + dy).clamp(-1.0, 1.0);
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
                let num_steps = self.params.modseqs[self.seq_idx]
                    .num_steps
                    .value()
                    .clamp(2, MAX_MODSEQ_STEPS as i32) as usize;
                let (step_idx, _lvl) = self.cursor_step_and_level(
                    bounds,
                    cx.mouse().cursorx,
                    cx.mouse().cursory,
                    num_steps,
                );
                let mode = if cx.modifiers().contains(Modifiers::ALT) {
                    let bank = self.params.modseq_steps.snapshot();
                    StepDragMode::Tension {
                        step_idx,
                        start_y: cx.mouse().cursory,
                        start_tension: bank.seqs[self.seq_idx][step_idx].tension,
                    }
                } else if cx.modifiers().shift() {
                    StepDragMode::EndLevel { step_idx }
                } else {
                    StepDragMode::Level
                };
                self.drag_mode = Some(mode);
                self.apply_step_edit(
                    cx.mouse().cursorx,
                    cx.mouse().cursory,
                    bounds,
                    mode,
                    cx.scale_factor(),
                );
                cx.capture();
                cx.needs_redraw();
                meta.consume();
            }
            WindowEvent::MouseDown(MouseButton::Right) => {
                let bounds = cx.bounds();
                let num_steps = self.params.modseqs[self.seq_idx]
                    .num_steps
                    .value()
                    .clamp(2, MAX_MODSEQ_STEPS as i32) as usize;
                let (step_idx, _) = self.cursor_step_and_level(
                    bounds,
                    cx.mouse().cursorx,
                    cx.mouse().cursory,
                    num_steps,
                );
                let bank = self.params.modseq_steps.snapshot();
                let mode = StepDragMode::Tension {
                    step_idx,
                    start_y: cx.mouse().cursory,
                    start_tension: bank.seqs[self.seq_idx][step_idx].tension,
                };
                self.drag_mode = Some(mode);
                cx.capture();
                meta.consume();
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                let bounds = cx.bounds();
                let num_steps = self.params.modseqs[self.seq_idx]
                    .num_steps
                    .value()
                    .clamp(2, MAX_MODSEQ_STEPS as i32) as usize;
                let (step_idx, _) = self.cursor_step_and_level(
                    bounds,
                    cx.mouse().cursorx,
                    cx.mouse().cursory,
                    num_steps,
                );
                let mut bank = self.params.modseq_steps.snapshot();
                bank.seqs[self.seq_idx][step_idx] = j_syn_dsp::ModSeqStep::default();
                self.params.modseq_steps.store_value(bank);
                cx.needs_redraw();
                meta.consume();
            }
            WindowEvent::MouseMove(x, y) => {
                if let Some(mode) = self.drag_mode {
                    let bounds = cx.bounds();
                    self.apply_step_edit(*x, *y, bounds, mode, cx.scale_factor());
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
        let num_steps = self.params.modseqs[self.seq_idx]
            .num_steps
            .value()
            .clamp(2, MAX_MODSEQ_STEPS as i32) as usize;
        let bank = self.params.modseq_steps.snapshot();
        let steps = &bank.seqs[self.seq_idx];

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
        for s in 1..num_steps {
            let gx = x0 + (s as f32 / num_steps as f32) * w;
            let mut line = vg::Path::new();
            line.move_to(gx, y0);
            line.line_to(gx, y0 + h);
            canvas.stroke_path(&line, &grid_paint);
        }

        // Filled + stroked continuous curve
        let samples = (num_steps * 10).max(64);
        let mut fill_path = vg::Path::new();
        let mut stroke_path = vg::Path::new();
        fill_path.move_to(x0, y0 + h);
        for i in 0..=samples {
            let t = (i as f32 / samples as f32).min(0.999_999);
            let val = evaluate_modseq_curve(steps, num_steps, t);
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

        // Step start/end node dots
        for s in 0..num_steps {
            let sx0 = x0 + (s as f32 / num_steps as f32) * w;
            let sx1 = x0 + ((s + 1) as f32 / num_steps as f32) * w;
            let sy0 = y0 + (1.0 - steps[s].start_level) * h;
            let sy1 = y0 + (1.0 - steps[s].end_level) * h;
            let mut dot = vg::Path::new();
            dot.circle(sx0 + 1.5 * dpx, sy0, 2.0 * dpx);
            if (sy1 - sy0).abs() > 2.0 * dpx {
                dot.circle(sx1 - 1.5 * dpx, sy1, 1.8 * dpx);
            }
            canvas.fill_path(&dot, &vg::Paint::color(vg::Color::rgb(255, 244, 228)));
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

        let cur_val = evaluate_modseq_curve(steps, num_steps, phase.min(0.999_999));
        let mut ph_dot = vg::Path::new();
        ph_dot.circle(ph_x, y0 + (1.0 - cur_val) * h, 3.0 * dpx);
        canvas.fill_path(&ph_dot, &vg::Paint::color(vg::Color::rgb(ar, ag, ab)));
        let mut ph_ring = vg::Paint::color(vg::Color::rgb(255, 248, 232));
        ph_ring.set_line_width(1.1 * dpx);
        canvas.stroke_path(&ph_dot, &ph_ring);
    }
}

/// Live `femtovg` ADSR shape preview canvas for `ENV 2` – `ENV 4`.
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
        let adsr = self.params.mod_envs[self.env_idx].to_dsp();

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
        let live_lvl = self.mod_ui.telemetry.env_level(1 + self.env_idx).clamp(0.0, 1.0);
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
    .width(Pixels(54.0))
    .height(Pixels(20.0));
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
            .height(Pixels(10.0))
            .child_space(Stretch(1.0));
        build_enum_dropdown(cx, params.clone(), gui_context.clone(), map_fn, items)
            .width(Stretch(1.0))
            .height(Pixels(19.0));
    })
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(1.0));
}

pub(super) fn build_modseq_tab(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    VStack::new(cx, move |cx| {
        // Top sub-selector bar (MSEQ 1..4 / ENV 2..4 + shape presets)
        HStack::new(cx, {
            let params_top = params.clone();
            move |cx| {
                build_subtab_button(cx, ModSeqSubTab::MSeq(0), "MSEQ 1");
                build_subtab_button(cx, ModSeqSubTab::MSeq(1), "MSEQ 2");
                build_subtab_button(cx, ModSeqSubTab::MSeq(2), "MSEQ 3");
                build_subtab_button(cx, ModSeqSubTab::MSeq(3), "MSEQ 4");
                build_subtab_button(cx, ModSeqSubTab::Env(0), "ENV 2");
                build_subtab_button(cx, ModSeqSubTab::Env(1), "ENV 3");
                build_subtab_button(cx, ModSeqSubTab::Env(2), "ENV 4");

                Element::new(cx).width(Stretch(1.0));

                let params_presets = params_top.clone();
                Binding::new(cx, UiData::modseq_subtab, move |cx, sub| {
                    let p_inner = params_presets.clone();
                    match sub.get(cx) {
                        ModSeqSubTab::MSeq(seq_idx) => {
                            HStack::new(cx, move |cx| {
                                bold_label(cx, "SHAPE", "mini-label")
                                    .width(Pixels(38.0))
                                    .height(Stretch(1.0))
                                    .child_space(Stretch(1.0));
                                for &preset in &ModSeqShapePreset::ALL {
                                    let p_btn = p_inner.clone();
                                    Button::new(
                                        cx,
                                        move |cx| {
                                            let n = p_btn.modseqs[seq_idx]
                                                .num_steps
                                                .value()
                                                .clamp(2, MAX_MODSEQ_STEPS as i32)
                                                as usize;
                                            let mut bank = p_btn.modseq_steps.snapshot();
                                            preset.apply_to_sequence(&mut bank.seqs[seq_idx], n);
                                            p_btn.modseq_steps.store_value(bank);
                                            cx.needs_redraw();
                                        },
                                        move |cx| {
                                            bold_label(cx, preset.label(), "preset-btn-label")
                                                .child_space(Stretch(1.0))
                                                .hoverable(false)
                                        },
                                    )
                                    .class("preset-btn")
                                    .width(Pixels(48.0))
                                    .height(Pixels(19.0));
                                }
                            })
                            .col_between(Pixels(4.0))
                            .width(Auto)
                            .height(Pixels(20.0));
                        }
                        ModSeqSubTab::Env(env_idx) => {
                            let hint = format!(
                                "ENV {} ADSR  |  DRAG E{} PILL TO MODULATE",
                                env_idx + 2,
                                env_idx + 2
                            );
                            bold_label(cx, &hint, "mini-label")
                                .width(Auto)
                                .height(Pixels(20.0))
                                .child_top(Stretch(1.0))
                                .child_bottom(Stretch(1.0));
                        }
                    }
                });
            }
        })
        .col_between(Pixels(4.0))
        .width(Stretch(1.0))
        .height(Pixels(20.0));

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
                                            "SYNC",
                                            p.clone(),
                                            g.clone(),
                                            move |pr| &pr.modseqs[seq_idx].sync_mode,
                                            SYNC_ITEMS,
                                        );
                                        build_labeled_dropdown(
                                            cx,
                                            "RATE",
                                            p,
                                            g,
                                            move |pr| &pr.modseqs[seq_idx].sync_rate,
                                            RATE_ITEMS,
                                        );
                                    }
                                })
                                .row_between(Pixels(4.0))
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
                                            "PLAY",
                                            p.clone(),
                                            g.clone(),
                                            move |pr| &pr.modseqs[seq_idx].play_mode,
                                            PLAY_ITEMS,
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
                                .row_between(Pixels(4.0))
                                .width(Pixels(82.0))
                                .height(Auto)
                                .top(Stretch(1.0))
                                .bottom(Stretch(1.0));

                                VStack::new(cx, move |cx| {
                                    build_flex_pill_cell(
                                        cx,
                                        UiData::params,
                                        move |pr| &pr.modseqs[seq_idx].num_steps,
                                        "STEPS",
                                        15,
                                    );
                                })
                                .width(Pixels(54.0))
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
                                    42.0,
                                    68.0,
                                );

                                build_knob_cell(
                                    cx,
                                    UiData::params,
                                    move |pr| &pr.modseqs[seq_idx].smooth,
                                    "SMOOTH",
                                    false,
                                    CORAL_RGB,
                                    42.0,
                                    68.0,
                                );
                            }
                        })
                        .col_between(Pixels(6.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0))
                        .width(Pixels(376.0))
                        .height(Stretch(1.0));
                    })
                    .col_between(Pixels(8.0))
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
                }
                ModSeqSubTab::Env(env_idx) => {
                    HStack::new(cx, move |cx| {
                        AdsrPreviewScope::new(cx, env_idx, p_sub.clone(), m_sub.clone())
                            .width(Stretch(1.35))
                            .height(Stretch(1.0));

                        HStack::new(cx, move |cx| {
                            let accent = subtab_accent(ModSeqSubTab::Env(env_idx));
                            build_knob_cell(
                                cx,
                                UiData::params,
                                move |p| &p.mod_envs[env_idx].attack,
                                "ATTACK",
                                false,
                                accent,
                                46.0,
                                74.0,
                            );
                            build_knob_cell(
                                cx,
                                UiData::params,
                                move |p| &p.mod_envs[env_idx].decay,
                                "DECAY",
                                false,
                                accent,
                                46.0,
                                74.0,
                            );
                            build_knob_cell(
                                cx,
                                UiData::params,
                                move |p| &p.mod_envs[env_idx].sustain,
                                "SUSTAIN",
                                false,
                                GOLD_RGB,
                                46.0,
                                74.0,
                            );
                            build_knob_cell(
                                cx,
                                UiData::params,
                                move |p| &p.mod_envs[env_idx].release,
                                "RELEASE",
                                false,
                                CORAL_RGB,
                                46.0,
                                74.0,
                            );
                        })
                        .col_between(Pixels(16.0))
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
    .row_between(Pixels(4.0))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}
