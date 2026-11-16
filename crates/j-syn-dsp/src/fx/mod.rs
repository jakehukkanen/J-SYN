//! Parallel dual-bus 8-slot serial Master FX Rack (`Bus A` & `Bus B` + `Sub Bus`),
//! with per-slot stacking (`STACK 1..=4`) and zero audio-thread heap allocations.

pub mod diopser;
pub mod modulation;

use std::f64::consts::FRAC_1_SQRT_2;

use crate::adaa::TanhAdaa2;
use crate::filter::{
    cutoff_norm_to_hz, evaluate_filter_response_db, FilterMode, FilterParams, FilterSlope,
    FilterState,
};
use crate::insert::{mech_clip, tube_saturate};
use crate::mod_matrix::{ModDestination, ModUiTelemetry, NUM_MOD_DESTS};
use crate::oscillator::EngineMode;
use crate::oversample::{Biquad, BiquadCoeffs, Oversampler4x};

use self::diopser::DiopserStereo;
use self::modulation::{ChorusStereo, DelayStereo, PhaserStereo, ReverbStereo};

pub use self::diopser::{
    diopser_freq_hz, diopser_stage_count, evaluate_diopser_ripple, DIOPSER_STAGE_COUNTS,
};
pub use self::modulation::{
    fx_high_cut_hz, fx_low_cut_hz, DELAY_SYNC_LABELS,
};

pub const NUM_FX_BUSES: usize = 2;
pub const NUM_FX_SLOTS: usize = 8;
pub const MAX_FX_STACK: usize = 4;

const SCOPE_HZ: f32 = 150.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FxType {
    Off = 0,
    Ott = 1,
    Distortion = 2,
    Chorus = 3,
    Delay = 4,
    Reverb = 5,
    JSatClipper = 6,
    Filter = 7,
    Phaser = 8,
    Diopser = 9,
}

impl FxType {
    pub const ALL: [Self; 10] = [
        Self::Off,
        Self::Ott,
        Self::Distortion,
        Self::Chorus,
        Self::Delay,
        Self::Reverb,
        Self::JSatClipper,
        Self::Filter,
        Self::Phaser,
        Self::Diopser,
    ];

    #[inline]
    pub const fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::Ott,
            2 => Self::Distortion,
            3 => Self::Chorus,
            4 => Self::Delay,
            5 => Self::Reverb,
            6 => Self::JSatClipper,
            7 => Self::Filter,
            8 => Self::Phaser,
            9 => Self::Diopser,
            _ => Self::Off,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "Empty",
            Self::Ott => "OTT",
            Self::Distortion => "Distortion",
            Self::Chorus => "Chorus",
            Self::Delay => "Delay",
            Self::Reverb => "Reverb",
            Self::JSatClipper => "Clipper",
            Self::Filter => "Filter",
            Self::Phaser => "Phaser",
            Self::Diopser => "Diopser",
        }
    }

    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::Ott => "OTT",
            Self::Distortion => "DIST",
            Self::Chorus => "CHORUS",
            Self::Delay => "DELAY",
            Self::Reverb => "REVERB",
            Self::JSatClipper => "J-SAT",
            Self::Filter => "FILTER",
            Self::Phaser => "PHASER",
            Self::Diopser => "DIOP",
        }
    }

    #[inline]
    pub const fn has_eq_subtab(self) -> bool {
        matches!(self, Self::Chorus | Self::Delay | Self::Reverb)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum FxFilterSlope {
    #[default]
    Db12 = 0,
    Db24 = 1,
    Db48 = 2,
}

impl FxFilterSlope {
    #[inline]
    pub const fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::Db24,
            2 => Self::Db48,
            _ => Self::Db12,
        }
    }

    #[inline]
    pub const fn to_filter_slope(self) -> FilterSlope {
        match self {
            Self::Db12 => FilterSlope::Db12,
            Self::Db24 => FilterSlope::Db24,
            Self::Db48 => FilterSlope::Db48,
        }
    }

    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Db12 => "12 dB",
            Self::Db24 => "24 dB",
            Self::Db48 => "48 dB",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FxSlotParams {
    pub fx_type: FxType,
    pub enabled: bool,
    pub stack: u8,
    pub params: [f32; 4],
    pub eq_params: [f32; 4],
    pub filter_mode: FilterMode,
    pub filter_slope: FxFilterSlope,
    /// `0..=4` index into `DIOPSER_STAGE_COUNTS` (`[8, 16, 32, 64, 128]`).
    pub diopser_stages: u8,
}

impl Default for FxSlotParams {
    fn default() -> Self {
        Self {
            fx_type: FxType::Off,
            enabled: false,
            stack: 1,
            params: [0.5, 0.5, 0.5, 0.5],
            eq_params: [0.0, 1.0, 0.5, 0.5],
            filter_mode: FilterMode::Lp,
            filter_slope: FxFilterSlope::Db12,
            diopser_stages: 2,
        }
    }
}

impl FxSlotParams {
    /// Default normalized `[P1, P2, P3, P4]` knob positions when switching to `fx_type`.
    pub const fn default_params_for_type(fx_type: FxType) -> [f32; 4] {
        match fx_type {
            FxType::Off => [0.5, 0.5, 0.5, 1.0],
            // OTT: Depth 100%, Time 1.00x (0.50), In Gain 0 dB (0.50), Out Gain 0 dB (0.50)
            FxType::Ott => [1.00, 0.50, 0.50, 0.50],
            // Distortion: Mode=Tube (0.0), Drive 30%, Tone Neutral (0.50), Mix 100%
            FxType::Distortion => [0.00, 0.30, 0.50, 1.00],
            // Chorus / Dimension: Rate ~0.5 Hz (0.35), Depth 50%, Dim 75%, Mix 100%
            FxType::Chorus => [0.35, 0.50, 0.75, 1.00],
            // Delay: Time=1/8D (0.43), Feedback 38%, Mod 15%, Mix 100%
            FxType::Delay => [0.43, 0.38, 0.15, 1.00],
            // Reverb: Size 55%, Decay 60%, Damp 45%, Mix 100%
            FxType::Reverb => [0.55, 0.60, 0.45, 1.00],
            // Clipper: Drive 0.0 dB (0.0), Ceil 0.0 dB (1.0), Knee 0%, Mix 100%
            FxType::JSatClipper => [0.00, 1.00, 0.00, 1.00],
            // Filter: Cutoff ~2.5 kHz (0.65), Res 25%, Drive/Scream 0%, Mix 100%
            FxType::Filter => [0.65, 0.25, 0.00, 1.00],
            // Phaser: Rate ~0.21 Hz (0.30), Depth 70%, Feedback 50%, Mix 100%
            FxType::Phaser => [0.30, 0.70, 0.50, 1.00],
            // Diopser: Freq ~405 Hz (0.45), Res 50%, Spread 35%, Mix 100%
            FxType::Diopser => [0.45, 0.50, 0.35, 1.00],
        }
    }

    /// Default normalized secondary `[P5, P6, P7, P8]` (`EQ`) knob positions for `fx_type`.
    pub const fn default_eq_params_for_type(fx_type: FxType) -> [f32; 4] {
        match fx_type {
            // Chorus EQ: Low Cut 20 Hz (0.0), High Cut 20 kHz (1.0), Spread 100% (0.5), Feedback 0% (0.0)
            FxType::Chorus => [0.0, 1.0, 0.50, 0.0],
            // Delay EQ: Low Cut 20 Hz (0.0), High Cut ~8.5 kHz (0.77), Ping-Pong 0% (0.0), Width 100% (0.5)
            FxType::Delay => [0.0, 0.77, 0.0, 0.50],
            // Reverb EQ: Low Cut ~60 Hz (0.24), High Cut 20 kHz (1.0), Pre-Dly 0 ms (0.0), Width 100% (0.5)
            FxType::Reverb => [0.24, 1.0, 0.0, 0.50],
            _ => [0.0, 1.0, 0.50, 0.50],
        }
    }

    /// Default live plugin Init Patch FX chain: all 8 FX slots start `ON` (`enabled = true`)
    /// with `FxType::Off` (`STACK = 1`) and `P4 = 1.0` (`100%` Mix).
    pub const fn init_patch_slot(_slot_idx: usize) -> Self {
        Self {
            fx_type: FxType::Off,
            enabled: true,
            stack: 1,
            params: Self::default_params_for_type(FxType::Off),
            eq_params: [0.0, 1.0, 0.50, 0.50],
            filter_mode: FilterMode::Lp,
            filter_slope: FxFilterSlope::Db12,
            diopser_stages: 2,
        }
    }
}

/// Parameters for one parallel 8-slot serial FX Bus (`Bus A` or `Bus B`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FxBusParams {
    pub slots: [FxSlotParams; NUM_FX_SLOTS],
    pub level: f32,
    pub pan: f32,
}

impl Default for FxBusParams {
    fn default() -> Self {
        Self {
            slots: [FxSlotParams::default(); NUM_FX_SLOTS],
            level: 1.0,
            pan: 0.0,
        }
    }
}

/// Dedicated Sub / Direct Bus bypassing `Bus A` and `Bus B` straight into Master.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubBusParams {
    pub enabled: bool,
    pub lp_cutoff_hz: f32,
    pub clip_drive_db: f32,
    pub level: f32,
}

impl Default for SubBusParams {
    fn default() -> Self {
        Self {
            enabled: true,
            lp_cutoff_hz: 2_000.0,
            clip_drive_db: 0.0,
            level: 1.0,
        }
    }
}

/// Maps normalized `0.0..=1.0` Sub Bus LP Cutoff to `20.0..=2_000.0 Hz`.
#[inline]
pub fn sub_cutoff_norm_to_hz(norm: f32) -> f32 {
    20.0 * 100.0_f32.powf(norm.clamp(0.0, 1.0))
}

/// Maps `20.0..=2_000.0 Hz` Sub Bus LP Cutoff back to normalized `0.0..=1.0`.
#[inline]
pub fn sub_cutoff_hz_to_norm(hz: f32) -> f32 {
    (hz.clamp(20.0, 2_000.0) / 20.0).log(100.0).clamp(0.0, 1.0)
}

/// Full parallel FX Rack (`Bus A` + `Bus B` + `Sub Bus`).
#[derive(Debug, Clone, Copy, Default)]
pub struct FxRackParams {
    pub buses: [FxBusParams; NUM_FX_BUSES],
    pub sub: SubBusParams,
}

#[inline]
fn flush_denormal(x: f32) -> f32 {
    if x.is_finite() && x.abs() >= 1e-24 {
        x
    } else {
        0.0
    }
}

#[inline]
fn track_idle(idle_samples: &mut u32, in_l: f32, in_r: f32) -> bool {
    if in_l.abs() < 1e-8 && in_r.abs() < 1e-8 {
        *idle_samples = idle_samples.saturating_add(1);
        true
    } else {
        *idle_samples = 0;
        false
    }
}

#[inline]
fn butterworth_lp_coeffs(sample_rate: f64, freq_hz: f64) -> BiquadCoeffs {
    BiquadCoeffs::low_pass(sample_rate, freq_hz, FRAC_1_SQRT_2)
}

#[inline]
fn butterworth_hp_coeffs(sample_rate: f64, freq_hz: f64) -> BiquadCoeffs {
    BiquadCoeffs::high_pass(sample_rate, freq_hz, FRAC_1_SQRT_2)
}

#[inline]
fn butterworth_allpass_coeffs(sample_rate: f64, freq_hz: f64) -> BiquadCoeffs {
    BiquadCoeffs::all_pass(sample_rate, freq_hz, FRAC_1_SQRT_2)
}

#[derive(Debug, Clone, Copy, Default)]
struct Lr4BandFilter {
    lp1: Biquad,
    lp2: Biquad,
    hp1: Biquad,
    hp2: Biquad,
}

impl Lr4BandFilter {
    fn set_freq(&mut self, sample_rate: f64, freq_hz: f64) {
        let lp = butterworth_lp_coeffs(sample_rate, freq_hz);
        let hp = butterworth_hp_coeffs(sample_rate, freq_hz);
        self.lp1.set_coeffs(lp);
        self.lp2.set_coeffs(lp);
        self.hp1.set_coeffs(hp);
        self.hp2.set_coeffs(hp);
    }

    fn reset(&mut self) {
        self.lp1.reset();
        self.lp2.reset();
        self.hp1.reset();
        self.hp2.reset();
    }

    #[inline]
    fn split(&mut self, x: f32) -> (f32, f32) {
        let low = self.lp2.process(self.lp1.process(x));
        let high = self.hp2.process(self.hp1.process(x));
        (low, high)
    }
}

#[derive(Debug, Clone, Copy)]
struct OttBandComp {
    env: f32,
    up_thresh_db: f32,
    down_thresh_db: f32,
}

impl OttBandComp {
    const fn new(up_thresh_db: f32, down_thresh_db: f32) -> Self {
        Self {
            env: 0.0,
            up_thresh_db,
            down_thresh_db,
        }
    }

    fn reset(&mut self) {
        self.env = 0.0;
    }

    #[inline]
    fn compute_gain(&mut self, abs_sample: f32, atk_coeff: f32, rel_coeff: f32) -> (f32, f32) {
        let coeff = if abs_sample > self.env {
            atk_coeff
        } else {
            rel_coeff
        };
        self.env += coeff * (abs_sample - self.env);
        self.env = flush_denormal(self.env);

        if self.env <= 1e-5 {
            return (1.0, 0.0);
        }

        let env_db = 20.0 * self.env.log10();
        let gate = ((env_db + 78.0) / 12.0).clamp(0.0, 1.0);

        let boost_db = if env_db < self.up_thresh_db {
            ((self.up_thresh_db - env_db) * 0.72).min(18.0) * gate
        } else {
            0.0
        };

        let cut_db = if env_db > self.down_thresh_db {
            (env_db - self.down_thresh_db) * 0.85
        } else {
            0.0
        };

        let gain_db = boost_db - cut_db;
        let gain_lin = 10.0_f32.powf(gain_db * 0.05);
        let activity = ((boost_db + cut_db) / 18.0).clamp(0.0, 1.0);
        (gain_lin, activity)
    }
}

/// 3-band OTT compressor inspired by `J-SAT` / `vitOTTx` (88.3 Hz & 2500 Hz LR4 phase-coherent crossover).
struct OttStereo {
    low_split: [Lr4BandFilter; 2],
    high_split: [Lr4BandFilter; 2],
    low_ap: [Biquad; 2],
    bands: [OttBandComp; 3],
    sample_rate: f32,
    idle_samples: u32,
}

impl OttStereo {
    fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            low_split: [Lr4BandFilter::default(); 2],
            high_split: [Lr4BandFilter::default(); 2],
            low_ap: [Biquad::default(); 2],
            bands: [
                OttBandComp::new(-40.0, -24.0),
                OttBandComp::new(-38.0, -22.0),
                OttBandComp::new(-38.0, -22.0),
            ],
            sample_rate: 44_100.0,
            idle_samples: 1024,
        };
        s.set_sample_rate(sample_rate);
        s
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(1_000.0) as f64;
        self.sample_rate = sr as f32;
        let ap_high = butterworth_allpass_coeffs(sr, 2500.0);
        for ch in 0..2 {
            self.low_split[ch].set_freq(sr, 88.3);
            self.high_split[ch].set_freq(sr, 2500.0);
            self.low_ap[ch].set_coeffs(ap_high);
        }
    }

    fn reset(&mut self) {
        for ch in 0..2 {
            self.low_split[ch].reset();
            self.high_split[ch].reset();
            self.low_ap[ch].reset();
        }
        for band in &mut self.bands {
            band.reset();
        }
        self.idle_samples = 1024;
    }

    #[inline]
    fn process(&mut self, in_l: f32, in_r: f32, p: [f32; 4]) -> (f32, f32, f32) {
        if track_idle(&mut self.idle_samples, in_l, in_r)
            && self.idle_samples > 256
            && self.bands[0].env < 1e-6
            && self.bands[1].env < 1e-6
            && self.bands[2].env < 1e-6
        {
            return (0.0, 0.0, 0.0);
        }

        let depth = p[0].clamp(0.0, 1.0);
        let time_scale = 0.25 * 16.0_f32.powf(p[1].clamp(0.0, 1.0));
        let in_db = -18.0 + p[2].clamp(0.0, 1.0) * 36.0;
        let out_db = -18.0 + p[3].clamp(0.0, 1.0) * 36.0;
        let in_gain = 10.0_f32.powf(in_db * 0.05);
        let out_gain = 10.0_f32.powf(out_db * 0.05);

        let x_l = in_l * in_gain;
        let x_r = in_r * in_gain;

        // Phase-coherent LR4 3-band split: low passed through 2500 Hz allpass matches mid+high group delay
        let (low_raw_l, rest_l) = self.low_split[0].split(x_l);
        let (low_raw_r, rest_r) = self.low_split[1].split(x_r);
        let low_l = self.low_ap[0].process(low_raw_l);
        let low_r = self.low_ap[1].process(low_raw_r);
        let (mid_l, high_l) = self.high_split[0].split(rest_l);
        let (mid_r, high_r) = self.high_split[1].split(rest_r);

        let sr = self.sample_rate.max(1_000.0);
        let atk_s = 0.012 * time_scale;
        let rel_s = 0.140 * time_scale;
        let atk_c = 1.0 - (-1.0 / (atk_s * sr)).exp();
        let rel_c = 1.0 - (-1.0 / (rel_s * sr)).exp();

        let (g0, a0) = self.bands[0].compute_gain(low_l.abs().max(low_r.abs()), atk_c, rel_c);
        let (g1, a1) = self.bands[1].compute_gain(mid_l.abs().max(mid_r.abs()), atk_c, rel_c);
        let (g2, a2) = self.bands[2].compute_gain(high_l.abs().max(high_r.abs()), atk_c, rel_c);

        let wet_l = (low_l * g0 + mid_l * g1 + high_l * g2) * out_gain;
        let wet_r = (low_r * g0 + mid_r * g1 + high_r * g2) * out_gain;
        let dry_l = (low_l + mid_l + high_l) * out_gain;
        let dry_r = (low_r + mid_r + high_r) * out_gain;

        let l = dry_l + (wet_l - dry_l) * depth;
        let r = dry_r + (wet_r - dry_r) * depth;
        let activity = ((a0 + a1 + a2) * (1.0 / 3.0) * depth).clamp(0.0, 1.0);
        (flush_denormal(l), flush_denormal(r), activity)
    }
}

/// 4x oversampled stereo master distortion (`Tube`, `Mech`, `Sine`, `Crush`).
struct DistortionStereo {
    oversamplers: [Oversampler4x; 2],
    mech_low_shelf: [Biquad; 2],
    mech_high_shelf: [Biquad; 2],
    tone_lp_l: f32,
    tone_lp_r: f32,
    crush_phase: f32,
    hold_l: f32,
    hold_r: f32,
    sample_rate: f32,
    idle_samples: u32,
}

impl DistortionStereo {
    fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            oversamplers: [Oversampler4x::new(); 2],
            mech_low_shelf: [Biquad::default(); 2],
            mech_high_shelf: [Biquad::default(); 2],
            tone_lp_l: 0.0,
            tone_lp_r: 0.0,
            crush_phase: 1.0,
            hold_l: 0.0,
            hold_r: 0.0,
            sample_rate: 44_100.0,
            idle_samples: 1024,
        };
        s.set_sample_rate(sample_rate);
        s
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1_000.0);
        let os_rate = self.sample_rate as f64 * 4.0;
        let low_coeffs = BiquadCoeffs::low_shelf_linear(os_rate, 200.0, 0.7, 1.8);
        let high_coeffs = BiquadCoeffs::high_shelf_linear(os_rate, 4000.0, 0.7, 2.0);
        for ch in 0..2 {
            self.mech_low_shelf[ch].set_coeffs(low_coeffs);
            self.mech_high_shelf[ch].set_coeffs(high_coeffs);
        }
    }

    fn reset(&mut self) {
        for ch in 0..2 {
            self.oversamplers[ch].reset();
            self.mech_low_shelf[ch].reset();
            self.mech_high_shelf[ch].reset();
        }
        self.tone_lp_l = 0.0;
        self.tone_lp_r = 0.0;
        self.crush_phase = 1.0;
        self.hold_l = 0.0;
        self.hold_r = 0.0;
        self.idle_samples = 1024;
    }

    #[inline]
    pub fn sub_mode_index(p0: f32) -> usize {
        if p0 < 0.25 {
            0
        } else if p0 < 0.50 {
            1
        } else if p0 < 0.75 {
            2
        } else {
            3
        }
    }

    #[inline]
    fn process(&mut self, in_l: f32, in_r: f32, p: [f32; 4]) -> (f32, f32, f32) {
        if track_idle(&mut self.idle_samples, in_l, in_r) && self.idle_samples > 128 {
            return (0.0, 0.0, 0.0);
        }

        let mode_idx = Self::sub_mode_index(p[0].clamp(0.0, 1.0));
        let drive = p[1].clamp(0.0, 1.0);
        let tone = p[2].clamp(0.0, 1.0);
        let mix = p[3].clamp(0.0, 1.0);

        if mix <= 1e-5 {
            return (in_l, in_r, 0.0);
        }

        let (mut wl, mut wr) = if mode_idx == 3 {
            // Crush: sample-rate decimation & bit reduction
            let step = 1.0 / (1.0 + drive * drive * 31.0);
            let levels = 2.0_f32.powf(11.0 - drive * 8.5);
            self.crush_phase += step;
            if self.crush_phase >= 1.0 {
                self.crush_phase = self.crush_phase.fract();
                self.hold_l = (in_l.clamp(-1.5, 1.5) * levels).round() / levels;
                self.hold_r = (in_r.clamp(-1.5, 1.5) * levels).round() / levels;
            }
            (self.hold_l, self.hold_r)
        } else {
            let mut ch_out = [0.0_f32; 2];
            let inputs = [in_l, in_r];
            for ch in 0..2 {
                let low_shelf = &mut self.mech_low_shelf[ch];
                let high_shelf = &mut self.mech_high_shelf[ch];
                ch_out[ch] = self.oversamplers[ch].process_sample(inputs[ch], |x| match mode_idx {
                    0 => tube_saturate(x, drive),
                    1 => {
                        let m = high_shelf.process(low_shelf.process(x));
                        mech_clip(m, drive)
                    }
                    _ => (x * (1.0 + drive * 6.0)).sin() * 0.85,
                });
            }
            (ch_out[0], ch_out[1])
        };

        // Post-saturation tilt/tone (< 0.5 warms high end, > 0.5 emphasizes air)
        if tone < 0.495 {
            let t_norm = (0.5 - tone) * 2.0;
            let fc = 18_000.0 * (800.0_f32 / 18_000.0).powf(t_norm);
            let alpha = (2.0 * std::f32::consts::PI * fc / self.sample_rate).clamp(0.02, 0.99);
            self.tone_lp_l += alpha * (wl - self.tone_lp_l);
            self.tone_lp_r += alpha * (wr - self.tone_lp_r);
            wl = self.tone_lp_l;
            wr = self.tone_lp_r;
        } else {
            let alpha = (2.0 * std::f32::consts::PI * 2200.0 / self.sample_rate).clamp(0.02, 0.99);
            self.tone_lp_l += alpha * (wl - self.tone_lp_l);
            self.tone_lp_r += alpha * (wr - self.tone_lp_r);
            let hi_boost = ((tone - 0.5) * 2.0).max(0.0) * 0.85;
            wl += (wl - self.tone_lp_l) * hi_boost;
            wr += (wr - self.tone_lp_r) * hi_boost;
        }

        let out_l = in_l + (wl - in_l) * mix;
        let out_r = in_r + (wr - in_r) * mix;
        let activity = (drive * mix).clamp(0.0, 1.0);
        (flush_denormal(out_l), flush_denormal(out_r), activity)
    }
}

/// 4x oversampled J-SAT Clipper (`TanhAdaa2` soft-knee + digital hard ceiling).
struct JSatClipperStereo {
    oversamplers: [Oversampler4x; 2],
    adaa: [TanhAdaa2; 2],
    idle_samples: u32,
}

impl JSatClipperStereo {
    fn new() -> Self {
        Self {
            oversamplers: [Oversampler4x::new(); 2],
            adaa: [TanhAdaa2::default(); 2],
            idle_samples: 1024,
        }
    }

    fn reset(&mut self) {
        for ch in 0..2 {
            self.oversamplers[ch].reset();
            self.adaa[ch].reset();
        }
        self.idle_samples = 1024;
    }

    #[inline]
    fn process(&mut self, in_l: f32, in_r: f32, p: [f32; 4]) -> (f32, f32, f32) {
        if track_idle(&mut self.idle_samples, in_l, in_r) && self.idle_samples > 128 {
            return (0.0, 0.0, 0.0);
        }

        let drive_db = p[0].clamp(0.0, 1.0) * 24.0;
        let ceil_db = -18.0 + p[1].clamp(0.0, 1.0) * 18.0;
        let knee = p[2].clamp(0.0, 1.0);
        let mix = p[3].clamp(0.0, 1.0);

        let drive_lin = 10.0_f32.powf(drive_db * 0.05);
        let ceil_lin = 10.0_f32.powf(ceil_db * 0.05).max(0.02);
        let inv_ceil = (1.0 / ceil_lin) as f64;

        let driven_l = in_l * drive_lin;
        let driven_r = in_r * drive_lin;
        let driven_peak = driven_l.abs().max(driven_r.abs());

        let mut ch_out = [0.0_f32; 2];
        let inputs = [driven_l, driven_r];
        for ch in 0..2 {
            let adaa = &mut self.adaa[ch];
            ch_out[ch] = self.oversamplers[ch].process_sample(inputs[ch], |x| {
                let hard = x.clamp(-ceil_lin, ceil_lin);
                if knee <= 1e-4 {
                    hard
                } else {
                    let soft = (adaa.process((x as f64) * inv_ceil) * ceil_lin as f64) as f32;
                    hard + (soft - hard) * knee
                }
            });
        }

        let post_peak = ch_out[0].abs().max(ch_out[1].abs());
        let gr_activity = if driven_peak > 1e-5 {
            ((driven_peak - post_peak).max(0.0) / driven_peak).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let out_l = in_l + (ch_out[0] - in_l) * mix;
        let out_r = in_r + (ch_out[1] - in_r) * mix;
        (flush_denormal(out_l), flush_denormal(out_r), gr_activity * mix)
    }
}

/// Full-parity multimode stereo Filter FX reusing per-voice ZDF SVF + Comb + ADAA Scream engine.
struct FxFilterStereo {
    state: FilterState,
    sample_rate: f32,
}

impl FxFilterStereo {
    fn new(sample_rate: f32) -> Self {
        Self {
            state: FilterState::new(2_500.0),
            sample_rate: sample_rate.max(1_000.0),
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1_000.0);
    }

    fn reset(&mut self) {
        self.state.reset();
    }

    #[inline]
    fn filter_params(
        mode: FilterMode,
        slope: FxFilterSlope,
        p: [f32; 4],
    ) -> FilterParams {
        let fc = cutoff_norm_to_hz(p[0]);
        let eff_slope = if mode.has_fixed_12db_slope() {
            FilterSlope::Db12
        } else {
            slope.to_filter_slope()
        };
        let aux = match mode {
            FilterMode::Scream => p[2].clamp(0.0, 1.0),
            FilterMode::Comb => 0.35,
            m if m.is_multi() => {
                let freq2 = (fc * 1.65).clamp(20.0, 20_000.0);
                (freq2 / 20.0).log(1000.0).clamp(0.0, 1.0)
            }
            _ => p[3].clamp(0.0, 1.0),
        };
        let drive = if mode == FilterMode::Scream {
            0.0
        } else {
            p[2].clamp(0.0, 1.0)
        };
        FilterParams {
            enabled: true,
            mode,
            slope: eff_slope,
            cutoff_hz: fc,
            resonance: p[1].clamp(0.0, 1.0),
            drive,
            keytrack: 0.0,
            aux,
        }
    }

    #[inline]
    fn process(
        &mut self,
        in_l: f32,
        in_r: f32,
        mode: FilterMode,
        slope: FxFilterSlope,
        p: [f32; 4],
    ) -> (f32, f32, f32) {
        let fp = Self::filter_params(mode, slope, p);
        let mix = p[3].clamp(0.0, 1.0);
        let (wl, wr) = self.state.process_stereo(
            in_l,
            in_r,
            &fp,
            60.0,
            self.sample_rate,
            EngineMode::Hq,
        );
        // `Scream`, `Comb`, and multi-filters repurpose `fp.aux`, so apply Mix manually here.
        let (ol, or) = if mode == FilterMode::Scream || mode == FilterMode::Comb || mode.is_multi() {
            (in_l + (wl - in_l) * mix, in_r + (wr - in_r) * mix)
        } else {
            (wl, wr)
        };
        let act = ol.abs().max(or.abs()).clamp(0.0, 1.0) * mix;
        (flush_denormal(ol), flush_denormal(or), act)
    }
}

/// Evaluates steady-state magnitude response in dB (`-36.0` .. `+24.0` dB) for an FX Filter slot.
pub fn filter_response_db_from_slot(
    mode: FilterMode,
    slope: FxFilterSlope,
    p: [f32; 4],
    freq_hz: f32,
) -> f32 {
    let fp = FxFilterStereo::filter_params(mode, slope, p);
    let mix = p[3].clamp(0.0, 1.0);
    let wet_db = evaluate_filter_response_db(&fp, freq_hz);
    if (mode == FilterMode::Scream || mode == FilterMode::Comb || mode.is_multi()) && mix < 0.999 {
        let wet_mag = 10.0_f32.powf(wet_db / 20.0);
        let mag = (1.0 - mix) + mix * wet_mag;
        (20.0 * mag.max(1e-4).log10()).clamp(-36.0, 24.0)
    } else {
        wet_db
    }
}

/// Complete single FX processing stage holding pre-allocated DSP sub-engines.
struct StereoFxStage {
    ott: OttStereo,
    dist: DistortionStereo,
    chorus: ChorusStereo,
    delay: DelayStereo,
    reverb: ReverbStereo,
    clipper: JSatClipperStereo,
    filter: FxFilterStereo,
    phaser: PhaserStereo,
    diopser: DiopserStereo,
}

impl StereoFxStage {
    fn new(sample_rate: f32) -> Self {
        Self {
            ott: OttStereo::new(sample_rate),
            dist: DistortionStereo::new(sample_rate),
            chorus: ChorusStereo::new(sample_rate),
            delay: DelayStereo::new(sample_rate),
            reverb: ReverbStereo::new(sample_rate),
            clipper: JSatClipperStereo::new(),
            filter: FxFilterStereo::new(sample_rate),
            phaser: PhaserStereo::new(sample_rate),
            diopser: DiopserStereo::new(sample_rate),
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.ott.set_sample_rate(sample_rate);
        self.dist.set_sample_rate(sample_rate);
        self.chorus.set_sample_rate(sample_rate);
        self.delay.set_sample_rate(sample_rate);
        self.reverb.set_sample_rate(sample_rate);
        self.filter.set_sample_rate(sample_rate);
        self.phaser.set_sample_rate(sample_rate);
        self.diopser.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.ott.reset();
        self.dist.reset();
        self.chorus.reset();
        self.delay.reset();
        self.reverb.reset();
        self.clipper.reset();
        self.filter.reset();
        self.phaser.reset();
        self.diopser.reset();
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn process_type(
        &mut self,
        slot: &FxSlotParams,
        in_l: f32,
        in_r: f32,
        p: [f32; 4],
        eq: [f32; 4],
        tempo_bpm: f32,
    ) -> (f32, f32, f32) {
        match slot.fx_type {
            FxType::Off => (in_l, in_r, 0.0),
            FxType::Ott => self.ott.process(in_l, in_r, p),
            FxType::Distortion => self.dist.process(in_l, in_r, p),
            FxType::Chorus => self.chorus.process(in_l, in_r, p, eq),
            FxType::Delay => self.delay.process(in_l, in_r, p, eq, tempo_bpm),
            FxType::Reverb => self.reverb.process(in_l, in_r, p, eq),
            FxType::JSatClipper => self.clipper.process(in_l, in_r, p),
            FxType::Filter => self.filter.process(in_l, in_r, slot.filter_mode, slot.filter_slope, p),
            FxType::Phaser => self.phaser.process(in_l, in_r, p),
            FxType::Diopser => self.diopser.process(in_l, in_r, p, slot.diopser_stages as usize),
        }
    }
}

/// Pre-allocated single 8-slot serial FX Bus (`Bus A` or `Bus B`).
struct FxBusState {
    bus_idx: usize,
    stages: Box<[[StereoFxStage; MAX_FX_STACK]]>,
    last_slot_types: [Option<FxType>; NUM_FX_SLOTS],
    scope_accum: [(f32, f32, f32, usize); NUM_FX_SLOTS],
    scope_samples_per_col: usize,
}

impl FxBusState {
    fn new(bus_idx: usize, sample_rate: f32) -> Self {
        let sr = sample_rate.max(1_000.0);
        let mut stages_vec = Vec::with_capacity(NUM_FX_SLOTS);
        for _ in 0..NUM_FX_SLOTS {
            stages_vec.push(std::array::from_fn(|_| StereoFxStage::new(sr)));
        }
        Self {
            bus_idx,
            stages: stages_vec.into_boxed_slice(),
            last_slot_types: [None; NUM_FX_SLOTS],
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
        self.last_slot_types = [None; NUM_FX_SLOTS];
        self.scope_accum = [(0.0, 0.0, 0.0, 0); NUM_FX_SLOTS];
    }

    #[inline]
    fn process_sample(
        &mut self,
        in_l: f32,
        in_r: f32,
        bus: &FxBusParams,
        mod_offsets: &[f32; NUM_MOD_DESTS],
        tempo_bpm: f32,
        telemetry: Option<&ModUiTelemetry>,
    ) -> (f32, f32) {
        let mut l = in_l;
        let mut r = in_r;

        for (slot_idx, slot) in bus.slots.iter().enumerate() {
            match self.last_slot_types[slot_idx] {
                Some(prev) if prev != slot.fx_type => {
                    for stage in &mut self.stages[slot_idx] {
                        stage.reset();
                    }
                    self.last_slot_types[slot_idx] = Some(slot.fx_type);
                    if let Some(tel) = telemetry {
                        tel.fx_scope(self.bus_idx, slot_idx).clear();
                    }
                }
                None => {
                    self.last_slot_types[slot_idx] = Some(slot.fx_type);
                }
                Some(_) => {}
            }

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

                let stack_count = (slot.stack as usize).clamp(1, MAX_FX_STACK);
                for stage_idx in 0..stack_count {
                    let (nl, nr, act) = self.stages[slot_idx][stage_idx].process_type(
                        slot,
                        l,
                        r,
                        eff,
                        eff_eq,
                        tempo_bpm,
                    );
                    l = nl;
                    r = nr;
                    slot_activity = slot_activity.max(act);
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
        }

        let (lvl_dest, pan_dest) = if self.bus_idx == 0 {
            (ModDestination::BusALevel, ModDestination::BusAPan)
        } else {
            (ModDestination::BusBLevel, ModDestination::BusBPan)
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
}

impl SubBusProcessor {
    fn new(sample_rate: f32) -> Self {
        Self {
            lp1: [Biquad::default(); 2],
            lp2: [Biquad::default(); 2],
            oversamplers: [Oversampler4x::new(); 2],
            adaa: [TanhAdaa2::default(); 2],
            sample_rate: sample_rate.max(1_000.0),
            last_cutoff_hz: -1.0,
            last_sr: 0.0,
            idle_samples: 1024,
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1_000.0);
        self.last_sr = 0.0;
    }

    fn reset(&mut self) {
        for ch in 0..2 {
            self.lp1[ch].reset();
            self.lp2[ch].reset();
            self.oversamplers[ch].reset();
            self.adaa[ch].reset();
        }
        self.idle_samples = 1024;
    }

    #[inline]
    fn process_sample(
        &mut self,
        in_l: f32,
        in_r: f32,
        sub: &SubBusParams,
        mod_offsets: &[f32; NUM_MOD_DESTS],
    ) -> (f32, f32) {
        if !sub.enabled {
            return (0.0, 0.0);
        }
        if track_idle(&mut self.idle_samples, in_l, in_r) && self.idle_samples > 256 {
            return (0.0, 0.0);
        }

        let cutoff_norm =
            (sub_cutoff_hz_to_norm(sub.lp_cutoff_hz) + mod_offsets[ModDestination::SubCutoff.index()])
                .clamp(0.0, 1.0);
        let cutoff_hz = sub_cutoff_norm_to_hz(cutoff_norm);
        let drive_db =
            (sub.clip_drive_db + mod_offsets[ModDestination::SubDrive.index()] * 24.0).clamp(0.0, 24.0);
        let level = (sub.level + mod_offsets[ModDestination::SubLevel.index()] * 2.0).clamp(0.0, 2.0);

        if (cutoff_hz - self.last_cutoff_hz).abs() > 0.25
            || (self.sample_rate - self.last_sr).abs() > 0.5
        {
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

        for (ch, &x) in [in_l, in_r].iter().enumerate() {
            let filt = self.lp2[ch].process(self.lp1[ch].process(x));
            let driven = filt * drive_lin;
            let adaa = &mut self.adaa[ch];
            let clipped = self.oversamplers[ch].process_sample(driven, |s| {
                let hard = s.clamp(-1.0, 1.0);
                let soft = adaa.process(s as f64) as f32;
                hard + (soft - hard) * soft_knee
            });
            out[ch] = flush_denormal(clipped * level);
        }

        (out[0], out[1])
    }
}

/// Complete parallel dual-bus 8-slot FX Rack + dedicated Sub Bus.
pub struct FxRackState {
    buses: [FxBusState; NUM_FX_BUSES],
    sub_bus: SubBusProcessor,
}

impl FxRackState {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1_000.0);
        Self {
            buses: [FxBusState::new(0, sr), FxBusState::new(1, sr)],
            sub_bus: SubBusProcessor::new(sr),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(1_000.0);
        self.buses[0].set_sample_rate(sr);
        self.buses[1].set_sample_rate(sr);
        self.sub_bus.set_sample_rate(sr);
    }

    pub fn reset(&mut self) {
        self.buses[0].reset();
        self.buses[1].reset();
        self.sub_bus.reset();
    }

    #[inline]
    pub fn format_distortion_submode(p0: f32) -> &'static str {
        match DistortionStereo::sub_mode_index(p0) {
            0 => "Tube",
            1 => "Mech",
            2 => "Sine",
            _ => "Crush",
        }
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
    ) -> FilterParams {
        FxFilterStereo::filter_params(mode, slope, p)
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
        let (al, ar) = self.buses[0].process_sample(
            bus_a_in.0,
            bus_a_in.1,
            &rack.buses[0],
            mod_offsets,
            tempo_bpm,
            telemetry,
        );
        let (bl, br) = self.buses[1].process_sample(
            bus_b_in.0,
            bus_b_in.1,
            &rack.buses[1],
            mod_offsets,
            tempo_bpm,
            telemetry,
        );
        let (sl, sr) = self
            .sub_bus
            .process_sample(sub_in.0, sub_in.1, &rack.sub, mod_offsets);

        (flush_denormal(al + bl + sl), flush_denormal(ar + br + sr))
    }
}
