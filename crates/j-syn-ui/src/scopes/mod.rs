//! Shared `femtovg` scope primitives, filter frequency-response curve renderer,
//! and `MasterMeter` stereo Peak/RMS meter.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModUiTelemetry;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use crate::theme::{self, ThemePalette};

pub mod fx_scopes;

pub use fx_scopes::{
    fx_accent_rgb, FxSlotScopeSnapshot, FxSlotVisualizer, MbBandScopeSnapshot,
    MultibandXoverDragOverlay, MultibandXoverHost,
};

#[derive(Clone, Copy)]
pub struct ScopeFrame {
    pub bounds: BoundingBox,
    pub dpx: f32,
    pub radius: f32,
    pub enabled: bool,
    pub ix: f32,
    pub iy: f32,
    pub iw: f32,
    pub ih: f32,
    pub bottom: f32,
}

impl ScopeFrame {
    #[inline]
    pub fn begin(
        canvas: &mut Canvas,
        bounds: BoundingBox,
        dpx: f32,
        pad_x_dp: f32,
        pad_y_dp: f32,
        enabled: bool,
        pal: &ThemePalette,
    ) -> Self {
        let radius = 3.5 * dpx;
        paint_scope_bg(canvas, bounds, radius, pal, enabled);
        let pad_x = pad_x_dp * dpx;
        let pad_y = pad_y_dp * dpx;
        let ix = bounds.x + pad_x;
        let iy = bounds.y + pad_y;
        let iw = (bounds.w - 2.0 * pad_x).max(1.0);
        let ih = (bounds.h - 2.0 * pad_y).max(1.0);
        Self {
            bounds,
            dpx,
            radius,
            enabled,
            ix,
            iy,
            iw,
            ih,
            bottom: iy + ih,
        }
    }

    #[inline]
    pub fn end(&self, canvas: &mut Canvas, pal: &ThemePalette) {
        paint_scope_border(canvas, self.bounds, self.dpx, self.radius, pal, self.enabled);
    }
}

#[inline]
pub fn paint_scope_bg(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    radius: f32,
    pal: &ThemePalette,
    enabled: bool,
) {
    let mut bg = vg::Path::new();
    bg.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, radius);
    canvas.fill_path(
        &bg,
        &vg::Paint::color(theme::vg_color(
            &pal.scope_bg,
            if enabled { 255 } else { 180 },
        )),
    );
}

#[inline]
pub fn paint_scope_border(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    radius: f32,
    pal: &ThemePalette,
    enabled: bool,
) {
    let mut border = vg::Path::new();
    border.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, radius);
    let mut bp = vg::Paint::color(theme::vg_color(
        if enabled {
            &pal.border_mid
        } else {
            &pal.border_dark
        },
        255,
    ));
    bp.set_line_width(1.0 * dpx);
    canvas.stroke_path(&border, &bp);
}

#[inline]
pub fn scope_trace_rgb(rgb: (u8, u8, u8), pal: &ThemePalette, enabled: bool) -> (u8, u8, u8) {
    if enabled {
        rgb
    } else {
        theme::lerp_rgb(rgb, theme::hex_to_rgb(&pal.text_dim), 0.60)
    }
}

#[inline]
#[allow(clippy::too_many_arguments)]
pub fn paint_vertical_gradient_fill(
    canvas: &mut Canvas,
    path: &vg::Path,
    x: f32,
    top_y: f32,
    bottom_y: f32,
    rgb: (u8, u8, u8),
    top_alpha: u8,
    bot_alpha: u8,
) {
    let paint = vg::Paint::linear_gradient(
        x,
        top_y,
        x,
        bottom_y,
        vg::Color::rgba(rgb.0, rgb.1, rgb.2, top_alpha),
        vg::Color::rgba(rgb.0, rgb.1, rgb.2, bot_alpha),
    );
    canvas.fill_path(path, &paint);
}

/// Renders a unipolar scrolling/amplitude waveform slice anchored at the bottom with
/// a vertical gradient area fill and optional top trace stroke.
#[allow(clippy::too_many_arguments)]
pub fn paint_filled_waveform_trace(
    canvas: &mut Canvas,
    samples: &[f32],
    ix: f32,
    iy: f32,
    iw: f32,
    ih: f32,
    y_scale: f32,
    rgb: (u8, u8, u8),
    fill_top_alpha: u8,
    fill_bot_alpha: u8,
    stroke_alpha: u8,
    stroke_w: f32,
) {
    if samples.len() < 2 {
        return;
    }
    let bottom = iy + ih;
    let denom = (samples.len() - 1) as f32;
    let scale = y_scale.max(1e-4);

    let mut fill = vg::Path::new();
    let mut stroke = vg::Path::new();
    fill.move_to(ix, bottom);
    for (i, &val) in samples.iter().enumerate() {
        let x = ix + (i as f32 / denom) * iw;
        let cy = bottom - (val / scale).clamp(0.0, 1.0) * ih;
        fill.line_to(x, cy);
        if i == 0 {
            stroke.move_to(x, cy);
        } else {
            stroke.line_to(x, cy);
        }
    }
    fill.line_to(ix + iw, bottom);
    fill.close();

    if fill_top_alpha > 0 {
        paint_vertical_gradient_fill(
            canvas,
            &fill,
            ix,
            iy,
            bottom,
            rgb,
            fill_top_alpha,
            fill_bot_alpha,
        );
    }
    if stroke_alpha > 0 {
        let mut paint = vg::Paint::color(vg::Color::rgba(rgb.0, rgb.1, rgb.2, stroke_alpha));
        paint.set_line_width(stroke_w);
        paint.set_line_join(vg::LineJoin::Round);
        canvas.stroke_path(&stroke, &paint);
    }
}

#[inline]
pub fn filter_freq_to_norm(freq_hz: f32) -> f32 {
    ((freq_hz.clamp(20.0, 20_000.0) / 20.0).ln() / 1000.0_f32.ln()).clamp(0.0, 1.0)
}

#[inline]
pub fn filter_db_to_raw_y(
    db: f32,
    inner_top: f32,
    inner_h: f32,
    db_min: f32,
    db_max: f32,
) -> f32 {
    let span = (db_max - db_min).max(1.0);
    let norm = ((db - db_min) / span).min(1.0);
    inner_top + (1.0 - norm) * inner_h
}

/// Shared logarithmic frequency & horizontal dB grid for Filter & FX Filter scopes.
#[allow(clippy::too_many_arguments)]
pub fn paint_filter_freq_grid(
    canvas: &mut Canvas,
    ix: f32,
    iy: f32,
    iw: f32,
    ih: f32,
    dpx: f32,
    pal: &ThemePalette,
    enabled: bool,
    db_lines: &[(f32, u8, u8)],
    db_min: f32,
    db_max: f32,
) {
    let bottom = iy + ih;
    let mut v_paint = vg::Paint::color(theme::vg_color(
        &pal.scope_grid,
        if enabled { 150 } else { 65 },
    ));
    v_paint.set_line_width(1.0 * dpx);
    for freq in [100.0_f32, 1_000.0_f32, 10_000.0_f32] {
        let x = ix + filter_freq_to_norm(freq) * iw;
        let mut line = vg::Path::new();
        line.move_to(x, iy);
        line.line_to(x, bottom);
        canvas.stroke_path(&line, &v_paint);
    }

    for &(db, en_alpha, dis_alpha) in db_lines {
        let y = filter_db_to_raw_y(db, iy, ih, db_min, db_max).clamp(iy, bottom);
        let mut line = vg::Path::new();
        line.move_to(ix, y);
        line.line_to(ix + iw, y);
        let mut p = vg::Paint::color(theme::vg_color(
            &pal.scope_grid,
            if enabled { en_alpha } else { dis_alpha },
        ));
        p.set_line_width(1.0 * dpx);
        canvas.stroke_path(&line, &p);
    }
}

/// Shared filter magnitude curve + vertical gradient fill under the response line + cutoff dot.
#[allow(clippy::too_many_arguments)]
pub fn paint_filter_response_curve(
    canvas: &mut Canvas,
    params: &j_syn_dsp::FilterParams,
    ix: f32,
    iy: f32,
    iw: f32,
    ih: f32,
    db_min: f32,
    db_max: f32,
    pts: usize,
    dpx: f32,
    stroke_w: f32,
    rgb: (u8, u8, u8),
    pal: &ThemePalette,
) {
    paint_filter_response_curve_with_eval(
        canvas,
        params.enabled,
        params.cutoff_hz,
        ix,
        iy,
        iw,
        ih,
        db_min,
        db_max,
        pts,
        dpx,
        stroke_w,
        rgb,
        pal,
        |freq_hz| j_syn_dsp::evaluate_filter_response_db(params, freq_hz),
    );
}

#[allow(clippy::too_many_arguments)]
pub fn paint_filter_response_curve_with_eval<F>(
    canvas: &mut Canvas,
    enabled: bool,
    cutoff_hz: f32,
    ix: f32,
    iy: f32,
    iw: f32,
    ih: f32,
    db_min: f32,
    db_max: f32,
    pts: usize,
    dpx: f32,
    stroke_w: f32,
    rgb: (u8, u8, u8),
    pal: &ThemePalette,
    eval_db: F,
) where
    F: Fn(f32) -> f32,
{
    let bottom = iy + ih;
    let floor_y = bottom - 0.5 * dpx;
    let mut curve = vg::Path::new();
    let mut fill = vg::Path::new();
    let mut prev_x = ix;
    let mut prev_raw_y = bottom;

    let count = pts.max(2);
    for i in 0..count {
        let t = i as f32 / (count - 1) as f32;
        let freq_hz = 20.0 * 1000.0_f32.powf(t);
        let db = eval_db(freq_hz);
        let x = ix + t * iw;
        let raw_y = filter_db_to_raw_y(db, iy, ih, db_min, db_max);
        let fill_y = raw_y.clamp(iy, bottom);
        if i == 0 {
            if raw_y < floor_y {
                curve.move_to(x, raw_y.max(iy));
            }
            fill.move_to(x, bottom);
            fill.line_to(x, fill_y);
        } else {
            let prev_in = prev_raw_y < floor_y;
            let curr_in = raw_y < floor_y;
            match (prev_in, curr_in) {
                (true, true) => curve.line_to(x, raw_y.max(iy)),
                (true, false) => {
                    let denom = raw_y - prev_raw_y;
                    let frac = if denom.abs() > 1e-6 {
                        ((floor_y - prev_raw_y) / denom).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    curve.line_to(prev_x + frac * (x - prev_x), floor_y);
                }
                (false, true) => {
                    let denom = raw_y - prev_raw_y;
                    let frac = if denom.abs() > 1e-6 {
                        ((floor_y - prev_raw_y) / denom).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                    curve.move_to(prev_x + frac * (x - prev_x), floor_y);
                    curve.line_to(x, raw_y.max(iy));
                }
                (false, false) => {}
            }
            fill.line_to(x, fill_y);
        }
        if i + 1 == count {
            fill.line_to(x, bottom);
            fill.close();
        }
        prev_x = x;
        prev_raw_y = raw_y;
    }

    let trace_rgb = scope_trace_rgb(rgb, pal, enabled);
    paint_vertical_gradient_fill(
        canvas,
        &fill,
        ix,
        iy,
        bottom,
        trace_rgb,
        if enabled { 92 } else { 24 },
        if enabled { 12 } else { 4 },
    );

    let stroke_color = vg::Color::rgba(
        trace_rgb.0,
        trace_rgb.1,
        trace_rgb.2,
        if enabled { 240 } else { 115 },
    );
    let mut curve_paint = vg::Paint::color(stroke_color);
    curve_paint.set_line_width(stroke_w);
    canvas.stroke_path(&curve, &curve_paint);

    let fc_x = ix + filter_freq_to_norm(cutoff_hz) * iw;
    let fc_db = eval_db(cutoff_hz);
    let fc_raw_y = filter_db_to_raw_y(fc_db, iy, ih, db_min, db_max);
    if fc_raw_y < bottom - 1.0 * dpx {
        let mut dot = vg::Path::new();
        dot.circle(fc_x, fc_raw_y.max(iy), 2.6 * dpx);
        canvas.fill_path(&dot, &vg::Paint::color(stroke_color));
    }
}

/// Shared clipper / limiter pre-vs-post waveform scope renderer.
#[allow(clippy::too_many_arguments)]
pub fn paint_clipper_waveform(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    pre: &[f32],
    post: &[f32],
    gr_act: f32,
    ceil_lin: f32,
    y_scale: f32,
    enabled: bool,
    pal: &ThemePalette,
) {
    let f = ScopeFrame::begin(canvas, bounds, dpx, 4.0, 3.0, enabled, pal);
    let (ix, iy, iw, ih, bottom) = (f.ix, f.iy, f.iw, f.ih, f.bottom);

    let pre_rgb = scope_trace_rgb(theme::hex_to_rgb(&pal.accent_primary), pal, enabled);
    let post_rgb = scope_trace_rgb(theme::hex_to_rgb(&pal.mod_midi), pal, enabled);
    let dim_base = theme::lerp_rgb(
        theme::hex_to_rgb(&pal.text_dim),
        theme::hex_to_rgb(&pal.text_muted),
        0.25,
    );
    let grey_rgb = if enabled {
        dim_base
    } else {
        theme::hex_to_rgb(&pal.text_dim)
    };

    let ceil_y = bottom - (ceil_lin / y_scale).clamp(0.0, 1.0) * ih;
    let split_y = ceil_y.clamp(iy, bottom);

    if split_y > iy {
        canvas.save();
        canvas.intersect_scissor(bounds.x, iy, bounds.w, (split_y - iy).max(0.0));
        paint_filled_waveform_trace(
            canvas,
            pre,
            ix,
            iy,
            iw,
            ih,
            y_scale,
            grey_rgb,
            if enabled { 52 } else { 16 },
            if enabled { 22 } else { 6 },
            if enabled { 140 } else { 60 },
            1.15 * dpx,
        );
        paint_filled_waveform_trace(
            canvas,
            post,
            ix,
            iy,
            iw,
            ih,
            y_scale,
            grey_rgb,
            if enabled { 58 } else { 18 },
            if enabled { 24 } else { 6 },
            if enabled { 155 } else { 65 },
            1.3 * dpx,
        );
        canvas.restore();
    }

    if bottom > split_y {
        canvas.save();
        canvas.intersect_scissor(bounds.x, split_y, bounds.w, (bottom - split_y).max(0.0));
        paint_filled_waveform_trace(
            canvas,
            pre,
            ix,
            iy,
            iw,
            ih,
            y_scale,
            pre_rgb,
            if enabled { 24 } else { 0 },
            if enabled { 4 } else { 0 },
            if enabled { 125 } else { 55 },
            1.0 * dpx,
        );
        paint_filled_waveform_trace(
            canvas,
            post,
            ix,
            iy,
            iw,
            ih,
            y_scale,
            post_rgb,
            if enabled { 96 } else { 24 },
            if enabled { 14 } else { 4 },
            if enabled { 235 } else { 110 },
            1.5 * dpx,
        );
        canvas.restore();
    }

    let mut ceil_path = vg::Path::new();
    ceil_path.move_to(ix, ceil_y);
    ceil_path.line_to(ix + iw, ceil_y);
    let mut ceil_paint = vg::Paint::color(vg::Color::rgba(
        post_rgb.0,
        post_rgb.1,
        post_rgb.2,
        if enabled { 195 } else { 90 },
    ));
    ceil_paint.set_line_width(1.2 * dpx);
    canvas.stroke_path(&ceil_path, &ceil_paint);

    if gr_act > 0.005 && enabled {
        let bar_w = 3.5 * dpx;
        let bar_h = (gr_act.clamp(0.0, 1.0) * ih).max(2.0 * dpx);
        let mut bar = vg::Path::new();
        bar.rect(ix + iw - bar_w, iy, bar_w, bar_h);
        canvas.fill_path(
            &bar,
            &vg::Paint::color(vg::Color::rgba(post_rgb.0, post_rgb.1, post_rgb.2, 210)),
        );
    }

    f.end(canvas, pal);
}

/// Stereo Peak/RMS master output meter with clickable clip indicator.
pub struct MasterMeter {
    telemetry: Arc<ModUiTelemetry>,
}

impl MasterMeter {
    pub fn new<'a>(cx: &'a mut Context, telemetry: Arc<ModUiTelemetry>) -> Handle<'a, Self> {
        Self { telemetry }.build(cx, |_| {})
    }

    #[inline]
    fn level_to_norm(lin: f32) -> f32 {
        if lin <= 1e-6 {
            return 0.0;
        }
        let db = 20.0 * lin.log10();
        ((db + 48.0) / 48.0).clamp(0.0, 1.0)
    }
}

impl View for MasterMeter {
    fn element(&self) -> Option<&'static str> {
        Some("master-meter")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                self.telemetry.reset_master_clip();
                cx.needs_redraw();
                meta.consume();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 2.0 || bounds.h <= 2.0 {
            return;
        }
        let dpx = cx.scale_factor();
        let (peak_l, peak_r, rms_l, rms_r, clip_l, clip_r) = self.telemetry.master_meters();
        let pal = theme::current_palette(cx);

        paint_scope_bg(canvas, bounds, 3.0 * dpx, &pal, true);
        paint_scope_border(canvas, bounds, dpx, 3.0 * dpx, &pal, true);

        let pad = 2.5 * dpx;
        let gap = 2.0 * dpx;
        let clip_h = 3.5 * dpx;
        let bar_w = ((bounds.w - 2.0 * pad - gap) * 0.5).max(2.0);
        let bar_top = bounds.y + pad + clip_h + 1.5 * dpx;
        let bar_h = (bounds.h - (bar_top - bounds.y) - pad).max(4.0);

        let channels = [
            (bounds.x + pad, peak_l, rms_l, clip_l),
            (bounds.x + pad + bar_w + gap, peak_r, rms_r, clip_r),
        ];

        for (x, pk, rms, clip) in channels {
            let mut led = vg::Path::new();
            led.rounded_rect(x, bounds.y + pad, bar_w, clip_h, 1.0 * dpx);
            let led_col = if clip {
                theme::vg_color(&pal.mod_midi, 255)
            } else {
                theme::vg_lerp(&pal.scope_bg, &pal.border_dark, 0.35, 255)
            };
            canvas.fill_path(&led, &vg::Paint::color(led_col));

            let mut track = vg::Path::new();
            track.rounded_rect(x, bar_top, bar_w, bar_h, 1.0 * dpx);
            canvas.fill_path(
                &track,
                &vg::Paint::color(theme::vg_lerp(&pal.scope_bg, &pal.panel_bg, 0.28, 255)),
            );

            let rms_n = Self::level_to_norm(rms);
            if rms_n > 0.005 {
                let fh = rms_n * bar_h;
                let mut fill = vg::Path::new();
                fill.rounded_rect(x, bar_top + bar_h - fh, bar_w, fh, 1.0 * dpx);
                let paint = vg::Paint::linear_gradient(
                    x,
                    bar_top + bar_h,
                    x,
                    bar_top,
                    theme::vg_color(&pal.mod_env, 255),
                    theme::vg_color(&pal.accent_primary, 255),
                );
                canvas.fill_path(&fill, &paint);
            }

            let pk_n = Self::level_to_norm(pk);
            if pk_n > 0.005 {
                let py =
                    (bar_top + bar_h - pk_n * bar_h).clamp(bar_top, bar_top + bar_h - 1.5 * dpx);
                let mut cap = vg::Path::new();
                cap.rect(x, py, bar_w, 1.5 * dpx);
                let cap_col = if pk > 0.95 {
                    theme::vg_color(&pal.mod_midi, 255)
                } else {
                    theme::vg_color(&pal.accent_gold, 255)
                };
                canvas.fill_path(&cap, &vg::Paint::color(cap_col));
            }
        }
    }
}
