//! Dual per-voice multimode filter cards (`FILTER 1`, `FILTER 2`) with live frequency response scope
//! and context-sensitive 4th parameter (`SCREAM`, `FREQ 2`, `DAMP`, or `MIX`).

use std::sync::Arc;

use nih_plug::prelude::{GuiContext, Param};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::RawParamEvent;

use super::controls::{bold_label, build_enum_dropdown, build_flex_knob_cell};
use super::widgets::{LedToggleSwitch, RotaryKnob};
use super::UiData;
use crate::{FilterModeParam, FilterPluginParams, FilterSlopeParam, JSynPluginParams};

const FILTER_MODE_ITEMS: &[(&str, FilterModeParam)] = &[
    ("LP", FilterModeParam::Lp),
    ("BP", FilterModeParam::Bp),
    ("HP", FilterModeParam::Hp),
    ("Scream", FilterModeParam::Scream),
    ("Daft", FilterModeParam::Daft),
    ("Comb", FilterModeParam::Comb),
    ("HP + Peak", FilterModeParam::HpPeak),
    ("HP + Notch", FilterModeParam::HpNotch),
    ("LP + Notch", FilterModeParam::LpNotch),
    ("LP + Peak", FilterModeParam::LpPeak),
];

const FILTER_SLOPE_ITEMS: &[(&str, FilterSlopeParam)] = &[
    ("6 dB", FilterSlopeParam::Db6),
    ("12 dB", FilterSlopeParam::Db12),
    ("24 dB", FilterSlopeParam::Db24),
    ("36 dB", FilterSlopeParam::Db36),
    ("48 dB", FilterSlopeParam::Db48),
];

#[inline]
fn aux_label_for_mode(mode: FilterModeParam) -> &'static str {
    match mode {
        FilterModeParam::Scream => "SCREAM",
        FilterModeParam::HpPeak
        | FilterModeParam::HpNotch
        | FilterModeParam::LpNotch
        | FilterModeParam::LpPeak => "FREQ 2",
        FilterModeParam::Comb => "DAMP",
        _ => "MIX",
    }
}

#[inline]
fn format_freq_hz(hz: f32) -> String {
    if hz >= 10_000.0 {
        format!("{:.1} kHz", hz / 1000.0)
    } else if hz >= 1000.0 {
        format!("{:.2} kHz", hz / 1000.0)
    } else {
        format!("{:.0} Hz", hz)
    }
}

/// Live `femtovg` frequency-response curve preview for a single filter slot.
pub struct FilterResponseScope {
    filter_idx: usize,
    params: Arc<JSynPluginParams>,
    accent: (u8, u8, u8),
}

#[inline]
fn filter_preview_signature(filter: &FilterPluginParams) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut mix = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x100000001b3);
    };
    mix(filter.enabled.value() as u64);
    mix(filter.mode.value() as u64);
    mix(filter.slope.value() as u64);
    mix(filter.cutoff.value().to_bits() as u64);
    mix(filter.cutoff.modulated_normalized_value().to_bits() as u64);
    mix(filter.resonance.value().to_bits() as u64);
    mix(filter.resonance.modulated_normalized_value().to_bits() as u64);
    mix(filter.drive.value().to_bits() as u64);
    mix(filter.drive.modulated_normalized_value().to_bits() as u64);
    mix(filter.aux.value().to_bits() as u64);
    mix(filter.aux.modulated_normalized_value().to_bits() as u64);
    h
}

#[inline]
fn freq_to_norm(freq_hz: f32) -> f32 {
    ((freq_hz.clamp(20.0, 20_000.0) / 20.0).ln() / 1000.0_f32.ln()).clamp(0.0, 1.0)
}

#[inline]
fn db_to_y(db: f32, inner_top: f32, inner_h: f32) -> f32 {
    let norm = ((db + 36.0) / 60.0).clamp(0.0, 1.0);
    inner_top + (1.0 - norm) * inner_h
}

impl FilterResponseScope {
    pub fn new(
        cx: &mut Context,
        filter_idx: usize,
        params: Arc<JSynPluginParams>,
        accent: (u8, u8, u8),
    ) -> Handle<'_, Self> {
        Self {
            filter_idx,
            params,
            accent,
        }
        .build(cx, move |cx| {
            let lens = UiData::params.map(move |p| filter_preview_signature(&p.filters[filter_idx]));
            Binding::new(cx, lens, |cx, _| {
                cx.needs_redraw();
                Element::new(cx).hoverable(false);
            });
        })
    }
}

impl View for FilterResponseScope {
    fn element(&self) -> Option<&'static str> {
        Some("filter-response-scope")
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
        let radius = 4.0 * dpx;

        let mut bg = vg::Path::new();
        bg.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, radius);
        canvas.fill_path(&bg, &vg::Paint::color(vg::Color::rgb(22, 19, 16)));

        let pad_x = 5.0 * dpx;
        let pad_y = 4.0 * dpx;
        let inner_x = bounds.x + pad_x;
        let inner_y = bounds.y + pad_y;
        let inner_w = (bounds.w - 2.0 * pad_x).max(1.0);
        let inner_h = (bounds.h - 2.0 * pad_y).max(1.0);
        let inner_bottom = inner_y + inner_h;

        // Vertical logarithmic frequency grid (100 Hz, 1 kHz, 10 kHz)
        let mut grid_paint = vg::Paint::color(vg::Color::rgba(95, 82, 68, 70));
        grid_paint.set_line_width(1.0 * dpx);
        for freq in [100.0_f32, 1_000.0_f32, 10_000.0_f32] {
            let x = inner_x + freq_to_norm(freq) * inner_w;
            let mut line = vg::Path::new();
            line.move_to(x, inner_y);
            line.line_to(x, inner_bottom);
            canvas.stroke_path(&line, &grid_paint);
        }

        // Horizontal dB grid (+12, 0, -12, -24 dB)
        for &(db, alpha) in &[(-24.0_f32, 55_u8), (-12.0, 55), (0.0, 110), (12.0, 55)] {
            let y = db_to_y(db, inner_y, inner_h);
            let mut line = vg::Path::new();
            line.move_to(inner_x, y);
            line.line_to(inner_x + inner_w, y);
            let mut p = vg::Paint::color(vg::Color::rgba(110, 96, 80, alpha));
            p.set_line_width(1.0 * dpx);
            canvas.stroke_path(&line, &p);
        }

        let dsp_params = self.params.filters[self.filter_idx].to_dsp();
        const CURVE_PTS: usize = 96;
        let mut curve = vg::Path::new();
        let mut fill = vg::Path::new();

        for i in 0..CURVE_PTS {
            let t = i as f32 / (CURVE_PTS - 1) as f32;
            let freq_hz = 20.0 * 1000.0_f32.powf(t);
            let db = j_syn_dsp::evaluate_filter_response_db(&dsp_params, freq_hz);
            let x = inner_x + t * inner_w;
            let y = db_to_y(db, inner_y, inner_h);
            if i == 0 {
                curve.move_to(x, y);
                fill.move_to(x, inner_bottom);
                fill.line_to(x, y);
            } else {
                curve.line_to(x, y);
                fill.line_to(x, y);
            }
            if i + 1 == CURVE_PTS {
                fill.line_to(x, inner_bottom);
                fill.close();
            }
        }

        let (r, g, b) = self.accent;
        let fill_color = if dsp_params.enabled {
            vg::Color::rgba(r, g, b, 30)
        } else {
            vg::Color::rgba(140, 120, 98, 12)
        };
        canvas.fill_path(&fill, &vg::Paint::color(fill_color));

        let stroke_color = if dsp_params.enabled {
            vg::Color::rgba(r, g, b, 240)
        } else {
            vg::Color::rgba(140, 120, 98, 130)
        };
        let mut curve_paint = vg::Paint::color(stroke_color);
        curve_paint.set_line_width(1.7 * dpx);
        canvas.stroke_path(&curve, &curve_paint);

        // Cutoff marker dot on curve
        let fc_x = inner_x + freq_to_norm(dsp_params.cutoff_hz) * inner_w;
        let fc_db = j_syn_dsp::evaluate_filter_response_db(&dsp_params, dsp_params.cutoff_hz);
        let fc_y = db_to_y(fc_db, inner_y, inner_h);
        let mut dot = vg::Path::new();
        dot.circle(fc_x, fc_y, 2.8 * dpx);
        canvas.fill_path(&dot, &vg::Paint::color(stroke_color));

        // Secondary Freq 2 marker dot for multi-filter modes
        if dsp_params.mode.is_multi() {
            let f2_hz = j_syn_dsp::filter::aux_to_freq2_hz(dsp_params.aux);
            let f2_x = inner_x + freq_to_norm(f2_hz) * inner_w;
            let f2_db = j_syn_dsp::evaluate_filter_response_db(&dsp_params, f2_hz);
            let f2_y = db_to_y(f2_db, inner_y, inner_h);
            let mut dot2 = vg::Path::new();
            dot2.circle(f2_x, f2_y, 2.4 * dpx);
            canvas.fill_path(&dot2, &vg::Paint::color(vg::Color::rgba(246, 239, 230, 200)));
        }

        let mut border = vg::Path::new();
        border.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, radius);
        let mut border_paint = vg::Paint::color(vg::Color::rgb(68, 58, 50));
        border_paint.set_line_width(1.0 * dpx);
        canvas.stroke_path(&border, &border_paint);
    }
}

fn build_filter_aux_knob_cell(
    cx: &mut Context,
    filter_idx: usize,
    params: Arc<JSynPluginParams>,
    accent_rgb: (u8, u8, u8),
    knob_sz: f32,
) {
    let params_for_readout = params.clone();
    VStack::new(cx, move |cx| {
        HStack::new(
            cx,
            ParamWidgetBase::build_view(
                UiData::params,
                move |p| &p.filters[filter_idx].mode,
                move |cx, param_data| {
                    let title_lens =
                        param_data.make_lens(|param| aux_label_for_mode(param.value()).to_string());
                    bold_label(cx, title_lens, "knob-title")
                        .width(Stretch(1.0))
                        .height(Pixels(12.0))
                        .child_space(Stretch(1.0));
                },
            ),
        )
        .width(Stretch(1.0))
        .height(Pixels(12.0));

        RotaryKnob::new(
            cx,
            UiData::params,
            move |p| &p.filters[filter_idx].aux,
            false,
            accent_rgb,
        )
        .width(Pixels(knob_sz))
        .height(Pixels(knob_sz))
        .left(Stretch(1.0))
        .right(Stretch(1.0));

        HStack::new(
            cx,
            ParamWidgetBase::build_view(
                UiData::params,
                move |p| &p.filters[filter_idx].aux,
                move |cx, param_data| {
                    let p_ref = params_for_readout.clone();
                    let readout_lens = param_data.make_lens(move |aux_param| {
                        let mode = p_ref.filters[filter_idx].mode.value();
                        if matches!(
                            mode,
                            FilterModeParam::HpPeak
                                | FilterModeParam::HpNotch
                                | FilterModeParam::LpNotch
                                | FilterModeParam::LpPeak
                        ) {
                            let hz = j_syn_dsp::filter::aux_to_freq2_hz(
                                aux_param.modulated_plain_value(),
                            );
                            format_freq_hz(hz)
                        } else {
                            aux_param.normalized_value_to_string(
                                aux_param.modulated_normalized_value(),
                                true,
                            )
                        }
                    });
                    bold_label(cx, readout_lens, "knob-readout-text")
                        .width(Stretch(1.0))
                        .height(Stretch(1.0))
                        .child_space(Stretch(1.0));
                },
            ),
        )
        .class("knob-readout-box")
        .width(Stretch(1.0))
        .height(Pixels(15.0));
    })
    .class("knob-cell")
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(1.0));
}

pub(super) fn build_filter_panel(
    cx: &mut Context,
    filter_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    accent: (u8, u8, u8),
) {
    let label = if filter_idx == 0 {
        "FILTER 1"
    } else {
        "FILTER 2"
    };

    VStack::new(cx, move |cx| {
        HStack::new(cx, {
            let params = params.clone();
            let gui_context = gui_context.clone();
            move |cx| {
                LedToggleSwitch::new(
                    cx,
                    UiData::params,
                    move |p| &p.filters[filter_idx].enabled,
                    label,
                    label,
                    accent,
                )
                .width(Pixels(62.0))
                .height(Pixels(20.0));

                build_enum_dropdown(
                    cx,
                    params.clone(),
                    gui_context.clone(),
                    move |p| &p.filters[filter_idx].mode,
                    FILTER_MODE_ITEMS,
                )
                .width(Stretch(1.0))
                .height(Pixels(20.0));

                build_enum_dropdown(
                    cx,
                    params.clone(),
                    gui_context.clone(),
                    move |p| &p.filters[filter_idx].slope,
                    FILTER_SLOPE_ITEMS,
                )
                .width(Pixels(60.0))
                .height(Pixels(20.0));
            }
        })
        .col_between(Pixels(6.0))
        .height(Pixels(20.0))
        .width(Stretch(1.0));

        FilterResponseScope::new(cx, filter_idx, params.clone(), accent)
            .width(Stretch(1.0))
            .height(Pixels(86.0));

        HStack::new(cx, {
            let params = params.clone();
            move |cx| {
                build_flex_knob_cell(
                    cx,
                    UiData::params,
                    move |p| &p.filters[filter_idx].cutoff,
                    "CUTOFF",
                    false,
                    accent,
                    32.0,
                );
                build_flex_knob_cell(
                    cx,
                    UiData::params,
                    move |p| &p.filters[filter_idx].resonance,
                    "RES",
                    false,
                    accent,
                    32.0,
                );
                build_flex_knob_cell(
                    cx,
                    UiData::params,
                    move |p| &p.filters[filter_idx].drive,
                    "DRIVE",
                    false,
                    accent,
                    32.0,
                );
                build_filter_aux_knob_cell(cx, filter_idx, params.clone(), accent, 32.0);
            }
        })
        .col_between(Pixels(4.0))
        .width(Stretch(1.0))
        .height(Auto);
    })
    .class("hardware-module")
    .row_between(Pixels(4.0))
    .width(Stretch(1.0))
    .height(Pixels(188.0));
}
