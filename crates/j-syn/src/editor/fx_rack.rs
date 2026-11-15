//! Master FX Rack UI (`FX 1`–`FX 4` column on `MAIN` and full 8-slot `FX 1`–`FX 8` view
//! with `▲`/`▼` slot swapping, `STACK 1..4` control, context-sensitive FX knobs, and live scopes).

use std::sync::Arc;

use j_syn_dsp::mod_matrix::{ModDestination, FX_SCOPE_LEN};
use j_syn_dsp::{FxRackState, FxType};
use nih_plug::prelude::{GuiContext, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::RawParamEvent;

use super::controls::{bold_label, build_enum_dropdown, build_inline_pill_cell};
use super::mod_bar::{ModUiState, FX_VIEW_TAG};
use super::widgets::{KnobDimExt, LedToggleSwitch, RotaryKnob};
use super::{UiData, UiEvent};
use crate::{FxTypeParam, JSynPluginParams};

const FX_TYPE_ITEMS: &[(&str, FxTypeParam)] = &[
    ("Off", FxTypeParam::Off),
    ("OTT", FxTypeParam::Ott),
    ("Distortion", FxTypeParam::Distortion),
    ("Chorus", FxTypeParam::Chorus),
    ("Delay", FxTypeParam::Delay),
    ("Reverb", FxTypeParam::Reverb),
    ("J-SAT Clip", FxTypeParam::JSatClipper),
];

#[inline]
fn fx0_enabled(p: &JSynPluginParams) -> bool {
    let fx = &p.fx_slots[0];
    fx.enabled.value() && fx.fx_type.value() != FxTypeParam::Off
}
#[inline]
fn fx1_enabled(p: &JSynPluginParams) -> bool {
    let fx = &p.fx_slots[1];
    fx.enabled.value() && fx.fx_type.value() != FxTypeParam::Off
}
#[inline]
fn fx2_enabled(p: &JSynPluginParams) -> bool {
    let fx = &p.fx_slots[2];
    fx.enabled.value() && fx.fx_type.value() != FxTypeParam::Off
}
#[inline]
fn fx3_enabled(p: &JSynPluginParams) -> bool {
    let fx = &p.fx_slots[3];
    fx.enabled.value() && fx.fx_type.value() != FxTypeParam::Off
}
#[inline]
fn fx4_enabled(p: &JSynPluginParams) -> bool {
    let fx = &p.fx_slots[4];
    fx.enabled.value() && fx.fx_type.value() != FxTypeParam::Off
}
#[inline]
fn fx5_enabled(p: &JSynPluginParams) -> bool {
    let fx = &p.fx_slots[5];
    fx.enabled.value() && fx.fx_type.value() != FxTypeParam::Off
}
#[inline]
fn fx6_enabled(p: &JSynPluginParams) -> bool {
    let fx = &p.fx_slots[6];
    fx.enabled.value() && fx.fx_type.value() != FxTypeParam::Off
}
#[inline]
fn fx7_enabled(p: &JSynPluginParams) -> bool {
    let fx = &p.fx_slots[7];
    fx.enabled.value() && fx.fx_type.value() != FxTypeParam::Off
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
) {
    let radius = 3.5 * dpx;
    let enabled = fx_slot_enabled_fn(slot_idx)(params);
    let mut bg = vg::Path::new();
    bg.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, radius);
    canvas.fill_path(&bg, &vg::Paint::color(vg::Color::rgb(20, 17, 14)));

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

    let ceil_norm = params.fx_slots[slot_idx].p2.value().clamp(0.0, 1.0);
    let ceil_db = -18.0 + ceil_norm * 18.0;
    let ceil_lin = 10.0_f32.powf(ceil_db * 0.05).clamp(0.05, 1.0);
    let y_scale = 1.25_f32;

    let to_y = |peak: f32| -> f32 {
        let norm = (peak / y_scale).clamp(0.0, 1.0);
        bottom - norm * ih
    };

    // Ceiling line
    let ceil_y = to_y(ceil_lin);
    let mut ceil_path = vg::Path::new();
    ceil_path.move_to(ix, ceil_y);
    ceil_path.line_to(ix + iw, ceil_y);
    let mut ceil_paint = vg::Paint::color(vg::Color::rgba(
        245,
        86,
        86,
        if enabled { 185 } else { 70 },
    ));
    ceil_paint.set_line_width(1.1 * dpx);
    canvas.stroke_path(&ceil_path, &ceil_paint);

    // Pre-clip waveform (amber) and Post-clip waveform (crimson)
    let mut pre_path = vg::Path::new();
    let mut post_path = vg::Path::new();
    for i in 0..FX_SCOPE_LEN {
        let x = ix + (i as f32 / (FX_SCOPE_LEN - 1) as f32) * iw;
        let py = to_y(pre[i]);
        let cy = to_y(post[i]);
        if i == 0 {
            pre_path.move_to(x, py);
            post_path.move_to(x, cy);
        } else {
            pre_path.line_to(x, py);
            post_path.line_to(x, cy);
        }
    }
    let mut pre_paint = vg::Paint::color(vg::Color::rgba(
        245,
        179,
        73,
        if enabled { 145 } else { 55 },
    ));
    pre_paint.set_line_width(1.1 * dpx);
    canvas.stroke_path(&pre_path, &pre_paint);

    let mut post_paint = vg::Paint::color(vg::Color::rgba(
        245,
        86,
        86,
        if enabled { 235 } else { 85 },
    ));
    post_paint.set_line_width(1.5 * dpx);
    canvas.stroke_path(&post_path, &post_paint);

    // Compact GR activity bar along right edge
    if gr_act > 0.005 && enabled {
        let bar_w = 3.5 * dpx;
        let bar_h = (gr_act.clamp(0.0, 1.0) * ih).max(2.0 * dpx);
        let mut bar = vg::Path::new();
        bar.rect(ix + iw - bar_w, iy, bar_w, bar_h);
        canvas.fill_path(&bar, &vg::Paint::color(vg::Color::rgba(255, 110, 90, 210)));
    }

    let mut border = vg::Path::new();
    border.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, radius);
    let mut b_paint = vg::Paint::color(vg::Color::rgba(
        90,
        74,
        62,
        if enabled { 160 } else { 75 },
    ));
    b_paint.set_line_width(1.0 * dpx);
    canvas.stroke_path(&border, &b_paint);
}

/// Live compact scope displayed in `MAIN` FX cards when `J-SAT Clipper` is active,
/// and inside the full `FX` Rack slot cards.
struct JSatClipperScope {
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    mod_ui: Arc<ModUiState>,
}

impl JSatClipperScope {
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

impl View for JSatClipperScope {
    fn element(&self) -> Option<&'static str> {
        Some("jsat-clipper-scope")
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
        paint_clipper_scope(
            canvas,
            bounds,
            cx.scale_factor(),
            self.slot_idx,
            &self.params,
            &self.mod_ui,
        );
    }
}

/// Multi-FX live waveform & activity visualizer used in the full 2×4 `FX` rack view.
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
        let fx_ty = self.params.fx_slots[self.slot_idx].fx_type.value();
        if fx_ty == FxTypeParam::JSatClipper {
            paint_clipper_scope(
                canvas,
                bounds,
                dpx,
                self.slot_idx,
                &self.params,
                &self.mod_ui,
            );
            return;
        }

        let enabled = fx_slot_enabled_fn(self.slot_idx)(&self.params);
        let radius = 4.0 * dpx;
        let mut bg = vg::Path::new();
        bg.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, radius);
        canvas.fill_path(&bg, &vg::Paint::color(vg::Color::rgb(20, 17, 14)));

        let (ar, ag, ab) = fx_accent_rgb(fx_ty);
        let pad_x = 5.0 * dpx;
        let pad_y = 4.0 * dpx;
        let ix = bounds.x + pad_x;
        let iy = bounds.y + pad_y;
        let iw = (bounds.w - 2.0 * pad_x).max(1.0);
        let ih = (bounds.h - 2.0 * pad_y).max(1.0);
        let bottom = iy + ih;

        let mut pre = [0.0_f32; FX_SCOPE_LEN];
        let mut post = [0.0_f32; FX_SCOPE_LEN];
        let activity = self.mod_ui.telemetry.fx_scopes[self.slot_idx].read_snapshot(&mut pre, &mut post);

        // Pre-signal dim baseline + post-signal accented trace
        let mut pre_path = vg::Path::new();
        let mut post_path = vg::Path::new();
        for i in 0..FX_SCOPE_LEN {
            let x = ix + (i as f32 / (FX_SCOPE_LEN - 1) as f32) * iw;
            let py = bottom - (pre[i] / 1.25).clamp(0.0, 1.0) * ih;
            let cy = bottom - (post[i] / 1.25).clamp(0.0, 1.0) * ih;
            if i == 0 {
                pre_path.move_to(x, py);
                post_path.move_to(x, cy);
            } else {
                pre_path.line_to(x, py);
                post_path.line_to(x, cy);
            }
        }
        let mut pre_paint = vg::Paint::color(vg::Color::rgba(140, 125, 110, if enabled { 95 } else { 35 }));
        pre_paint.set_line_width(1.0 * dpx);
        canvas.stroke_path(&pre_path, &pre_paint);

        let mut post_paint = vg::Paint::color(vg::Color::rgba(
            ar,
            ag,
            ab,
            if enabled { 225 } else { 75 },
        ));
        post_paint.set_line_width(1.4 * dpx);
        canvas.stroke_path(&post_path, &post_paint);

        // Subtle activity meter bar on right
        let meter_h = (activity.clamp(0.0, 1.0) * ih).max(0.0);
        if meter_h > 0.5 && enabled {
            let mut m = vg::Path::new();
            m.rect(ix + iw - 3.0 * dpx, bottom - meter_h, 3.0 * dpx, meter_h);
            canvas.fill_path(&m, &vg::Paint::color(vg::Color::rgba(ar, ag, ab, 185)));
        }

        let mut border = vg::Path::new();
        border.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, radius);
        let mut b_paint = vg::Paint::color(vg::Color::rgba(80, 68, 56, if enabled { 150 } else { 70 }));
        b_paint.set_line_width(1.0 * dpx);
        canvas.stroke_path(&border, &b_paint);
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

fn build_fx_type_dropdown(
    cx: &mut Context,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
) {
    build_enum_dropdown(
        cx,
        params,
        gui_context,
        move |p| &p.fx_slots[slot_idx].fx_type,
        FX_TYPE_ITEMS,
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
        .width(Pixels(36.0))
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
            );
        })
        .width(Pixels(48.0))
        .height(Pixels(19.0));

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
                        if FxType::from_index(fx_idx_lens.get(cx)) == FxType::JSatClipper {
                            JSatClipperScope::new(cx, slot_idx, p_scope.clone(), m_scope.clone())
                                .width(Stretch(1.0))
                                .height(Pixels(26.0));
                        }
                    },
                );

                build_fx_knob_row(cx, slot_idx, p_card, m_card, 32.0, 0);
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
                                    38.0,
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
