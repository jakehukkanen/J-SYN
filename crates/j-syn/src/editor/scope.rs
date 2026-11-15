//! Live single-cycle wavetable preview canvas (`WavetableScope`).

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::{EngineMode, ModOrder, WavetableBank};
use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::RawParamEvent;

use super::mod_bar::ModUiState;
use super::UiData;
use crate::{JSynPluginParams, ModSourceParam, ModTypeParam, OscPluginParams};

/// Live single-cycle wavetable preview canvas per oscillator,
/// applying `Phase`, `Warp`, and simulated `PM` / `RM` from `Mod Source`.
pub struct WavetableScope {
    osc_idx: usize,
    bank: Arc<WavetableBank>,
    params: Arc<JSynPluginParams>,
    mod_ui: Arc<ModUiState>,
}

#[inline]
fn osc_preview_signature(osc: &OscPluginParams) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut mix = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x100000001b3);
    };
    mix(osc.wt_pos.modulated_normalized_value().to_bits() as u64);
    mix(osc.level.value().to_bits() as u64);
    mix(osc.wt_select.value() as u64);
    mix(osc.phase.value().to_bits() as u64);
    mix(osc.warp_mode.value() as u64);
    mix(osc.warp_amount.value().to_bits() as u64);
    mix(osc.mod_source.value() as u64);
    mix(osc.mod_type.value() as u64);
    mix(osc.mod_order.value() as u64);
    mix(osc.mod_depth.value().to_bits() as u64);
    mix(osc.pitch.value().to_bits() as u64);
    h
}

#[inline]
fn mod_source_index(src: ModSourceParam) -> Option<usize> {
    match src {
        ModSourceParam::Off => None,
        ModSourceParam::Osc1 => Some(0),
        ModSourceParam::Osc2 => Some(1),
        ModSourceParam::Osc3 => Some(2),
        ModSourceParam::Osc4 => Some(3),
    }
}

impl WavetableScope {
    pub fn new(
        cx: &mut Context,
        osc_idx: usize,
        bank: Arc<WavetableBank>,
        params: Arc<JSynPluginParams>,
        mod_ui: Arc<ModUiState>,
    ) -> Handle<'_, Self> {
        Self {
            osc_idx,
            bank,
            params,
            mod_ui,
        }
        .build(cx, move |cx| {
            let scope_lens = UiData::params.map(move |p| {
                let osc = &p.oscs[osc_idx];
                let mod_sig = mod_source_index(osc.mod_source.value())
                    .map(|m_idx| osc_preview_signature(&p.oscs[m_idx]));
                (osc_preview_signature(osc), mod_sig)
            });
            Binding::new(cx, scope_lens, |cx, _| {
                cx.needs_redraw();
                Element::new(cx).hoverable(false);
            });
        })
    }
}

impl View for WavetableScope {
    fn element(&self) -> Option<&'static str> {
        Some("wavetable-scope")
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
        let osc = &self.params.oscs[self.osc_idx];
        let enabled = self.params.oscs[self.osc_idx].enabled.value();

        let mut bg = vg::Path::new();
        bg.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, radius);
        canvas.fill_path(
            &bg,
            &vg::Paint::color(if enabled {
                vg::Color::rgb(22, 19, 16)
            } else {
                vg::Color::rgb(18, 16, 14)
            }),
        );

        // Center zero-crossing line
        let mid_y = bounds.y + bounds.h * 0.5;
        let mut zero_line = vg::Path::new();
        zero_line.move_to(bounds.x + 3.0 * dpx, mid_y);
        zero_line.line_to(bounds.x + bounds.w - 3.0 * dpx, mid_y);
        let mut zero_paint = vg::Paint::color(if enabled {
            vg::Color::rgba(95, 82, 68, 100)
        } else {
            vg::Color::rgba(75, 66, 58, 55)
        });
        zero_paint.set_line_width(1.0 * dpx);
        canvas.stroke_path(&zero_line, &zero_paint);

        let choice = osc.wt_select.value();
        let custom_guard;
        let wt: &j_syn_dsp::Wavetable = match choice.factory_preset() {
            Some(preset) => self.bank.factory_table(preset),
            None => {
                custom_guard = self.bank.osc_tables[self.osc_idx].load();
                &custom_guard
            }
        };
        let tel = &self.mod_ui.telemetry;
        let wt_pos = (osc.wt_pos.value()
            + tel.dest_offset(ModDestination::osc_wt_pos(self.osc_idx)))
        .clamp(0.0, 1.0);
        let phase_offset = ((osc.phase.value() / 360.0)
            + tel.dest_offset(ModDestination::osc_phase(self.osc_idx)))
        .rem_euclid(1.0);
        let warp_mode = osc.warp_mode.value().into();
        let warp_amount = (osc.warp_amount.value()
            + tel.dest_offset(ModDestination::osc_warp(self.osc_idx)))
        .clamp(0.0, 1.0);
        let mod_type = osc.mod_type.value();
        let mod_depth = (osc.mod_depth.value()
            + tel.dest_offset(ModDestination::osc_mod_depth(self.osc_idx)))
        .clamp(0.0, 1.0);
        let mod_order: ModOrder = osc.mod_order.value().into();

        let mod_idx_opt = mod_source_index(osc.mod_source.value());
        let mod_custom_guard;
        let mod_info = if let Some(m_idx) = mod_idx_opt.filter(|_| mod_depth > 1e-4) {
            let m_osc = &self.params.oscs[m_idx];
            let m_wt: &j_syn_dsp::Wavetable = match m_osc.wt_select.value().factory_preset() {
                Some(preset) => self.bank.factory_table(preset),
                None => {
                    mod_custom_guard = self.bank.osc_tables[m_idx].load();
                    &mod_custom_guard
                }
            };
            let car_semi = osc.pitch.value()
                + tel.dest_offset(ModDestination::osc_pitch(self.osc_idx)) * 96.0;
            let mod_semi =
                m_osc.pitch.value() + tel.dest_offset(ModDestination::osc_pitch(m_idx)) * 96.0;
            let ratio = 2.0_f32.powf((mod_semi - car_semi) / 12.0).clamp(0.25, 8.0);
            Some((
                m_wt,
                (m_osc.wt_pos.value() + tel.dest_offset(ModDestination::osc_wt_pos(m_idx)))
                    .clamp(0.0, 1.0),
                ((m_osc.phase.value() / 360.0)
                    + tel.dest_offset(ModDestination::osc_phase(m_idx)))
                .rem_euclid(1.0),
                m_osc.warp_mode.value().into(),
                (m_osc.warp_amount.value() + tel.dest_offset(ModDestination::osc_warp(m_idx)))
                    .clamp(0.0, 1.0),
                m_osc.mod_order.value().into(),
                ratio,
            ))
        } else {
            None
        };

        const PREVIEW_PTS: usize = 128;
        let mut pts = [0.0_f32; PREVIEW_PTS];
        for (i, pt) in pts.iter_mut().enumerate() {
            let base_p = i as f32 / (PREVIEW_PTS - 1) as f32;
            let car_p = (base_p + phase_offset).rem_euclid(1.0);
            *pt = if let Some((
                m_wt,
                m_wt_pos,
                m_phase_off,
                m_warp_mode,
                m_warp_amt,
                m_mod_order,
                ratio,
            )) = mod_info
            {
                let mod_p = (base_p * ratio + m_phase_off).rem_euclid(1.0);
                let mod_sig = j_syn_dsp::sample_with_warp_and_pm(
                    m_wt,
                    m_wt_pos,
                    mod_p,
                    0.0,
                    m_warp_mode,
                    m_warp_amt,
                    0.0,
                    m_mod_order,
                    EngineMode::Vintage2007,
                );
                match mod_type {
                    ModTypeParam::Pm => j_syn_dsp::sample_with_warp_and_pm(
                        wt,
                        wt_pos,
                        car_p,
                        0.0,
                        warp_mode,
                        warp_amount,
                        mod_sig * mod_depth,
                        mod_order,
                        EngineMode::Vintage2007,
                    ),
                    ModTypeParam::Rm => {
                        let car_sig = j_syn_dsp::sample_with_warp_and_pm(
                            wt,
                            wt_pos,
                            car_p,
                            0.0,
                            warp_mode,
                            warp_amount,
                            0.0,
                            mod_order,
                            EngineMode::Vintage2007,
                        );
                        car_sig * ((1.0 - mod_depth) + mod_depth * mod_sig)
                    }
                }
            } else {
                j_syn_dsp::sample_with_warp_and_pm(
                    wt,
                    wt_pos,
                    car_p,
                    0.0,
                    warp_mode,
                    warp_amount,
                    0.0,
                    mod_order,
                    EngineMode::Vintage2007,
                )
            };
        }

        let pad_x = 4.0 * dpx;
        let pad_y = 4.0 * dpx;
        let inner_w = (bounds.w - 2.0 * pad_x).max(1.0);
        let half_h = ((bounds.h - 2.0 * pad_y) * 0.45).max(1.0);

        let mut wave = vg::Path::new();
        for (i, &s) in pts.iter().enumerate() {
            let x = bounds.x + pad_x + (i as f32 / (PREVIEW_PTS - 1) as f32) * inner_w;
            let y = mid_y - s.clamp(-1.05, 1.05) * half_h;
            if i == 0 {
                wave.move_to(x, y);
            } else {
                wave.line_to(x, y);
            }
        }

        let mut wave_paint = vg::Paint::color(if enabled {
            vg::Color::rgba(245, 179, 73, 240)
        } else {
            vg::Color::rgba(92, 84, 76, 95)
        });
        wave_paint.set_line_width(1.6 * dpx);
        canvas.stroke_path(&wave, &wave_paint);

        let mut border = vg::Path::new();
        border.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, radius);
        let mut border_paint = vg::Paint::color(if enabled {
            vg::Color::rgb(68, 58, 50)
        } else {
            vg::Color::rgb(48, 42, 36)
        });
        border_paint.set_line_width(1.0 * dpx);
        canvas.stroke_path(&border, &border_paint);
    }
}

/// Stereo Peak/RMS master output meter with clickable clip indicator.
pub(super) struct MasterMeter {
    telemetry: Arc<j_syn_dsp::mod_matrix::ModUiTelemetry>,
}

impl MasterMeter {
    pub(super) fn new<'a>(
        cx: &'a mut Context,
        telemetry: Arc<j_syn_dsp::mod_matrix::ModUiTelemetry>,
    ) -> Handle<'a, Self> {
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

        let mut bg = vg::Path::new();
        bg.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 3.0 * dpx);
        canvas.fill_path(&bg, &vg::Paint::color(vg::Color::rgb(20, 17, 15)));

        let mut border = vg::Path::new();
        border.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 3.0 * dpx);
        let mut bp = vg::Paint::color(vg::Color::rgb(68, 58, 50));
        bp.set_line_width(1.0 * dpx);
        canvas.stroke_path(&border, &bp);

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
            // Clip LED
            let mut led = vg::Path::new();
            led.rounded_rect(x, bounds.y + pad, bar_w, clip_h, 1.0 * dpx);
            let led_col = if clip {
                vg::Color::rgb(245, 75, 65)
            } else {
                vg::Color::rgb(46, 38, 34)
            };
            canvas.fill_path(&led, &vg::Paint::color(led_col));

            // Track background
            let mut track = vg::Path::new();
            track.rounded_rect(x, bar_top, bar_w, bar_h, 1.0 * dpx);
            canvas.fill_path(&track, &vg::Paint::color(vg::Color::rgb(32, 27, 24)));

            // RMS fill
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
                    vg::Color::rgb(82, 200, 128),
                    vg::Color::rgb(245, 179, 73),
                );
                canvas.fill_path(&fill, &paint);
            }

            // Peak cap line
            let pk_n = Self::level_to_norm(pk);
            if pk_n > 0.005 {
                let py = (bar_top + bar_h - pk_n * bar_h).clamp(bar_top, bar_top + bar_h - 1.5 * dpx);
                let mut cap = vg::Path::new();
                cap.rect(x, py, bar_w, 1.5 * dpx);
                let cap_col = if pk > 0.95 {
                    vg::Color::rgb(245, 82, 68)
                } else {
                    vg::Color::rgb(255, 224, 150)
                };
                canvas.fill_path(&cap, &vg::Paint::color(cap_col));
            }
        }
    }
}
