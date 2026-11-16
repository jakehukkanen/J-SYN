//! Pre-allocated 8-slot serial Master FX Rack (`FX 1`–`FX 8`) with per-slot stacking (`STACK 1..=4`).
//! Zero audio-thread heap allocations (`Box`ed stage matrix allocated once in `FxRackState::new`).

use std::f64::consts::{FRAC_1_SQRT_2, PI};

use crate::adaa::TanhAdaa2;
use crate::filter::fast_tanh;
use crate::insert::{mech_clip, tube_saturate};
use crate::mod_matrix::{ModDestination, ModUiTelemetry, NUM_MOD_DESTS};
use crate::oversample::{Biquad, BiquadCoeffs, Oversampler4x};

pub const NUM_FX_SLOTS: usize = 8;
pub const MAX_FX_STACK: usize = 4;

const CHORUS_BUF_LEN: usize = 4096;
const MAX_DELAY_SAMPLES: usize = 96_000;
const REVERB_COMB_LEN: usize = 4096;
const REVERB_AP_LEN: usize = 1024;
const SCOPE_HZ: f32 = 150.0;

use crate::filter::{cutoff_norm_to_hz, FilterMode, FilterParams, FilterSlope, FilterSvfCore};

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
}

impl FxType {
    pub const ALL: [Self; 9] = [
        Self::Off,
        Self::Ott,
        Self::Distortion,
        Self::Chorus,
        Self::Delay,
        Self::Reverb,
        Self::JSatClipper,
        Self::Filter,
        Self::Phaser,
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
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FxSlotParams {
    pub fx_type: FxType,
    pub enabled: bool,
    pub stack: u8,
    pub params: [f32; 4],
}

impl Default for FxSlotParams {
    fn default() -> Self {
        Self {
            fx_type: FxType::Off,
            enabled: false,
            stack: 1,
            params: [0.5, 0.5, 0.5, 0.5],
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
            // Chorus: Rate ~0.5 Hz (0.35), Depth 50%, Width 75%, Mix 100%
            FxType::Chorus => [0.35, 0.50, 0.75, 1.00],
            // Delay: Time=1/8D (0.43 -> index 3), Feedback 38%, Damp 35%, Mix 100%
            FxType::Delay => [0.43, 0.38, 0.35, 1.00],
            // Reverb: Size 55%, Decay 60%, Damp 45%, Mix 100%
            FxType::Reverb => [0.55, 0.60, 0.45, 1.00],
            // Clipper: Drive 0.0 dB (0.0), Ceil 0.0 dB (1.0), Knee 0%, Mix 100%
            FxType::JSatClipper => [0.00, 1.00, 0.00, 1.00],
            // Filter: Cutoff ~2.5 kHz (0.65), Res 25%, Type LP 12 (0.0), Mix 100%
            FxType::Filter => [0.65, 0.25, 0.00, 1.00],
            // Phaser: Rate ~0.21 Hz (0.30), Depth 70%, Feedback 50%, Mix 100%
            FxType::Phaser => [0.30, 0.70, 0.50, 1.00],
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
        }
    }
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
    let clamped = freq_hz.clamp(10.0, sample_rate * 0.45);
    let w0 = 2.0 * PI * clamped / sample_rate;
    let (sin_w0, cos_w0) = w0.sin_cos();
    let alpha = sin_w0 / (2.0 * FRAC_1_SQRT_2);
    let b0 = 0.5 * (1.0 - cos_w0);
    let b1 = 1.0 - cos_w0;
    let b2 = b0;
    let a0 = 1.0 + alpha;
    let inv = 1.0 / a0;
    BiquadCoeffs {
        b0: b0 * inv,
        b1: b1 * inv,
        b2: b2 * inv,
        a1: (-2.0 * cos_w0) * inv,
        a2: (1.0 - alpha) * inv,
    }
}

#[inline]
fn butterworth_hp_coeffs(sample_rate: f64, freq_hz: f64) -> BiquadCoeffs {
    let clamped = freq_hz.clamp(10.0, sample_rate * 0.45);
    let w0 = 2.0 * PI * clamped / sample_rate;
    let (sin_w0, cos_w0) = w0.sin_cos();
    let alpha = sin_w0 / (2.0 * FRAC_1_SQRT_2);
    let b0 = 0.5 * (1.0 + cos_w0);
    let b1 = -(1.0 + cos_w0);
    let b2 = b0;
    let a0 = 1.0 + alpha;
    let inv = 1.0 / a0;
    BiquadCoeffs {
        b0: b0 * inv,
        b1: b1 * inv,
        b2: b2 * inv,
        a1: (-2.0 * cos_w0) * inv,
        a2: (1.0 - alpha) * inv,
    }
}

#[inline]
fn butterworth_allpass_coeffs(sample_rate: f64, freq_hz: f64) -> BiquadCoeffs {
    let clamped = freq_hz.clamp(10.0, sample_rate * 0.45);
    let w0 = 2.0 * PI * clamped / sample_rate;
    let (sin_w0, cos_w0) = w0.sin_cos();
    let alpha = sin_w0 / (2.0 * FRAC_1_SQRT_2);
    let b0 = 1.0 - alpha;
    let b1 = -2.0 * cos_w0;
    let b2 = 1.0 + alpha;
    let a0 = 1.0 + alpha;
    let inv = 1.0 / a0;
    BiquadCoeffs {
        b0: b0 * inv,
        b1: b1 * inv,
        b2: b2 * inv,
        a1: (-2.0 * cos_w0) * inv,
        a2: (1.0 - alpha) * inv,
    }
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

/// Stereo modulated Chorus / Dimension processor with pre-allocated delay lines.
struct ChorusStereo {
    buf_l: Box<[f32]>,
    buf_r: Box<[f32]>,
    write_pos: usize,
    lfo_phase: f32,
    sample_rate: f32,
    dirty: bool,
    idle_samples: u32,
}

impl ChorusStereo {
    fn new(sample_rate: f32) -> Self {
        Self {
            buf_l: vec![0.0; CHORUS_BUF_LEN].into_boxed_slice(),
            buf_r: vec![0.0; CHORUS_BUF_LEN].into_boxed_slice(),
            write_pos: 0,
            lfo_phase: 0.0,
            sample_rate: sample_rate.max(1_000.0),
            dirty: false,
            idle_samples: 1024,
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1_000.0);
    }

    fn reset(&mut self) {
        if self.dirty {
            self.buf_l.fill(0.0);
            self.buf_r.fill(0.0);
            self.dirty = false;
        }
        self.write_pos = 0;
        self.lfo_phase = 0.0;
        self.idle_samples = 1024;
    }

    #[inline]
    fn read_linear(buf: &[f32], write_pos: usize, delay_samples: f32) -> f32 {
        let len = buf.len();
        let d = delay_samples.clamp(1.0, (len - 2) as f32);
        let d_int = d as usize;
        let frac = d - d_int as f32;
        let idx0 = (write_pos + len - d_int) % len;
        let idx1 = (write_pos + len - (d_int + 1)) % len;
        buf[idx0] + (buf[idx1] - buf[idx0]) * frac
    }

    #[inline]
    fn process(&mut self, in_l: f32, in_r: f32, p: [f32; 4]) -> (f32, f32, f32) {
        if track_idle(&mut self.idle_samples, in_l, in_r) {
            if !self.dirty {
                return (0.0, 0.0, 0.0);
            }
            if self.idle_samples > (CHORUS_BUF_LEN as u32 + 64) {
                self.reset();
                return (0.0, 0.0, 0.0);
            }
        }

        let rate_hz = 0.05 * 100.0_f32.powf(p[0].clamp(0.0, 1.0));
        let depth = p[1].clamp(0.0, 1.0);
        let width = p[2].clamp(0.0, 1.0);
        let mix = p[3].clamp(0.0, 1.0);

        self.dirty = true;
        self.buf_l[self.write_pos] = in_l;
        self.buf_r[self.write_pos] = in_r;

        self.lfo_phase = (self.lfo_phase + rate_hz / self.sample_rate).fract();
        let angle_l = self.lfo_phase * std::f32::consts::TAU;
        let angle_r = angle_l + width * std::f32::consts::FRAC_PI_2 * 2.0;

        let sr_ms = self.sample_rate * 0.001;
        let center_ms = 9.0 + width * 4.0;
        let swing_ms = depth * 4.5;
        let d_l = (center_ms + swing_ms * angle_l.sin()) * sr_ms;
        let d_r = (center_ms + 1.1 * width + swing_ms * angle_r.sin()) * sr_ms;

        let wet_raw_l = Self::read_linear(&self.buf_l, self.write_pos, d_l);
        let wet_raw_r = Self::read_linear(&self.buf_r, self.write_pos, d_r);

        // M/S stereo width enhancement on chorus wet signal
        let mid = 0.5 * (wet_raw_l + wet_raw_r);
        let side = 0.5 * (wet_raw_l - wet_raw_r) * (0.4 + width * 1.1);
        let wet_l = mid + side;
        let wet_r = mid - side;

        self.write_pos = (self.write_pos + 1) % CHORUS_BUF_LEN;
        let out_l = in_l + (wet_l - in_l) * mix;
        let out_r = in_r + (wet_r - in_r) * mix;
        let activity = (0.5 * (angle_l.sin().abs() + angle_r.sin().abs()) * mix).clamp(0.0, 1.0);
        (flush_denormal(out_l), flush_denormal(out_r), activity)
    }
}

const PHASER_STAGES: usize = 6;

/// 6-stage stereo all-pass Phaser with quadrature LFO sweep and soft-limited resonant feedback.
struct PhaserStereo {
    ap_z: [[f32; PHASER_STAGES]; 2],
    fb_state: [f32; 2],
    lfo_phase: f32,
    sample_rate: f32,
    idle_samples: u32,
}

impl PhaserStereo {
    fn new(sample_rate: f32) -> Self {
        Self {
            ap_z: [[0.0; PHASER_STAGES]; 2],
            fb_state: [0.0; 2],
            lfo_phase: 0.0,
            sample_rate: sample_rate.max(1_000.0),
            idle_samples: 1024,
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1_000.0);
    }

    fn reset(&mut self) {
        self.ap_z = [[0.0; PHASER_STAGES]; 2];
        self.fb_state = [0.0; 2];
        self.lfo_phase = 0.0;
        self.idle_samples = 1024;
    }

    #[inline]
    fn phase_to_allpass_coeff(&self, norm: f32) -> f32 {
        // Exponentially swept notch center frequency (~180 Hz .. ~4.6 kHz)
        let fc = (180.0 * 25.5_f32.powf(norm.clamp(0.0, 1.0))).min(self.sample_rate * 0.42);
        let t = (std::f32::consts::PI * fc / self.sample_rate).clamp(0.005, 1.4).tan();
        (t - 1.0) / (t + 1.0)
    }

    #[inline]
    fn process(&mut self, in_l: f32, in_r: f32, p: [f32; 4]) -> (f32, f32, f32) {
        if track_idle(&mut self.idle_samples, in_l, in_r) && self.idle_samples > 4096 {
            self.ap_z = [[0.0; PHASER_STAGES]; 2];
            self.fb_state = [0.0; 2];
            return (0.0, 0.0, 0.0);
        }

        let rate_hz = 0.05 * 120.0_f32.powf(p[0].clamp(0.0, 1.0));
        let depth = p[1].clamp(0.0, 1.0);
        let fb_gain = p[2].clamp(0.0, 1.0) * 0.88;
        let mix = p[3].clamp(0.0, 1.0);

        self.lfo_phase = (self.lfo_phase + rate_hz / self.sample_rate).fract();
        let lfo_l = 0.5 + 0.5 * (self.lfo_phase * std::f32::consts::TAU).sin();
        let lfo_r = 0.5
            + 0.5 * (self.lfo_phase * std::f32::consts::TAU + std::f32::consts::FRAC_PI_2).sin();

        let sweep_l = 0.5 + (lfo_l - 0.5) * depth;
        let sweep_r = 0.5 + (lfo_r - 0.5) * depth;
        let coeffs = [
            self.phase_to_allpass_coeff(sweep_l),
            self.phase_to_allpass_coeff(sweep_r),
        ];

        let mut out = [0.0_f32; 2];
        for (ch, &x) in [in_l, in_r].iter().enumerate() {
            let a = coeffs[ch];
            let mut u = flush_denormal(x + fb_gain * self.fb_state[ch]);
            for z in &mut self.ap_z[ch] {
                let y = a * u + *z;
                *z = flush_denormal(u - a * y);
                u = y;
            }
            self.fb_state[ch] = flush_denormal(fast_tanh(u * 0.85) * 1.15);
            let phased = 0.5 * (x + u);
            out[ch] = flush_denormal(x + (phased - x) * mix);
        }

        let activity = (0.5 * (lfo_l + lfo_r) * depth * mix).clamp(0.0, 1.0);
        (out[0], out[1], activity)
    }
}

pub const DELAY_SYNC_LABELS: [&str; 8] = ["1/16", "1/8T", "1/8", "1/8D", "1/4", "1/4D", "1/2", "1/1"];
const DELAY_SYNC_BEATS: [f32; 8] = [0.25, 1.0 / 3.0, 0.5, 0.75, 1.0, 1.5, 2.0, 4.0];

/// Tempo-synced & damped stereo feedback delay with sleek delay-time slewing.
struct DelayStereo {
    buf_l: Box<[f32]>,
    buf_r: Box<[f32]>,
    write_pos: usize,
    smooth_delay: f32,
    lp_l: f32,
    lp_r: f32,
    sample_rate: f32,
    dirty: bool,
    idle_samples: u32,
}

impl DelayStereo {
    fn new(sample_rate: f32) -> Self {
        let init_delay = (sample_rate.max(1_000.0) * 0.375).min((MAX_DELAY_SAMPLES - 4) as f32);
        Self {
            buf_l: vec![0.0; MAX_DELAY_SAMPLES].into_boxed_slice(),
            buf_r: vec![0.0; MAX_DELAY_SAMPLES].into_boxed_slice(),
            write_pos: 0,
            smooth_delay: init_delay,
            lp_l: 0.0,
            lp_r: 0.0,
            sample_rate: sample_rate.max(1_000.0),
            dirty: false,
            idle_samples: 1024,
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1_000.0);
    }

    fn reset(&mut self) {
        if self.dirty {
            self.buf_l.fill(0.0);
            self.buf_r.fill(0.0);
            self.dirty = false;
        }
        self.write_pos = 0;
        self.lp_l = 0.0;
        self.lp_r = 0.0;
        self.idle_samples = 1024;
    }

    #[inline]
    pub fn sync_index(p0: f32) -> usize {
        ((p0.clamp(0.0, 0.9999) * 8.0) as usize).min(7)
    }

    #[inline]
    fn read_linear(buf: &[f32], write_pos: usize, delay_samples: f32) -> f32 {
        let len = buf.len();
        let d = delay_samples.clamp(1.0, (len - 2) as f32);
        let d_int = d as usize;
        let frac = d - d_int as f32;
        let idx0 = (write_pos + len - d_int) % len;
        let idx1 = (write_pos + len - (d_int + 1)) % len;
        buf[idx0] + (buf[idx1] - buf[idx0]) * frac
    }

    #[inline]
    fn process(
        &mut self,
        in_l: f32,
        in_r: f32,
        p: [f32; 4],
        tempo_bpm: f32,
    ) -> (f32, f32, f32) {
        if track_idle(&mut self.idle_samples, in_l, in_r) && !self.dirty {
            return (0.0, 0.0, 0.0);
        }

        let idx = Self::sync_index(p[0]);
        let beats = DELAY_SYNC_BEATS[idx];
        let bpm = tempo_bpm.clamp(20.0, 300.0);
        let target_samples = ((60.0 / bpm) * beats * self.sample_rate)
            .clamp(16.0, (MAX_DELAY_SAMPLES - 4) as f32);

        let slew = 1.0 - (-1.0 / (0.025 * self.sample_rate)).exp();
        self.smooth_delay += slew * (target_samples - self.smooth_delay);

        let feedback = (p[1].clamp(0.0, 1.0) * 0.94).min(0.94);
        let damp = p[2].clamp(0.0, 1.0);
        let mix = p[3].clamp(0.0, 1.0);

        let damp_fc = 16_000.0 * (700.0_f32 / 16_000.0).powf(damp);
        let lp_coeff = (2.0 * std::f32::consts::PI * damp_fc / self.sample_rate).clamp(0.01, 0.99);

        let tap_l = Self::read_linear(&self.buf_l, self.write_pos, self.smooth_delay);
        let tap_r = Self::read_linear(&self.buf_r, self.write_pos, self.smooth_delay * 0.992);

        self.lp_l = flush_denormal(self.lp_l + lp_coeff * (tap_l - self.lp_l));
        self.lp_r = flush_denormal(self.lp_r + lp_coeff * (tap_r - self.lp_r));

        if self.idle_samples > (MAX_DELAY_SAMPLES as u32)
            && tap_l.abs() < 1e-8
            && tap_r.abs() < 1e-8
            && self.lp_l.abs() < 1e-8
            && self.lp_r.abs() < 1e-8
        {
            self.reset();
            return (0.0, 0.0, 0.0);
        }

        let fb_l = fast_tanh((in_l + self.lp_l * feedback + self.lp_r * (feedback * 0.12)) * 0.5) * 2.0;
        let fb_r = fast_tanh((in_r + self.lp_r * feedback + self.lp_l * (feedback * 0.12)) * 0.5) * 2.0;

        self.dirty = true;
        self.buf_l[self.write_pos] = fb_l;
        self.buf_r[self.write_pos] = fb_r;
        self.write_pos = (self.write_pos + 1) % MAX_DELAY_SAMPLES;

        let out_l = in_l + tap_l * mix;
        let out_r = in_r + tap_r * mix;
        let activity = (tap_l.abs().max(tap_r.abs()) * mix).clamp(0.0, 1.0);
        (flush_denormal(out_l), flush_denormal(out_r), activity)
    }
}

const COMB_BASE: [usize; 4] = [1116, 1277, 1422, 1617];
const AP_BASE: [usize; 2] = [556, 441];
const STEREO_SPREAD: usize = 29;

/// Freeverb/FDN-style stereo algorithmic plate/hall reverb with pre-allocated delay lines.
struct ReverbStereo {
    comb_l: [Box<[f32]>; 4],
    comb_r: [Box<[f32]>; 4],
    comb_pos_l: [usize; 4],
    comb_pos_r: [usize; 4],
    comb_damp_l: [f32; 4],
    comb_damp_r: [f32; 4],
    ap_l: [Box<[f32]>; 2],
    ap_r: [Box<[f32]>; 2],
    ap_pos_l: [usize; 2],
    ap_pos_r: [usize; 2],
    sample_rate: f32,
    dirty: bool,
    idle_samples: u32,
}

impl ReverbStereo {
    fn new(sample_rate: f32) -> Self {
        Self {
            comb_l: std::array::from_fn(|_| vec![0.0; REVERB_COMB_LEN].into_boxed_slice()),
            comb_r: std::array::from_fn(|_| vec![0.0; REVERB_COMB_LEN].into_boxed_slice()),
            comb_pos_l: [0; 4],
            comb_pos_r: [0; 4],
            comb_damp_l: [0.0; 4],
            comb_damp_r: [0.0; 4],
            ap_l: std::array::from_fn(|_| vec![0.0; REVERB_AP_LEN].into_boxed_slice()),
            ap_r: std::array::from_fn(|_| vec![0.0; REVERB_AP_LEN].into_boxed_slice()),
            ap_pos_l: [0; 2],
            ap_pos_r: [0; 2],
            sample_rate: sample_rate.max(1_000.0),
            dirty: false,
            idle_samples: 1024,
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1_000.0);
    }

    fn reset(&mut self) {
        if self.dirty {
            for i in 0..4 {
                self.comb_l[i].fill(0.0);
                self.comb_r[i].fill(0.0);
            }
            for i in 0..2 {
                self.ap_l[i].fill(0.0);
                self.ap_r[i].fill(0.0);
            }
            self.dirty = false;
        }
        self.comb_pos_l = [0; 4];
        self.comb_pos_r = [0; 4];
        self.comb_damp_l = [0.0; 4];
        self.comb_damp_r = [0.0; 4];
        self.ap_pos_l = [0; 2];
        self.ap_pos_r = [0; 2];
        self.idle_samples = 1024;
    }

    #[inline]
    fn comb_tick(
        buf: &mut [f32],
        pos: &mut usize,
        len: usize,
        input: f32,
        feedback: f32,
        damp: f32,
        filter_store: &mut f32,
    ) -> f32 {
        let cur = *pos % len;
        let out = buf[cur];
        *filter_store = flush_denormal(out * (1.0 - damp) + *filter_store * damp);
        buf[cur] = input + *filter_store * feedback;
        *pos = (cur + 1) % len;
        out
    }

    #[inline]
    fn ap_tick(buf: &mut [f32], pos: &mut usize, len: usize, input: f32) -> f32 {
        let cur = *pos % len;
        let buf_out = buf[cur];
        let y = -input + buf_out;
        buf[cur] = input + buf_out * 0.5;
        *pos = (cur + 1) % len;
        y
    }

    #[inline]
    fn process(&mut self, in_l: f32, in_r: f32, p: [f32; 4]) -> (f32, f32, f32) {
        if track_idle(&mut self.idle_samples, in_l, in_r) && !self.dirty {
            return (0.0, 0.0, 0.0);
        }

        let size = 0.45 + p[0].clamp(0.0, 1.0) * 0.90;
        let decay = 0.40 + p[1].clamp(0.0, 1.0) * 0.55;
        let damp = 0.08 + p[2].clamp(0.0, 1.0) * 0.72;
        let mix = p[3].clamp(0.0, 1.0);

        self.dirty = true;
        let mono_in = (in_l + in_r) * 0.06;
        let sr_scale = (self.sample_rate / 44_100.0).clamp(0.5, 2.2);

        let mut wet_l = 0.0_f32;
        let mut wet_r = 0.0_f32;
        for i in 0..4 {
            let len_l = ((COMB_BASE[i] as f32 * size * sr_scale) as usize)
                .clamp(64, REVERB_COMB_LEN - 1);
            let len_r = (((COMB_BASE[i] + STEREO_SPREAD) as f32 * size * sr_scale) as usize)
                .clamp(64, REVERB_COMB_LEN - 1);

            wet_l += Self::comb_tick(
                &mut self.comb_l[i],
                &mut self.comb_pos_l[i],
                len_l,
                mono_in,
                decay,
                damp,
                &mut self.comb_damp_l[i],
            );
            wet_r += Self::comb_tick(
                &mut self.comb_r[i],
                &mut self.comb_pos_r[i],
                len_r,
                mono_in,
                decay,
                damp,
                &mut self.comb_damp_r[i],
            );
        }

        for i in 0..2 {
            let ap_len_l = ((AP_BASE[i] as f32 * sr_scale) as usize).clamp(32, REVERB_AP_LEN - 1);
            let ap_len_r = (((AP_BASE[i] + STEREO_SPREAD) as f32 * sr_scale) as usize)
                .clamp(32, REVERB_AP_LEN - 1);
            wet_l = Self::ap_tick(&mut self.ap_l[i], &mut self.ap_pos_l[i], ap_len_l, wet_l);
            wet_r = Self::ap_tick(&mut self.ap_r[i], &mut self.ap_pos_r[i], ap_len_r, wet_r);
        }

        // Mild stereo cross-bleed & level calibration
        let stereo_l = (wet_l * 0.85 + wet_r * 0.15) * 1.4;
        let stereo_r = (wet_r * 0.85 + wet_l * 0.15) * 1.4;

        if self.idle_samples > (REVERB_COMB_LEN as u32 * 4)
            && stereo_l.abs() < 1e-8
            && stereo_r.abs() < 1e-8
        {
            self.reset();
            return (0.0, 0.0, 0.0);
        }

        let out_l = in_l + (stereo_l - in_l) * mix;
        let out_r = in_r + (stereo_r - in_r) * mix;
        let activity = (stereo_l.abs().max(stereo_r.abs()) * mix).clamp(0.0, 1.0);
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

/// Stereo multimode Filter FX (`LP 12`, `LP 24`, `HP 12`, `HP 24`, `BP`, `Notch`, `Peak`).
struct FxFilterStereo {
    core: FilterSvfCore,
    sample_rate: f32,
}

impl FxFilterStereo {
    const TYPE_LABELS: [&'static str; 7] =
        ["LP 12", "LP 24", "HP 12", "HP 24", "BP", "Notch", "Peak"];

    fn new(sample_rate: f32) -> Self {
        Self {
            core: FilterSvfCore::new(2_000.0),
            sample_rate: sample_rate.max(1_000.0),
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1_000.0);
    }

    fn reset(&mut self) {
        self.core.reset_states();
    }

    #[inline]
    fn type_index(p2: f32) -> usize {
        ((p2.clamp(0.0, 1.0) * 6.999) as usize).min(6)
    }

    #[inline]
    fn filter_params(p: [f32; 4]) -> FilterParams {
        let (mode, slope) = match Self::type_index(p[2]) {
            0 => (FilterMode::Lp, FilterSlope::Db12),
            1 => (FilterMode::Lp, FilterSlope::Db24),
            2 => (FilterMode::Hp, FilterSlope::Db12),
            3 => (FilterMode::Hp, FilterSlope::Db24),
            4 => (FilterMode::Bp, FilterSlope::Db12),
            5 => (FilterMode::Notch, FilterSlope::Db12),
            _ => (FilterMode::Peak, FilterSlope::Db12),
        };
        FilterParams {
            enabled: true,
            mode,
            slope,
            cutoff_hz: cutoff_norm_to_hz(p[0]),
            resonance: p[1].clamp(0.0, 1.0),
            drive: 0.0,
            keytrack: 0.0,
            aux: p[3].clamp(0.0, 1.0),
        }
    }

    #[inline]
    fn process(&mut self, in_l: f32, in_r: f32, p: [f32; 4]) -> (f32, f32, f32) {
        let fp = Self::filter_params(p);
        let (ol, or) = self.core.process_core_stereo(
            in_l,
            in_r,
            &fp,
            60.0,
            self.sample_rate,
            crate::oscillator::EngineMode::Hq,
        );
        let act = ol.abs().max(or.abs()).clamp(0.0, 1.0) * fp.aux;
        (flush_denormal(ol), flush_denormal(or), act)
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
    }

    #[inline]
    fn process_type(
        &mut self,
        fx_type: FxType,
        in_l: f32,
        in_r: f32,
        p: [f32; 4],
        tempo_bpm: f32,
    ) -> (f32, f32, f32) {
        match fx_type {
            FxType::Off => (in_l, in_r, 0.0),
            FxType::Ott => self.ott.process(in_l, in_r, p),
            FxType::Distortion => self.dist.process(in_l, in_r, p),
            FxType::Chorus => self.chorus.process(in_l, in_r, p),
            FxType::Delay => self.delay.process(in_l, in_r, p, tempo_bpm),
            FxType::Reverb => self.reverb.process(in_l, in_r, p),
            FxType::JSatClipper => self.clipper.process(in_l, in_r, p),
            FxType::Filter => self.filter.process(in_l, in_r, p),
            FxType::Phaser => self.phaser.process(in_l, in_r, p),
        }
    }
}

/// Master 8-slot serial FX Rack (`[[StereoFxStage; 4]; 8]` pre-allocated once on the heap).
pub struct FxRackState {
    stages: Box<[[StereoFxStage; MAX_FX_STACK]]>,
    last_slot_types: [Option<FxType>; NUM_FX_SLOTS],
    scope_accum: [(f32, f32, f32, usize); NUM_FX_SLOTS],
    scope_samples_per_col: usize,
}

impl FxRackState {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1_000.0);
        let mut stages_vec = Vec::with_capacity(NUM_FX_SLOTS);
        for _ in 0..NUM_FX_SLOTS {
            stages_vec.push(std::array::from_fn(|_| StereoFxStage::new(sr)));
        }
        Self {
            stages: stages_vec.into_boxed_slice(),
            last_slot_types: [None; NUM_FX_SLOTS],
            scope_accum: [(0.0, 0.0, 0.0, 0); NUM_FX_SLOTS],
            scope_samples_per_col: (sr / SCOPE_HZ).round().max(1.0) as usize,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(1_000.0);
        self.scope_samples_per_col = (sr / SCOPE_HZ).round().max(1.0) as usize;
        for slot in self.stages.iter_mut() {
            for stage in slot.iter_mut() {
                stage.set_sample_rate(sr);
            }
        }
    }

    pub fn reset(&mut self) {
        for slot in self.stages.iter_mut() {
            for stage in slot.iter_mut() {
                stage.reset();
            }
        }
        self.last_slot_types = [None; NUM_FX_SLOTS];
        self.scope_accum = [(0.0, 0.0, 0.0, 0); NUM_FX_SLOTS];
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
    pub fn format_filter_type(p2: f32) -> &'static str {
        FxFilterStereo::TYPE_LABELS[FxFilterStereo::type_index(p2)]
    }

    #[inline]
    pub fn filter_params_from_fx(p: [f32; 4]) -> FilterParams {
        FxFilterStereo::filter_params(p)
    }

    #[inline]
    pub fn process_sample(
        &mut self,
        in_l: f32,
        in_r: f32,
        slots: &[FxSlotParams; NUM_FX_SLOTS],
        mod_offsets: &[f32; NUM_MOD_DESTS],
        tempo_bpm: f32,
        telemetry: Option<&ModUiTelemetry>,
    ) -> (f32, f32) {
        let mut l = in_l;
        let mut r = in_r;

        for (slot_idx, slot) in slots.iter().enumerate() {
            match self.last_slot_types[slot_idx] {
                Some(prev) if prev != slot.fx_type => {
                    for stage in &mut self.stages[slot_idx] {
                        stage.reset();
                    }
                    self.last_slot_types[slot_idx] = Some(slot.fx_type);
                    if let Some(tel) = telemetry {
                        tel.fx_scopes[slot_idx].clear();
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
                for p in 0..4 {
                    let mod_val = mod_offsets[ModDestination::fx_param(slot_idx, p).index()];
                    eff[p] = (slot.params[p] + mod_val).clamp(0.0, 1.0);
                }

                let stack_count = (slot.stack as usize).clamp(1, MAX_FX_STACK);
                for stage_idx in 0..stack_count {
                    let (nl, nr, act) = self.stages[slot_idx][stage_idx].process_type(
                        slot.fx_type,
                        l,
                        r,
                        eff,
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
                    tel.fx_scopes[slot_idx].push(*acc_pre, *acc_post, *acc_act);
                    *acc_pre = 0.0;
                    *acc_post = 0.0;
                    *acc_act = 0.0;
                    *cnt = 0;
                }
            }
        }

        (flush_denormal(l), flush_denormal(r))
    }
}
