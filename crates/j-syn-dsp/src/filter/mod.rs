//! Dual per-voice Zero-Delay Feedback (ZDF) multimode filters.
//!
//! Includes a port of Cure Audio's `Scream` filter (`FilterMode::Scream`):
//! - Scream: MIT License, Copyright 2025 Cure Audio, exacoustics, Xynth Audio
//! - ADAA: BSD 3-Clause License, Copyright (c) 2020, jatinchowdhury18
//! - Polylogarithm (`li2`): MIT License, Copyright (c) 2021 Alexander Voigt

use std::f32::consts::PI;

use crate::adaa::TanhAdaa2;
use crate::oscillator::EngineMode;

mod response;
mod types;

pub use response::*;
pub use types::*;

/// Pre-allocated per-channel 2-pole stereo ZDF SVF state.
#[derive(Debug, Clone, Copy, Default)]
pub struct StereoZdfSvf {
    pub ic1eq: [f32; 2],
    pub ic2eq: [f32; 2],
}

/// Lightweight scalar filter core (copied into `TailVoice` on voice stealing).
#[derive(Debug, Clone, Copy)]
pub struct FilterSvfCore {
    pub svfs: [StereoZdfSvf; 4],
    pub combo_svfs: [StereoZdfSvf; 4],
    pub one_pole_z: [[f32; 2]; 2],
    pub scream_fb: [f32; 2],
    pub scream_peak: [f32; 2],
    pub scream_tanh_1: [TanhAdaa2; 2],
    pub scream_tanh_2: [TanhAdaa2; 2],
    pub prev_in: [f32; 2],
    pub comb_lp: [f32; 2],
    pub smooth_cutoff_hz: f32,
    pub smooth_res: f32,
    pub smooth_drive: f32,
}

impl Default for FilterSvfCore {
    fn default() -> Self {
        Self::new(20_000.0)
    }
}

impl FilterSvfCore {
    pub fn new(initial_cutoff_hz: f32) -> Self {
        Self {
            svfs: [StereoZdfSvf::default(); 4],
            combo_svfs: [StereoZdfSvf::default(); 4],
            one_pole_z: [[0.0; 2]; 2],
            scream_fb: [0.0; 2],
            scream_peak: [0.0; 2],
            scream_tanh_1: [TanhAdaa2::default(); 2],
            scream_tanh_2: [TanhAdaa2::default(); 2],
            prev_in: [0.0; 2],
            comb_lp: [0.0; 2],
            smooth_cutoff_hz: initial_cutoff_hz.clamp(20.0, 20_000.0),
            smooth_res: 0.0,
            smooth_drive: 0.0,
        }
    }

    pub fn reset_states(&mut self) {
        self.svfs = [StereoZdfSvf::default(); 4];
        self.combo_svfs = [StereoZdfSvf::default(); 4];
        self.one_pole_z = [[0.0; 2]; 2];
        self.scream_fb = [0.0; 2];
        self.scream_peak = [0.0; 2];
        self.scream_tanh_1[0].reset();
        self.scream_tanh_1[1].reset();
        self.scream_tanh_2[0].reset();
        self.scream_tanh_2[1].reset();
        self.prev_in = [0.0; 2];
        self.comb_lp = [0.0; 2];
    }

    #[inline]
    pub fn snap_params(&mut self, cutoff_hz: f32, res: f32, drive: f32) {
        self.smooth_cutoff_hz = cutoff_hz.clamp(20.0, 20_000.0);
        self.smooth_res = res.clamp(0.0, 1.0);
        self.smooth_drive = drive.clamp(0.0, 1.0);
    }

    /// 1-pole zero-delay feedback TPT filter returning `(lp, hp)`.
    #[inline]
    fn tpt_1p_step(g: f32, x: f32, z: &mut f32) -> (f32, f32) {
        let g1 = g / (1.0 + g);
        let v = (x - *z) * g1;
        let lp = v + *z;
        *z = soft_limit_state(lp + v);
        let hp = x - lp;
        (lp, hp)
    }

    #[inline]
    fn svf_step(
        g: f32,
        k: f32,
        x: f32,
        ic1: &mut f32,
        ic2: &mut f32,
        saturate_state: bool,
    ) -> (f32, f32, f32) {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = x - *ic2;
        let v1 = a1 * *ic1 + a2 * v3;
        let v2 = *ic2 + a2 * *ic1 + a3 * v3;
        let next_ic1 = 2.0 * v1 - *ic1;
        let next_ic2 = 2.0 * v2 - *ic2;
        if saturate_state {
            *ic1 = fast_tanh(next_ic1 * 0.4) * 2.5;
            *ic2 = fast_tanh(next_ic2 * 0.4) * 2.5;
        } else {
            *ic1 = soft_limit_state(next_ic1);
            *ic2 = soft_limit_state(next_ic2);
        }
        let hp = x - k * v1 - v2;
        (v2, v1, hp) // (lp, bp, hp)
    }

    /// Cure Audio `Scream` per-sample step:
    /// `x_in -> (+ fb) -> ADAA2 tanh -> 2-pole LP -> (out)`
    /// `-> (* fb_gain) -> 2-pole HP -> ADAA2 tanh -> expander gate -> fb_yn_1`.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn process_scream_step(
        &mut self,
        ch: usize,
        x: f32,
        g_lp: f32,
        k_lp: f32,
        g_hp: f32,
        k_hp: f32,
        in_gain: f32,
        feedback_gain: f32,
        exp_atk: f32,
        exp_rel: f32,
    ) -> f32 {
        let x_in = x * in_gain;
        let y_pre = (x_in + self.scream_fb[ch]) as f64;
        let y_sat = self.scream_tanh_1[ch].process(y_pre) as f32;
        let (lp, _, _) = Self::svf_step(
            g_lp,
            k_lp,
            y_sat,
            &mut self.svfs[0].ic1eq[ch],
            &mut self.svfs[0].ic2eq[ch],
            false,
        );
        let y = if lp.is_finite() { lp } else { 0.0 };

        let feed_pre = y * feedback_gain;
        let (_, _, hp) = Self::svf_step(
            g_hp,
            k_hp,
            feed_pre,
            &mut self.svfs[1].ic1eq[ch],
            &mut self.svfs[1].ic2eq[ch],
            false,
        );
        let mut feed = self.scream_tanh_2[ch].process(hp as f64) as f32;

        // Input-triggered feedback gate (-120 dB expander) to prevent infinite idle drone
        let abs_in = x_in.abs();
        self.scream_peak[ch] = if abs_in > self.scream_peak[ch] {
            exp_atk * self.scream_peak[ch] + (1.0 - exp_atk) * abs_in
        } else {
            exp_rel * self.scream_peak[ch]
        };
        let peak = self.scream_peak[ch];
        let peak_db = if peak > 1.0e-8 {
            20.0 * peak.log10()
        } else {
            -160.0
        };
        let reduction_db = if peak_db >= -120.0 {
            0.0
        } else {
            // hard_knee_expander(peak_db, -120.0, 2.0) - peak_db
            peak_db + 120.0
        };
        let reduction_gain = if reduction_db < -140.0 {
            0.0
        } else {
            10.0_f32.powf(reduction_db * 0.05)
        };
        feed *= reduction_gain;
        self.scream_fb[ch] = if feed.is_finite() { feed } else { 0.0 };

        y
    }

    /// Processes stereo audio for non-Comb filter modes (used directly by `TailVoice` as well).
    #[inline]
    pub fn process_core_stereo(
        &mut self,
        in_l: f32,
        in_r: f32,
        params: &FilterParams,
        midi_note: f32,
        sample_rate: f32,
        engine_mode: EngineMode,
    ) -> (f32, f32) {
        if !params.enabled {
            return (in_l, in_r);
        }

        const PARAM_COEFF: f32 = 0.04;
        self.smooth_cutoff_hz += (params.cutoff_hz - self.smooth_cutoff_hz) * PARAM_COEFF;
        self.smooth_res += (params.resonance - self.smooth_res) * PARAM_COEFF;
        self.smooth_drive += (params.drive - self.smooth_drive) * PARAM_COEFF;

        let sr = sample_rate.max(1000.0);
        let kt_semi = (midi_note - 60.0) * params.keytrack.clamp(0.0, 1.0);
        let fc = (self.smooth_cutoff_hz * 2.0_f32.powf(kt_semi / 12.0))
            .clamp(20.0, (sr * 0.47).min(20_000.0));
        let res = self.smooth_res.clamp(0.0, 1.0);
        let drive = self.smooth_drive.clamp(0.0, 1.0);
        let aux = params.aux.clamp(0.0, 1.0);

        match params.mode {
            FilterMode::Lp
            | FilterMode::Bp
            | FilterMode::Hp
            | FilterMode::Notch
            | FilterMode::Peak => {
                let g = (PI * fc / sr).tan().min(8.0);
                let mut out = [0.0_f32; 2];
                if params.slope == FilterSlope::Db6
                    && !matches!(params.mode, FilterMode::Notch | FilterMode::Peak)
                {
                    for (ch, &x) in [in_l, in_r].iter().enumerate() {
                        let x_in = if drive > 1e-4 {
                            fast_tanh(x * (1.0 + drive * 3.0))
                        } else {
                            x
                        };
                        let (lp1, hp1) = Self::tpt_1p_step(g, x_in, &mut self.one_pole_z[0][ch]);
                        let wet = match params.mode {
                            FilterMode::Lp => lp1,
                            FilterMode::Hp => hp1,
                            FilterMode::Bp => {
                                let (_, hp2) =
                                    Self::tpt_1p_step(g, lp1, &mut self.one_pole_z[1][ch]);
                                hp2 * 2.0
                            }
                            _ => lp1,
                        };
                        out[ch] = x * (1.0 - aux) + wet * aux;
                    }
                } else {
                    let stages = params.slope.num_2pole_stages().max(1);
                    let k0 = 2.0 - 1.95 * res;
                    let k_helper = (std::f32::consts::SQRT_2 - 0.45 * res).max(0.75);
                    let peak_boost = 0.6 + 1.8 * res;
                    let sat = drive > 0.25 || res > 0.85;
                    for (ch, &x) in [in_l, in_r].iter().enumerate() {
                        let x_in = if drive > 1e-4 {
                            fast_tanh(x * (1.0 + drive * 3.0))
                        } else {
                            x
                        };
                        let (lp0, bp0, hp0) = Self::svf_step(
                            g,
                            k0,
                            x_in,
                            &mut self.svfs[0].ic1eq[ch],
                            &mut self.svfs[0].ic2eq[ch],
                            sat,
                        );
                        let mut sig = match params.mode {
                            FilterMode::Lp => lp0,
                            FilterMode::Bp => bp0,
                            FilterMode::Hp => hp0,
                            FilterMode::Notch => lp0 + hp0,
                            FilterMode::Peak => x_in + peak_boost * k0 * bp0,
                            _ => lp0,
                        };
                        for svf in self.svfs.iter_mut().take(stages).skip(1) {
                            let mid = if drive > 1e-4 {
                                fast_tanh(sig * (1.0 + drive * 1.2))
                            } else {
                                sig
                            };
                            let (lp_s, bp_s, hp_s) = Self::svf_step(
                                g,
                                k_helper,
                                mid,
                                &mut svf.ic1eq[ch],
                                &mut svf.ic2eq[ch],
                                sat,
                            );
                            sig = match params.mode {
                                FilterMode::Lp => lp_s,
                                FilterMode::Bp => bp_s * k_helper,
                                FilterMode::Hp => hp_s,
                                FilterMode::Notch => lp_s + hp_s,
                                FilterMode::Peak => mid + 0.5 * peak_boost * k_helper * bp_s,
                                _ => lp_s,
                            };
                        }
                        out[ch] = x * (1.0 - aux) + sig * aux;
                    }
                }
                (out[0], out[1])
            }
            FilterMode::Scream => {
                const MIDI_20HZ: f32 = 15.486_821;
                const MIDI_20KHZ: f32 = 135.076_23;
                const HP_CUTOFF_MIN: f32 = MIDI_20HZ - 12.0;

                let lp_midi = 69.0 + 12.0 * (fc / 440.0).log2();
                let hp_midi =
                    HP_CUTOFF_MIN + aux * (MIDI_20KHZ - HP_CUTOFF_MIN) - (MIDI_20KHZ - lp_midi);
                let hp_fc = (440.0 * 2.0_f32.powf((hp_midi - 69.0) / 12.0))
                    .clamp(20.0, (sr * 0.47).min(20_000.0));

                let lp_q = std::f32::consts::FRAC_1_SQRT_2;
                let hp_q = lp_q + res * (std::f32::consts::SQRT_2 - lp_q);
                let k_lp = 1.0 / lp_q;
                let k_hp = 1.0 / hp_q;

                let fb_db = -12.0 + res * 24.0;
                let feedback_gain = 10.0_f32.powf(fb_db * 0.05);
                let in_gain = 10.0_f32.powf((drive * 18.0) * 0.05);

                let mut out = [0.0_f32; 2];
                match engine_mode {
                    EngineMode::Hq => {
                        let sr_os = 2.0 * sr;
                        let g_lp = (PI * fc / sr_os).tan().min(8.0);
                        let g_hp = (PI * hp_fc / sr_os).tan().min(8.0);
                        let exp_atk = (-1.0 / (0.001 * sr_os)).exp();
                        let exp_rel = (-1.0 / (0.080 * sr_os)).exp();
                        for (ch, &x) in [in_l, in_r].iter().enumerate() {
                            let x_mid = 0.5 * (self.prev_in[ch] + x);
                            self.prev_in[ch] = x;
                            let y0 = self.process_scream_step(
                                ch,
                                x_mid,
                                g_lp,
                                k_lp,
                                g_hp,
                                k_hp,
                                in_gain,
                                feedback_gain,
                                exp_atk,
                                exp_rel,
                            );
                            let y1 = self.process_scream_step(
                                ch,
                                x,
                                g_lp,
                                k_lp,
                                g_hp,
                                k_hp,
                                in_gain,
                                feedback_gain,
                                exp_atk,
                                exp_rel,
                            );
                            out[ch] = 0.5 * (y0 + y1);
                        }
                    }
                    EngineMode::Vintage2007 => {
                        let g_lp = (PI * fc / sr).tan().min(8.0);
                        let g_hp = (PI * hp_fc / sr).tan().min(8.0);
                        let exp_atk = (-1.0 / (0.001 * sr)).exp();
                        let exp_rel = (-1.0 / (0.080 * sr)).exp();
                        for (ch, &x) in [in_l, in_r].iter().enumerate() {
                            self.prev_in[ch] = x;
                            out[ch] = self.process_scream_step(
                                ch,
                                x,
                                g_lp,
                                k_lp,
                                g_hp,
                                k_hp,
                                in_gain,
                                feedback_gain,
                                exp_atk,
                                exp_rel,
                            );
                        }
                    }
                }
                (out[0], out[1])
            }
            FilterMode::Daft => {
                let g = (PI * fc / sr).tan().min(8.0);
                let k0 = 1.65 - 1.3 * res;
                let k1 = 1.85 - 1.75 * res;
                let mut out = [0.0_f32; 2];
                for (ch, &x) in [in_l, in_r].iter().enumerate() {
                    let x_drv = fast_tanh(x * (1.0 + drive * 2.8));
                    let (lp1, _, _) = Self::svf_step(
                        g,
                        k0,
                        x_drv,
                        &mut self.svfs[0].ic1eq[ch],
                        &mut self.svfs[0].ic2eq[ch],
                        true,
                    );
                    let (lp2, _, _) = Self::svf_step(
                        g,
                        k1,
                        lp1,
                        &mut self.svfs[1].ic1eq[ch],
                        &mut self.svfs[1].ic2eq[ch],
                        true,
                    );
                    let wet = lp2 + 0.35 * res * fast_tanh(lp1);
                    out[ch] = x * (1.0 - aux) + wet * aux;
                }
                (out[0], out[1])
            }
            FilterMode::Bandreject => {
                let g = (PI * fc / sr).tan().min(8.0);
                // Bandreject grows rejected bandwidth (`k = 1 / Q`) with `res`, fixed 2-pole (12 dB).
                let k = 1.0 + 3.5 * res;
                let sat = drive > 0.25;
                let mut out = [0.0_f32; 2];
                for (ch, &x) in [in_l, in_r].iter().enumerate() {
                    let x_in = if drive > 1e-4 {
                        fast_tanh(x * (1.0 + drive * 3.0))
                    } else {
                        x
                    };
                    let (lp0, _, hp0) = Self::svf_step(
                        g,
                        k,
                        x_in,
                        &mut self.svfs[0].ic1eq[ch],
                        &mut self.svfs[0].ic2eq[ch],
                        sat,
                    );
                    let notch = lp0 + hp0;
                    out[ch] = x * (1.0 - aux) + notch * aux;
                }
                (out[0], out[1])
            }
            FilterMode::HpPeak
            | FilterMode::HpNotch
            | FilterMode::LpNotch
            | FilterMode::LpPeak
            | FilterMode::PeakPeak
            | FilterMode::NotchNotch => {
                let g1 = (PI * fc / sr).tan().min(8.0);
                let freq2 = aux_to_freq2_hz(aux).clamp(20.0, (sr * 0.47).min(20_000.0));
                let g2 = (PI * freq2 / sr).tan().min(8.0);
                let (band1, band2) = ComboBandKind::from_mode(params.mode);

                let mut out = [0.0_f32; 2];
                for (ch, &x) in [in_l, in_r].iter().enumerate() {
                    let x_in = if drive > 1e-4 {
                        fast_tanh(x * (1.0 + drive * 3.0))
                    } else {
                        x
                    };
                    let stage1 = Self::process_combo_band(
                        x_in,
                        ch,
                        band1,
                        params.slope,
                        g1,
                        res,
                        drive,
                        &mut self.svfs,
                        &mut self.one_pole_z[0],
                    );
                    out[ch] = Self::process_combo_band(
                        stage1,
                        ch,
                        band2,
                        params.slope,
                        g2,
                        res,
                        drive,
                        &mut self.combo_svfs,
                        &mut self.one_pole_z[1],
                    );
                }
                (out[0], out[1])
            }
            FilterMode::Comb => (in_l, in_r),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ComboBandKind {
    Hp,
    Lp,
    Peak,
    Notch,
}

impl ComboBandKind {
    #[inline]
    pub(super) fn from_mode(mode: FilterMode) -> (Self, Self) {
        match mode {
            FilterMode::HpPeak => (Self::Hp, Self::Peak),
            FilterMode::HpNotch => (Self::Hp, Self::Notch),
            FilterMode::LpNotch => (Self::Lp, Self::Notch),
            FilterMode::LpPeak => (Self::Lp, Self::Peak),
            FilterMode::PeakPeak => (Self::Peak, Self::Peak),
            FilterMode::NotchNotch => (Self::Notch, Self::Notch),
            _ => (Self::Lp, Self::Notch),
        }
    }

    #[inline]
    pub(super) fn is_peak_or_notch(self) -> bool {
        matches!(self, Self::Peak | Self::Notch)
    }
}

impl FilterSvfCore {
    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn process_combo_band(
        x_in: f32,
        ch: usize,
        kind: ComboBandKind,
        slope: FilterSlope,
        g: f32,
        res: f32,
        drive: f32,
        svfs: &mut [StereoZdfSvf; 4],
        one_pole_z: &mut [f32; 2],
    ) -> f32 {
        if slope == FilterSlope::Db6 && !kind.is_peak_or_notch() {
            let (lp1, hp1) = Self::tpt_1p_step(g, x_in, &mut one_pole_z[ch]);
            return match kind {
                ComboBandKind::Hp => hp1,
                _ => lp1,
            };
        }

        let stages = slope.num_2pole_stages().max(1);
        let k0 = if kind.is_peak_or_notch() {
            1.4 - 1.25 * res
        } else {
            2.0 - 1.95 * res
        };
        let k_helper = (std::f32::consts::SQRT_2 - 0.45 * res).max(0.75);
        let peak_boost = (0.6 + 1.8 * res) / (stages as f32).sqrt();
        let sat = drive > 0.25 || res > 0.85;

        let (lp0, bp0, hp0) = Self::svf_step(
            g,
            k0,
            x_in,
            &mut svfs[0].ic1eq[ch],
            &mut svfs[0].ic2eq[ch],
            sat,
        );
        let mut sig = match kind {
            ComboBandKind::Hp => hp0,
            ComboBandKind::Lp => lp0,
            ComboBandKind::Peak => x_in + peak_boost * k0 * bp0,
            ComboBandKind::Notch => lp0 + hp0,
        };
        for stage in &mut svfs[1..stages] {
            let mid = if drive > 1e-4 {
                fast_tanh(sig * (1.0 + drive * 1.2))
            } else {
                sig
            };
            let (lp_s, bp_s, hp_s) = Self::svf_step(
                g,
                k_helper,
                mid,
                &mut stage.ic1eq[ch],
                &mut stage.ic2eq[ch],
                sat,
            );
            sig = match kind {
                ComboBandKind::Hp => hp_s,
                ComboBandKind::Lp => lp_s,
                ComboBandKind::Peak => mid + 0.5 * peak_boost * k_helper * bp_s,
                ComboBandKind::Notch => lp_s + hp_s,
            };
        }
        sig
    }
}

/// Full per-voice multimode filter including pre-allocated Comb delay buffers.
#[derive(Debug, Clone)]
pub struct FilterState {
    pub core: FilterSvfCore,
    pub comb_buf: [Box<[f32; COMB_BUF_LEN]>; 2],
    pub comb_write: usize,
}

impl Default for FilterState {
    fn default() -> Self {
        Self::new(20_000.0)
    }
}

fn alloc_comb_channel() -> Box<[f32; COMB_BUF_LEN]> {
    vec![0.0_f32; COMB_BUF_LEN]
        .into_boxed_slice()
        .try_into()
        .expect("COMB_BUF_LEN mismatch")
}

impl FilterState {
    pub fn new(initial_cutoff_hz: f32) -> Self {
        Self {
            core: FilterSvfCore::new(initial_cutoff_hz),
            comb_buf: [alloc_comb_channel(), alloc_comb_channel()],
            comb_write: 0,
        }
    }

    pub fn reset(&mut self) {
        self.core.reset_states();
        self.comb_buf[0].fill(0.0);
        self.comb_buf[1].fill(0.0);
        self.comb_write = 0;
    }

    pub fn on_note_on(&mut self, target_cutoff_hz: f32, target_res: f32, target_drive: f32) {
        self.reset();
        self.core
            .snap_params(target_cutoff_hz, target_res, target_drive);
    }

    #[inline]
    pub fn process_stereo(
        &mut self,
        in_l: f32,
        in_r: f32,
        params: &FilterParams,
        midi_note: f32,
        sample_rate: f32,
        engine_mode: EngineMode,
    ) -> (f32, f32) {
        if !params.enabled {
            return (in_l, in_r);
        }

        if params.mode != FilterMode::Comb {
            return self.core.process_core_stereo(
                in_l,
                in_r,
                params,
                midi_note,
                sample_rate,
                engine_mode,
            );
        }

        const PARAM_COEFF: f32 = 0.04;
        self.core.smooth_cutoff_hz += (params.cutoff_hz - self.core.smooth_cutoff_hz) * PARAM_COEFF;
        self.core.smooth_res += (params.resonance - self.core.smooth_res) * PARAM_COEFF;
        self.core.smooth_drive += (params.drive - self.core.smooth_drive) * PARAM_COEFF;

        let sr = sample_rate.max(1000.0);
        let kt_semi = (midi_note - 60.0) * params.keytrack.clamp(0.0, 1.0);
        let fc = (self.core.smooth_cutoff_hz * 2.0_f32.powf(kt_semi / 12.0))
            .clamp(20.0, (sr * 0.47).min(20_000.0));
        let res = self.core.smooth_res.clamp(0.0, 1.0);
        let drive = self.core.smooth_drive.clamp(0.0, 1.0);
        let damp_param = params.aux.clamp(0.0, 1.0);

        let delay = (sr / fc).clamp(2.0, (COMB_BUF_LEN - 2) as f32);
        let delay_i = delay.floor() as usize;
        let frac = delay - delay_i as f32;
        let mask = COMB_BUF_LEN - 1;
        let idx0 = (self.comb_write + COMB_BUF_LEN - delay_i) & mask;
        let idx1 = (idx0 + COMB_BUF_LEN - 1) & mask;

        // `aux` controls high-frequency feedback damping (`0` = bright, `1` = heavily damped)
        let damp = 0.08 + (1.0 - damp_param) * 0.88;
        let fb = res * 0.95;
        let drive_gain = 1.0 + drive * 2.0;

        let mut out = [0.0_f32; 2];
        for (ch, &x) in [in_l, in_r].iter().enumerate() {
            let s0 = self.comb_buf[ch][idx0];
            let s1 = self.comb_buf[ch][idx1];
            let delayed = s0 + (s1 - s0) * frac;
            self.core.comb_lp[ch] += (delayed - self.core.comb_lp[ch]) * damp;
            let damped = self.core.comb_lp[ch];
            let next = fast_tanh(x * drive_gain + fb * damped);
            self.comb_buf[ch][self.comb_write] = next;
            out[ch] = next;
        }
        self.comb_write = (self.comb_write + 1) & mask;

        (out[0], out[1])
    }
}
