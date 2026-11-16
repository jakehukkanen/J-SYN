//! Custom `femtovg` FX scope visualizers for all FX types (`Clipper`, `Filter`, `Diopser`,
//! `OTT`, `Distortion`, `Delay`, `Reverb`, `Chorus`, `Phaser`), shared across `MAIN` and `FX`.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::{ModDestination, FX_SCOPE_LEN};
use j_syn_dsp::{evaluate_diopser_ripple, filter_response_db_from_slot, FxRackState};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::RawParamEvent;

use super::mod_bar::ModUiState;
use super::scope::{
    paint_filled_waveform_trace, paint_filter_freq_grid, paint_filter_response_curve_with_eval,
    paint_scope_bg, paint_scope_border, paint_vertical_gradient_fill, scope_trace_rgb,
};
use super::{theme, theme::ThemePalette};
use crate::{FxTypeParam, JSynPluginParams};

#[inline]
pub(super) fn fx_accent_rgb(fx_ty: FxTypeParam) -> (u8, u8, u8) {
    match fx_ty {
        FxTypeParam::Off => (120, 110, 100),
        FxTypeParam::Ott => (72, 200, 185),
        FxTypeParam::Distortion => (235, 118, 62),
        FxTypeParam::Chorus => (138, 178, 245),
        FxTypeParam::Delay => (245, 179, 73),
        FxTypeParam::Reverb => (195, 145, 245),
        FxTypeParam::JSatClipper => (245, 86, 86),
        FxTypeParam::Filter => (112, 224, 154),
        FxTypeParam::Phaser => (116, 208, 255),
        FxTypeParam::Diopser => (92, 226, 210),
    }
}

struct ScopeFrame {
    bounds: BoundingBox,
    dpx: f32,
    radius: f32,
    enabled: bool,
    ix: f32,
    iy: f32,
    iw: f32,
    ih: f32,
    bottom: f32,
}

impl ScopeFrame {
    #[inline]
    fn begin(
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
    fn end(&self, canvas: &mut Canvas, pal: &ThemePalette) {
        paint_scope_border(canvas, self.bounds, self.dpx, self.radius, pal, self.enabled);
    }
}

#[inline]
fn read_scope_snapshot(
    mod_ui: &ModUiState,
    bus_idx: usize,
    slot_idx: usize,
) -> ([f32; FX_SCOPE_LEN], [f32; FX_SCOPE_LEN], f32) {
    let mut pre = [0.0_f32; FX_SCOPE_LEN];
    let mut post = [0.0_f32; FX_SCOPE_LEN];
    let act = mod_ui
        .telemetry
        .fx_scope(bus_idx, slot_idx)
        .read_snapshot(&mut pre, &mut post);
    (pre, post, act)
}

#[inline]
fn slot_eff_params(
    slot: &crate::FxSlotPluginParams,
    mod_ui: &ModUiState,
    bus_idx: usize,
    slot_idx: usize,
) -> [f32; 4] {
    let tel = &mod_ui.telemetry;
    [
        (slot.p1.value()
            + tel.dest_offset(ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 0)))
        .clamp(0.0, 1.0),
        (slot.p2.value()
            + tel.dest_offset(ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 1)))
        .clamp(0.0, 1.0),
        (slot.p3.value()
            + tel.dest_offset(ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 2)))
        .clamp(0.0, 1.0),
        (slot.p4.value()
            + tel.dest_offset(ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 3)))
        .clamp(0.0, 1.0),
    ]
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn paint_clipper_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    bus_idx: usize,
    slot_idx: usize,
    params: &JSynPluginParams,
    mod_ui: &ModUiState,
    pal: &ThemePalette,
) {
    let slot = &params.fx_bus_slots(bus_idx)[slot_idx];
    let enabled = slot.enabled.value();
    let f = ScopeFrame::begin(canvas, bounds, dpx, 4.0, 3.0, enabled, pal);
    let (ix, iy, iw, ih, bottom) = (f.ix, f.iy, f.iw, f.ih, f.bottom);

    let (pre, post, gr_act) = read_scope_snapshot(mod_ui, bus_idx, slot_idx);
    let eff = slot_eff_params(slot, mod_ui, bus_idx, slot_idx);
    let drive_lin = 10.0_f32.powf((eff[0] * 24.0) * 0.05);
    let ceil_db = -18.0 + eff[1] * 18.0;
    let ceil_lin = 10.0_f32.powf(ceil_db * 0.05).clamp(0.05, 1.0);
    let y_scale = 1.25_f32;

    let mut pre_driven = [0.0_f32; FX_SCOPE_LEN];
    for i in 0..FX_SCOPE_LEN {
        pre_driven[i] = pre[i] * drive_lin;
    }

    let pre_rgb = scope_trace_rgb((245, 179, 73), pal, enabled);
    let post_rgb = scope_trace_rgb((245, 86, 86), pal, enabled);
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

    // Region above the ceiling: any pre-clip or post overshoot is greyed out
    if split_y > iy {
        canvas.save();
        canvas.intersect_scissor(bounds.x, iy, bounds.w, (split_y - iy).max(0.0));
        paint_filled_waveform_trace(
            canvas,
            &pre_driven,
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
            &post,
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

    // Region at/below the ceiling: active colored pre-clip & clipped post-clip waveforms
    if bottom > split_y {
        canvas.save();
        canvas.intersect_scissor(bounds.x, split_y, bounds.w, (bottom - split_y).max(0.0));
        paint_filled_waveform_trace(
            canvas,
            &pre_driven,
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
            &post,
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

    // Ceiling line drawn cleanly along the split boundary
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

    // Compact GR activity bar along right edge
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

#[inline]
#[allow(clippy::too_many_arguments)]
fn paint_filter_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    bus_idx: usize,
    slot_idx: usize,
    params: &JSynPluginParams,
    mod_ui: &ModUiState,
    pal: &ThemePalette,
) {
    let slot = &params.fx_bus_slots(bus_idx)[slot_idx];
    let enabled = slot.enabled.value();
    let f = ScopeFrame::begin(canvas, bounds, dpx, 4.0, 3.0, enabled, pal);
    let eff = slot_eff_params(slot, mod_ui, bus_idx, slot_idx);
    let mode: j_syn_dsp::FilterMode = slot.filter_mode.value().into();
    let slope: j_syn_dsp::FxFilterSlope = slot.filter_slope.value().into();
    let fp = FxRackState::filter_params_from_fx(mode, slope, eff);

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
        fx_accent_rgb(FxTypeParam::Filter),
        pal,
        |freq_hz| filter_response_db_from_slot(mode, slope, eff, freq_hz),
    );

    f.end(canvas, pal);
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn paint_diopser_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    bus_idx: usize,
    slot_idx: usize,
    params: &JSynPluginParams,
    mod_ui: &ModUiState,
    pal: &ThemePalette,
) {
    let slot = &params.fx_bus_slots(bus_idx)[slot_idx];
    let enabled = slot.enabled.value();
    let f = ScopeFrame::begin(canvas, bounds, dpx, 4.0, 3.0, enabled, pal);

    paint_filter_freq_grid(canvas, f.ix, f.iy, f.iw, f.ih, dpx, pal, enabled, &[], 0.0, 1.0);

    let (_pre, post, _) = read_scope_snapshot(mod_ui, bus_idx, slot_idx);

    let accent = scope_trace_rgb(fx_accent_rgb(FxTypeParam::Diopser), pal, enabled);
    paint_filled_waveform_trace(
        canvas,
        &post,
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

    let eff = slot_eff_params(slot, mod_ui, bus_idx, slot_idx);
    let stages_idx = slot.diopser_stages.value().to_index() as usize;
    let stack = slot.stack.value().clamp(1, 4) as u8;

    let mut curve = vg::Path::new();
    let mut fill = vg::Path::new();
    fill.move_to(f.ix, f.bottom);
    const PTS: usize = 64;
    for i in 0..PTS {
        let t = i as f32 / (PTS - 1) as f32;
        let freq_hz = 20.0 * 1000.0_f32.powf(t);
        let disp = evaluate_diopser_ripple(eff, stages_idx, stack, freq_hz);
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

#[inline]
#[allow(clippy::too_many_arguments)]
fn paint_ott_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    bus_idx: usize,
    slot_idx: usize,
    params: &JSynPluginParams,
    mod_ui: &ModUiState,
    pal: &ThemePalette,
) {
    let slot = &params.fx_bus_slots(bus_idx)[slot_idx];
    let enabled = slot.enabled.value();
    let radius = 3.5 * dpx;
    paint_scope_bg(canvas, bounds, radius, pal, enabled);

    let pad_x = 5.0 * dpx;
    let pad_y = 4.0 * dpx;
    let ix = bounds.x + pad_x;
    let iy = bounds.y + pad_y;
    let iw = (bounds.w - 2.0 * pad_x).max(1.0);
    let ih = (bounds.h - 2.0 * pad_y).max(1.0);
    let mid_y = iy + ih * 0.5;

    let mut pre = [0.0_f32; FX_SCOPE_LEN];
    let mut post = [0.0_f32; FX_SCOPE_LEN];
    let activity = mod_ui
        .telemetry
        .fx_scope(bus_idx, slot_idx)
        .read_snapshot(&mut pre, &mut post);
    let depth = slot.p1.value().clamp(0.0, 1.0);
    let pre_pk = pre[FX_SCOPE_LEN - 1].clamp(0.0, 1.5);
    let post_pk = post[FX_SCOPE_LEN - 1].clamp(0.0, 1.5);

    let mut zero_line = vg::Path::new();
    zero_line.move_to(ix, mid_y);
    zero_line.line_to(ix + iw, mid_y);
    let mut zero_paint = vg::Paint::color(theme::vg_color(
        &pal.scope_grid,
        if enabled { 170 } else { 70 },
    ));
    zero_paint.set_line_width(1.0 * dpx);
    canvas.stroke_path(&zero_line, &zero_paint);

    let base_accent = fx_accent_rgb(FxTypeParam::Ott);
    let hi_accent = theme::lerp_rgb(base_accent, theme::hex_to_rgb(&pal.text_bright), 0.35);
    let band_colors = [
        base_accent,
        theme::lerp_rgb(base_accent, hi_accent, 0.5),
        hi_accent,
    ];
    let band_w = (iw - 8.0 * dpx) / 3.0;
    let band_deltas = [
        (post_pk - pre_pk * 0.85) * depth,
        (activity * 0.8 - pre_pk * 0.45) * depth,
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
        bar.rounded_rect(bx + 1.0 * dpx, bar_y, (band_w - 2.0 * dpx).max(1.0), bar_h, 1.5 * dpx);
        canvas.fill_path(
            &bar,
            &vg::Paint::color(vg::Color::rgba(r, g, bl, if enabled { 220 } else { 95 })),
        );
    }

    paint_scope_border(canvas, bounds, dpx, radius, pal, enabled);
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn paint_distortion_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    bus_idx: usize,
    slot_idx: usize,
    params: &JSynPluginParams,
    mod_ui: &ModUiState,
    pal: &ThemePalette,
) {
    let slot = &params.fx_bus_slots(bus_idx)[slot_idx];
    let enabled = slot.enabled.value();
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

    let mut pre = [0.0_f32; FX_SCOPE_LEN];
    let mut post = [0.0_f32; FX_SCOPE_LEN];
    let _ = mod_ui
        .telemetry
        .fx_scope(bus_idx, slot_idx)
        .read_snapshot(&mut pre, &mut post);

    let wave_rgb = scope_trace_rgb(fx_accent_rgb(FxTypeParam::Distortion), pal, enabled);
    paint_filled_waveform_trace(
        canvas,
        &post,
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

    let submode = (slot.p1.value() * 3.99) as usize;
    let drive = 1.0 + slot.p2.value().clamp(0.0, 1.0) * 11.0;
    let mut xfer = vg::Path::new();
    const PTS: usize = 48;
    for i in 0..PTS {
        let t = i as f32 / (PTS - 1) as f32;
        let x_in = t * 2.0 - 1.0;
        let y_out = match submode {
            0 => (x_in * drive).tanh(),
            1 => (x_in * drive).clamp(-1.0, 1.0),
            2 => (x_in * drive * 1.3).sin(),
            _ => ((x_in * drive).tanh() * 6.0).round() / 6.0,
        }
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

#[inline]
#[allow(clippy::too_many_arguments)]
fn paint_time_fx_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    bus_idx: usize,
    slot_idx: usize,
    fx_ty: FxTypeParam,
    params: &JSynPluginParams,
    mod_ui: &ModUiState,
    pal: &ThemePalette,
) {
    let slot = &params.fx_bus_slots(bus_idx)[slot_idx];
    let enabled = slot.enabled.value();
    let radius = 3.5 * dpx;
    paint_scope_bg(canvas, bounds, radius, pal, enabled);

    let (ar, ag, ab) = scope_trace_rgb(fx_accent_rgb(fx_ty), pal, enabled);
    let pad_x = 5.0 * dpx;
    let pad_y = 4.0 * dpx;
    let ix = bounds.x + pad_x;
    let iy = bounds.y + pad_y;
    let iw = (bounds.w - 2.0 * pad_x).max(1.0);
    let ih = (bounds.h - 2.0 * pad_y).max(1.0);
    let bottom = iy + ih;

    let mut pre = [0.0_f32; FX_SCOPE_LEN];
    let mut post = [0.0_f32; FX_SCOPE_LEN];
    let _ = mod_ui
        .telemetry
        .fx_scope(bus_idx, slot_idx)
        .read_snapshot(&mut pre, &mut post);

    let p1 = slot.p1.value().clamp(0.0, 1.0);
    let p2 = slot.p2.value().clamp(0.0, 1.0);
    let p3 = slot.p3.value().clamp(0.0, 1.0);
    let p7 = slot.p7.value().clamp(0.0, 1.0);

    match fx_ty {
        FxTypeParam::Delay => {
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
        FxTypeParam::Reverb => {
            let predelay_x = p7 * 0.22 * iw;
            let decay = 1.2 + (1.0 - p2) * 3.5;
            let mut tail = vg::Path::new();
            tail.move_to(ix + predelay_x, bottom);
            let span = (iw - predelay_x).max(1.0);
            for i in 0..32 {
                let t = i as f32 / 31.0;
                let env = (-t * decay).exp() * (0.4 + p1 * 0.6);
                tail.line_to(ix + predelay_x + t * span, bottom - env.clamp(0.0, 1.0) * ih);
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
        FxTypeParam::Chorus => {
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
        FxTypeParam::Phaser => {
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
        &post,
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

/// Multi-FX live waveform, filter curve, dispersion ripple, & activity visualizer.
pub(super) struct FxSlotVisualizer {
    bus_idx: usize,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    mod_ui: Arc<ModUiState>,
}

impl FxSlotVisualizer {
    pub(super) fn new(
        cx: &mut Context,
        bus_idx: usize,
        slot_idx: usize,
        params: Arc<JSynPluginParams>,
        mod_ui: Arc<ModUiState>,
    ) -> Handle<'_, Self> {
        Self {
            bus_idx,
            slot_idx,
            params,
            mod_ui,
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
        let fx_ty = self.params.fx_bus_slots(self.bus_idx)[self.slot_idx]
            .fx_type
            .value();
        match fx_ty {
            FxTypeParam::JSatClipper => paint_clipper_scope(
                canvas,
                bounds,
                dpx,
                self.bus_idx,
                self.slot_idx,
                &self.params,
                &self.mod_ui,
                &pal,
            ),
            FxTypeParam::Filter => paint_filter_scope(
                canvas,
                bounds,
                dpx,
                self.bus_idx,
                self.slot_idx,
                &self.params,
                &self.mod_ui,
                &pal,
            ),
            FxTypeParam::Diopser => paint_diopser_scope(
                canvas,
                bounds,
                dpx,
                self.bus_idx,
                self.slot_idx,
                &self.params,
                &self.mod_ui,
                &pal,
            ),
            FxTypeParam::Ott => paint_ott_scope(
                canvas,
                bounds,
                dpx,
                self.bus_idx,
                self.slot_idx,
                &self.params,
                &self.mod_ui,
                &pal,
            ),
            FxTypeParam::Distortion => paint_distortion_scope(
                canvas,
                bounds,
                dpx,
                self.bus_idx,
                self.slot_idx,
                &self.params,
                &self.mod_ui,
                &pal,
            ),
            FxTypeParam::Delay
            | FxTypeParam::Chorus
            | FxTypeParam::Reverb
            | FxTypeParam::Phaser => paint_time_fx_scope(
                canvas,
                bounds,
                dpx,
                self.bus_idx,
                self.slot_idx,
                fx_ty,
                &self.params,
                &self.mod_ui,
                &pal,
            ),
            FxTypeParam::Off => {}
        }
    }
}
