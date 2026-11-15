//! Hybrid 16-step Performer / MSEG modulator (`MODSEQ 1` – `MODSEQ 4`) with curve tension,
//! shape presets, DAW tempo sync, loop/one-shot/ping-pong modes, and slew smoothing.

use std::f32::consts::PI;

use serde::{Deserialize, Serialize};

pub const MAX_MODSEQ_STEPS: usize = 16;

/// Single step segment in a `MODSEQ` curve (`0.0..=1.0` levels, `-1.0..=1.0` curve tension).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ModSeqStep {
    pub start_level: f32,
    pub end_level: f32,
    pub tension: f32,
}

impl Default for ModSeqStep {
    fn default() -> Self {
        Self {
            start_level: 0.5,
            end_level: 0.5,
            tension: 0.0,
        }
    }
}

/// Fixed-size persistence/ArcSwap container holding step curves for all 4 `MODSEQ` engines.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ModSeqStepBank {
    pub seqs: [[ModSeqStep; MAX_MODSEQ_STEPS]; 4],
}

impl Default for ModSeqStepBank {
    fn default() -> Self {
        let mut s0 = [ModSeqStep::default(); MAX_MODSEQ_STEPS];
        let mut s1 = [ModSeqStep::default(); MAX_MODSEQ_STEPS];
        let mut s2 = [ModSeqStep::default(); MAX_MODSEQ_STEPS];
        let mut s3 = [ModSeqStep::default(); MAX_MODSEQ_STEPS];

        // MODSEQ 1: Expressive 8-step rhythmic pluck/ramp
        ModSeqShapePreset::RampDown.apply_to_sequence(&mut s0, 8);
        s0[0] = ModSeqStep {
            start_level: 1.0,
            end_level: 0.15,
            tension: -0.4,
        };
        s0[2] = ModSeqStep {
            start_level: 0.75,
            end_level: 0.1,
            tension: -0.35,
        };
        s0[4] = ModSeqStep {
            start_level: 0.9,
            end_level: 0.25,
            tension: -0.3,
        };
        s0[6] = ModSeqStep {
            start_level: 0.65,
            end_level: 0.0,
            tension: -0.5,
        };

        // MODSEQ 2: Smooth triangle wave
        ModSeqShapePreset::Triangle.apply_to_sequence(&mut s1, 8);
        // MODSEQ 3: Smooth sine wave
        ModSeqShapePreset::Sine.apply_to_sequence(&mut s2, 8);
        // MODSEQ 4: Stepped sample & hold groove
        ModSeqShapePreset::SampleAndHold.apply_to_sequence(&mut s3, 8);

        Self {
            seqs: [s0, s1, s2, s3],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSeqShapePreset {
    RampUp,
    RampDown,
    Pulse,
    Triangle,
    Sine,
    SampleAndHold,
}

impl ModSeqShapePreset {
    pub const ALL: [Self; 6] = [
        Self::RampUp,
        Self::RampDown,
        Self::Pulse,
        Self::Triangle,
        Self::Sine,
        Self::SampleAndHold,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::RampUp => "RAMP +",
            Self::RampDown => "RAMP -",
            Self::Pulse => "PULSE",
            Self::Triangle => "TRI",
            Self::Sine => "SINE",
            Self::SampleAndHold => "S&H",
        }
    }

    pub fn apply_to_sequence(self, steps: &mut [ModSeqStep; MAX_MODSEQ_STEPS], num_steps: usize) {
        let n = num_steps.clamp(2, MAX_MODSEQ_STEPS);
        for (i, step) in steps.iter_mut().take(MAX_MODSEQ_STEPS).enumerate() {
            let idx = i % n;
            let t0 = idx as f32 / n as f32;
            let t1 = (idx + 1) as f32 / n as f32;
            *step = match self {
                Self::RampUp => ModSeqStep {
                    start_level: t0,
                    end_level: t1,
                    tension: 0.0,
                },
                Self::RampDown => ModSeqStep {
                    start_level: 1.0 - t0,
                    end_level: 1.0 - t1,
                    tension: 0.0,
                },
                Self::Pulse => {
                    let lvl = if idx < n / 2 { 1.0 } else { 0.0 };
                    ModSeqStep {
                        start_level: lvl,
                        end_level: lvl,
                        tension: 0.0,
                    }
                }
                Self::Triangle => {
                    let tri = |t: f32| {
                        if t < 0.5 {
                            t * 2.0
                        } else {
                            (1.0 - t) * 2.0
                        }
                    };
                    ModSeqStep {
                        start_level: tri(t0),
                        end_level: tri(t1),
                        tension: 0.0,
                    }
                }
                Self::Sine => {
                    let s = |t: f32| 0.5 - 0.5 * (2.0 * PI * t).cos();
                    ModSeqStep {
                        start_level: s(t0),
                        end_level: s(t1),
                        tension: 0.0,
                    }
                }
                Self::SampleAndHold => {
                    let v = ((idx.wrapping_mul(73) + 19) % 13) as f32 / 12.0;
                    ModSeqStep {
                        start_level: v,
                        end_level: v,
                        tension: 0.0,
                    }
                }
            };
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSeqSyncMode {
    TempoSync = 0,
    FreeHz = 1,
}

impl ModSeqSyncMode {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::FreeHz,
            _ => Self::TempoSync,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSeqRate {
    Div1_32 = 0,
    Div1_16 = 1,
    Div1_8 = 2,
    Div1_4 = 3,
    Div1_2 = 4,
    Bars1 = 5,
    Bars2 = 6,
    Bars4 = 7,
}

impl ModSeqRate {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::Div1_32,
            1 => Self::Div1_16,
            2 => Self::Div1_8,
            3 => Self::Div1_4,
            4 => Self::Div1_2,
            5 => Self::Bars1,
            6 => Self::Bars2,
            _ => Self::Bars4,
        }
    }

    #[inline]
    pub fn cycles_per_beat(self) -> f32 {
        match self {
            Self::Div1_32 => 8.0,
            Self::Div1_16 => 4.0,
            Self::Div1_8 => 2.0,
            Self::Div1_4 => 1.0,
            Self::Div1_2 => 0.5,
            Self::Bars1 => 0.25,
            Self::Bars2 => 0.125,
            Self::Bars4 => 0.0625,
        }
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
    pub sync_mode: ModSeqSyncMode,
    pub sync_rate: ModSeqRate,
    pub free_hz: f32,
    pub play_mode: ModSeqPlayMode,
    pub retrig: ModSeqRetrig,
    pub num_steps: usize,
    pub smooth: f32,
}

impl Default for ModSeqParams {
    fn default() -> Self {
        Self {
            sync_mode: ModSeqSyncMode::TempoSync,
            sync_rate: ModSeqRate::Bars1,
            free_hz: 1.5,
            play_mode: ModSeqPlayMode::Loop,
            retrig: ModSeqRetrig::PolyVoice,
            num_steps: 8,
            smooth: 0.05,
        }
    }
}

/// Evaluates the shaped step curve at normalized cycle phase `phase` in `[0.0, 1.0)`.
#[inline]
pub fn evaluate_modseq_curve(
    steps: &[ModSeqStep; MAX_MODSEQ_STEPS],
    num_steps: usize,
    phase: f32,
) -> f32 {
    let n = num_steps.clamp(2, MAX_MODSEQ_STEPS);
    let scaled = phase.clamp(0.0, 0.999_999) * (n as f32);
    let step_idx = (scaled.floor() as usize).min(n - 1);
    let local_t = (scaled - (step_idx as f32)).clamp(0.0, 1.0);
    let step = steps[step_idx];

    let exp = 2.0_f32.powf(step.tension.clamp(-1.0, 1.0) * 2.5);
    let curved_t = local_t.powf(exp);
    (step.start_level + (step.end_level - step.start_level) * curved_t).clamp(0.0, 1.0)
}

/// Per-voice (or global) runtime state for one `MODSEQ` unit. Zero allocations.
#[derive(Debug, Clone, Copy)]
pub struct ModSeqState {
    pub phase: f32,
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
            direction: 1.0,
            completed: false,
            smoothed_out: 0.0,
        }
    }

    #[inline]
    pub fn trigger(&mut self) {
        self.phase = 0.0;
        self.direction = 1.0;
        self.completed = false;
    }

    #[inline]
    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.direction = 1.0;
        self.completed = false;
        self.smoothed_out = 0.0;
    }

    /// Advances phase by `dt_seconds` and returns the slew-smoothed curve value in `[0.0, 1.0]`.
    #[inline]
    pub fn advance(
        &mut self,
        dt_seconds: f32,
        tempo_bpm: f32,
        params: &ModSeqParams,
        steps: &[ModSeqStep; MAX_MODSEQ_STEPS],
    ) -> f32 {
        let freq_hz = match params.sync_mode {
            ModSeqSyncMode::TempoSync => {
                (tempo_bpm.clamp(20.0, 300.0) / 60.0) * params.sync_rate.cycles_per_beat()
            }
            ModSeqSyncMode::FreeHz => params.free_hz.clamp(0.05, 30.0),
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

        let raw = evaluate_modseq_curve(steps, params.num_steps, self.phase);
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
}
