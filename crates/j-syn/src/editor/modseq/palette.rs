//! Interactive `4×9` `MODSEQ` stamp palette, `LFO` preset strip, and `ADSR` preview scope.

use std::sync::Arc;

use j_syn_dsp::{
    AdsrParams, LfoPreset, MODSEQ_PALETTE_TILES, ModSeqStep, evaluate_adsr_preview,
    evaluate_mseg_curve, evaluate_step_shape, fill_modseq_row_with_tile,
};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use super::super::UiEvent;
use super::super::mod_bar::ModUiState;
use crate::JSynPluginParams;

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

/// Graphical 9-preset vector strip shown in the `LFO` sub-bar next to `RESET CURVE`.
pub(super) struct LfoPresetBar {
    seq_idx: usize,
    params: Arc<JSynPluginParams>,
    hovered_idx: Option<usize>,
}

impl LfoPresetBar {
    pub(super) fn new(
        cx: &mut Context,
        seq_idx: usize,
        params: Arc<JSynPluginParams>,
    ) -> Handle<'_, Self> {
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
        let pal = super::super::theme::current_palette(cx);
        let accent = pal.mod_seq_rgb();
        let gap = 2.0 * dpx;

        for (idx, &preset) in LfoPreset::ALL.iter().enumerate() {
            let (tx, ty, tw, th) = grid_tile_rect(bounds, idx, LfoPreset::ALL.len(), 1, 0.0, gap);
            let hovered = self.hovered_idx == Some(idx);

            let mut tile = vg::Path::new();
            tile.rounded_rect(tx, ty, tw, th, 2.5 * dpx);
            canvas.fill_path(
                &tile,
                &vg::Paint::color(if hovered {
                    super::super::theme::vg_color(&pal.surface_hover, 255)
                } else {
                    super::super::theme::vg_color(&pal.surface_dark, 255)
                }),
            );
            let mut border = vg::Paint::color(if hovered {
                super::super::theme::vg_color(&pal.accent_primary, 255)
            } else {
                super::super::theme::vg_color(&pal.border_mid, 255)
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

/// Interactive `4x9` graphical shape palette for `MODSEQ` step stamping & quick fill.
pub(super) struct ModSeqPaletteGrid {
    seq_idx: usize,
    params: Arc<JSynPluginParams>,
    mod_ui: Arc<ModUiState>,
    hovered_idx: Option<usize>,
    bold_font: std::cell::Cell<Option<vg::FontId>>,
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
            bold_font: std::cell::Cell::new(None),
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
                    MODSEQ_PALETTE_TILES.len(),
                    9,
                    4,
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
                    MODSEQ_PALETTE_TILES.len(),
                    9,
                    4,
                    2.0 * dpx,
                    2.0 * dpx,
                ) {
                    self.mod_ui.set_active_tile_idx(idx);
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
                    MODSEQ_PALETTE_TILES.len(),
                    9,
                    4,
                    2.0 * dpx,
                    2.0 * dpx,
                ) {
                    self.mod_ui.set_active_tile_idx(idx);
                    let tile = MODSEQ_PALETTE_TILES[idx];
                    let row = self.mod_ui.active_modseq_row(self.seq_idx);
                    let mut bank = self.params.modseq_steps.snapshot();
                    fill_modseq_row_with_tile(&mut bank.seqs[self.seq_idx], row, tile);
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
        let active_idx = self.mod_ui.active_tile_idx();
        let pal = super::super::theme::current_palette(cx);
        let accent = pal.mod_seq_rgb();
        let font_id = j_syn_ui::shell::ensure_canvas_bold_font(canvas, &self.bold_font);

        for (idx, &tile_def) in MODSEQ_PALETTE_TILES.iter().enumerate() {
            let (tx, ty, tw, th) = grid_tile_rect(bounds, idx, 9, 4, pad, gap);
            let is_active = idx == active_idx;
            let hovered = self.hovered_idx == Some(idx);

            let mut tile = vg::Path::new();
            tile.rounded_rect(tx, ty, tw, th, 2.5 * dpx);
            canvas.fill_path(
                &tile,
                &vg::Paint::color(if is_active {
                    super::super::theme::vg_color(&pal.surface_active, 255)
                } else if hovered {
                    super::super::theme::vg_color(&pal.surface_hover, 255)
                } else {
                    super::super::theme::vg_color(&pal.surface_dark, 255)
                }),
            );
            let mut border = vg::Paint::color(if is_active {
                super::super::theme::vg_color(&pal.accent_primary, 255)
            } else if hovered {
                super::super::theme::vg_color(&pal.border_light, 255)
            } else {
                super::super::theme::vg_color(&pal.border_mid, 255)
            });
            border.set_line_width((if is_active { 1.25 } else { 0.9 }) * dpx);
            canvas.stroke_path(&tile, &border);

            let sub = tile_def.shape.subdivision();
            if sub > 1 {
                let mut div_paint =
                    vg::Paint::color(super::super::theme::vg_color(&pal.text_dim, 95));
                div_paint.set_line_width(0.75 * dpx);
                for k in 1..sub {
                    let dx = tx + (k as f32 / sub as f32) * tw;
                    let mut line = vg::Path::new();
                    line.move_to(dx, ty + 2.0 * dpx);
                    line.line_to(dx, ty + th - 2.0 * dpx);
                    canvas.stroke_path(&line, &div_paint);
                }
            }

            if tile_def.stamp_steps > 1 {
                let step = ModSeqStep::stamped(tile_def.shape, 1.0, 0.0);
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
                if let Some(fid) = font_id {
                    let badge = if tile_def.stamp_steps == 4 { "4" } else { "2" };
                    let mut tp =
                        vg::Paint::color(vg::Color::rgba(accent.0, accent.1, accent.2, 200));
                    tp.set_font(&[fid]);
                    tp.set_font_size(6.5 * dpx);
                    tp.set_text_align(vg::Align::Right);
                    tp.set_text_baseline(vg::Baseline::Top);
                    let _ = canvas.fill_text(tx + tw - 2.0 * dpx, ty + 1.0 * dpx, badge, &tp);
                }
            } else {
                let step = ModSeqStep::stamped_with_span(tile_def.shape, 1.0, 0.0, tile_def.span);
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
}

/// Live `femtovg` ADSR shape preview canvas for `ENV 1` – `ENV 8`.
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
        let pal = super::super::theme::current_palette(cx);
        let (ar, ag, ab) = pal.mod_env_rgb();

        let mut bg = vg::Path::new();
        bg.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 4.0 * dpx);
        canvas.fill_path(
            &bg,
            &vg::Paint::color(super::super::theme::vg_color(&pal.scope_bg, 255)),
        );
        let mut border = vg::Paint::color(super::super::theme::vg_color(&pal.border_mid, 255));
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

        let live_lvl = self
            .mod_ui
            .telemetry
            .env_level(self.env_idx)
            .clamp(0.0, 1.0);
        if live_lvl > 0.005 {
            let meter_h = live_lvl * h;
            let mut bar = vg::Path::new();
            bar.rounded_rect(
                x0 + w - 4.0 * dpx,
                y0 + h - meter_h,
                4.0 * dpx,
                meter_h,
                1.5 * dpx,
            );
            canvas.fill_path(&bar, &vg::Paint::color(vg::Color::rgba(ar, ag, ab, 230)));
        }
    }
}
