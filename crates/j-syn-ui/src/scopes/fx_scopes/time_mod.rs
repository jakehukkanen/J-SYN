//! Time-based and modulation FX scope painters (`Chorus`, `Phaser`, `Delay`, `Reverb`, `Flanger`).

use j_syn_dsp::{FxType, evaluate_flanger_scope_response, flanger_rate_hz};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use crate::theme::{self, ThemePalette};

use super::super::{
    ScopeFrame, paint_filled_waveform_trace, paint_filter_freq_grid, paint_scope_bg,
    paint_scope_border, paint_vertical_gradient_fill, scope_trace_rgb,
};
use super::{FxSlotScopeSnapshot, fx_accent_rgb, scope_elapsed_secs};

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
                let mut tp = vg::Paint::color(vg::Color::rgba(
                    ar,
                    ag,
                    ab,
                    if enabled { alpha } else { 55 },
                ));
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
pub fn paint_flanger_scope(
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
        if enabled {
            34 + (snap.activity * 24.0) as u8
        } else {
            10
        },
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
