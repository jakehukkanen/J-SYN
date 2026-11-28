//! Dynamics and distortion FX scope painters (`Clipper`, `Limiter`, `OTT`, `Distortion`, `MB Dist`).

use j_syn_dsp::mod_matrix::FX_SCOPE_LEN;
use j_syn_dsp::{
    FxType, limiter_ceil_db, limiter_gain_db, mb_band_level_db, mb_band_magnitude_at_hz,
    mb_ordered_xover_hz,
};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use crate::theme::{self, ThemePalette};

use super::super::{
    ScopeFrame, filter_db_to_raw_y, filter_freq_to_norm, paint_clipper_waveform,
    paint_filled_waveform_trace, paint_filter_freq_grid, paint_scope_bg, paint_scope_border,
    paint_vertical_gradient_fill, scope_trace_rgb,
};
use super::{FxSlotScopeSnapshot, fx_accent_rgb};

pub fn paint_clipper_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    snap: &FxSlotScopeSnapshot,
    pal: &ThemePalette,
) {
    let drive_lin = 10.0_f32.powf((snap.eff_p[0] * 24.0) * 0.05);
    let ceil_db = -18.0 + snap.eff_p[1] * 18.0;
    let ceil_lin = 10.0_f32.powf(ceil_db * 0.05).clamp(0.05, 1.0);

    let mut pre_driven = [0.0_f32; FX_SCOPE_LEN];
    for i in 0..FX_SCOPE_LEN {
        pre_driven[i] = snap.pre[i] * drive_lin;
    }

    paint_clipper_waveform(
        canvas,
        bounds,
        dpx,
        &pre_driven,
        &snap.post,
        snap.activity,
        ceil_lin,
        1.25,
        snap.enabled,
        pal,
    );
}

pub fn paint_limiter_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    snap: &FxSlotScopeSnapshot,
    pal: &ThemePalette,
) {
    let gain_lin = 10.0_f32.powf(limiter_gain_db(snap.eff_p[0]) * 0.05);
    let ceil_lin = 10.0_f32
        .powf(limiter_ceil_db(snap.eff_p[1]) * 0.05)
        .clamp(0.05, 1.0);

    let mut pre_driven = [0.0_f32; FX_SCOPE_LEN];
    for i in 0..FX_SCOPE_LEN {
        pre_driven[i] = snap.pre[i] * gain_lin;
    }

    paint_clipper_waveform(
        canvas,
        bounds,
        dpx,
        &pre_driven,
        &snap.post,
        snap.activity,
        ceil_lin,
        1.25,
        snap.enabled,
        pal,
    );
}

pub fn paint_ott_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    snap: &FxSlotScopeSnapshot,
    pal: &ThemePalette,
) {
    let enabled = snap.enabled;
    let radius = 3.5 * dpx;
    paint_scope_bg(canvas, bounds, radius, pal, enabled);

    let pad_x = 5.0 * dpx;
    let pad_y = 4.0 * dpx;
    let ix = bounds.x + pad_x;
    let iy = bounds.y + pad_y;
    let iw = (bounds.w - 2.0 * pad_x).max(1.0);
    let ih = (bounds.h - 2.0 * pad_y).max(1.0);
    let mid_y = iy + ih * 0.5;

    let depth = snap.eff_p[0].clamp(0.0, 1.0);
    let pre_pk = snap.pre[FX_SCOPE_LEN - 1].clamp(0.0, 1.5);
    let post_pk = snap.post[FX_SCOPE_LEN - 1].clamp(0.0, 1.5);

    let mut zero_line = vg::Path::new();
    zero_line.move_to(ix, mid_y);
    zero_line.line_to(ix + iw, mid_y);
    let mut zero_paint = vg::Paint::color(theme::vg_color(
        &pal.scope_grid,
        if enabled { 170 } else { 70 },
    ));
    zero_paint.set_line_width(1.0 * dpx);
    canvas.stroke_path(&zero_line, &zero_paint);

    let base_accent = fx_accent_rgb(FxType::Ott);
    let hi_accent = theme::lerp_rgb(base_accent, theme::hex_to_rgb(&pal.text_bright), 0.35);
    let band_colors = [
        base_accent,
        theme::lerp_rgb(base_accent, hi_accent, 0.5),
        hi_accent,
    ];
    let band_w = (iw - 8.0 * dpx) / 3.0;
    let band_deltas = [
        (post_pk - pre_pk * 0.85) * depth,
        (snap.activity * 0.8 - pre_pk * 0.45) * depth,
        (post_pk * 0.9 - pre_pk * 0.65) * depth,
    ];
    let slot_back = theme::vg_lerp(&pal.scope_bg, &pal.panel_bg, 0.35, 200);
    for (b, &base_rgb) in band_colors.iter().enumerate() {
        let bx = ix + (b as f32) * (band_w + 4.0 * dpx);
        let mut slot_bg = vg::Path::new();
        slot_bg.rounded_rect(bx, iy, band_w, ih, 2.0 * dpx);
        canvas.fill_path(&slot_bg, &vg::Paint::color(slot_back));

        let (r, g, bl) = scope_trace_rgb(base_rgb, pal, enabled);
        let delta = band_deltas[b].clamp(-1.0, 1.0);
        let bar_h = (delta.abs() * (ih * 0.48)).max(if enabled { 2.0 * dpx } else { 0.0 });
        let bar_y = if delta >= 0.0 { mid_y - bar_h } else { mid_y };
        let mut bar = vg::Path::new();
        bar.rounded_rect(
            bx + 1.0 * dpx,
            bar_y,
            (band_w - 2.0 * dpx).max(1.0),
            bar_h,
            1.5 * dpx,
        );
        canvas.fill_path(
            &bar,
            &vg::Paint::color(vg::Color::rgba(r, g, bl, if enabled { 220 } else { 95 })),
        );
    }

    paint_scope_border(canvas, bounds, dpx, radius, pal, enabled);
}

pub fn paint_distortion_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    snap: &FxSlotScopeSnapshot,
    pal: &ThemePalette,
) {
    let enabled = snap.enabled;
    let radius = 3.5 * dpx;
    paint_scope_bg(canvas, bounds, radius, pal, enabled);

    let pad_x = 5.0 * dpx;
    let pad_y = 4.0 * dpx;
    let ix = bounds.x + pad_x;
    let iy = bounds.y + pad_y;
    let iw = (bounds.w - 2.0 * pad_x).max(1.0);
    let ih = (bounds.h - 2.0 * pad_y).max(1.0);

    let mid_y = iy + ih * 0.5;
    let mut mid_line = vg::Path::new();
    mid_line.move_to(ix, mid_y);
    mid_line.line_to(ix + iw, mid_y);
    let mut mid_paint = vg::Paint::color(theme::vg_color(
        &pal.scope_grid,
        if enabled { 130 } else { 55 },
    ));
    mid_paint.set_line_width(1.0 * dpx);
    canvas.stroke_path(&mid_line, &mid_paint);

    let wave_rgb = scope_trace_rgb(fx_accent_rgb(FxType::Distortion), pal, enabled);
    paint_filled_waveform_trace(
        canvas,
        &snap.post,
        ix,
        iy,
        iw,
        ih,
        1.25,
        wave_rgb,
        if enabled { 64 } else { 18 },
        if enabled { 10 } else { 4 },
        if enabled { 110 } else { 45 },
        1.1 * dpx,
    );

    let bias = (snap.eff_p[0].clamp(0.0, 1.0) * 2.0) - 1.0;
    let drive = snap.eff_p[1].clamp(0.0, 1.0);
    let mut xfer = vg::Path::new();
    const PTS: usize = 64;
    for i in 0..PTS {
        let t = i as f32 / (PTS - 1) as f32;
        let x_in = t * 2.0 - 1.0;
        let y_out = j_syn_dsp::waveshaper::shape_sample(snap.distortion_mode, x_in, drive, bias)
            .clamp(-1.0, 1.0);
        let px = ix + t * iw;
        let py = iy + (1.0 - (y_out * 0.5 + 0.5)) * ih;
        if i == 0 {
            xfer.move_to(px, py);
        } else {
            xfer.line_to(px, py);
        }
    }
    let curve_rgb = scope_trace_rgb(theme::hex_to_rgb(&pal.accent_gold), pal, enabled);
    let mut xfer_paint = vg::Paint::color(vg::Color::rgba(
        curve_rgb.0,
        curve_rgb.1,
        curve_rgb.2,
        if enabled { 235 } else { 105 },
    ));
    xfer_paint.set_line_width(1.6 * dpx);
    canvas.stroke_path(&xfer, &xfer_paint);

    paint_scope_border(canvas, bounds, dpx, radius, pal, enabled);
}

pub fn paint_multiband_dist_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    snap: &FxSlotScopeSnapshot,
    pal: &ThemePalette,
) {
    let enabled = snap.enabled;
    let f = ScopeFrame::begin(canvas, bounds, dpx, 4.0, 3.0, enabled, pal);

    const DB_MIN: f32 = -24.0;
    const DB_MAX: f32 = 14.0;
    paint_filter_freq_grid(
        canvas,
        f.ix,
        f.iy,
        f.iw,
        f.ih,
        dpx,
        pal,
        enabled,
        &[(0.0, 145, 58)],
        DB_MIN,
        DB_MAX,
    );

    let (low_hz, high_hz) = mb_ordered_xover_hz(snap.eff_p[0], snap.eff_p[1]);
    let active_tab = snap.mb_tab;
    let any_solo = snap.mb_bands.iter().any(|b| b.solo);

    let band_colors: [(u8, u8, u8); 3] = [(255, 118, 78), (245, 184, 72), (110, 212, 245)];

    const STEPS: usize = 72;
    for (b_idx, (band, &base_rgb)) in snap.mb_bands.iter().zip(band_colors.iter()).enumerate() {
        let is_selected = active_tab == (b_idx as u8 + 1);
        let is_audible = !any_solo || band.solo;
        let is_active = band.enabled;
        let drive = band.drive.clamp(0.0, 1.0);
        let lvl_db = if is_active {
            mb_band_level_db(band.level)
        } else {
            0.0
        };

        let rgb = scope_trace_rgb(base_rgb, pal, enabled && is_audible);
        let mut fill = vg::Path::new();
        let mut stroke = vg::Path::new();
        for i in 0..STEPS {
            let t = i as f32 / (STEPS - 1) as f32;
            let freq = 20.0 * 1000.0_f32.powf(t);
            let mag = mb_band_magnitude_at_hz(b_idx, low_hz, high_hz, freq).max(1e-4);
            let db = (20.0 * mag.log10() + lvl_db).clamp(DB_MIN, DB_MAX);
            let x = f.ix + t * f.iw;
            let y = filter_db_to_raw_y(db, f.iy, f.ih, DB_MIN, DB_MAX).clamp(f.iy, f.bottom);
            if i == 0 {
                fill.move_to(x, f.bottom);
                fill.line_to(x, y);
                stroke.move_to(x, y);
            } else {
                fill.line_to(x, y);
                stroke.line_to(x, y);
            }
        }
        fill.line_to(f.ix + f.iw, f.bottom);
        fill.close();

        let drive_boost = if is_active { (drive * 32.0) as u8 } else { 0 };
        let top_a = if !enabled || !is_audible {
            10
        } else if is_selected {
            72 + drive_boost / 2
        } else if active_tab == 0 {
            34 + drive_boost / 2
        } else {
            18
        };
        let bot_a = top_a / 4;
        paint_vertical_gradient_fill(canvas, &fill, f.ix, f.iy, f.bottom, rgb, top_a, bot_a);

        let stroke_a = if !enabled || !is_audible {
            55
        } else if is_selected {
            245
        } else if active_tab == 0 {
            195
        } else {
            115
        };
        let mut sp = vg::Paint::color(vg::Color::rgba(rgb.0, rgb.1, rgb.2, stroke_a));
        sp.set_line_width((if is_selected { 1.8 } else { 1.25 }) * dpx);
        sp.set_line_join(vg::LineJoin::Round);
        canvas.stroke_path(&stroke, &sp);
    }

    let wave_rgb = scope_trace_rgb(fx_accent_rgb(FxType::MultibandDist), pal, enabled);
    paint_filled_waveform_trace(
        canvas,
        &snap.post,
        f.ix,
        f.iy,
        f.iw,
        f.ih,
        1.25,
        wave_rgb,
        if enabled {
            34 + (snap.activity * 28.0) as u8
        } else {
            10
        },
        if enabled { 4 } else { 2 },
        if enabled { 100 } else { 40 },
        1.0 * dpx,
    );

    for xov_hz in [low_hz, high_hz] {
        let x = f.ix + filter_freq_to_norm(xov_hz) * f.iw;
        let mut line = vg::Path::new();
        line.move_to(x, f.iy);
        line.line_to(x, f.bottom);
        let mut lp = vg::Paint::color(theme::vg_color(
            &pal.text_primary,
            if enabled { 185 } else { 75 },
        ));
        lp.set_line_width(1.2 * dpx);
        canvas.stroke_path(&line, &lp);

        let mut tick = vg::Path::new();
        tick.rounded_rect(
            x - 2.5 * dpx,
            f.bottom - 5.0 * dpx,
            5.0 * dpx,
            5.0 * dpx,
            1.5 * dpx,
        );
        let tp = vg::Paint::color(vg::Color::rgba(
            wave_rgb.0,
            wave_rgb.1,
            wave_rgb.2,
            if enabled { 220 } else { 90 },
        ));
        canvas.fill_path(&tick, &tp);
    }

    f.end(canvas, pal);
}
