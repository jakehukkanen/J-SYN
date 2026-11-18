//! Custom `femtovg` FX scope visualizers for all 14 FX Unit types (`Utility`, `Clipper`,
//! `Limiter`, `Filter`, `OTT`, `Distortion`, `MB Dist`, `Chorus`, `Flanger`, `Phaser`,
//! `Delay`, `Reverb`, `Dispersion`, `TubeResonator`), shared by `J-SYN` and standalone FX plugins.

use std::sync::{Arc, OnceLock};
use std::time::Instant;

use j_syn_dsp::mod_matrix::FX_SCOPE_LEN;
use j_syn_dsp::{
    evaluate_diopser_ripple, evaluate_flanger_scope_response, evaluate_tube_res_scope_response,
    filter_response_db_from_slot, flanger_rate_hz, limiter_ceil_db, limiter_gain_db,
    mb_band_level_db, mb_band_magnitude_at_hz, mb_ordered_xover_hz, mb_xover_high_norm,
    mb_xover_low_norm, tube_res_lfo_rate_hz, utility_gain_db_from_norm, utility_pan_bipolar,
    utility_width_from_norm, DistortionMode, FilterMode, FxFilterSlope, FxRackState, FxType,
    MB_XOVER_HIGH_DEFAULT_NORM, MB_XOVER_LOW_DEFAULT_NORM,
};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::RawParamEvent;

use crate::shell::{TooltipState, UiKitEvent};
use crate::theme::{self, ThemePalette};
use crate::widgets::{set_resize_cursor, ResizeCursorAxis};

use super::{
    filter_db_to_raw_y, filter_freq_to_norm, paint_clipper_waveform, paint_filled_waveform_trace,
    paint_filter_freq_grid, paint_filter_response_curve_with_eval, paint_scope_bg,
    paint_scope_border, paint_vertical_gradient_fill, scope_trace_rgb, ScopeFrame,
};

#[derive(Clone, Copy, Default)]
pub struct MbBandScopeSnapshot {
    pub enabled: bool,
    pub solo: bool,
    pub drive: f32,
    pub level: f32,
}

#[derive(Clone)]
pub struct FxSlotScopeSnapshot {
    pub enabled: bool,
    pub fx_type: FxType,
    pub stack: u8,
    pub eff_p: [f32; 4],
    pub eff_eq: [f32; 4],
    pub filter_mode: FilterMode,
    pub filter_slope: FxFilterSlope,
    pub diopser_stages_idx: usize,
    pub tube_res_filter_enabled: bool,
    pub utility_inv_l: bool,
    pub utility_inv_r: bool,
    pub distortion_mode: DistortionMode,
    pub dirty_lfo: bool,
    pub mb_xover_p1: f32,
    pub mb_xover_p2: f32,
    pub mb_tab: u8,
    pub mb_bands: [MbBandScopeSnapshot; 3],
    pub pre: [f32; FX_SCOPE_LEN],
    pub post: [f32; FX_SCOPE_LEN],
    pub activity: f32,
}

impl Default for FxSlotScopeSnapshot {
    fn default() -> Self {
        Self {
            enabled: true,
            fx_type: FxType::Off,
            stack: 1,
            eff_p: [0.0; 4],
            eff_eq: [0.0; 4],
            filter_mode: FilterMode::Lp,
            filter_slope: FxFilterSlope::Db12,
            diopser_stages_idx: 2,
            tube_res_filter_enabled: true,
            utility_inv_l: false,
            utility_inv_r: false,
            distortion_mode: DistortionMode::Tube,
            dirty_lfo: false,
            mb_xover_p1: MB_XOVER_LOW_DEFAULT_NORM,
            mb_xover_p2: MB_XOVER_HIGH_DEFAULT_NORM,
            mb_tab: 0,
            mb_bands: [MbBandScopeSnapshot::default(); 3],
            pre: [0.0; FX_SCOPE_LEN],
            post: [0.0; FX_SCOPE_LEN],
            activity: 0.0,
        }
    }
}

#[inline]
pub fn fx_accent_rgb(fx_type: FxType) -> (u8, u8, u8) {
    match fx_type {
        FxType::Off => (120, 110, 100),
        FxType::Ott => (72, 200, 185),
        FxType::Distortion => (235, 118, 62),
        FxType::Chorus => (138, 178, 245),
        FxType::Delay => (245, 179, 73),
        FxType::Reverb => (195, 145, 245),
        FxType::JSatClipper => (245, 86, 86),
        FxType::Filter => (112, 224, 154),
        FxType::Phaser => (116, 208, 255),
        FxType::Diopser => (92, 226, 210),
        FxType::TubeResonator => (255, 164, 88),
        FxType::Utility => (108, 214, 255),
        FxType::Limiter => (255, 122, 89),
        FxType::MultibandDist => (255, 108, 72),
        FxType::Flanger => (156, 168, 255),
    }
}

#[inline]
fn scope_elapsed_secs() -> f32 {
    static START_EPOCH: OnceLock<Instant> = OnceLock::new();
    START_EPOCH.get_or_init(Instant::now).elapsed().as_secs_f32()
}

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

pub fn paint_filter_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    snap: &FxSlotScopeSnapshot,
    pal: &ThemePalette,
) {
    let enabled = snap.enabled;
    let f = ScopeFrame::begin(canvas, bounds, dpx, 4.0, 3.0, enabled, pal);
    let fp = FxRackState::filter_params_from_fx(snap.filter_mode, snap.filter_slope, snap.eff_p);

    const DB_MIN: f32 = -24.0;
    const DB_MAX: f32 = 18.0;
    paint_filter_freq_grid(
        canvas,
        f.ix,
        f.iy,
        f.iw,
        f.ih,
        dpx,
        pal,
        enabled,
        &[(0.0, 165, 75)],
        DB_MIN,
        DB_MAX,
    );
    paint_filter_response_curve_with_eval(
        canvas,
        enabled,
        fp.cutoff_hz,
        f.ix,
        f.iy,
        f.iw,
        f.ih,
        DB_MIN,
        DB_MAX,
        72,
        dpx,
        1.6 * dpx,
        fx_accent_rgb(FxType::Filter),
        pal,
        |freq_hz| {
            filter_response_db_from_slot(snap.filter_mode, snap.filter_slope, snap.eff_p, freq_hz)
        },
    );

    f.end(canvas, pal);
}

pub fn paint_diopser_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    snap: &FxSlotScopeSnapshot,
    pal: &ThemePalette,
) {
    let enabled = snap.enabled;
    let f = ScopeFrame::begin(canvas, bounds, dpx, 4.0, 3.0, enabled, pal);

    paint_filter_freq_grid(canvas, f.ix, f.iy, f.iw, f.ih, dpx, pal, enabled, &[], 0.0, 1.0);

    let accent = scope_trace_rgb(fx_accent_rgb(FxType::Diopser), pal, enabled);
    paint_filled_waveform_trace(
        canvas,
        &snap.post,
        f.ix,
        f.iy,
        f.iw,
        f.ih,
        1.25,
        accent,
        if enabled { 34 } else { 10 },
        if enabled { 4 } else { 2 },
        if enabled { 85 } else { 35 },
        1.0 * dpx,
    );

    let mut curve = vg::Path::new();
    let mut fill = vg::Path::new();
    fill.move_to(f.ix, f.bottom);
    const PTS: usize = 64;
    for i in 0..PTS {
        let t = i as f32 / (PTS - 1) as f32;
        let freq_hz = 20.0 * 1000.0_f32.powf(t);
        let disp =
            evaluate_diopser_ripple(snap.eff_p, snap.diopser_stages_idx, snap.stack, freq_hz);
        let x = f.ix + t * f.iw;
        let y = (f.bottom - disp * f.ih * 0.92).clamp(f.iy + 2.0 * dpx, f.bottom);
        fill.line_to(x, y);
        if i == 0 {
            curve.move_to(x, y);
        } else {
            curve.line_to(x, y);
        }
    }
    fill.line_to(f.ix + f.iw, f.bottom);
    fill.close();

    paint_vertical_gradient_fill(
        canvas,
        &fill,
        f.ix,
        f.iy,
        f.bottom,
        accent,
        if enabled { 88 } else { 22 },
        if enabled { 10 } else { 4 },
    );
    let mut paint = vg::Paint::color(vg::Color::rgba(
        accent.0,
        accent.1,
        accent.2,
        if enabled { 235 } else { 105 },
    ));
    paint.set_line_width(1.6 * dpx);
    canvas.stroke_path(&curve, &paint);

    f.end(canvas, pal);
}

pub fn paint_tube_res_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    snap: &FxSlotScopeSnapshot,
    pal: &ThemePalette,
) {
    let elapsed_s = scope_elapsed_secs();
    let enabled = snap.enabled;
    let f = ScopeFrame::begin(canvas, bounds, dpx, 4.0, 4.0, enabled, pal);

    paint_filter_freq_grid(canvas, f.ix, f.iy, f.iw, f.ih, dpx, pal, enabled, &[], 0.0, 1.0);

    let accent = scope_trace_rgb(fx_accent_rgb(FxType::TubeResonator), pal, enabled);
    paint_filled_waveform_trace(
        canvas,
        &snap.post,
        f.ix,
        f.iy,
        f.iw,
        f.ih,
        1.25,
        accent,
        if enabled { 30 } else { 10 },
        if enabled { 4 } else { 2 },
        if enabled { 80 } else { 32 },
        1.0 * dpx,
    );

    let eff_eq = [snap.eff_eq[0], snap.eff_eq[1], snap.eff_eq[2], snap.eff_p[3]];
    let flt_on = snap.tube_res_filter_enabled;
    let lfo_hz = tube_res_lfo_rate_hz(eff_eq[1]);
    let mut lfo_phase =
        (elapsed_s * lfo_hz * std::f32::consts::TAU).rem_euclid(std::f32::consts::TAU);
    if snap.dirty_lfo {
        lfo_phase = (lfo_phase * 2.5).floor() / 2.5;
    }

    let mut comb_curve = vg::Path::new();
    let mut comb_fill = vg::Path::new();
    let mut bp_curve = vg::Path::new();
    comb_fill.move_to(f.ix, f.bottom);

    const PTS: usize = 96;
    for i in 0..PTS {
        let t = i as f32 / (PTS - 1) as f32;
        let freq_hz = 20.0 * 1000.0_f32.powf(t);
        let (mag, bp_env) =
            evaluate_tube_res_scope_response(snap.eff_p, eff_eq, flt_on, lfo_phase, freq_hz);
        let x = f.ix + t * f.iw;
        let y = (f.bottom - mag * f.ih * 0.90).clamp(f.iy + 2.0 * dpx, f.bottom);
        let y_bp = (f.bottom - bp_env * f.ih * 0.88).clamp(f.iy + 2.0 * dpx, f.bottom);
        comb_fill.line_to(x, y);
        if i == 0 {
            comb_curve.move_to(x, y);
            bp_curve.move_to(x, y_bp);
        } else {
            comb_curve.line_to(x, y);
            bp_curve.line_to(x, y_bp);
        }
    }
    comb_fill.line_to(f.ix + f.iw, f.bottom);
    comb_fill.close();

    if flt_on {
        let mut bp_paint = vg::Paint::color(vg::Color::rgba(
            accent.0,
            accent.1,
            accent.2,
            if enabled { 105 } else { 45 },
        ));
        bp_paint.set_line_width(1.0 * dpx);
        canvas.stroke_path(&bp_curve, &bp_paint);
    }

    paint_vertical_gradient_fill(
        canvas,
        &comb_fill,
        f.ix,
        f.iy,
        f.bottom,
        accent,
        if enabled { 86 } else { 22 },
        if enabled { 10 } else { 4 },
    );
    let mut paint = vg::Paint::color(vg::Color::rgba(
        accent.0,
        accent.1,
        accent.2,
        if enabled { 225 } else { 100 },
    ));
    paint.set_line_width(1.4 * dpx);
    canvas.stroke_path(&comb_curve, &paint);

    f.end(canvas, pal);
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
        let y_out =
            j_syn_dsp::waveshaper::shape_sample(snap.distortion_mode, x_in, drive, bias)
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

pub fn paint_time_fx_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    snap: &FxSlotScopeSnapshot,
    pal: &ThemePalette,
) {
    let enabled = snap.enabled;
    let radius = 3.5 * dpx;
    paint_scope_bg(canvas, bounds, radius, pal, enabled);

    let (ar, ag, ab) = scope_trace_rgb(fx_accent_rgb(snap.fx_type), pal, enabled);
    let pad_x = 5.0 * dpx;
    let pad_y = 4.0 * dpx;
    let ix = bounds.x + pad_x;
    let iy = bounds.y + pad_y;
    let iw = (bounds.w - 2.0 * pad_x).max(1.0);
    let ih = (bounds.h - 2.0 * pad_y).max(1.0);
    let bottom = iy + ih;

    let p1 = snap.eff_p[0].clamp(0.0, 1.0);
    let p2 = snap.eff_p[1].clamp(0.0, 1.0);
    let p3 = snap.eff_p[2].clamp(0.0, 1.0);
    let p7 = snap.eff_eq[2].clamp(0.0, 1.0);

    match snap.fx_type {
        FxType::Delay => {
            let spacing = (0.08 + p1 * 0.14) * iw;
            let fb = 0.25 + p2 * 0.65;
            let mut amp = ih * 0.85;
            let mut x = ix + 4.0 * dpx;
            while x < ix + iw && amp > 2.0 * dpx {
                let mut tap = vg::Path::new();
                tap.move_to(x, bottom);
                tap.line_to(x, bottom - amp);
                let alpha = if p7 > 0.2 { 155 } else { 130 };
                let mut tp =
                    vg::Paint::color(vg::Color::rgba(ar, ag, ab, if enabled { alpha } else { 55 }));
                tp.set_line_width(2.0 * dpx);
                canvas.stroke_path(&tap, &tp);
                x += spacing;
                amp *= fb;
            }
        }
        FxType::Reverb => {
            let predelay_x = p7 * 0.22 * iw;
            let decay = 1.2 + (1.0 - p2) * 3.5;
            let mut tail = vg::Path::new();
            tail.move_to(ix + predelay_x, bottom);
            let span = (iw - predelay_x).max(1.0);
            for i in 0..32 {
                let t = i as f32 / 31.0;
                let env = (-t * decay).exp() * (0.4 + p1 * 0.6);
                tail.line_to(
                    ix + predelay_x + t * span,
                    bottom - env.clamp(0.0, 1.0) * ih,
                );
            }
            tail.line_to(ix + iw, bottom);
            tail.close();
            paint_vertical_gradient_fill(
                canvas,
                &tail,
                ix,
                iy,
                bottom,
                (ar, ag, ab),
                if enabled { 36 } else { 12 },
                if enabled { 6 } else { 2 },
            );
        }
        FxType::Chorus => {
            let mid_y = iy + ih * 0.5;
            let cycles = 1.5 + p1 * 3.5;
            let amp = (0.15 + p2 * 0.35) * ih;
            let mut lfo = vg::Path::new();
            for i in 0..48 {
                let t = i as f32 / 47.0;
                let y = mid_y + (t * cycles * std::f32::consts::TAU).sin() * amp;
                if i == 0 {
                    lfo.move_to(ix + t * iw, y);
                } else {
                    lfo.line_to(ix + t * iw, y);
                }
            }
            let mut lp =
                vg::Paint::color(vg::Color::rgba(ar, ag, ab, if enabled { 115 } else { 50 }));
            lp.set_line_width(1.2 * dpx);
            canvas.stroke_path(&lfo, &lp);
        }
        FxType::Phaser => {
            let mid_y = iy + ih * 0.45;
            let shift = 0.25 + p2 * 0.45;
            let amp = (0.28 + p3 * 0.32) * ih;
            let mut notch_curve = vg::Path::new();
            let mut notch_fill = vg::Path::new();
            notch_fill.move_to(ix, bottom);
            for i in 0..48 {
                let t = i as f32 / 47.0;
                let phase = (t * 3.0 + shift) * std::f32::consts::PI;
                let dip = 1.0 - phase.sin().powi(4);
                let y = (mid_y + (0.6 - dip) * amp).clamp(iy, bottom);
                let x = ix + t * iw;
                notch_fill.line_to(x, y);
                if i == 0 {
                    notch_curve.move_to(x, y);
                } else {
                    notch_curve.line_to(x, y);
                }
            }
            notch_fill.line_to(ix + iw, bottom);
            notch_fill.close();
            paint_vertical_gradient_fill(
                canvas,
                &notch_fill,
                ix,
                iy,
                bottom,
                (ar, ag, ab),
                if enabled { 30 } else { 10 },
                if enabled { 4 } else { 2 },
            );
            let mut np =
                vg::Paint::color(vg::Color::rgba(ar, ag, ab, if enabled { 130 } else { 55 }));
            np.set_line_width(1.25 * dpx);
            canvas.stroke_path(&notch_curve, &np);
        }
        _ => {}
    }

    paint_filled_waveform_trace(
        canvas,
        &snap.post,
        ix,
        iy,
        iw,
        ih,
        1.25,
        (ar, ag, ab),
        if enabled { 92 } else { 24 },
        if enabled { 12 } else { 4 },
        if enabled { 225 } else { 105 },
        1.4 * dpx,
    );

    paint_scope_border(canvas, bounds, dpx, radius, pal, enabled);
}

/// Renders Utility's live stereo-field Mid/Side trapezoid (`WIDTH` cone, `PAN` tilt, `GAIN` height,
/// and `Ø L` / `Ø R` phase inversion indicators).
pub fn paint_utility_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    snap: &FxSlotScopeSnapshot,
    pal: &ThemePalette,
) {
    let enabled = snap.enabled;
    let frame = ScopeFrame::begin(canvas, bounds, dpx, 4.0, 4.0, enabled, pal);
    let (ix, iy, iw, ih, bottom) = (frame.ix, frame.iy, frame.iw, frame.ih, frame.bottom);

    let accent = scope_trace_rgb(fx_accent_rgb(FxType::Utility), pal, enabled);
    let (ar, ag, ab) = accent;
    let inv_rgb = scope_trace_rgb(theme::hex_to_rgb(&pal.accent_secondary), pal, enabled);
    let (gr, gg, gb) = theme::hex_to_rgb(&pal.border_mid);

    paint_filled_waveform_trace(
        canvas,
        &snap.post,
        ix,
        iy,
        iw,
        ih,
        1.25,
        (ar, ag, ab),
        if enabled { 34 } else { 10 },
        if enabled { 4 } else { 2 },
        if enabled { 90 } else { 35 },
        1.0 * dpx,
    );

    let cx_mid = ix + iw * 0.5;
    let cy_mid = iy + ih * 0.52;
    let mut axis = vg::Path::new();
    axis.move_to(cx_mid, iy + 2.0 * dpx);
    axis.line_to(cx_mid, bottom - 2.0 * dpx);
    axis.move_to(ix + 6.0 * dpx, cy_mid);
    axis.line_to(ix + iw - 6.0 * dpx, cy_mid);
    let mut axis_paint = vg::Paint::color(vg::Color::rgba(gr, gg, gb, 95));
    axis_paint.set_line_width(1.0 * dpx);
    canvas.stroke_path(&axis, &axis_paint);

    let width = utility_width_from_norm(snap.eff_p[0]);
    let pan = utility_pan_bipolar(snap.eff_p[1]);
    let gain_db = utility_gain_db_from_norm(snap.eff_p[2]);

    let spread_norm = if width <= 1.0 {
        width * 0.20
    } else if width <= 2.0 {
        0.20 + (width - 1.0) * 0.14
    } else {
        0.34 + ((width - 2.0) / 3.0) * 0.13
    };
    let half_w = (spread_norm * iw).max(1.5 * dpx);
    let gain_norm = ((gain_db + 60.0) / 120.0).clamp(0.0, 1.0);
    let cone_h = (0.18 + gain_norm * 0.68) * ih;
    let base_y = bottom - 3.0 * dpx;
    let top_y = (base_y - cone_h).max(iy + 16.0 * dpx);

    let pan_shift = pan * (iw * 0.24);
    let top_cx = (cx_mid + pan_shift).clamp(
        ix + half_w * 0.35 + 3.0 * dpx,
        ix + iw - half_w * 0.35 - 3.0 * dpx,
    );
    let neck_w = (half_w * 0.22).max(2.0 * dpx);

    let tilt = pan * (cone_h * 0.22);
    let top_l_y = (top_y - tilt.min(0.0)).clamp(iy + 14.0 * dpx, base_y - 4.0 * dpx);
    let top_r_y = (top_y + tilt.max(0.0)).clamp(iy + 14.0 * dpx, base_y - 4.0 * dpx);

    let mut cone = vg::Path::new();
    cone.move_to(cx_mid - neck_w, base_y);
    cone.line_to(top_cx - half_w, top_l_y);
    cone.line_to(top_cx + half_w, top_r_y);
    cone.line_to(cx_mid + neck_w, base_y);
    cone.close();

    paint_vertical_gradient_fill(
        canvas,
        &cone,
        ix,
        iy,
        bottom,
        (ar, ag, ab),
        if enabled { 72 } else { 22 },
        if enabled { 14 } else { 5 },
    );

    let mut cone_stroke =
        vg::Paint::color(vg::Color::rgba(ar, ag, ab, if enabled { 225 } else { 100 }));
    cone_stroke.set_line_width(1.5 * dpx);
    cone_stroke.set_line_join(vg::LineJoin::Round);
    canvas.stroke_path(&cone, &cone_stroke);

    for (is_right, inv) in [(false, snap.utility_inv_l), (true, snap.utility_inv_r)] {
        let bx = if is_right {
            ix + iw - 12.0 * dpx
        } else {
            ix + 3.0 * dpx
        };
        let by = bottom - 5.0 * dpx;
        let (pr, pg, pb, pa) = if inv {
            (inv_rgb.0, inv_rgb.1, inv_rgb.2, if enabled { 235 } else { 110 })
        } else {
            (ar, ag, ab, if enabled { 110 } else { 45 })
        };
        let mut bar = vg::Path::new();
        bar.rounded_rect(bx, by, 9.0 * dpx, 2.5 * dpx, 1.0 * dpx);
        canvas.fill_path(&bar, &vg::Paint::color(vg::Color::rgba(pr, pg, pb, pa)));
    }

    frame.end(canvas, pal);
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

    let band_colors: [(u8, u8, u8); 3] = [
        (255, 118, 78),
        (245, 184, 72),
        (110, 212, 245),
    ];

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
        if enabled { 34 + (snap.activity * 28.0) as u8 } else { 10 },
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
        tick.rounded_rect(x - 2.5 * dpx, f.bottom - 5.0 * dpx, 5.0 * dpx, 5.0 * dpx, 1.5 * dpx);
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

pub fn paint_flanger_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    snap: &FxSlotScopeSnapshot,
    pal: &ThemePalette,
) {
    let enabled = snap.enabled;
    let f = ScopeFrame::begin(canvas, bounds, dpx, 4.0, 3.0, enabled, pal);

    paint_filter_freq_grid(canvas, f.ix, f.iy, f.iw, f.ih, dpx, pal, enabled, &[], 0.0, 1.0);

    let accent = scope_trace_rgb(fx_accent_rgb(FxType::Flanger), pal, enabled);
    paint_filled_waveform_trace(
        canvas,
        &snap.post,
        f.ix,
        f.iy,
        f.iw,
        f.ih,
        1.25,
        accent,
        if enabled { 34 + (snap.activity * 24.0) as u8 } else { 10 },
        if enabled { 4 } else { 2 },
        if enabled { 85 } else { 32 },
        1.0 * dpx,
    );

    let rate_hz = flanger_rate_hz(snap.eff_p[0]);
    let lfo_phase = (scope_elapsed_secs() * rate_hz).fract();

    let mut path_l = vg::Path::new();
    let mut path_r = vg::Path::new();
    let mut fill_l = vg::Path::new();
    fill_l.move_to(f.ix, f.bottom);

    const PTS: usize = 96;
    for i in 0..PTS {
        let t = i as f32 / (PTS - 1) as f32;
        let freq_hz = 20.0 * 1000.0_f32.powf(t);
        let (mag_l, mag_r, _, _) =
            evaluate_flanger_scope_response(snap.eff_p, snap.eff_eq, lfo_phase, freq_hz);
        let x = f.ix + t * f.iw;
        let y_l =
            (f.bottom - (mag_l * 0.42).clamp(0.0, 0.96) * f.ih).clamp(f.iy + 2.0 * dpx, f.bottom);
        let y_r =
            (f.bottom - (mag_r * 0.42).clamp(0.0, 0.96) * f.ih).clamp(f.iy + 2.0 * dpx, f.bottom);
        fill_l.line_to(x, y_l);
        if i == 0 {
            path_l.move_to(x, y_l);
            path_r.move_to(x, y_r);
        } else {
            path_l.line_to(x, y_l);
            path_r.line_to(x, y_r);
        }
    }
    fill_l.line_to(f.ix + f.iw, f.bottom);
    fill_l.close();

    paint_vertical_gradient_fill(
        canvas,
        &fill_l,
        f.ix,
        f.iy,
        f.bottom,
        accent,
        if enabled { 56 } else { 16 },
        if enabled { 6 } else { 2 },
    );

    let r_rgb = scope_trace_rgb(theme::hex_to_rgb(&pal.accent_gold), pal, enabled);
    let mut rp = vg::Paint::color(vg::Color::rgba(
        r_rgb.0,
        r_rgb.1,
        r_rgb.2,
        if enabled { 155 } else { 65 },
    ));
    rp.set_line_width(1.1 * dpx);
    canvas.stroke_path(&path_r, &rp);

    let mut lp = vg::Paint::color(vg::Color::rgba(
        accent.0,
        accent.1,
        accent.2,
        if enabled { 225 } else { 95 },
    ));
    lp.set_line_width(1.4 * dpx);
    canvas.stroke_path(&path_l, &lp);

    let rad = 7.0 * dpx;
    let cx_o = f.ix + f.iw - rad - 3.0 * dpx;
    let cy_o = f.bottom - rad - 3.0 * dpx;
    let mut ring = vg::Path::new();
    ring.circle(cx_o, cy_o, rad);
    let mut ring_p = vg::Paint::color(theme::vg_color(
        &pal.scope_grid,
        if enabled { 140 } else { 55 },
    ));
    ring_p.set_line_width(1.0 * dpx);
    canvas.stroke_path(&ring, &ring_p);

    let ang_l = lfo_phase * std::f32::consts::TAU;
    let ang_r = ang_l + snap.eff_eq[1] * std::f32::consts::PI;
    for (ang, col) in [(ang_r, r_rgb), (ang_l, accent)] {
        let mut dot = vg::Path::new();
        dot.circle(
            cx_o + ang.cos() * rad * 0.72,
            cy_o + ang.sin() * rad * 0.72,
            1.8 * dpx,
        );
        let dp = vg::Paint::color(vg::Color::rgba(
            col.0,
            col.1,
            col.2,
            if enabled { 235 } else { 100 },
        ));
        canvas.fill_path(&dot, &dp);
    }

    f.end(canvas, pal);
}

/// Multi-FX live waveform, filter curve, dispersion ripple, & activity visualizer
/// powered by any plugin's `FxSlotScopeSnapshot` provider closure.
pub struct FxSlotVisualizer {
    provider: Arc<dyn Fn() -> FxSlotScopeSnapshot + Send + Sync>,
}

impl FxSlotVisualizer {
    pub fn new<'a, F>(cx: &'a mut Context, provider: F) -> Handle<'a, Self>
    where
        F: Fn() -> FxSlotScopeSnapshot + Send + Sync + 'static,
    {
        Self {
            provider: Arc::new(provider),
        }
        .build(cx, |_| {})
    }
}

impl View for FxSlotVisualizer {
    fn element(&self) -> Option<&'static str> {
        Some("fx-slot-visualizer")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|raw_param_event, _| {
            if let RawParamEvent::ParametersChanged = raw_param_event {
                cx.needs_redraw();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 4.0 || bounds.h <= 4.0 {
            return;
        }
        let dpx = cx.scale_factor();
        let pal = theme::current_palette(cx);
        let snap = (self.provider)();
        match snap.fx_type {
            FxType::JSatClipper => paint_clipper_scope(canvas, bounds, dpx, &snap, &pal),
            FxType::Limiter => paint_limiter_scope(canvas, bounds, dpx, &snap, &pal),
            FxType::MultibandDist => paint_multiband_dist_scope(canvas, bounds, dpx, &snap, &pal),
            FxType::Flanger => paint_flanger_scope(canvas, bounds, dpx, &snap, &pal),
            FxType::Filter => paint_filter_scope(canvas, bounds, dpx, &snap, &pal),
            FxType::Diopser => paint_diopser_scope(canvas, bounds, dpx, &snap, &pal),
            FxType::TubeResonator => paint_tube_res_scope(canvas, bounds, dpx, &snap, &pal),
            FxType::Utility => paint_utility_scope(canvas, bounds, dpx, &snap, &pal),
            FxType::Ott => paint_ott_scope(canvas, bounds, dpx, &snap, &pal),
            FxType::Distortion => paint_distortion_scope(canvas, bounds, dpx, &snap, &pal),
            FxType::Delay | FxType::Chorus | FxType::Reverb | FxType::Phaser => {
                paint_time_fx_scope(canvas, bounds, dpx, &snap, &pal)
            }
            FxType::Off => {}
        }
    }
}

/// Host abstraction for dragging `LOW XOV` / `HIGH XOV` markers on `MultibandDist` scopes.
pub trait MultibandXoverHost: Send + Sync + 'static {
    fn xover_norms(&self) -> (f32, f32);
    fn begin_xover(&self, handle: usize);
    fn set_xover(&self, handle: usize, norm: f32);
    fn end_xover(&self, handle: usize);
    fn select_band_tab(&self, tab: u8);
}

/// Transparent overlay enabling horizontal drag of `LOW XOV` / `HIGH XOV` markers on `MultibandDist` scopes.
pub struct MultibandXoverDragOverlay {
    host: Arc<dyn MultibandXoverHost>,
    dragging_handle: Option<usize>,
    hover_cursor_active: bool,
}

impl MultibandXoverDragOverlay {
    pub fn new<'a>(cx: &'a mut Context, host: Arc<dyn MultibandXoverHost>) -> Handle<'a, Self> {
        Self {
            host,
            dragging_handle: None,
            hover_cursor_active: false,
        }
        .build(cx, |_| {})
    }

    fn x_to_hz(&self, bounds: BoundingBox, dpx: f32, mouse_x: f32) -> f32 {
        let pad_x = 4.0 * dpx;
        let ix = bounds.x + pad_x;
        let iw = (bounds.w - 2.0 * pad_x).max(1.0);
        let t = ((mouse_x - ix) / iw).clamp(0.0, 1.0);
        20.0 * 1000.0_f32.powf(t)
    }

    fn nearest_handle(&self, bounds: BoundingBox, dpx: f32, mouse_x: f32) -> Option<usize> {
        let (p1, p2) = self.host.xover_norms();
        let (low_hz, high_hz) = mb_ordered_xover_hz(p1, p2);
        let pad_x = 4.0 * dpx;
        let ix = bounds.x + pad_x;
        let iw = (bounds.w - 2.0 * pad_x).max(1.0);
        let x_low = ix + filter_freq_to_norm(low_hz) * iw;
        let x_high = ix + filter_freq_to_norm(high_hz) * iw;
        let d_low = (mouse_x - x_low).abs();
        let d_high = (mouse_x - x_high).abs();
        if d_low.min(d_high) <= 10.0 * dpx {
            Some(if d_low <= d_high { 0 } else { 1 })
        } else {
            None
        }
    }

    fn emit_xover_tooltip(
        &self,
        cx: &mut EventContext,
        bounds: BoundingBox,
        dpx: f32,
        handle: usize,
    ) {
        let (p1, p2) = self.host.xover_norms();
        let (low_hz, high_hz) = mb_ordered_xover_hz(p1, p2);
        let hz = if handle == 0 { low_hz } else { high_hz };
        let text = if hz >= 10_000.0 {
            format!("{:.1} kHz", hz / 1000.0)
        } else if hz >= 1000.0 {
            format!("{:.2} kHz", hz / 1000.0)
        } else {
            format!("{:.0} Hz", hz)
        };
        let pad_x = 4.0 * dpx;
        let ix = bounds.x + pad_x;
        let iw = (bounds.w - 2.0 * pad_x).max(1.0);
        let handle_x = ix + filter_freq_to_norm(hz) * iw;
        let s = dpx.max(0.1);
        cx.emit(UiKitEvent::ShowTooltip(TooltipState {
            visible: true,
            text,
            center_x: handle_x / s,
            anchor_top: bounds.y / s,
            anchor_bottom: (bounds.y + bounds.h) / s,
        }));
    }
}

impl View for MultibandXoverDragOverlay {
    fn element(&self) -> Option<&'static str> {
        Some("mb-xover-drag")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                let bounds = cx.bounds();
                let dpx = cx.scale_factor();
                let mx = cx.mouse().cursorx;
                if let Some(handle) = self.nearest_handle(bounds, dpx, mx) {
                    self.host.begin_xover(handle);
                    self.dragging_handle = Some(handle);
                    self.hover_cursor_active = true;
                    cx.capture();
                    set_resize_cursor(cx, ResizeCursorAxis::Horizontal);
                    self.emit_xover_tooltip(cx, bounds, dpx, handle);
                    meta.consume();
                } else {
                    let hz = self.x_to_hz(bounds, dpx, mx);
                    let (p1, p2) = self.host.xover_norms();
                    let (low_hz, high_hz) = mb_ordered_xover_hz(p1, p2);
                    let target_tab = if hz < low_hz {
                        1
                    } else if hz <= high_hz {
                        2
                    } else {
                        3
                    };
                    self.host.select_band_tab(target_tab);
                    cx.emit(UiKitEvent::ModUiChanged);
                    meta.consume();
                }
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                let bounds = cx.bounds();
                let dpx = cx.scale_factor();
                let mx = cx.mouse().cursorx;
                if let Some(handle) = self.nearest_handle(bounds, dpx, mx) {
                    let def = if handle == 0 {
                        MB_XOVER_LOW_DEFAULT_NORM
                    } else {
                        MB_XOVER_HIGH_DEFAULT_NORM
                    };
                    self.host.begin_xover(handle);
                    self.host.set_xover(handle, def);
                    self.host.end_xover(handle);
                    cx.emit(UiKitEvent::HideTooltip);
                    cx.emit(UiKitEvent::ModValChanged);
                    meta.consume();
                }
            }
            WindowEvent::MouseMove(x, _) => {
                let bounds = cx.bounds();
                let dpx = cx.scale_factor();
                if let Some(handle) = self.dragging_handle {
                    let hz = self.x_to_hz(bounds, dpx, *x);
                    let norm = if handle == 0 {
                        mb_xover_low_norm(hz)
                    } else {
                        mb_xover_high_norm(hz)
                    };
                    self.host.set_xover(handle, norm);
                    set_resize_cursor(cx, ResizeCursorAxis::Horizontal);
                    self.emit_xover_tooltip(cx, bounds, dpx, handle);
                    cx.emit(UiKitEvent::ModValChanged);
                    meta.consume();
                } else if self.nearest_handle(bounds, dpx, *x).is_some() {
                    self.hover_cursor_active = true;
                    set_resize_cursor(cx, ResizeCursorAxis::Horizontal);
                } else if self.hover_cursor_active {
                    self.hover_cursor_active = false;
                    set_resize_cursor(cx, ResizeCursorAxis::Default);
                }
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if let Some(handle) = self.dragging_handle.take() {
                    self.host.end_xover(handle);
                    cx.release();
                    self.hover_cursor_active = false;
                    set_resize_cursor(cx, ResizeCursorAxis::Default);
                    cx.emit(UiKitEvent::HideTooltip);
                    meta.consume();
                }
            }
            WindowEvent::MouseOut => {
                if self.dragging_handle.is_none() && self.hover_cursor_active {
                    self.hover_cursor_active = false;
                    set_resize_cursor(cx, ResizeCursorAxis::Default);
                }
            }
            _ => {}
        });
    }
}
