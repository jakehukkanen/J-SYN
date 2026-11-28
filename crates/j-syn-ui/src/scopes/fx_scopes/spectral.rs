//! Spectral, resonant, and utility FX scope painters (`Filter`, `Dispersion`, `TubeResonator`, `Utility`).

use j_syn_dsp::{
    FxRackState, FxType, evaluate_diopser_ripple, evaluate_tube_res_scope_response,
    filter_response_db_from_slot, tube_res_lfo_rate_hz, utility_gain_db_from_norm,
    utility_pan_bipolar, utility_width_from_norm,
};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use crate::theme::{self, ThemePalette};

use super::super::{
    ScopeFrame, paint_filled_waveform_trace, paint_filter_freq_grid,
    paint_filter_response_curve_with_eval, paint_vertical_gradient_fill, scope_trace_rgb,
};
use super::{FxSlotScopeSnapshot, fx_accent_rgb, scope_elapsed_secs};

pub fn paint_filter_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    snap: &FxSlotScopeSnapshot,
    pal: &ThemePalette,
) {
    let enabled = snap.enabled;
    let f = ScopeFrame::begin(canvas, bounds, dpx, 4.0, 3.0, enabled, pal);
    let fp = FxRackState::filter_params_from_fx(
        snap.filter_mode,
        snap.filter_slope,
        snap.eff_p,
        snap.eff_eq[0],
    );

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
            filter_response_db_from_slot(
                snap.filter_mode,
                snap.filter_slope,
                snap.eff_p,
                snap.eff_eq[0],
                freq_hz,
            )
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

    paint_filter_freq_grid(
        canvas,
        f.ix,
        f.iy,
        f.iw,
        f.ih,
        dpx,
        pal,
        enabled,
        &[],
        0.0,
        1.0,
    );

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

    paint_filter_freq_grid(
        canvas,
        f.ix,
        f.iy,
        f.iw,
        f.ih,
        dpx,
        pal,
        enabled,
        &[],
        0.0,
        1.0,
    );

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

    let eff_eq = [
        snap.eff_eq[0],
        snap.eff_eq[1],
        snap.eff_eq[2],
        snap.eff_p[3],
    ];
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
            (
                inv_rgb.0,
                inv_rgb.1,
                inv_rgb.2,
                if enabled { 235 } else { 110 },
            )
        } else {
            (ar, ag, ab, if enabled { 110 } else { 45 })
        };
        let mut bar = vg::Path::new();
        bar.rounded_rect(bx, by, 9.0 * dpx, 2.5 * dpx, 1.0 * dpx);
        canvas.fill_path(&bar, &vg::Paint::color(vg::Color::rgba(pr, pg, pb, pa)));
    }

    frame.end(canvas, pal);
}
