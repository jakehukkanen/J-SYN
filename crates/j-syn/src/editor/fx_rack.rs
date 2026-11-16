//! Master FX Rack UI (`FX 1`–`FX 4` column on `MAIN` and full 8-slot `FX 1`–`FX 8` view
//! with `▲`/`▼` slot swapping, `STACK 1..4` control, context-sensitive FX knobs, and live scopes).

use std::sync::Arc;

use j_syn_dsp::filter::cutoff_norm_to_hz;
use j_syn_dsp::mod_matrix::{ModDestination, FX_SCOPE_LEN};
use j_syn_dsp::{FxRackState, FxType};
use nih_plug::prelude::{GuiContext, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::RawParamEvent;

use super::controls::{
    bold_label, build_dropdown_trigger_labels, build_inline_pill_cell, ScrollDropdown,
};
use super::mod_bar::{ModUiState, FX_VIEW_TAG};
use super::scope::{
    paint_filled_waveform_trace, paint_filter_freq_grid, paint_filter_response_curve,
    paint_scope_bg, paint_scope_border, paint_vertical_gradient_fill, scope_trace_rgb,
};
use super::widgets::{KnobDimExt, LedToggleSwitch, RotaryKnob};
use super::{theme, theme::ThemePalette, UiData, UiEvent};
use crate::{FxTypeParam, JSynPluginParams};

const FX_TYPE_ITEMS: &[(&str, FxTypeParam)] = &[
    ("Empty", FxTypeParam::Off),
    ("OTT", FxTypeParam::Ott),
    ("Distortion", FxTypeParam::Distortion),
    ("Chorus", FxTypeParam::Chorus),
    ("Phaser", FxTypeParam::Phaser),
    ("Delay", FxTypeParam::Delay),
    ("Reverb", FxTypeParam::Reverb),
    ("Clipper", FxTypeParam::JSatClipper),
    ("Filter", FxTypeParam::Filter),
];

#[inline]
fn fx0_enabled(p: &JSynPluginParams) -> bool {
    p.fx_slots[0].enabled.value()
}
#[inline]
fn fx1_enabled(p: &JSynPluginParams) -> bool {
    p.fx_slots[1].enabled.value()
}
#[inline]
fn fx2_enabled(p: &JSynPluginParams) -> bool {
    p.fx_slots[2].enabled.value()
}
#[inline]
fn fx3_enabled(p: &JSynPluginParams) -> bool {
    p.fx_slots[3].enabled.value()
}
#[inline]
fn fx4_enabled(p: &JSynPluginParams) -> bool {
    p.fx_slots[4].enabled.value()
}
#[inline]
fn fx5_enabled(p: &JSynPluginParams) -> bool {
    p.fx_slots[5].enabled.value()
}
#[inline]
fn fx6_enabled(p: &JSynPluginParams) -> bool {
    p.fx_slots[6].enabled.value()
}
#[inline]
fn fx7_enabled(p: &JSynPluginParams) -> bool {
    p.fx_slots[7].enabled.value()
}

#[inline]
fn fx_slot_enabled_fn(slot_idx: usize) -> fn(&JSynPluginParams) -> bool {
    match slot_idx {
        0 => fx0_enabled,
        1 => fx1_enabled,
        2 => fx2_enabled,
        3 => fx3_enabled,
        4 => fx4_enabled,
        5 => fx5_enabled,
        6 => fx6_enabled,
        _ => fx7_enabled,
    }
}

#[inline]
fn fx_accent_rgb(fx_ty: FxTypeParam) -> (u8, u8, u8) {
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
    }
}

#[inline]
pub(super) fn fx_param_label(fx_ty: FxTypeParam, param_idx: usize) -> &'static str {
    match fx_ty {
        FxTypeParam::Off => "--",
        FxTypeParam::Ott => match param_idx {
            0 => "DEPTH",
            1 => "TIME",
            2 => "IN GAIN",
            _ => "OUT GN",
        },
        FxTypeParam::Distortion => match param_idx {
            0 => "TYPE",
            1 => "DRIVE",
            2 => "TONE",
            _ => "MIX",
        },
        FxTypeParam::Chorus => match param_idx {
            0 => "RATE",
            1 => "DEPTH",
            2 => "WIDTH",
            _ => "MIX",
        },
        FxTypeParam::Delay => match param_idx {
            0 => "TIME",
            1 => "FEEDBK",
            2 => "DAMP",
            _ => "MIX",
        },
        FxTypeParam::Reverb => match param_idx {
            0 => "SIZE",
            1 => "DECAY",
            2 => "DAMP",
            _ => "MIX",
        },
        FxTypeParam::JSatClipper => match param_idx {
            0 => "DRIVE",
            1 => "CEIL",
            2 => "KNEE",
            _ => "MIX",
        },
        FxTypeParam::Filter => match param_idx {
            0 => "CUTOFF",
            1 => "RES",
            2 => "TYPE",
            _ => "MIX",
        },
        FxTypeParam::Phaser => match param_idx {
            0 => "RATE",
            1 => "DEPTH",
            2 => "FEEDBK",
            _ => "MIX",
        },
    }
}

#[inline]
pub(super) fn format_fx_param_value(fx_ty: FxTypeParam, param_idx: usize, norm: f32) -> String {
    let n = norm.clamp(0.0, 1.0);
    match fx_ty {
        FxTypeParam::Off => "--".to_string(),
        FxTypeParam::Ott => match param_idx {
            0 => format!("{:.0}%", n * 100.0),
            1 => format!("{:.2}x", 0.25 * 16.0_f32.powf(n)),
            2 | 3 => format!("{:+.1} dB", -18.0 + n * 36.0),
            _ => String::new(),
        },
        FxTypeParam::Distortion => match param_idx {
            0 => FxRackState::format_distortion_submode(n).to_string(),
            1 | 3 => format!("{:.0}%", n * 100.0),
            2 => format!("{:+.0}%", (n - 0.5) * 200.0),
            _ => String::new(),
        },
        FxTypeParam::Chorus => match param_idx {
            0 => format!("{:.2} Hz", 0.05 * 100.0_f32.powf(n)),
            _ => format!("{:.0}%", n * 100.0),
        },
        FxTypeParam::Delay => match param_idx {
            0 => FxRackState::format_delay_sync(n).to_string(),
            _ => format!("{:.0}%", n * 100.0),
        },
        FxTypeParam::Reverb => format!("{:.0}%", n * 100.0),
        FxTypeParam::JSatClipper => match param_idx {
            0 => format!("+{:.1} dB", n * 24.0),
            1 => format!("{:.1} dB", -18.0 + n * 18.0),
            _ => format!("{:.0}%", n * 100.0),
        },
        FxTypeParam::Filter => match param_idx {
            0 => {
                let hz = cutoff_norm_to_hz(n);
                if hz >= 10_000.0 {
                    format!("{:.1} kHz", hz / 1000.0)
                } else if hz >= 1000.0 {
                    format!("{:.2} kHz", hz / 1000.0)
                } else {
                    format!("{:.0} Hz", hz)
                }
            }
            2 => FxRackState::format_filter_type(n).to_string(),
            _ => format!("{:.0}%", n * 100.0),
        },
        FxTypeParam::Phaser => match param_idx {
            0 => format!("{:.2} Hz", 0.05 * 120.0_f32.powf(n)),
            _ => format!("{:.0}%", n * 100.0),
        },
    }
}

fn swap_mod_destinations_for_slots(params: &JSynPluginParams, slot_a: usize, slot_b: usize) {
    let mut mat = params.mod_matrix.snapshot();
    let mut changed = false;
    for slot in &mut mat.slots {
        if slot.destination.is_fx_slot(slot_a) {
            let p_idx = (slot.destination.index() - 67) % 4;
            slot.destination = ModDestination::fx_param(slot_b, p_idx);
            changed = true;
        } else if slot.destination.is_fx_slot(slot_b) {
            let p_idx = (slot.destination.index() - 67) % 4;
            slot.destination = ModDestination::fx_param(slot_a, p_idx);
            changed = true;
        }
    }
    if changed {
        params.mod_matrix.store_value(mat);
    }
}

/// Swaps all settings and modulation assignments between FX slots `slot_a` and `slot_b`.
pub(super) fn swap_fx_slots(
    params: &Arc<JSynPluginParams>,
    gui_context: &dyn GuiContext,
    slot_a: usize,
    slot_b: usize,
) {
    if slot_a == slot_b || slot_a >= 8 || slot_b >= 8 {
        return;
    }

    let a_en = params.fx_slots[slot_a].enabled.value();
    let a_ty = params.fx_slots[slot_a].fx_type.value();
    let a_stk = params.fx_slots[slot_a].stack.value();
    let a_p = [
        params.fx_slots[slot_a].p1.value(),
        params.fx_slots[slot_a].p2.value(),
        params.fx_slots[slot_a].p3.value(),
        params.fx_slots[slot_a].p4.value(),
    ];

    let b_en = params.fx_slots[slot_b].enabled.value();
    let b_ty = params.fx_slots[slot_b].fx_type.value();
    let b_stk = params.fx_slots[slot_b].stack.value();
    let b_p = [
        params.fx_slots[slot_b].p1.value(),
        params.fx_slots[slot_b].p2.value(),
        params.fx_slots[slot_b].p3.value(),
        params.fx_slots[slot_b].p4.value(),
    ];

    let setter = ParamSetter::new(gui_context);

    let set_slot = |idx: usize, en: bool, ty: FxTypeParam, stk: i32, p: [f32; 4]| {
        let s = &params.fx_slots[idx];
        setter.begin_set_parameter(&s.enabled);
        setter.set_parameter(&s.enabled, en);
        setter.end_set_parameter(&s.enabled);

        setter.begin_set_parameter(&s.fx_type);
        setter.set_parameter(&s.fx_type, ty);
        setter.end_set_parameter(&s.fx_type);

        setter.begin_set_parameter(&s.stack);
        setter.set_parameter(&s.stack, stk);
        setter.end_set_parameter(&s.stack);

        for (k, param) in s.params_array().into_iter().enumerate() {
            setter.begin_set_parameter(param);
            setter.set_parameter(param, p[k]);
            setter.end_set_parameter(param);
        }
    };

    set_slot(slot_a, b_en, b_ty, b_stk, b_p);
    set_slot(slot_b, a_en, a_ty, a_stk, a_p);
    swap_mod_destinations_for_slots(params, slot_a, slot_b);
}

#[inline]
fn paint_clipper_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    slot_idx: usize,
    params: &JSynPluginParams,
    mod_ui: &ModUiState,
    pal: &ThemePalette,
) {
    let radius = 3.5 * dpx;
    let enabled = fx_slot_enabled_fn(slot_idx)(params);
    paint_scope_bg(canvas, bounds, radius, pal, enabled);

    let pad_x = 4.0 * dpx;
    let pad_y = 3.0 * dpx;
    let ix = bounds.x + pad_x;
    let iy = bounds.y + pad_y;
    let iw = (bounds.w - 2.0 * pad_x).max(1.0);
    let ih = (bounds.h - 2.0 * pad_y).max(1.0);
    let bottom = iy + ih;

    let mut pre = [0.0_f32; FX_SCOPE_LEN];
    let mut post = [0.0_f32; FX_SCOPE_LEN];
    let gr_act = mod_ui.telemetry.fx_scopes[slot_idx].read_snapshot(&mut pre, &mut post);

    let slot = &params.fx_slots[slot_idx];
    let tel = &mod_ui.telemetry;
    let drive_norm =
        (slot.p1.value() + tel.dest_offset(ModDestination::fx_param(slot_idx, 0))).clamp(0.0, 1.0);
    let ceil_norm =
        (slot.p2.value() + tel.dest_offset(ModDestination::fx_param(slot_idx, 1))).clamp(0.0, 1.0);
    let drive_lin = 10.0_f32.powf((drive_norm * 24.0) * 0.05);
    let ceil_db = -18.0 + ceil_norm * 18.0;
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
        canvas.fill_path(&bar, &vg::Paint::color(vg::Color::rgba(255, 110, 90, 210)));
    }

    paint_scope_border(canvas, bounds, dpx, radius, pal, enabled);
}

#[inline]
fn paint_filter_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    slot_idx: usize,
    params: &JSynPluginParams,
    mod_ui: &ModUiState,
    pal: &ThemePalette,
) {
    let radius = 3.5 * dpx;
    let enabled = fx_slot_enabled_fn(slot_idx)(params);
    paint_scope_bg(canvas, bounds, radius, pal, enabled);

    let pad_x = 4.0 * dpx;
    let pad_y = 3.0 * dpx;
    let ix = bounds.x + pad_x;
    let iy = bounds.y + pad_y;
    let iw = (bounds.w - 2.0 * pad_x).max(1.0);
    let ih = (bounds.h - 2.0 * pad_y).max(1.0);

    let slot = &params.fx_slots[slot_idx];
    let tel = &mod_ui.telemetry;
    let eff = [
        (slot.p1.value() + tel.dest_offset(ModDestination::fx_param(slot_idx, 0))).clamp(0.0, 1.0),
        (slot.p2.value() + tel.dest_offset(ModDestination::fx_param(slot_idx, 1))).clamp(0.0, 1.0),
        (slot.p3.value() + tel.dest_offset(ModDestination::fx_param(slot_idx, 2))).clamp(0.0, 1.0),
        (slot.p4.value() + tel.dest_offset(ModDestination::fx_param(slot_idx, 3))).clamp(0.0, 1.0),
    ];
    let mut fp = FxRackState::filter_params_from_fx(eff);
    fp.enabled = enabled;

    const DB_MIN: f32 = -24.0;
    const DB_MAX: f32 = 18.0;
    paint_filter_freq_grid(
        canvas,
        ix,
        iy,
        iw,
        ih,
        dpx,
        pal,
        enabled,
        &[(0.0, 165, 75)],
        DB_MIN,
        DB_MAX,
    );
    paint_filter_response_curve(
        canvas,
        &fp,
        ix,
        iy,
        iw,
        ih,
        DB_MIN,
        DB_MAX,
        64,
        dpx,
        1.6 * dpx,
        fx_accent_rgb(FxTypeParam::Filter),
        pal,
    );

    paint_scope_border(canvas, bounds, dpx, radius, pal, enabled);
}

#[inline]
fn paint_ott_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    slot_idx: usize,
    params: &JSynPluginParams,
    mod_ui: &ModUiState,
    pal: &ThemePalette,
) {
    let radius = 3.5 * dpx;
    let enabled = fx_slot_enabled_fn(slot_idx)(params);
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
    let activity = mod_ui.telemetry.fx_scopes[slot_idx].read_snapshot(&mut pre, &mut post);
    let depth = params.fx_slots[slot_idx].p1.value().clamp(0.0, 1.0);
    let pre_pk = pre[FX_SCOPE_LEN - 1].clamp(0.0, 1.5);
    let post_pk = post[FX_SCOPE_LEN - 1].clamp(0.0, 1.5);

    // Center 0 dB line
    let mut zero_line = vg::Path::new();
    zero_line.move_to(ix, mid_y);
    zero_line.line_to(ix + iw, mid_y);
    let mut zero_paint = vg::Paint::color(theme::vg_color(
        &pal.scope_grid,
        if enabled { 170 } else { 70 },
    ));
    zero_paint.set_line_width(1.0 * dpx);
    canvas.stroke_path(&zero_line, &zero_paint);

    // 3-band (LOW / MID / HIGH) upward/downward gain-reduction bars
    let band_colors = [(72, 200, 185), (120, 222, 195), (170, 235, 210)];
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
fn paint_distortion_scope(
    canvas: &mut Canvas,
    bounds: BoundingBox,
    dpx: f32,
    slot_idx: usize,
    params: &JSynPluginParams,
    mod_ui: &ModUiState,
    pal: &ThemePalette,
) {
    let radius = 3.5 * dpx;
    let enabled = fx_slot_enabled_fn(slot_idx)(params);
    paint_scope_bg(canvas, bounds, radius, pal, enabled);

    let pad_x = 5.0 * dpx;
    let pad_y = 4.0 * dpx;
    let ix = bounds.x + pad_x;
    let iy = bounds.y + pad_y;
    let iw = (bounds.w - 2.0 * pad_x).max(1.0);
    let ih = (bounds.h - 2.0 * pad_y).max(1.0);

    // Subtle 0-level reference axis
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
    let _ = mod_ui.telemetry.fx_scopes[slot_idx].read_snapshot(&mut pre, &mut post);

    // Live post waveform filled silhouette behind waveshaper curve
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

    // Live transfer curve from DRIVE & TYPE
    let submode = (params.fx_slots[slot_idx].p1.value() * 3.99) as usize;
    let drive = 1.0 + params.fx_slots[slot_idx].p2.value().clamp(0.0, 1.0) * 11.0;
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
    let curve_rgb = scope_trace_rgb((255, 162, 92), pal, enabled);
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
    slot_idx: usize,
    fx_ty: FxTypeParam,
    params: &JSynPluginParams,
    mod_ui: &ModUiState,
    pal: &ThemePalette,
) {
    let radius = 3.5 * dpx;
    let enabled = fx_slot_enabled_fn(slot_idx)(params);
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
    let _ = mod_ui.telemetry.fx_scopes[slot_idx].read_snapshot(&mut pre, &mut post);

    let slot = &params.fx_slots[slot_idx];
    let p1 = slot.p1.value().clamp(0.0, 1.0);
    let p2 = slot.p2.value().clamp(0.0, 1.0);
    let p3 = slot.p3.value().clamp(0.0, 1.0);

    // Structural FX envelope (Delay taps, Reverb decay tail, Chorus dual LFO, Phaser notch sweep)
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
                let mut tp = vg::Paint::color(vg::Color::rgba(ar, ag, ab, if enabled { 130 } else { 55 }));
                tp.set_line_width(2.0 * dpx);
                canvas.stroke_path(&tap, &tp);
                x += spacing;
                amp *= fb;
            }
        }
        FxTypeParam::Reverb => {
            let decay = 1.2 + (1.0 - p2) * 3.5;
            let mut tail = vg::Path::new();
            tail.move_to(ix, bottom);
            for i in 0..32 {
                let t = i as f32 / 31.0;
                let env = (-t * decay).exp() * (0.4 + p1 * 0.6);
                tail.line_to(ix + t * iw, bottom - env.clamp(0.0, 1.0) * ih);
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
            let mut lp = vg::Paint::color(vg::Color::rgba(ar, ag, ab, if enabled { 115 } else { 50 }));
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
            let mut np = vg::Paint::color(vg::Color::rgba(ar, ag, ab, if enabled { 130 } else { 55 }));
            np.set_line_width(1.25 * dpx);
            canvas.stroke_path(&notch_curve, &np);
        }
        _ => {}
    }

    // Live filled post waveform trace
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

/// Multi-FX live waveform, filter curve, & activity visualizer used in `MAIN` and `FX` views.
struct FxSlotVisualizer {
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    mod_ui: Arc<ModUiState>,
}

impl FxSlotVisualizer {
    fn new(
        cx: &mut Context,
        slot_idx: usize,
        params: Arc<JSynPluginParams>,
        mod_ui: Arc<ModUiState>,
    ) -> Handle<'_, Self> {
        Self {
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
        let fx_ty = self.params.fx_slots[self.slot_idx].fx_type.value();
        match fx_ty {
            FxTypeParam::JSatClipper => {
                paint_clipper_scope(
                    canvas,
                    bounds,
                    dpx,
                    self.slot_idx,
                    &self.params,
                    &self.mod_ui,
                    &pal,
                );
            }
            FxTypeParam::Filter => {
                paint_filter_scope(
                    canvas,
                    bounds,
                    dpx,
                    self.slot_idx,
                    &self.params,
                    &self.mod_ui,
                    &pal,
                );
            }
            FxTypeParam::Ott => {
                paint_ott_scope(
                    canvas,
                    bounds,
                    dpx,
                    self.slot_idx,
                    &self.params,
                    &self.mod_ui,
                    &pal,
                );
            }
            FxTypeParam::Distortion => {
                paint_distortion_scope(
                    canvas,
                    bounds,
                    dpx,
                    self.slot_idx,
                    &self.params,
                    &self.mod_ui,
                    &pal,
                );
            }
            FxTypeParam::Delay
            | FxTypeParam::Chorus
            | FxTypeParam::Reverb
            | FxTypeParam::Phaser => {
                paint_time_fx_scope(
                    canvas,
                    bounds,
                    dpx,
                    self.slot_idx,
                    fx_ty,
                    &self.params,
                    &self.mod_ui,
                    &pal,
                );
            }
            FxTypeParam::Off => {}
        }
    }
}

fn build_fx_knob_row(
    cx: &mut Context,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    mod_ui: Arc<ModUiState>,
    knob_sz: f32,
    tab_tag: u8,
) {
    let slot_check = fx_slot_enabled_fn(slot_idx);
    HStack::new(cx, move |cx| {
        for p_idx in 0..4 {
            let p_dim = params.clone();
            let mod_ui = mod_ui.clone();
            Binding::new(
                cx,
                UiData::params.map(move |p| p.fx_slots[slot_idx].fx_type.value() as usize),
                move |cx, fx_idx_lens| {
                    let fx_ty = FxTypeParam::from(FxType::from_index(fx_idx_lens.get(cx)));
                    let title = fx_param_label(fx_ty, p_idx);
                    let accent = fx_accent_rgb(fx_ty);
                    let p_cell = p_dim.clone();
                    let mod_cell = mod_ui.clone();
                    let formatter = Arc::new(move |norm: f32, _off: Option<f32>| {
                        format_fx_param_value(fx_ty, p_idx, norm)
                    });
                    VStack::new(cx, move |cx| {
                        bold_label(cx, title, "knob-title")
                            .width(Stretch(1.0))
                            .height(Pixels(13.0))
                            .child_space(Stretch(1.0));

                        RotaryKnob::new_with_mod_formatter(
                            cx,
                            UiData::params,
                            move |p| match p_idx {
                                0 => &p.fx_slots[slot_idx].p1,
                                1 => &p.fx_slots[slot_idx].p2,
                                2 => &p.fx_slots[slot_idx].p3,
                                _ => &p.fx_slots[slot_idx].p4,
                            },
                            false,
                            accent,
                            ModDestination::fx_param(slot_idx, p_idx),
                            mod_cell,
                            tab_tag,
                            formatter,
                        )
                        .enabled_by(p_cell, slot_check)
                        .width(Pixels(knob_sz))
                        .height(Pixels(knob_sz))
                        .left(Stretch(1.0))
                        .right(Stretch(1.0));
                    })
                    .class("knob-cell")
                    .width(Stretch(1.0))
                    .height(Auto)
                    .row_between(Pixels(1.0));
                },
            );
        }
    })
    .col_between(Pixels(4.0))
    .width(Stretch(1.0))
    .height(Auto);
}

fn set_fx_slot_type_with_defaults(
    params: &JSynPluginParams,
    gui_context: &Arc<dyn GuiContext>,
    slot_idx: usize,
    val: FxTypeParam,
) {
    let setter = ParamSetter::new(gui_context.as_ref());
    let slot = &params.fx_slots[slot_idx];
    let prev = slot.fx_type.value();
    setter.begin_set_parameter(&slot.fx_type);
    setter.set_parameter(&slot.fx_type, val);
    setter.end_set_parameter(&slot.fx_type);
    if prev != val {
        let defs = j_syn_dsp::FxSlotParams::default_params_for_type(val.into());
        for (p, def_v) in slot.params_array().into_iter().zip(defs.into_iter()) {
            setter.begin_set_parameter(p);
            setter.set_parameter(p, def_v);
            setter.end_set_parameter(p);
        }
    }
}

fn build_fx_type_dropdown(
    cx: &mut Context,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
) {
    use nih_plug::prelude::Param;
    use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

    let p_scroll = params.clone();
    let g_scroll = gui_context.clone();
    ScrollDropdown::new(
        cx,
        move |_cx, delta| {
            let cur = p_scroll.fx_slots[slot_idx].fx_type.value();
            let pos = FX_TYPE_ITEMS
                .iter()
                .position(|&(_, v)| v == cur)
                .unwrap_or(0);
            let next = (pos as isize + delta).clamp(0, FX_TYPE_ITEMS.len() as isize - 1) as usize;
            if next != pos {
                set_fx_slot_type_with_defaults(&p_scroll, &g_scroll, slot_idx, FX_TYPE_ITEMS[next].1);
            }
        },
        move |cx| {
            Dropdown::new(
                cx,
                move |cx| {
                    let text_lens = ParamWidgetBase::make_lens(
                        UiData::params,
                        move |p| &p.fx_slots[slot_idx].fx_type,
                        |param| {
                            param.normalized_value_to_string(
                                param.modulated_normalized_value(),
                                true,
                            )
                        },
                    );
                    HStack::new(cx, move |cx| {
                        build_dropdown_trigger_labels(cx, text_lens);
                    })
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_top(Pixels(0.0))
                    .child_bottom(Pixels(0.0))
                    .overflow(Overflow::Hidden)
                },
                move |cx| {
                    for &(label, val) in FX_TYPE_ITEMS {
                        let p_ref = params.clone();
                        let gui_ctx = gui_context.clone();
                        bold_label(cx, label, "dropdown-item").on_press(move |cx| {
                            set_fx_slot_type_with_defaults(&p_ref, &gui_ctx, slot_idx, val);
                            cx.emit(PopupEvent::Close);
                        });
                    }
                },
            )
        },
    )
    .width(Stretch(1.0))
    .height(Pixels(19.0));
}

fn build_fx_slot_header(
    cx: &mut Context,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    show_reorder: bool,
) {
    let slot_title: &'static str = match slot_idx {
        0 => "FX 1",
        1 => "FX 2",
        2 => "FX 3",
        3 => "FX 4",
        4 => "FX 5",
        5 => "FX 6",
        6 => "FX 7",
        _ => "FX 8",
    };

    HStack::new(cx, move |cx| {
        LedToggleSwitch::new(
            cx,
            UiData::params,
            move |p| &p.fx_slots[slot_idx].enabled,
            "OFF",
            "ON",
            (245, 179, 73),
        )
        .width(Pixels(48.0))
        .height(Pixels(18.0))
        .top(Stretch(1.0))
        .bottom(Stretch(1.0));

        bold_label(cx, slot_title, "module-title")
            .width(Pixels(26.0))
            .height(Stretch(1.0))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0));

        build_fx_type_dropdown(cx, slot_idx, params.clone(), gui_context.clone());

        HStack::new(cx, move |cx| {
            build_inline_pill_cell(
                cx,
                UiData::params,
                move |p| &p.fx_slots[slot_idx].stack,
                "STK",
                4,
                1.0,
            )
            .class("stk-pill");
        })
        .width(Pixels(60.0))
        .height(Pixels(20.0));

        if show_reorder {
            let p_up = params.clone();
            let g_up = gui_context.clone();
            Button::new(
                cx,
                move |cx| {
                    if slot_idx > 0 {
                        swap_fx_slots(&p_up, g_up.as_ref(), slot_idx, slot_idx - 1);
                        cx.emit(UiEvent::ModUiChanged);
                    }
                },
                |cx| bold_label(cx, "▲", "fx-move-label").hoverable(false),
            )
            .class("fx-move-btn")
            .toggle_class("fx-move-disabled", UiData::mod_ui_rev.map(move |_| slot_idx == 0))
            .width(Pixels(18.0))
            .height(Pixels(18.0));

            let p_dn = params.clone();
            let g_dn = gui_context.clone();
            Button::new(
                cx,
                move |cx| {
                    if slot_idx < 7 {
                        swap_fx_slots(&p_dn, g_dn.as_ref(), slot_idx, slot_idx + 1);
                        cx.emit(UiEvent::ModUiChanged);
                    }
                },
                |cx| bold_label(cx, "▼", "fx-move-label").hoverable(false),
            )
            .class("fx-move-btn")
            .toggle_class("fx-move-disabled", UiData::mod_ui_rev.map(move |_| slot_idx == 7))
            .width(Pixels(18.0))
            .height(Pixels(18.0));
        }
    })
    .col_between(Pixels(4.0))
    .height(Pixels(20.0))
    .width(Stretch(1.0));
}

/// Builds the compact `FX 1`–`FX 4` Master FX column on the right side of `MAIN`.
pub(crate) fn build_main_fx_column(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    VStack::new(cx, move |cx| {
        for slot_idx in 0..4 {
            let p_card = params.clone();
            let g_card = gui_context.clone();
            let m_card = mod_ui.clone();
            let p_dim = params.clone();
            let check = fx_slot_enabled_fn(slot_idx);
            VStack::new(cx, move |cx| {
                build_fx_slot_header(cx, slot_idx, p_card.clone(), g_card.clone(), false);

                let p_scope = p_card.clone();
                let m_scope = m_card.clone();
                Binding::new(
                    cx,
                    UiData::params.map(move |p| p.fx_slots[slot_idx].fx_type.value() as usize),
                    move |cx, fx_idx_lens| {
                        if FxType::from_index(fx_idx_lens.get(cx)) != FxType::Off {
                            FxSlotVisualizer::new(cx, slot_idx, p_scope.clone(), m_scope.clone())
                                .width(Stretch(1.0))
                                .height(Stretch(1.0));
                        }
                    },
                );

                build_fx_knob_row(cx, slot_idx, p_card, m_card, 36.0, 0);
            })
            .class("hardware-module")
            .class("fx-compact-card")
            .toggle_class(
                "module-disabled",
                UiData::mod_ui_rev.map(move |_| !check(&p_dim)),
            )
            .width(Stretch(1.0))
            .height(Stretch(1.0))
            .row_between(Pixels(3.0));
        }
    })
    .row_between(Pixels(4.0))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}

/// Builds the full-screen `FX 1`–`FX 8` 2-column × 4-row Master FX Rack view (`MainViewMode::Fx`).
pub(crate) fn build_fx_rack_view(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    VStack::new(cx, move |cx| {
        HStack::new(cx, |cx| {
            bold_label(cx, "MASTER FX RACK", "module-title")
                .width(Auto)
                .height(Stretch(1.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0));
            bold_label(
                cx,
                "SERIAL SIGNAL FLOW:  FX 1 → FX 2 → FX 3 → FX 4 → FX 5 → FX 6 → FX 7 → FX 8   (STACK 1x–4x SERIES PER SLOT)",
                "fx-rack-sub",
            )
            .width(Stretch(1.0))
            .height(Stretch(1.0))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0));
        })
        .class("fx-rack-banner")
        .col_between(Pixels(12.0))
        .height(Pixels(24.0))
        .width(Stretch(1.0));

        for row in 0..4 {
            let p_row = params.clone();
            let g_row = gui_context.clone();
            let m_row = mod_ui.clone();
            HStack::new(cx, move |cx| {
                for col in 0..2 {
                    let slot_idx = row * 2 + col;
                    let p_card = p_row.clone();
                    let g_card = g_row.clone();
                    let m_card = m_row.clone();
                    let p_dim = p_row.clone();
                    let check = fx_slot_enabled_fn(slot_idx);
                    VStack::new(cx, move |cx| {
                        build_fx_slot_header(cx, slot_idx, p_card.clone(), g_card.clone(), true);

                        HStack::new(cx, move |cx| {
                            FxSlotVisualizer::new(cx, slot_idx, p_card.clone(), m_card.clone())
                                .width(Pixels(145.0))
                                .height(Stretch(1.0));

                            VStack::new(cx, move |cx| {
                                build_fx_knob_row(
                                    cx,
                                    slot_idx,
                                    p_card.clone(),
                                    m_card.clone(),
                                    42.0,
                                    FX_VIEW_TAG,
                                );
                            })
                            .width(Stretch(1.0))
                            .height(Stretch(1.0))
                            .child_top(Stretch(1.0))
                            .child_bottom(Stretch(1.0));
                        })
                        .col_between(Pixels(8.0))
                        .width(Stretch(1.0))
                        .height(Stretch(1.0));
                    })
                    .class("hardware-module")
                    .class("fx-full-card")
                    .toggle_class(
                        "module-disabled",
                        UiData::mod_ui_rev.map(move |_| !check(&p_dim)),
                    )
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .row_between(Pixels(6.0));
                }
            })
            .col_between(Pixels(6.0))
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        }
    })
    .row_between(Pixels(6.0))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}
