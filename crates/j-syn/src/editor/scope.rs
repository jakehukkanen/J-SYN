//! Live single-cycle wavetable preview canvas (`WavetableScope`).

use std::sync::Arc;

use j_syn_dsp::{EngineMode, ModOrder, WavetableBank};
use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::RawParamEvent;

use super::UiData;
use crate::{JSynPluginParams, ModSourceParam, ModTypeParam, OscPluginParams};

/// Live single-cycle wavetable preview canvas per oscillator,
/// applying `Phase`, `Warp`, and simulated `PM` / `RM` from `Mod Source`.
pub struct WavetableScope {
    osc_idx: usize,
    bank: Arc<WavetableBank>,
    params: Arc<JSynPluginParams>,
}

#[inline]
fn osc_preview_signature(osc: &OscPluginParams) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut mix = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x100000001b3);
    };
    mix(osc.wt_pos.modulated_normalized_value().to_bits() as u64);
    mix(osc.enabled.value() as u64);
    mix(osc.wt_select.value() as u64);
    mix(osc.phase.value().to_bits() as u64);
    mix(osc.warp_mode.value() as u64);
    mix(osc.warp_amount.value().to_bits() as u64);
    mix(osc.mod_source.value() as u64);
    mix(osc.mod_type.value() as u64);
    mix(osc.mod_order.value() as u64);
    mix(osc.mod_depth.value().to_bits() as u64);
    mix(osc.octave.value() as u32 as u64);
    mix(osc.semi.value() as u32 as u64);
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
    ) -> Handle<'_, Self> {
        Self {
            osc_idx,
            bank,
            params,
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
        let enabled = osc.enabled.value();

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
        let wt_pos = osc.wt_pos.value();
        let phase_offset = (osc.phase.value() / 360.0).rem_euclid(1.0).fract();
        let warp_mode = osc.warp_mode.value().into();
        let warp_amount = osc.warp_amount.value();
        let mod_type = osc.mod_type.value();
        let mod_depth = osc.mod_depth.value().clamp(0.0, 1.0);
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
            let car_semi = osc.octave.value() * 12 + osc.semi.value();
            let mod_semi = m_osc.octave.value() * 12 + m_osc.semi.value();
            let ratio = 2.0_f32.powf((mod_semi - car_semi) as f32 / 12.0).clamp(0.25, 8.0);
            Some((
                m_wt,
                m_osc.wt_pos.value(),
                (m_osc.phase.value() / 360.0).rem_euclid(1.0).fract(),
                m_osc.warp_mode.value().into(),
                m_osc.warp_amount.value(),
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
