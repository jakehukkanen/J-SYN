//! Hybrid 32-step (2×16) Performer / MSEG modulator (`MODSEQ 1` – `MODSEQ 8`) with curve tension,
//! half-speed curves, DAW tempo sync, loop/one-shot/ping-pong modes, and slew smoothing.

mod mseg;
mod steps;

pub use mseg::*;
pub use steps::*;

use serde::{Deserialize, Serialize};

pub const MAX_MODSEQ_STEPS: usize = 32;
pub const MODSEQ_ROW_STEPS: usize = 16;
pub const NUM_ENVS: usize = 8;
pub const NUM_MOD_ENVS: usize = NUM_ENVS - 1;
pub const NUM_MODSEQS: usize = 8;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ModSeqMode {
    #[default]
    Performer = 0,
    Mseg = 1,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ModSeqRowStart {
    #[default]
    Row1 = 0,
    Row2 = 1,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ModSeqRowMode {
    #[default]
    Continue = 0,
    LoopRow = 1,
}

/// Fixed-size persistence/ArcSwap container holding Performer steps and MSEG curves for all 8 `MODSEQ` engines.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ModSeqStepBank {
    pub seqs: [[ModSeqStep; MAX_MODSEQ_STEPS]; NUM_MODSEQS],
    pub msegs: [MsegCurve; NUM_MODSEQS],
}

#[derive(Deserialize)]
#[serde(transparent)]
struct ModSeqRowWire(
    #[serde(deserialize_with = "crate::serde_lenient::fixed_array")] [ModSeqStep; MAX_MODSEQ_STEPS],
);

impl Default for ModSeqRowWire {
    fn default() -> Self {
        Self([ModSeqStep::default(); MAX_MODSEQ_STEPS])
    }
}

#[derive(Deserialize)]
#[serde(default)]
struct ModSeqStepBankWire {
    #[serde(deserialize_with = "crate::serde_lenient::fixed_array")]
    seqs: [ModSeqRowWire; NUM_MODSEQS],
    #[serde(deserialize_with = "crate::serde_lenient::fixed_array")]
    msegs: [MsegCurve; NUM_MODSEQS],
}

impl Default for ModSeqStepBankWire {
    fn default() -> Self {
        let d = ModSeqStepBank::default();
        Self {
            seqs: std::array::from_fn(|i| ModSeqRowWire(d.seqs[i])),
            msegs: d.msegs,
        }
    }
}

impl<'de> Deserialize<'de> for ModSeqStepBank {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Some(w) = Option::<ModSeqStepBankWire>::deserialize(deserializer)? else {
            return Ok(Self::default());
        };
        Ok(Self {
            seqs: std::array::from_fn(|i| w.seqs[i].0),
            msegs: w.msegs,
        })
    }
}

impl Default for ModSeqStepBank {
    fn default() -> Self {
        let mut s0 = [ModSeqStep::default(); MAX_MODSEQ_STEPS];
        let mut s1 = [ModSeqStep::default(); MAX_MODSEQ_STEPS];
        let mut s2 = [ModSeqStep::default(); MAX_MODSEQ_STEPS];
        let mut s3 = [ModSeqStep::default(); MAX_MODSEQ_STEPS];

        // MODSEQ 1: Expressive 8-step rhythmic pluck/ramp (zero swing tension by default)
        ModSeqShapePreset::RampDown.apply_to_sequence(&mut s0, 8);
        s0[0] = ModSeqStep {
            start_level: 1.0,
            end_level: 0.15,
            tension: 0.0,
            shape: ModSeqStepShape::Linear,
            span: ModSeqStepSpan::Full,
        };
        s0[2] = ModSeqStep {
            start_level: 0.75,
            end_level: 0.1,
            tension: 0.0,
            shape: ModSeqStepShape::Linear,
            span: ModSeqStepSpan::Full,
        };
        s0[4] = ModSeqStep {
            start_level: 0.9,
            end_level: 0.25,
            tension: 0.0,
            shape: ModSeqStepShape::Linear,
            span: ModSeqStepSpan::Full,
        };
        s0[6] = ModSeqStep {
            start_level: 0.65,
            end_level: 0.0,
            tension: 0.0,
            shape: ModSeqStepShape::Linear,
            span: ModSeqStepSpan::Full,
        };

        // MODSEQ 2: Smooth triangle wave
        ModSeqShapePreset::Triangle.apply_to_sequence(&mut s1, 8);
        // MODSEQ 3: Smooth sine wave
        ModSeqShapePreset::Sine.apply_to_sequence(&mut s2, 8);
        // MODSEQ 4: Stepped sample & hold groove
        ModSeqShapePreset::SampleAndHold.apply_to_sequence(&mut s3, 8);

        let mut s4 = [ModSeqStep::default(); MAX_MODSEQ_STEPS];
        let mut s5 = [ModSeqStep::default(); MAX_MODSEQ_STEPS];
        let mut s6 = [ModSeqStep::default(); MAX_MODSEQ_STEPS];
        let mut s7 = [ModSeqStep::default(); MAX_MODSEQ_STEPS];
        ModSeqShapePreset::RampUp.apply_to_sequence(&mut s4, 8);
        ModSeqShapePreset::Pulse.apply_to_sequence(&mut s5, 8);
        ModSeqShapePreset::RampDown.apply_to_sequence(&mut s6, 8);
        ModSeqShapePreset::Triangle.apply_to_sequence(&mut s7, 8);

        Self {
            seqs: [s0, s1, s2, s3, s4, s5, s6, s7],
            msegs: std::array::from_fn(|_| MsegCurve::default()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSeqSyncMode {
    Free = 0,
    Straight = 1,
    Triplet = 2,
    Dotted = 3,
    StepRatio = 4,
    StepMs = 5,
}

impl ModSeqSyncMode {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::Free,
            2 => Self::Triplet,
            3 => Self::Dotted,
            4 => Self::StepRatio,
            5 => Self::StepMs,
            _ => Self::Straight,
        }
    }
}

pub const MODSEQ_STRAIGHT_DIVS: [(&str, f32); 10] = [
    ("8 Bars", 1.0 / 32.0),
    ("4 Bars", 1.0 / 16.0),
    ("2 Bars", 1.0 / 8.0),
    ("1 Bar", 1.0 / 4.0),
    ("1/2", 0.5),
    ("1/4", 1.0),
    ("1/8", 2.0),
    ("1/16", 4.0),
    ("1/32", 8.0),
    ("1/64", 16.0),
];

pub const MODSEQ_TRIPLET_DIVS: [(&str, f32); 5] = [
    ("1/2T", 0.75),
    ("1/4T", 1.5),
    ("1/8T", 3.0),
    ("1/16T", 6.0),
    ("1/32T", 12.0),
];

pub const MODSEQ_DOTTED_DIVS: [(&str, f32); 5] = [
    ("1/2D", 1.0 / 3.0),
    ("1/4D", 2.0 / 3.0),
    ("1/8D", 4.0 / 3.0),
    ("1/16D", 8.0 / 3.0),
    ("1/32D", 16.0 / 3.0),
];

#[inline]
fn sync_divs(sync_mode: ModSeqSyncMode) -> &'static [(&'static str, f32)] {
    match sync_mode {
        ModSeqSyncMode::Free | ModSeqSyncMode::StepRatio | ModSeqSyncMode::StepMs => &[],
        ModSeqSyncMode::Straight => &MODSEQ_STRAIGHT_DIVS,
        ModSeqSyncMode::Triplet => &MODSEQ_TRIPLET_DIVS,
        ModSeqSyncMode::Dotted => &MODSEQ_DOTTED_DIVS,
    }
}

/// Maps normalized `0.0..=1.0` knob position to logarithmic `1.0..=2000.0 ms` per step.
#[inline]
pub fn modseq_step_ms_from_norm(norm: f32) -> f32 {
    (2000.0_f32.powf(norm.clamp(0.0, 1.0))).clamp(1.0, 2000.0)
}

/// Computes step frequency in Hz for a tempo-synced `num / den` bar ratio per step.
#[inline]
pub fn modseq_step_ratio_hz(num: u8, den: u8, tempo_bpm: f32) -> f32 {
    let n = num.clamp(1, 16) as f32;
    let d = den.clamp(1, 64) as f32;
    let bpm = tempo_bpm.clamp(20.0, 300.0);
    ((bpm / 240.0) * (d / n)).clamp(0.01, 1000.0)
}

/// Maps normalized `0.0..=1.0` rate knob position to continuous `0.05..=50.0 Hz` in `Free` mode.
#[inline]
pub fn modseq_free_norm_to_hz(norm: f32) -> f32 {
    (0.05 * 1000.0_f32.powf(norm.clamp(0.0, 1.0))).clamp(0.05, 50.0)
}

/// Maps normalized `0.0..=1.0` rate knob position to the snapped division label for sync modes.
#[inline]
pub fn modseq_division_label(sync_mode: ModSeqSyncMode, rate_norm: f32) -> &'static str {
    let divs = sync_divs(sync_mode);
    if divs.is_empty() {
        return "Free";
    }
    let idx = ((rate_norm.clamp(0.0, 1.0) * (divs.len() as f32 - 0.001)).floor() as usize)
        .min(divs.len() - 1);
    divs[idx].0
}

/// Computes effective modulator frequency in Hz for any `ModSeqSyncMode` and normalized rate.
#[inline]
pub fn modseq_rate_hz(sync_mode: ModSeqSyncMode, rate_norm: f32, tempo_bpm: f32) -> f32 {
    let divs = sync_divs(sync_mode);
    if divs.is_empty() {
        modseq_free_norm_to_hz(rate_norm)
    } else {
        let idx = ((rate_norm.clamp(0.0, 1.0) * (divs.len() as f32 - 0.001)).floor() as usize)
            .min(divs.len() - 1);
        (tempo_bpm.clamp(20.0, 300.0) / 60.0) * divs[idx].1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSeqPlayMode {
    Loop = 0,
    OneShot = 1,
    PingPong = 2,
}

impl ModSeqPlayMode {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::OneShot,
            2 => Self::PingPong,
            _ => Self::Loop,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSeqRetrig {
    PolyVoice = 0,
    MonoGlobal = 1,
}

impl ModSeqRetrig {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::MonoGlobal,
            _ => Self::PolyVoice,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ModSeqParams {
    pub mode: ModSeqMode,
    pub sync_mode: ModSeqSyncMode,
    /// Unified normalized `0.0..=1.0` rate knob position (continuous `0.05..=50.0 Hz` in `Free`,
    /// snapped across musical divisions in `Straight` / `Triplet` / `Dotted`).
    pub rate_norm: f32,
    pub play_mode: ModSeqPlayMode,
    pub retrig: ModSeqRetrig,
    pub num_steps: usize,
    pub smooth: f32,
    pub rate_mod: f32,
    pub groove_swing: bool,
    pub start_row: ModSeqRowStart,
    pub row_mode: ModSeqRowMode,
    pub row_mod: f32,
    pub step_ratio_num: u8,
    pub step_ratio_den: u8,
}

impl Default for ModSeqParams {
    fn default() -> Self {
        Self {
            mode: ModSeqMode::Performer,
            sync_mode: ModSeqSyncMode::Straight,
            rate_norm: 0.35, // 1 Bar in Straight mode
            play_mode: ModSeqPlayMode::Loop,
            retrig: ModSeqRetrig::PolyVoice,
            num_steps: 8,
            smooth: 0.05,
            rate_mod: 0.0,
            groove_swing: false,
            start_row: ModSeqRowStart::Row1,
            row_mode: ModSeqRowMode::Continue,
            row_mod: 0.0,
            step_ratio_num: 1,
            step_ratio_den: 16,
        }
    }
}

/// Resolves `(actual_step_0_to_31, local_t)` for a cycle `phase` in `[0.0, 1.0)`.
#[inline]
pub fn resolve_modseq_step(
    num_steps: usize,
    start_row: ModSeqRowStart,
    row_mode: ModSeqRowMode,
    row_mod: f32,
    phase: f32,
) -> (usize, f32) {
    let n = num_steps.clamp(2, MAX_MODSEQ_STEPS);
    let base_row = if start_row == ModSeqRowStart::Row2 {
        1.0
    } else {
        0.0
    };
    let eff_row_mod = if row_mod.is_finite() { row_mod } else { 0.0 };
    let row2 = (base_row + eff_row_mod) >= 0.5;
    let active_len = match row_mode {
        ModSeqRowMode::LoopRow => n.min(MODSEQ_ROW_STEPS),
        ModSeqRowMode::Continue => n,
    };
    let scaled = phase.clamp(0.0, 0.999_999) * (active_len as f32);
    let step_pos = (scaled.floor() as usize).min(active_len - 1);
    let local_t = (scaled - (step_pos as f32)).clamp(0.0, 1.0);
    let actual_step = match row_mode {
        ModSeqRowMode::LoopRow => {
            if row2 {
                MODSEQ_ROW_STEPS + step_pos
            } else {
                step_pos
            }
        }
        ModSeqRowMode::Continue => {
            if n <= MODSEQ_ROW_STEPS {
                if row2 {
                    MODSEQ_ROW_STEPS + step_pos
                } else {
                    step_pos
                }
            } else if !row2 {
                step_pos
            } else {
                let r2_len = n - MODSEQ_ROW_STEPS;
                if step_pos < r2_len {
                    MODSEQ_ROW_STEPS + step_pos
                } else {
                    step_pos - r2_len
                }
            }
        }
    };
    (actual_step.min(MAX_MODSEQ_STEPS - 1), local_t)
}

/// Experimental alternating step-phase swing tension pattern (`[-0.40, 0.0, -0.35, 0.0, ...]`).
const GROOVE_SWING_TENSIONS: [f32; 8] = [-0.40, 0.0, -0.35, 0.0, -0.30, 0.0, -0.50, 0.0];

/// Applies the experimental groove swing tension offset for `step_idx` when `groove_swing` is enabled.
#[inline]
pub fn apply_groove_swing_tension(step_idx: usize, base_tension: f32, groove_swing: bool) -> f32 {
    if groove_swing {
        let swing_t = GROOVE_SWING_TENSIONS[step_idx % GROOVE_SWING_TENSIONS.len()];
        (base_tension + swing_t).clamp(-1.0, 1.0)
    } else {
        base_tension
    }
}

/// Evaluates the shaped Performer step curve with optional experimental alternating groove swing.
#[inline]
pub fn evaluate_modseq_curve_with_swing(
    steps: &[ModSeqStep; MAX_MODSEQ_STEPS],
    num_steps: usize,
    phase: f32,
    groove_swing: bool,
) -> f32 {
    let (step_idx, local_t) = resolve_modseq_step(
        num_steps,
        ModSeqRowStart::Row1,
        ModSeqRowMode::Continue,
        0.0,
        phase,
    );
    let mut step = steps[step_idx];
    step.tension = apply_groove_swing_tension(step_idx, step.tension, groove_swing);
    evaluate_step_shape(step, local_t)
}

/// Evaluates the shaped Performer step curve at normalized cycle phase `phase` in `[0.0, 1.0)`.
#[inline]
pub fn evaluate_modseq_curve(
    steps: &[ModSeqStep; MAX_MODSEQ_STEPS],
    num_steps: usize,
    phase: f32,
) -> f32 {
    evaluate_modseq_curve_with_swing(steps, num_steps, phase, false)
}

/// Per-voice (or global) runtime state for one `MODSEQ` unit. Zero allocations.
#[derive(Debug, Clone, Copy)]
pub struct ModSeqState {
    pub phase: f32,
    pub display_phase: f32,
    pub direction: f32,
    pub completed: bool,
    pub smoothed_out: f32,
}

impl Default for ModSeqState {
    fn default() -> Self {
        Self::new()
    }
}

impl ModSeqState {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            display_phase: 0.0,
            direction: 1.0,
            completed: false,
            smoothed_out: 0.0,
        }
    }

    #[inline]
    pub fn trigger(&mut self) {
        self.phase = 0.0;
        self.display_phase = 0.0;
        self.direction = 1.0;
        self.completed = false;
    }

    #[inline]
    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.display_phase = 0.0;
        self.direction = 1.0;
        self.completed = false;
        self.smoothed_out = 0.0;
    }

    /// Advances phase by `dt_seconds` and returns the slew-smoothed Performer or MSEG value in `[0.0, 1.0]`.
    #[inline]
    pub fn advance_with_mseg(
        &mut self,
        dt_seconds: f32,
        tempo_bpm: f32,
        params: &ModSeqParams,
        steps: &[ModSeqStep; MAX_MODSEQ_STEPS],
        mseg: &MsegCurve,
    ) -> f32 {
        let eff_norm = (params.rate_norm + params.rate_mod).clamp(0.0, 1.0);
        let n = params.num_steps.clamp(2, MAX_MODSEQ_STEPS);
        let freq_hz = match params.mode {
            ModSeqMode::Performer => {
                let active_len = match params.row_mode {
                    ModSeqRowMode::LoopRow => n.min(MODSEQ_ROW_STEPS),
                    ModSeqRowMode::Continue => n,
                };
                match params.sync_mode {
                    ModSeqSyncMode::StepRatio => {
                        let step_hz = modseq_step_ratio_hz(
                            params.step_ratio_num,
                            params.step_ratio_den,
                            tempo_bpm,
                        );
                        (step_hz / active_len as f32).clamp(0.001, 100.0)
                    }
                    ModSeqSyncMode::StepMs => {
                        let step_ms = modseq_step_ms_from_norm(eff_norm);
                        ((1000.0 / step_ms) / active_len as f32).clamp(0.001, 100.0)
                    }
                    _ => {
                        let cycle_hz =
                            modseq_rate_hz(params.sync_mode, eff_norm, tempo_bpm).clamp(0.01, 100.0);
                        cycle_hz * (n as f32 / active_len as f32)
                    }
                }
            }
            ModSeqMode::Mseg => match params.sync_mode {
                ModSeqSyncMode::StepRatio => modseq_step_ratio_hz(
                    params.step_ratio_num,
                    params.step_ratio_den,
                    tempo_bpm,
                )
                .clamp(0.01, 100.0),
                ModSeqSyncMode::StepMs => {
                    let step_ms = modseq_step_ms_from_norm(eff_norm);
                    (1000.0 / (step_ms * MODSEQ_ROW_STEPS as f32)).clamp(0.01, 100.0)
                }
                _ => modseq_rate_hz(params.sync_mode, eff_norm, tempo_bpm).clamp(0.01, 100.0),
            },
        };
        let d_phase = freq_hz * dt_seconds.max(0.0);

        match params.play_mode {
            ModSeqPlayMode::Loop => {
                self.completed = false;
                self.direction = 1.0;
                self.phase = (self.phase + d_phase).rem_euclid(1.0);
            }
            ModSeqPlayMode::OneShot => {
                if !self.completed {
                    self.phase += d_phase;
                    if self.phase >= 1.0 {
                        self.phase = 0.999_999;
                        self.completed = true;
                    }
                }
            }
            ModSeqPlayMode::PingPong => {
                self.completed = false;
                self.phase += d_phase * self.direction;
                if self.phase >= 1.0 {
                    self.phase = (2.0 - self.phase).clamp(0.0, 0.999_999);
                    self.direction = -1.0;
                } else if self.phase <= 0.0 {
                    self.phase = (-self.phase).clamp(0.0, 0.999_999);
                    self.direction = 1.0;
                }
            }
        }

        let raw = match params.mode {
            ModSeqMode::Performer => {
                let (step_idx, local_t) = resolve_modseq_step(
                    n,
                    params.start_row,
                    params.row_mode,
                    params.row_mod,
                    self.phase,
                );
                self.display_phase = (step_idx as f32 + local_t) / (MAX_MODSEQ_STEPS as f32);
                let mut step = steps[step_idx];
                step.tension =
                    apply_groove_swing_tension(step_idx, step.tension, params.groove_swing);
                evaluate_step_shape(step, local_t)
            }
            ModSeqMode::Mseg => {
                self.display_phase = self.phase;
                evaluate_mseg_curve(mseg, self.phase)
            }
        };
        let smooth = params.smooth.clamp(0.0, 1.0);
        if smooth <= 1e-4 {
            self.smoothed_out = raw;
        } else {
            let tau_s = 0.0005 + smooth * smooth * 0.060;
            let coeff = (1.0 - (-dt_seconds.max(1e-6) / tau_s).exp()).clamp(0.0, 1.0);
            self.smoothed_out += (raw - self.smoothed_out) * coeff;
        }
        self.smoothed_out
    }

    #[inline]
    pub fn advance(
        &mut self,
        dt_seconds: f32,
        tempo_bpm: f32,
        params: &ModSeqParams,
        steps: &[ModSeqStep; MAX_MODSEQ_STEPS],
    ) -> f32 {
        self.advance_with_mseg(dt_seconds, tempo_bpm, params, steps, &MsegCurve::default())
    }
}
