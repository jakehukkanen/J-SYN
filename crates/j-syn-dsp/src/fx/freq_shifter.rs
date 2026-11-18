//! Broadband IIR Hilbert-transform SSB Frequency Shifter (`FxType::FreqShifter` &
//! `InsertMode::FreqShifter`), with optional `LR4` band split (`Hi` / `Lo` / `Full`),
//! barberpole feedback, and wet EQ stage. Zero audio-thread heap allocations.

use std::f64::consts::{PI, TAU};

use crate::filter::fast_tanh;

use super::dynamics::Lr4BandFilter;
use super::modulation::EqStageStereo;
use super::{flush_denormal, track_idle};

const HILBERT_STAGES: usize = 7;
const HILBERT_EDGE_HZ: f64 = 10.0;
const MAX_SHIFT_HZ: f32 = 2_000.0;
const MAX_FINE_HZ: f32 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum FreqShiftMode {
    #[default]
    Up = 0,
    Down = 1,
    Dual = 2,
}

impl FreqShiftMode {
    #[inline]
    pub const fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::Down,
            2 => Self::Dual,
            _ => Self::Up,
        }
    }

    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Up => "UP",
            Self::Down => "DOWN",
            Self::Dual => "DUAL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum FreqSplitBand {
    Hi = 0,
    Lo = 1,
    #[default]
    Full = 2,
}

impl FreqSplitBand {
    #[inline]
    pub const fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::Hi,
            1 => Self::Lo,
            _ => Self::Full,
        }
    }

    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Hi => "HI",
            Self::Lo => "LO",
            Self::Full => "FULL",
        }
    }
}

/// Center-zeroed cubic coarse shift frequency mapping (`norm` `0..=1` -> `-2000..=+2000 Hz`).
#[inline]
pub fn freq_shift_coarse_hz(norm: f32) -> f32 {
    let u = (norm.clamp(0.0, 1.0) - 0.5) * 2.0;
    u.signum() * u.abs().powi(3) * MAX_SHIFT_HZ
}

/// Center-zeroed quadratic fine shift frequency mapping (`norm` `0..=1` -> `-20..=+20 Hz`).
#[inline]
pub fn freq_shift_fine_hz(norm: f32) -> f32 {
    let u = (norm.clamp(0.0, 1.0) - 0.5) * 2.0;
    u * u.abs() * MAX_FINE_HZ
}

/// Log-mapped split crossover frequency (`norm` `0..=1` -> `20..=20000 Hz`).
#[inline]
pub fn freq_shift_split_hz(norm: f32) -> f32 {
    20.0 * (1_000.0_f32).powf(norm.clamp(0.0, 1.0))
}

#[inline]
pub fn freq_shift_feedback(norm: f32) -> f32 {
    norm.clamp(0.0, 1.0) * 0.95
}

pub fn format_freq_shift_hz(hz: f32) -> String {
    let abs = hz.abs();
    if abs < 0.05 {
        "0.0 Hz".to_string()
    } else if abs >= 100.0 {
        format!("{:+.0} Hz", hz)
    } else if abs >= 10.0 {
        format!("{:+.1} Hz", hz)
    } else {
        format!("{:+.2} Hz", hz)
    }
}

/// Compute 7+7 Gazsi discrete halfband Hilbert `a_k^2` coefficients for any sample rate
/// (`>= 59 dB` image rejection at `44.1/48 kHz`, `> 48 dB` at `96 kHz`, `> 43 dB` at `192 kHz`
/// across `20 Hz .. 20 kHz`).
fn compute_hilbert_coeffs(sample_rate: f64) -> ([f64; HILBERT_STAGES], [f64; HILBERT_STAGES]) {
    let sr = sample_rate.max(8_000.0);
    let wt = (TAU * HILBERT_EDGE_HZ / sr).clamp(1e-5, PI * 0.25);
    let t = ((PI * 0.5 - wt) * 0.5).tan();
    let k = (t * t).clamp(1e-6, 0.999_999_99);
    let kp = (1.0 - k * k).sqrt().max(1e-12);

    const MAX_ITER: usize = 18;
    let mut a = [0.0_f64; MAX_ITER];
    let mut c = [0.0_f64; MAX_ITER];
    a[0] = 1.0;
    let mut b_val = kp;
    c[0] = k;
    let mut steps = 0usize;
    for i in 0..(MAX_ITER - 1) {
        let an = a[i];
        let bn = b_val;
        a[i + 1] = 0.5 * (an + bn);
        b_val = (an * bn).sqrt();
        c[i + 1] = 0.5 * (an - bn);
        steps = i + 1;
        if c[i + 1].abs() < 1e-15 {
            break;
        }
    }
    let big_k = PI / (2.0 * a[steps]);
    let n_total = HILBERT_STAGES * 2;
    let denom = (n_total + 1) as f64;
    let scale = (2.0_f64).powi(steps as i32) * a[steps] * big_k;

    let mut ca2 = [0.0_f64; HILBERT_STAGES];
    let mut cb2 = [0.0_f64; HILBERT_STAGES];
    let mut ia = 0usize;
    let mut ib = 0usize;
    for idx in 1..=n_total {
        let mut phi = scale * (idx as f64 / denom);
        for n in (1..=steps).rev() {
            let ratio = (c[n] / a[n]).clamp(-1.0, 1.0);
            phi = 0.5 * (phi + (ratio * phi.sin()).asin());
        }
        let s = phi.sin();
        let a2 = s * s;
        if idx % 2 == 0 {
            ca2[ia] = a2;
            ia += 1;
        } else {
            cb2[ib] = a2;
            ib += 1;
        }
    }
    (ca2, cb2)
}

/// 14-pole (`7+7` `z^-2` allpass + 1-sample `z^-1` offset) `f64`-state Hilbert pair for one mono channel.
/// Uses Direct-Form II (`w[n] = x + a2 * w[n-2]; y = a2 * w[n] - w[n-2]`) to halve delay state size.
#[derive(Debug, Clone, Copy)]
struct HilbertChannel {
    wa1: [f64; HILBERT_STAGES],
    wa2: [f64; HILBERT_STAGES],
    wb1: [f64; HILBERT_STAGES],
    wb2: [f64; HILBERT_STAGES],
    delay1: f64,
}

impl HilbertChannel {
    const fn new() -> Self {
        Self {
            wa1: [0.0; HILBERT_STAGES],
            wa2: [0.0; HILBERT_STAGES],
            wb1: [0.0; HILBERT_STAGES],
            wb2: [0.0; HILBERT_STAGES],
            delay1: 0.0,
        }
    }

    fn reset(&mut self) {
        self.wa1.fill(0.0);
        self.wa2.fill(0.0);
        self.wb1.fill(0.0);
        self.wb2.fill(0.0);
        self.delay1 = 0.0;
    }

    /// Returns `(i, q)` analytical pair where `i * cos(w_s t) + q * sin(w_s t)` shifts frequencies UP
    /// by `+w_s` and `i * cos(w_s t) - q * sin(w_s t)` shifts DOWN by `-w_s`.
    #[inline]
    fn process(
        &mut self,
        input: f32,
        ca2: &[f64; HILBERT_STAGES],
        cb2: &[f64; HILBERT_STAGES],
    ) -> (f32, f32) {
        let mut xa = input as f64;
        for k in 0..HILBERT_STAGES {
            let a2 = ca2[k];
            let w2 = self.wa2[k];
            let w = xa + a2 * w2;
            let y = a2 * w - w2;
            self.wa2[k] = self.wa1[k];
            self.wa1[k] = w;
            xa = y;
        }
        let i_val = self.delay1;
        self.delay1 = xa;

        let mut xb = input as f64;
        for k in 0..HILBERT_STAGES {
            let a2 = cb2[k];
            let w2 = self.wb2[k];
            let w = xb + a2 * w2;
            let y = a2 * w - w2;
            self.wb2[k] = self.wb1[k];
            self.wb1[k] = w;
            xb = y;
        }
        (flush_denormal(i_val as f32), flush_denormal(xb as f32))
    }
}

#[inline]
fn ssb_mix(i: f32, q: f32, phase_rad: f64, mode: FreqShiftMode) -> f32 {
    let (s, c) = phase_rad.sin_cos();
    let c32 = c as f32;
    let s32 = s as f32;
    match mode {
        FreqShiftMode::Up => i * c32 + q * s32,
        FreqShiftMode::Down => i * c32 - q * s32,
        FreqShiftMode::Dual => i * c32,
    }
}

/// Shared stereo Hilbert I/Q + quadrature oscillator pair (used by both Master FX and per-voice Insert FX).
#[derive(Debug, Clone, Copy)]
pub struct HilbertIqPair {
    ca2: [f64; HILBERT_STAGES],
    cb2: [f64; HILBERT_STAGES],
    channels: [HilbertChannel; 2],
    phase_l: f64,
    phase_r: f64,
    sample_rate: f32,
}

impl HilbertIqPair {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(8_000.0);
        let (ca2, cb2) = compute_hilbert_coeffs(sr as f64);
        Self {
            ca2,
            cb2,
            channels: [HilbertChannel::new(), HilbertChannel::new()],
            phase_l: 0.0,
            phase_r: 0.0,
            sample_rate: sr,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(8_000.0);
        if (self.sample_rate - sr).abs() > 0.5 {
            self.sample_rate = sr;
            let (ca2, cb2) = compute_hilbert_coeffs(sr as f64);
            self.ca2 = ca2;
            self.cb2 = cb2;
        }
    }

    pub fn reset(&mut self) {
        self.channels[0].reset();
        self.channels[1].reset();
        self.phase_l = 0.0;
        self.phase_r = 0.0;
    }

    #[inline]
    pub fn process_stereo(
        &mut self,
        in_l: f32,
        in_r: f32,
        shift_l_hz: f32,
        shift_r_hz: f32,
        mode: FreqShiftMode,
    ) -> (f32, f32) {
        let sr = self.sample_rate as f64;
        let max_f = 0.45 * self.sample_rate;
        let inc_l = TAU * (shift_l_hz.clamp(-max_f, max_f) as f64) / sr;
        let inc_r = TAU * (shift_r_hz.clamp(-max_f, max_f) as f64) / sr;

        let (il, ql) = self.channels[0].process(in_l, &self.ca2, &self.cb2);
        let (ir, qr) = self.channels[1].process(in_r, &self.ca2, &self.cb2);

        let out_l = ssb_mix(il, ql, self.phase_l, mode);
        let out_r = ssb_mix(ir, qr, self.phase_r, mode);

        self.phase_l = (self.phase_l + inc_l).rem_euclid(TAU);
        self.phase_r = (self.phase_r + inc_r).rem_euclid(TAU);

        (flush_denormal(out_l), flush_denormal(out_r))
    }
}

/// Master FX Rack SSB Frequency Shifter (`FxType::FreqShifter`).
/// - `MAIN`: `SHIFT`, `SPLIT`, `FEEDBK`, `MIX`
/// - `EQ`: `FINE Hz`, `STEREO`, `LOW CUT`, `HIGH CUT`
pub(crate) struct FreqShifterStereo {
    hilbert: HilbertIqPair,
    xover: [Lr4BandFilter; 2],
    eq: EqStageStereo,
    fb_l: f32,
    fb_r: f32,
    last_split_hz: f32,
    last_sr: f32,
    sample_rate: f32,
    idle_samples: u32,
}

impl FreqShifterStereo {
    pub(crate) fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(8_000.0);
        Self {
            hilbert: HilbertIqPair::new(sr),
            xover: [Lr4BandFilter::default(); 2],
            eq: EqStageStereo::new(sr),
            fb_l: 0.0,
            fb_r: 0.0,
            last_split_hz: -1.0,
            last_sr: 0.0,
            sample_rate: sr,
            idle_samples: 0,
        }
    }

    pub(crate) fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(8_000.0);
        self.sample_rate = sr;
        self.hilbert.set_sample_rate(sr);
        self.eq.set_sample_rate(sr);
        self.last_sr = 0.0;
    }

    pub(crate) fn reset(&mut self) {
        self.hilbert.reset();
        self.xover[0].reset();
        self.xover[1].reset();
        self.eq.reset();
        self.fb_l = 0.0;
        self.fb_r = 0.0;
        self.idle_samples = 0;
    }

    #[inline]
    pub(crate) fn process(
        &mut self,
        in_l: f32,
        in_r: f32,
        p: [f32; 4],
        eq: [f32; 4],
        mode: FreqShiftMode,
        split_band: FreqSplitBand,
    ) -> (f32, f32, f32) {
        let mix = p[3].clamp(0.0, 1.0);
        if track_idle(&mut self.idle_samples, in_l, in_r)
            && self.fb_l.abs() < 1e-6
            && self.fb_r.abs() < 1e-6
            && self.idle_samples > (self.sample_rate * 0.5) as u32
        {
            return (in_l, in_r, 0.0);
        }

        let shift_hz = freq_shift_coarse_hz(p[0]) + freq_shift_fine_hz(eq[0]);
        let stereo_spread = eq[1].clamp(0.0, 1.0);
        let shift_r_hz = shift_hz * (1.0 - 2.0 * stereo_spread);
        let fb_amt = freq_shift_feedback(p[2]);

        let (band_l, pass_l, band_r, pass_r) = if split_band == FreqSplitBand::Full {
            (in_l, 0.0, in_r, 0.0)
        } else {
            let nyq_safe = self.sample_rate * 0.44;
            let split_hz = freq_shift_split_hz(p[1]).clamp(20.0, nyq_safe);
            if (split_hz - self.last_split_hz).abs() > 0.25
                || (self.sample_rate - self.last_sr).abs() > 0.5
            {
                let sr64 = self.sample_rate as f64;
                self.xover[0].set_freq(sr64, split_hz as f64);
                self.xover[1].set_freq(sr64, split_hz as f64);
                self.last_split_hz = split_hz;
                self.last_sr = self.sample_rate;
            }
            let (lo_l, hi_l) = self.xover[0].split(in_l);
            let (lo_r, hi_r) = self.xover[1].split(in_r);
            match split_band {
                FreqSplitBand::Hi => (hi_l, lo_l, hi_r, lo_r),
                FreqSplitBand::Lo => (lo_l, hi_l, lo_r, hi_r),
                FreqSplitBand::Full => (in_l, 0.0, in_r, 0.0),
            }
        };

        let drv_l = band_l + self.fb_l * fb_amt;
        let drv_r = band_r + self.fb_r * fb_amt;
        let (sh_l, sh_r) = self.hilbert.process_stereo(drv_l, drv_r, shift_hz, shift_r_hz, mode);
        let (eq_l, eq_r) = self.eq.process(sh_l, sh_r, eq[2], eq[3]);

        // Soft-limit feedback state so high feedback creates dense barberpole spirals without blowup.
        self.fb_l = flush_denormal(fast_tanh(eq_l * 0.85) * 1.15);
        self.fb_r = flush_denormal(fast_tanh(eq_r * 0.85) * 1.15);

        let wet_l = eq_l + pass_l;
        let wet_r = eq_r + pass_r;
        let out_l = flush_denormal(in_l + (wet_l - in_l) * mix);
        let out_r = flush_denormal(in_r + (wet_r - in_r) * mix);
        let diff_mag = ((wet_l - band_l).abs() + (wet_r - band_r).abs()).clamp(0.0, 2.0) * 0.5;
        (out_l, out_r, diff_mag)
    }
}
