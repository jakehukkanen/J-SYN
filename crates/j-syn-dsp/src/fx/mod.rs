//! Parallel 3-bus 30-slot serial Master FX Rack (`Bus A`, `Bus B`, `Bus C` + `Sub Bus`),
//! with lazy per-slot engine provisioning (`ActiveFxEngine`), explicit drag-and-drop ordering
//! (`FxRackOrder`), per-slot stacking (`STACK 1..=4`), and zero audio-thread heap allocations.

pub mod diopser;
pub mod dynamics;
pub mod engine_pool;
pub mod limiter;
pub mod modulation;
pub mod multiband_dist;
pub mod tube_resonator;
pub mod utility;

mod fx_filter;
mod types;

use std::f64::consts::FRAC_1_SQRT_2;
use std::sync::Arc;

use crate::adaa::TanhAdaa2;
use crate::filter::{FilterMode, FilterParams};
use crate::mod_matrix::{ModDestination, ModUiTelemetry, NUM_MOD_DESTS};
use crate::oversample::{Biquad, BiquadCoeffs, Oversampler4x};
use crate::waveshaper::DistortionMode;

use self::modulation::DelayStereo;

pub use self::diopser::{
    diopser_freq_hz, diopser_stage_count, evaluate_diopser_ripple, DIOPSER_STAGE_COUNTS,
};
pub use self::engine_pool::{ActiveFxEngine, FxEngineSwapQueue, FxRackOrder, FxSwapCommand};
pub use self::limiter::{
    limiter_ceil_db, limiter_gain_db, limiter_lookahead_ms, limiter_lookahead_samples,
    limiter_release_ms,
};
pub use self::modulation::{
    evaluate_flanger_scope_response, flanger_delay_ms, flanger_feedback, flanger_phase_deg,
    flanger_rate_hz, fx_high_cut_hz, fx_low_cut_hz, DELAY_SYNC_LABELS,
};
pub use self::multiband_dist::{
    mb_band_level_db, mb_band_magnitude_at_hz, mb_ordered_xover_hz, mb_out_gain_db,
    mb_xover_high_hz, mb_xover_high_norm, mb_xover_low_hz, mb_xover_low_norm, MbDistBandParams,
    MB_BAND_LEVEL_DEFAULT_NORM, MB_XOVER_HIGH_DEFAULT_NORM, MB_XOVER_LOW_DEFAULT_NORM,
};
pub use self::tube_resonator::{
    evaluate_tube_res_scope_response, format_tube_res_tune, tube_res_decay_ms,
    tube_res_flt_freq_hz, tube_res_flt_width_oct, tube_res_lfo_rate_hz, tube_res_tune_hz,
    tube_res_tune_midi,
};
pub use self::utility::{
    format_utility_pan, utility_gain_db_from_norm, utility_pan_bipolar, utility_width_from_norm,
    UTILITY_WIDTH_DEFAULT_NORM,
};

pub use self::fx_filter::*;
pub use self::types::*;

pub const NUM_FX_BUSES: usize = 3;
pub const NUM_FX_SLOTS: usize = 30;
pub const MAX_FX_STACK: usize = 4;

const SCOPE_HZ: f32 = 150.0;

#[inline]
pub(crate) fn flush_denormal(x: f32) -> f32 {
    if x.is_finite() && x.abs() >= 1e-24 {
        x
    } else {
        0.0
    }
}

#[inline]
pub(crate) fn track_idle(idle_samples: &mut u32, in_l: f32, in_r: f32) -> bool {
    if in_l.abs() < 1e-8 && in_r.abs() < 1e-8 {
        *idle_samples = idle_samples.saturating_add(1);
        true
    } else {
        *idle_samples = 0;
        false
    }
}

#[inline]
pub(crate) fn butterworth_lp_coeffs(sample_rate: f64, freq_hz: f64) -> BiquadCoeffs {
    BiquadCoeffs::low_pass(sample_rate, freq_hz, FRAC_1_SQRT_2)
}

#[inline]
pub(crate) fn butterworth_hp_coeffs(sample_rate: f64, freq_hz: f64) -> BiquadCoeffs {
    BiquadCoeffs::high_pass(sample_rate, freq_hz, FRAC_1_SQRT_2)
}

#[inline]
pub(crate) fn butterworth_allpass_coeffs(sample_rate: f64, freq_hz: f64) -> BiquadCoeffs {
    BiquadCoeffs::all_pass(sample_rate, freq_hz, FRAC_1_SQRT_2)
}

/// Lazy single 30-slot serial FX Bus (`Bus A`, `Bus B`, or `Bus C`).
struct FxBusState {
    bus_idx: usize,
    stages: Box<[[ActiveFxEngine; MAX_FX_STACK]]>,
    scope_accum: [(f32, f32, f32, usize); NUM_FX_SLOTS],
    scope_samples_per_col: usize,
}

impl FxBusState {
    fn new(bus_idx: usize, sample_rate: f32) -> Self {
        let sr = sample_rate.max(1_000.0);
        let stages = (0..NUM_FX_SLOTS)
            .map(|_| std::array::from_fn(|_| ActiveFxEngine::Off))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            bus_idx,
            stages,
            scope_accum: [(0.0, 0.0, 0.0, 0); NUM_FX_SLOTS],
            scope_samples_per_col: (sr / SCOPE_HZ).round().max(1.0) as usize,
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(1_000.0);
        self.scope_samples_per_col = (sr / SCOPE_HZ).round().max(1.0) as usize;
        for slot in self.stages.iter_mut() {
            for stage in slot.iter_mut() {
                stage.set_sample_rate(sr);
            }
        }
    }

    fn reset(&mut self) {
        for slot in self.stages.iter_mut() {
            for stage in slot.iter_mut() {
                stage.reset();
            }
        }
        self.scope_accum = [(0.0, 0.0, 0.0, 0); NUM_FX_SLOTS];
    }

    /// Synchronously provisions all active engines for `bus` (call off the audio thread).
    fn prepare_for_bus(&mut self, bus: &FxBusParams, sample_rate: f32) {
        for (slot_idx, slot) in bus.slots.iter().enumerate() {
            let active_stages = if slot.fx_type == FxType::Off {
                0
            } else {
                (slot.stack as usize).clamp(1, MAX_FX_STACK)
            };
            for (stack_idx, stage) in self.stages[slot_idx].iter_mut().enumerate() {
                let target_ty = if stack_idx < active_stages {
                    slot.fx_type
                } else {
                    FxType::Off
                };
                if stage.fx_type() != target_ty {
                    *stage = ActiveFxEngine::new_for_type(target_ty, sample_rate);
                }
            }
        }
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn process_slot(
        &mut self,
        slot_idx: usize,
        slot: &FxSlotParams,
        mut l: f32,
        mut r: f32,
        mod_offsets: &[f32; NUM_MOD_DESTS],
        tempo_bpm: f32,
        swap_queue: &FxEngineSwapQueue,
        telemetry: Option<&ModUiTelemetry>,
    ) -> (f32, f32) {
        let pre_peak = l.abs().max(r.abs());
        let mut slot_activity = 0.0_f32;

        if slot.enabled && slot.fx_type != FxType::Off {
            let mut eff = [0.0_f32; 4];
            let mut eff_eq = [0.0_f32; 4];
            for p in 0..4 {
                let mod_val = mod_offsets
                    [ModDestination::fx_bus_slot_param(self.bus_idx, slot_idx, p).index()];
                eff[p] = (slot.params[p] + mod_val).clamp(0.0, 1.0);
                let eq_mod = mod_offsets
                    [ModDestination::fx_bus_slot_param(self.bus_idx, slot_idx, 4 + p).index()];
                eff_eq[p] = (slot.eq_params[p] + eq_mod).clamp(0.0, 1.0);
            }

            let eff_bands = if slot.fx_type == FxType::MultibandDist {
                let mut bands = slot.mb_bands;
                for (b, band) in bands.iter_mut().enumerate() {
                    let base = 4 + b * 4;
                    band.drive = (band.drive
                        + mod_offsets[ModDestination::fx_bus_slot_param(
                            self.bus_idx,
                            slot_idx,
                            base,
                        )
                        .index()])
                    .clamp(0.0, 1.0);
                    band.bias = (band.bias
                        + mod_offsets[ModDestination::fx_bus_slot_param(
                            self.bus_idx,
                            slot_idx,
                            base + 1,
                        )
                        .index()])
                    .clamp(0.0, 1.0);
                    band.level = (band.level
                        + mod_offsets[ModDestination::fx_bus_slot_param(
                            self.bus_idx,
                            slot_idx,
                            base + 2,
                        )
                        .index()])
                    .clamp(0.0, 1.0);
                    band.mix = (band.mix
                        + mod_offsets[ModDestination::fx_bus_slot_param(
                            self.bus_idx,
                            slot_idx,
                            base + 3,
                        )
                        .index()])
                    .clamp(0.0, 1.0);
                }
                bands
            } else {
                slot.mb_bands
            };

            let stack_count = (slot.stack as usize).clamp(1, MAX_FX_STACK);
            let mut needs_provision = false;
            for stage in self.stages[slot_idx].iter_mut().take(stack_count) {
                if stage.fx_type() == slot.fx_type {
                    let (nl, nr, act) = stage.process_sample(
                        slot,
                        l,
                        r,
                        eff,
                        eff_eq,
                        &eff_bands,
                        tempo_bpm,
                    );
                    l = nl;
                    r = nr;
                    slot_activity = slot_activity.max(act);
                } else {
                    needs_provision = true;
                }
            }
            if needs_provision {
                swap_queue.mark_needs_provision(self.bus_idx, slot_idx);
            }
        }

        let post_peak = l.abs().max(r.abs());
        if let Some(tel) = telemetry {
            let (ref mut acc_pre, ref mut acc_post, ref mut acc_act, ref mut cnt) =
                self.scope_accum[slot_idx];
            *acc_pre = acc_pre.max(pre_peak);
            *acc_post = acc_post.max(post_peak);
            *acc_act = acc_act.max(slot_activity);
            *cnt += 1;
            if *cnt >= self.scope_samples_per_col {
                tel.fx_scope(self.bus_idx, slot_idx)
                    .push(*acc_pre, *acc_post, *acc_act);
                *acc_pre = 0.0;
                *acc_post = 0.0;
                *acc_act = 0.0;
                *cnt = 0;
            }
        }

        (l, r)
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn process_sample(
        &mut self,
        in_l: f32,
        in_r: f32,
        bus: &FxBusParams,
        order: &FxRackOrder,
        mod_offsets: &[f32; NUM_MOD_DESTS],
        tempo_bpm: f32,
        swap_queue: &FxEngineSwapQueue,
        telemetry: Option<&ModUiTelemetry>,
    ) -> (f32, f32) {
        if bus.muted {
            return (0.0, 0.0);
        }

        let mut l = in_l;
        let mut r = in_r;

        if bus.enabled {
            if order.explicit {
                for &slot_u8 in order.active_slots(self.bus_idx) {
                    let slot_idx = slot_u8 as usize;
                    if slot_idx < NUM_FX_SLOTS {
                        let slot = &bus.slots[slot_idx];
                        if slot.fx_type != FxType::Off {
                            (l, r) = self.process_slot(
                                slot_idx,
                                slot,
                                l,
                                r,
                                mod_offsets,
                                tempo_bpm,
                                swap_queue,
                                telemetry,
                            );
                        }
                    }
                }
            } else {
                for (slot_idx, slot) in bus.slots.iter().enumerate() {
                    if slot.fx_type != FxType::Off {
                        (l, r) = self.process_slot(
                            slot_idx,
                            slot,
                            l,
                            r,
                            mod_offsets,
                            tempo_bpm,
                            swap_queue,
                            telemetry,
                        );
                    }
                }
            }
        }

        let (lvl_dest, pan_dest) = match self.bus_idx {
            0 => (ModDestination::BusALevel, ModDestination::BusAPan),
            1 => (ModDestination::BusBLevel, ModDestination::BusBPan),
            _ => (ModDestination::BusCLevel, ModDestination::BusCPan),
        };
        let eff_level = (bus.level + mod_offsets[lvl_dest.index()] * 2.0).clamp(0.0, 2.0);
        let eff_pan = (bus.pan + mod_offsets[pan_dest.index()] * 2.0).clamp(-1.0, 1.0);
        let gain_l = eff_level * (1.0 - eff_pan.max(0.0));
        let gain_r = eff_level * (1.0 + eff_pan.min(0.0));

        (flush_denormal(l * gain_l), flush_denormal(r * gain_r))
    }
}

/// Dedicated Sub / Direct Bus processor (`24 dB/oct LP` -> `4x ADAA2 Clipper` -> `Level`).
struct SubBusProcessor {
    lp1: [Biquad; 2],
    lp2: [Biquad; 2],
    oversamplers: [Oversampler4x; 2],
    adaa: [TanhAdaa2; 2],
    sample_rate: f32,
    last_cutoff_hz: f32,
    last_sr: f32,
    idle_samples: u32,
    scope_accum: (f32, f32, f32, usize),
    scope_samples_per_col: usize,
}

impl SubBusProcessor {
    fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1_000.0);
        Self {
            lp1: [Biquad::default(); 2],
            lp2: [Biquad::default(); 2],
            oversamplers: [Oversampler4x::new(); 2],
            adaa: [TanhAdaa2::default(); 2],
            sample_rate: sr,
            last_cutoff_hz: -1.0,
            last_sr: 0.0,
            idle_samples: 1024,
            scope_accum: (0.0, 0.0, 0.0, 0),
            scope_samples_per_col: (sr / SCOPE_HZ).round().max(1.0) as usize,
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(1_000.0);
        self.sample_rate = sr;
        self.last_sr = 0.0;
        self.scope_samples_per_col = (sr / SCOPE_HZ).round().max(1.0) as usize;
    }

    fn reset(&mut self) {
        for ch in 0..2 {
            self.lp1[ch].reset();
            self.lp2[ch].reset();
            self.oversamplers[ch].reset();
            self.adaa[ch].reset();
        }
        self.last_cutoff_hz = -1.0;
        self.idle_samples = 1024;
        self.scope_accum = (0.0, 0.0, 0.0, 0);
    }

    #[inline]
    fn push_scope(&mut self, telemetry: Option<&ModUiTelemetry>, pre: f32, post: f32, act: f32) {
        if let Some(tel) = telemetry {
            let (ref mut acc_pre, ref mut acc_post, ref mut acc_act, ref mut cnt) =
                self.scope_accum;
            *acc_pre = acc_pre.max(pre);
            *acc_post = acc_post.max(post);
            *acc_act = acc_act.max(act);
            *cnt += 1;
            if *cnt >= self.scope_samples_per_col {
                tel.sub_scope.push(*acc_pre, *acc_post, *acc_act);
                *acc_pre = 0.0;
                *acc_post = 0.0;
                *acc_act = 0.0;
                *cnt = 0;
            }
        }
    }

    #[inline]
    fn process_sample(
        &mut self,
        in_l: f32,
        in_r: f32,
        sub: &SubBusParams,
        mod_offsets: &[f32; NUM_MOD_DESTS],
        telemetry: Option<&ModUiTelemetry>,
    ) -> (f32, f32) {
        let level = (sub.level + mod_offsets[ModDestination::SubLevel.index()] * 2.0).clamp(0.0, 2.0);
        if !sub.enabled {
            // Sub FX bypassed: pass raw direct signal straight through scaled by SUB LEVEL.
            let out_l = flush_denormal(in_l * level);
            let out_r = flush_denormal(in_r * level);
            let raw_peak = in_l.abs().max(in_r.abs());
            let out_peak = out_l.abs().max(out_r.abs());
            self.push_scope(telemetry, raw_peak, out_peak, 0.0);
            return (out_l, out_r);
        }
        if track_idle(&mut self.idle_samples, in_l, in_r) && self.idle_samples > 256 {
            if self.idle_samples == 257
                && let Some(tel) = telemetry
            {
                tel.sub_scope.clear();
                self.scope_accum = (0.0, 0.0, 0.0, 0);
            }
            return (0.0, 0.0);
        }

        let cutoff_norm =
            (sub_cutoff_hz_to_norm(sub.lp_cutoff_hz) + mod_offsets[ModDestination::SubCutoff.index()])
                .clamp(0.0, 1.0);
        let lp_off = cutoff_norm >= 0.995;
        let cutoff_hz = sub_cutoff_norm_to_hz(cutoff_norm);
        let drive_db =
            (sub.clip_drive_db + mod_offsets[ModDestination::SubDrive.index()] * 24.0).clamp(0.0, 24.0);

        if lp_off {
            self.last_cutoff_hz = -1.0;
        } else if (cutoff_hz - self.last_cutoff_hz).abs() > 0.25
            || (self.sample_rate - self.last_sr).abs() > 0.5
        {
            if self.last_cutoff_hz < 0.0 {
                for ch in 0..2 {
                    self.lp1[ch].reset();
                    self.lp2[ch].reset();
                }
            }
            let coeffs = butterworth_lp_coeffs(self.sample_rate as f64, cutoff_hz as f64);
            for ch in 0..2 {
                self.lp1[ch].set_coeffs(coeffs);
                self.lp2[ch].set_coeffs(coeffs);
            }
            self.last_cutoff_hz = cutoff_hz;
            self.last_sr = self.sample_rate;
        }

        let drive_lin = 10.0_f32.powf(drive_db * 0.05);
        let soft_knee = 0.25 + 0.75 * (drive_db / 24.0).clamp(0.0, 1.0);
        let mut out = [0.0_f32; 2];
        let mut pre_peak = 0.0_f32;
        let mut post_peak = 0.0_f32;

        for (ch, &x) in [in_l, in_r].iter().enumerate() {
            let filt = if lp_off {
                x
            } else {
                self.lp2[ch].process(self.lp1[ch].process(x))
            };
            let driven = filt * drive_lin;
            pre_peak = pre_peak.max(driven.abs());
            let adaa = &mut self.adaa[ch];
            let clipped = self.oversamplers[ch].process_sample(driven, |s| {
                let hard = s.clamp(-1.0, 1.0);
                let soft = adaa.process(s as f64) as f32;
                hard + (soft - hard) * soft_knee
            });
            post_peak = post_peak.max(clipped.abs());
            out[ch] = flush_denormal(clipped * level);
        }

        let clip_act = ((pre_peak - post_peak).max(0.0) / pre_peak.max(0.05)).clamp(0.0, 1.0);
        self.push_scope(telemetry, pre_peak, post_peak, clip_act);

        (out[0], out[1])
    }
}

/// Complete parallel 3-bus 30-slot lazy FX Rack + dedicated Sub Bus.
pub struct FxRackState {
    buses: Box<[FxBusState]>,
    sub_bus: SubBusProcessor,
    swap_queue: Arc<FxEngineSwapQueue>,
    sample_rate: f32,
}

impl FxRackState {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1_000.0);
        Self {
            buses: (0..NUM_FX_BUSES)
                .map(|idx| FxBusState::new(idx, sr))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            sub_bus: SubBusProcessor::new(sr),
            swap_queue: Arc::new(FxEngineSwapQueue::new(sr)),
            sample_rate: sr,
        }
    }

    pub fn swap_queue(&self) -> Arc<FxEngineSwapQueue> {
        Arc::clone(&self.swap_queue)
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(1_000.0);
        self.sample_rate = sr;
        self.swap_queue.set_sample_rate(sr);
        self.buses[0].set_sample_rate(sr);
        self.buses[1].set_sample_rate(sr);
        self.buses[2].set_sample_rate(sr);
        self.sub_bus.set_sample_rate(sr);
    }

    pub fn reset(&mut self) {
        self.buses[0].reset();
        self.buses[1].reset();
        self.buses[2].reset();
        self.sub_bus.reset();
    }

    /// Synchronously provisions all active FX engines for the given buses (call off-RT).
    pub fn prepare_for_buses(&mut self, buses: &[&FxBusParams; NUM_FX_BUSES]) {
        self.drain_swap_commands(None);
        self.swap_queue.drain_garbage();
        let sr = self.sample_rate;
        for b in 0..NUM_FX_BUSES {
            self.buses[b].prepare_for_bus(buses[b], sr);
        }
        let _ = self.swap_queue.take_provision_masks();
    }

    /// Synchronously provisions all active FX engines for `rack` (call off-RT).
    pub fn prepare_for_params(&mut self, rack: &FxRackParams) {
        self.prepare_for_buses(&[&rack.buses[0], &rack.buses[1], &rack.buses[2]]);
    }

    #[inline]
    fn drain_swap_commands(&mut self, telemetry: Option<&ModUiTelemetry>) {
        while self.swap_queue.can_retire() {
            let Some(cmd) = self.swap_queue.try_pop_cmd() else {
                break;
            };
            let b = cmd.bus as usize;
            let s = cmd.slot as usize;
            let k = cmd.stack_idx as usize;
            if b < NUM_FX_BUSES && s < NUM_FX_SLOTS && k < MAX_FX_STACK {
                let prev_ty = self.buses[b].stages[s][k].fx_type();
                let new_ty = cmd.engine.fx_type();
                let old = std::mem::replace(&mut self.buses[b].stages[s][k], cmd.engine);
                self.swap_queue.retire_engine(old);
                if k == 0 && prev_ty != new_ty {
                    if let Some(tel) = telemetry {
                        tel.fx_scope(b, s).clear();
                    }
                }
            } else {
                self.swap_queue.retire_engine(cmd.engine);
            }
        }
    }

    #[inline]
    pub fn format_distortion_submode(p0: f32) -> &'static str {
        DistortionMode::from_index(crate::waveshaper::legacy_distortion_submode_index(p0)).name()
    }

    #[inline]
    pub fn format_delay_sync(p0: f32) -> &'static str {
        DELAY_SYNC_LABELS[DelayStereo::sync_index(p0)]
    }

    #[inline]
    pub fn filter_params_from_fx(
        mode: FilterMode,
        slope: FxFilterSlope,
        p: [f32; 4],
        freq2_norm: f32,
    ) -> FilterParams {
        FxFilterStereo::filter_params(mode, slope, p, freq2_norm)
    }

    /// Zero-copy stereo sample processing accepting borrowed bus parameter slices directly.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub fn process_buses_refs_sample(
        &mut self,
        bus_a_in: (f32, f32),
        bus_b_in: (f32, f32),
        sub_in: (f32, f32),
        buses: &[&FxBusParams; NUM_FX_BUSES],
        sub: &SubBusParams,
        topology: FxBusTopology,
        order: &FxRackOrder,
        mod_offsets: &[f32; NUM_MOD_DESTS],
        tempo_bpm: f32,
        telemetry: Option<&ModUiTelemetry>,
    ) -> (f32, f32) {
        self.drain_swap_commands(telemetry);

        let (sub_l, sub_r) =
            self.sub_bus
                .process_sample(sub_in.0, sub_in.1, sub, mod_offsets, telemetry);

        let (al, ar) = self.buses[0].process_sample(
            bus_a_in.0,
            bus_a_in.1,
            buses[0],
            order,
            mod_offsets,
            tempo_bpm,
            &self.swap_queue,
            telemetry,
        );

        let mut master_l = 0.0_f32;
        let mut master_r = 0.0_f32;
        let mut b_in_l = bus_b_in.0;
        let mut b_in_r = bus_b_in.1;
        let mut c_in_l = sub_l;
        let mut c_in_r = sub_r;

        match topology.bus_a_target() {
            0 => {
                master_l += al;
                master_r += ar;
            }
            1 => {
                b_in_l += al;
                b_in_r += ar;
            }
            _ => {
                c_in_l += al;
                c_in_r += ar;
            }
        }

        let (bl, br) = self.buses[1].process_sample(
            b_in_l,
            b_in_r,
            buses[1],
            order,
            mod_offsets,
            tempo_bpm,
            &self.swap_queue,
            telemetry,
        );
        match topology.bus_b_target() {
            0 => {
                master_l += bl;
                master_r += br;
            }
            _ => {
                c_in_l += bl;
                c_in_r += br;
            }
        }

        let (cl, cr) = self.buses[2].process_sample(
            c_in_l,
            c_in_r,
            buses[2],
            order,
            mod_offsets,
            tempo_bpm,
            &self.swap_queue,
            telemetry,
        );

        (flush_denormal(master_l + cl), flush_denormal(master_r + cr))
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub fn process_buses_sample(
        &mut self,
        bus_a_in: (f32, f32),
        bus_b_in: (f32, f32),
        sub_in: (f32, f32),
        rack: &FxRackParams,
        mod_offsets: &[f32; NUM_MOD_DESTS],
        tempo_bpm: f32,
        telemetry: Option<&ModUiTelemetry>,
    ) -> (f32, f32) {
        self.process_buses_refs_sample(
            bus_a_in,
            bus_b_in,
            sub_in,
            &[&rack.buses[0], &rack.buses[1], &rack.buses[2]],
            &rack.sub,
            rack.topology,
            &rack.order,
            mod_offsets,
            tempo_bpm,
            telemetry,
        )
    }
}
