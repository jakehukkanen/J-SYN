//! Dual per-voice multimode filter cards (`FILTER 1`, `FILTER 2`) with live frequency response scope
//! and context-sensitive 4th parameter (`SCREAM`, `FREQ 2`, `DAMP`, or `MIX`).

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use nih_plug::prelude::{GuiContext, Param};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::RawParamEvent;

use super::controls::{
    bold_label, build_enum_dropdown, build_enum_dropdown_with_override, build_flex_knob_cell_mod_dim,
};
use super::mod_bar::ModUiState;
use super::scope::{
    filter_db_to_raw_y, filter_freq_to_norm, paint_filter_freq_grid, paint_filter_response_curve,
    paint_scope_bg, paint_scope_border, scope_trace_rgb,
};
use super::tokens;
use super::widgets::{KnobDimExt, LedToggleSwitch, RotaryKnob};
use super::{theme, UiData, AMBER_RGB, CORAL_RGB};
use crate::{FilterModeParam, FilterPluginParams, FilterSlopeParam, JSynPluginParams};

#[inline]
fn filter0_enabled(p: &JSynPluginParams) -> bool {
    p.filters[0].enabled.value()
}

#[inline]
fn filter1_enabled(p: &JSynPluginParams) -> bool {
    p.filters[1].enabled.value()
}

#[inline]
fn filter_enabled_fn(filter_idx: usize) -> fn(&JSynPluginParams) -> bool {
    if filter_idx == 0 {
        filter0_enabled
    } else {
        filter1_enabled
    }
}

pub(super) const FILTER_MODE_ITEMS: &[(&str, FilterModeParam)] = &[
    ("LP", FilterModeParam::Lp),
    ("BP", FilterModeParam::Bp),
    ("HP", FilterModeParam::Hp),
    ("Notch", FilterModeParam::Notch),
    ("Bandreject", FilterModeParam::Bandreject),
    ("Peak", FilterModeParam::Peak),
    ("Scream", FilterModeParam::Scream),
    ("Daft", FilterModeParam::Daft),
    ("Comb", FilterModeParam::Comb),
    ("HP + Peak", FilterModeParam::HpPeak),
    ("HP + Notch", FilterModeParam::HpNotch),
    ("LP + Notch", FilterModeParam::LpNotch),
    ("LP + Peak", FilterModeParam::LpPeak),
    ("Peak + Peak", FilterModeParam::PeakPeak),
    ("Notch + Notch", FilterModeParam::NotchNotch),
];

const FILTER_SLOPE_ITEMS: &[(&str, FilterSlopeParam)] = &[
    ("6 dB", FilterSlopeParam::Db6),
    ("12 dB", FilterSlopeParam::Db12),
    ("24 dB", FilterSlopeParam::Db24),
    ("36 dB", FilterSlopeParam::Db36),
    ("48 dB", FilterSlopeParam::Db48),
];

#[inline]
fn is_multi_mode_param(mode: FilterModeParam) -> bool {
    j_syn_dsp::FilterMode::from(mode).is_multi()
}

#[inline]
fn aux_label_for_mode(mode: FilterModeParam) -> &'static str {
    if is_multi_mode_param(mode) {
        return "FREQ 2";
    }
    match mode {
        FilterModeParam::Scream => "SCREAM",
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
    mod_ui: Arc<ModUiState>,
    #[allow(dead_code)]
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

impl FilterResponseScope {
    pub fn new(
        cx: &mut Context,
        filter_idx: usize,
        params: Arc<JSynPluginParams>,
        mod_ui: Arc<ModUiState>,
        accent: (u8, u8, u8),
    ) -> Handle<'_, Self> {
        Self {
            filter_idx,
            params,
            mod_ui,
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
        let base_params = self.params.filters[self.filter_idx].to_dsp();
        let [live_fc, live_res, live_drive, live_aux] =
            self.mod_ui.telemetry.filter_live(self.filter_idx);
        let mut dsp_params = base_params;
        if live_fc >= 20.0 {
            dsp_params.cutoff_hz = live_fc.clamp(20.0, 20_000.0);
            dsp_params.resonance = live_res.clamp(0.0, 1.0);
            dsp_params.drive = live_drive.clamp(0.0, 1.0);
            dsp_params.aux = live_aux.clamp(0.0, 1.0);
        }
        let enabled = dsp_params.enabled;

        let pal = theme::current_palette(cx);
        paint_scope_bg(canvas, bounds, radius, &pal, enabled);

        let pad_x = 5.0 * dpx;
        let pad_y = 4.0 * dpx;
        let inner_x = bounds.x + pad_x;
        let inner_y = bounds.y + pad_y;
        let inner_w = (bounds.w - 2.0 * pad_x).max(1.0);
        let inner_h = (bounds.h - 2.0 * pad_y).max(1.0);
        let inner_bottom = inner_y + inner_h;

        const DB_MIN: f32 = -36.0;
        const DB_MAX: f32 = 24.0;
        paint_filter_freq_grid(
            canvas,
            inner_x,
            inner_y,
            inner_w,
            inner_h,
            dpx,
            &pal,
            enabled,
            &[(-24.0, 85, 35), (-12.0, 85, 35), (0.0, 180, 80), (12.0, 85, 35)],
            DB_MIN,
            DB_MAX,
        );

        let rgb = if self.filter_idx == 0 {
            pal.accent_rgb()
        } else {
            pal.secondary_rgb()
        };
        paint_filter_response_curve(
            canvas,
            &dsp_params,
            inner_x,
            inner_y,
            inner_w,
            inner_h,
            DB_MIN,
            DB_MAX,
            96,
            dpx,
            1.7 * dpx,
            rgb,
            &pal,
        );

        // Ghost base cutoff marker when modulated
        if (dsp_params.cutoff_hz - base_params.cutoff_hz).abs() / base_params.cutoff_hz.max(20.0)
            > 0.005
        {
            let trace_rgb = scope_trace_rgb(rgb, &pal, enabled);
            let base_x = inner_x + filter_freq_to_norm(base_params.cutoff_hz) * inner_w;
            let base_db =
                j_syn_dsp::evaluate_filter_response_db(&base_params, base_params.cutoff_hz);
            let base_raw_y = filter_db_to_raw_y(base_db, inner_y, inner_h, DB_MIN, DB_MAX);
            if base_raw_y < inner_bottom - 1.0 * dpx {
                let mut ghost = vg::Path::new();
                ghost.circle(base_x, base_raw_y.max(inner_y), 2.4 * dpx);
                let mut ghost_paint = vg::Paint::color(vg::Color::rgba(
                    trace_rgb.0,
                    trace_rgb.1,
                    trace_rgb.2,
                    if enabled { 115 } else { 55 },
                ));
                ghost_paint.set_line_width(1.1 * dpx);
                canvas.stroke_path(&ghost, &ghost_paint);
            }
        }

        // Secondary Freq 2 marker dot for multi-filter modes
        if dsp_params.mode.is_multi() {
            let f2_hz = j_syn_dsp::filter::aux_to_freq2_hz(dsp_params.aux);
            let f2_x = inner_x + filter_freq_to_norm(f2_hz) * inner_w;
            let f2_db = j_syn_dsp::evaluate_filter_response_db(&dsp_params, f2_hz);
            let f2_raw_y = filter_db_to_raw_y(f2_db, inner_y, inner_h, DB_MIN, DB_MAX);
            if f2_raw_y < inner_bottom - 1.0 * dpx {
                let mut dot2 = vg::Path::new();
                dot2.circle(f2_x, f2_raw_y.max(inner_y), 2.4 * dpx);
                canvas.fill_path(
                    &dot2,
                    &vg::Paint::color(if enabled {
                        theme::vg_color(&pal.text_bright, 210)
                    } else {
                        theme::vg_color(&pal.text_dim, 95)
                    }),
                );
            }
        }

        paint_scope_border(canvas, bounds, dpx, radius, &pal, enabled);
    }
}

fn build_filter_aux_knob_cell(
    cx: &mut Context,
    filter_idx: usize,
    params: Arc<JSynPluginParams>,
    mod_ui: Arc<ModUiState>,
    accent_rgb: (u8, u8, u8),
    knob_sz: f32,
) {
    let params_for_tt = params.clone();
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
                        .height(Pixels(14.0))
                        .child_space(Stretch(1.0));
                },
            ),
        )
        .width(Stretch(1.0))
        .height(Pixels(14.0));

        let p_tt = params_for_tt.clone();
        let filter_check = filter_enabled_fn(filter_idx);
        RotaryKnob::new_with_mod_formatter(
            cx,
            UiData::params,
            move |p| &p.filters[filter_idx].aux,
            false,
            accent_rgb,
            ModDestination::filter_aux(filter_idx),
            mod_ui,
            0,
            Arc::new(move |norm, _mod_offset| {
                let mode = p_tt.filters[filter_idx].mode.value();
                if is_multi_mode_param(mode) {
                    let hz = j_syn_dsp::filter::aux_to_freq2_hz(norm);
                    format_freq_hz(hz)
                } else {
                    p_tt.filters[filter_idx]
                        .aux
                        .normalized_value_to_string(norm, true)
                }
            }),
        )
        .enabled_by(params, filter_check)
        .width(Pixels(knob_sz))
        .height(Pixels(knob_sz))
        .left(Stretch(1.0))
        .right(Stretch(1.0));
    })
    .class("knob-cell")
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(2.0));
}

pub(super) fn build_filter_panel(
    cx: &mut Context,
    filter_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
    accent: (u8, u8, u8),
) {
    let label = if filter_idx == 0 {
        "FILTER 1"
    } else {
        "FILTER 2"
    };
    let filter_check = filter_enabled_fn(filter_idx);
    let p_dim = params.clone();

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

                build_enum_dropdown_with_override(
                    cx,
                    params.clone(),
                    gui_context.clone(),
                    move |p| &p.filters[filter_idx].slope,
                    FILTER_SLOPE_ITEMS,
                    move |p| {
                        let mode = j_syn_dsp::FilterMode::from(p.filters[filter_idx].mode.value());
                        mode.has_fixed_12db_slope().then_some("12 dB")
                    },
                )
                .width(Pixels(60.0))
                .height(Pixels(20.0));
            }
        })
        .col_between(Pixels(6.0))
        .height(Pixels(20.0))
        .width(Stretch(1.0));

        FilterResponseScope::new(cx, filter_idx, params.clone(), mod_ui.clone(), accent)
            .width(Stretch(1.0))
            .height(Pixels(52.0));

        HStack::new(cx, {
            let params = params.clone();
            let mod_ui = mod_ui.clone();
            move |cx| {
                build_flex_knob_cell_mod_dim(
                    cx,
                    UiData::params,
                    move |p| &p.filters[filter_idx].cutoff,
                    "CUTOFF",
                    false,
                    accent,
                    tokens::KNOB_FLT,
                    ModDestination::filter_cutoff(filter_idx),
                    mod_ui.clone(),
                    0,
                    params.clone(),
                    filter_check,
                );
                build_flex_knob_cell_mod_dim(
                    cx,
                    UiData::params,
                    move |p| &p.filters[filter_idx].resonance,
                    "RES",
                    false,
                    accent,
                    tokens::KNOB_FLT,
                    ModDestination::filter_res(filter_idx),
                    mod_ui.clone(),
                    0,
                    params.clone(),
                    filter_check,
                );
                build_flex_knob_cell_mod_dim(
                    cx,
                    UiData::params,
                    move |p| &p.filters[filter_idx].drive,
                    "DRIVE",
                    false,
                    accent,
                    tokens::KNOB_FLT,
                    ModDestination::filter_drive(filter_idx),
                    mod_ui.clone(),
                    0,
                    params.clone(),
                    filter_check,
                );
                build_filter_aux_knob_cell(
                    cx,
                    filter_idx,
                    params.clone(),
                    mod_ui.clone(),
                    accent,
                    tokens::KNOB_FLT,
                );
            }
        })
        .col_between(Pixels(tokens::SPACE_SM))
        .width(Stretch(1.0))
        .height(Auto);
    })
    .class("hardware-module")
    .toggle_class(
        "module-disabled",
        UiData::mod_ui_rev.map(move |_| !filter_check(&p_dim)),
    )
    .row_between(Pixels(tokens::SPACE_SM))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}

pub(super) fn build_dual_filter_section(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    // Side-by-side Filter 1 & Filter 2 cards (routing lives in the ROUTE tab)
    HStack::new(cx, move |cx| {
        build_filter_panel(
            cx,
            0,
            params.clone(),
            gui_context.clone(),
            mod_ui.clone(),
            AMBER_RGB,
        );
        build_filter_panel(cx, 1, params, gui_context, mod_ui, CORAL_RGB);
    })
    .col_between(Pixels(tokens::SPACE_MD))
    .width(Stretch(1.0))
    .height(Pixels(148.0));
}
