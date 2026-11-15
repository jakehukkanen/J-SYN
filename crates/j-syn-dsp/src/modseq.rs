//! Hybrid 16-step Performer / MSEG modulator (`MODSEQ 1` – `MODSEQ 4`) with curve tension,
//! shape presets, DAW tempo sync, loop/one-shot/ping-pong modes, and slew smoothing.

use std::f32::consts::PI;

use serde::{Deserialize, Serialize};

pub const MAX_MODSEQ_STEPS: usize = 16;
pub const MAX_MSEG_NODES: usize = 16;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ModSeqMode {
    #[default]
    Performer = 0,
    Mseg = 1,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ModSeqStepShape {
    #[default]
    Linear = 0,
    Sine = 1,
    RampDown = 2,
    RampUp = 3,
    Triangle = 4,
    Pulse = 5,
    Exp = 6,
    Flat = 7,
}

impl ModSeqStepShape {
    pub const STAMPS: [Self; 7] = [
        Self::Sine,
        Self::RampDown,
        Self::RampUp,
        Self::Triangle,
        Self::Pulse,
        Self::Exp,
        Self::Flat,
    ];

    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Sine,
            2 => Self::RampDown,
            3 => Self::RampUp,
            4 => Self::Triangle,
            5 => Self::Pulse,
            6 => Self::Exp,
            7 => Self::Flat,
            _ => Self::Linear,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Linear => "LIN",
            Self::Sine => "SINE",
            Self::RampDown => "RAMP -",
            Self::RampUp => "RAMP +",
            Self::Triangle => "TRI",
            Self::Pulse => "PULSE",
            Self::Exp => "EXP",
            Self::Flat => "FLAT",
        }
    }
}

/// Single step segment in a `MODSEQ` Performer curve (`0.0..=1.0` levels, `-1.0..=1.0` tension).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ModSeqStep {
    pub start_level: f32,
    pub end_level: f32,
    pub tension: f32,
    #[serde(default)]
    pub shape: ModSeqStepShape,
}

impl Default for ModSeqStep {
    fn default() -> Self {
        Self {
            start_level: 0.5,
            end_level: 0.5,
            tension: 0.0,
            shape: ModSeqStepShape::Linear,
        }
    }
}

impl ModSeqStep {
    #[inline]
    pub fn stamped(shape: ModSeqStepShape, depth: f32, tension: f32) -> Self {
        let d = depth.clamp(0.0, 1.0);
        let end = match shape {
            ModSeqStepShape::Flat | ModSeqStepShape::RampUp | ModSeqStepShape::Linear => d,
            _ => 0.0,
        };
        Self {
            start_level: d,
            end_level: end,
            tension: tension.clamp(-1.0, 1.0),
            shape,
        }
    }
}

/// Movable breakpoint node in a continuous `MSEG` envelope (`time_x`, `level_y`, `tension`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MsegNode {
    pub time_x: f32,
    pub level_y: f32,
    pub tension: f32,
}

impl Default for MsegNode {
    fn default() -> Self {
        Self {
            time_x: 0.0,
            level_y: 0.0,
            tension: 0.0,
        }
    }
}

/// Fixed-size zero-allocation multi-segment envelope (`2..=16` nodes).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MsegCurve {
    pub nodes: [MsegNode; MAX_MSEG_NODES],
    pub num_nodes: usize,
}

impl Default for MsegCurve {
    fn default() -> Self {
        let mut nodes = [MsegNode::default(); MAX_MSEG_NODES];
        nodes[0] = MsegNode {
            time_x: 0.0,
            level_y: 0.0,
            tension: -0.35,
        };
        nodes[1] = MsegNode {
            time_x: 0.15,
            level_y: 1.0,
            tension: 0.45,
        };
        nodes[2] = MsegNode {
            time_x: 0.55,
            level_y: 0.35,
            tension: 0.25,
        };
        nodes[3] = MsegNode {
            time_x: 1.0,
            level_y: 0.0,
            tension: 0.0,
        };
        Self {
            nodes,
            num_nodes: 4,
        }
    }
}

#[inline]
fn default_msegs() -> [MsegCurve; 4] {
    [
        MsegCurve::default(),
        MsegCurve::default(),
        MsegCurve::default(),
        MsegCurve::default(),
    ]
}

/// Fixed-size persistence/ArcSwap container holding Performer steps and MSEG curves for all 4 `MODSEQ` engines.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ModSeqStepBank {
    pub seqs: [[ModSeqStep; MAX_MODSEQ_STEPS]; 4],
    #[serde(default = "default_msegs")]
    pub msegs: [MsegCurve; 4],
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
            shape: ModSeqStepShape::Linear,
        };
        s0[2] = ModSeqStep {
            start_level: 0.75,
            end_level: 0.1,
            tension: -0.35,
            shape: ModSeqStepShape::Linear,
        };
        s0[4] = ModSeqStep {
            start_level: 0.9,
            end_level: 0.25,
            tension: -0.3,
            shape: ModSeqStepShape::Linear,
        };
        s0[6] = ModSeqStep {
            start_level: 0.65,
            end_level: 0.0,
            tension: -0.5,
            shape: ModSeqStepShape::Linear,
        };

        // MODSEQ 2: Smooth triangle wave
        ModSeqShapePreset::Triangle.apply_to_sequence(&mut s1, 8);
        // MODSEQ 3: Smooth sine wave
        ModSeqShapePreset::Sine.apply_to_sequence(&mut s2, 8);
        // MODSEQ 4: Stepped sample & hold groove
        ModSeqShapePreset::SampleAndHold.apply_to_sequence(&mut s3, 8);

        Self {
            seqs: [s0, s1, s2, s3],
            msegs: default_msegs(),
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
                    shape: ModSeqStepShape::Linear,
                },
                Self::RampDown => ModSeqStep {
                    start_level: 1.0 - t0,
                    end_level: 1.0 - t1,
                    tension: 0.0,
                    shape: ModSeqStepShape::Linear,
                },
                Self::Pulse => {
                    let lvl = if idx < n / 2 { 1.0 } else { 0.0 };
                    ModSeqStep {
                        start_level: lvl,
                        end_level: lvl,
                        tension: 0.0,
                        shape: ModSeqStepShape::Linear,
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
                        shape: ModSeqStepShape::Linear,
                    }
                }
                Self::Sine => {
                    let s = |t: f32| 0.5 - 0.5 * (2.0 * PI * t).cos();
                    ModSeqStep {
                        start_level: s(t0),
                        end_level: s(t1),
                        tension: 0.0,
                        shape: ModSeqStepShape::Linear,
                    }
                }
                Self::SampleAndHold => {
                    let v = ((idx.wrapping_mul(73) + 19) % 13) as f32 / 12.0;
                    ModSeqStep {
                        start_level: v,
                        end_level: v,
                        tension: 0.0,
                        shape: ModSeqStepShape::Linear,
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
    pub mode: ModSeqMode,
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
            mode: ModSeqMode::Performer,
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

/// Evaluates the shaped Performer step curve at normalized cycle phase `phase` in `[0.0, 1.0)`.
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
    match step.shape {
        ModSeqStepShape::Linear => {
            (step.start_level + (step.end_level - step.start_level) * curved_t).clamp(0.0, 1.0)
        }
        ModSeqStepShape::Sine => {
            (step.start_level * (0.5 - 0.5 * (2.0 * PI * curved_t).cos())).clamp(0.0, 1.0)
        }
        ModSeqStepShape::RampDown => (step.start_level * (1.0 - curved_t)).clamp(0.0, 1.0),
        ModSeqStepShape::RampUp => (step.start_level * curved_t).clamp(0.0, 1.0),
        ModSeqStepShape::Triangle => {
            let tri = if curved_t < 0.5 {
                curved_t * 2.0
            } else {
                (1.0 - curved_t) * 2.0
            };
            (step.start_level * tri).clamp(0.0, 1.0)
        }
        ModSeqStepShape::Pulse => {
            if curved_t < 0.5 {
                step.start_level.clamp(0.0, 1.0)
            } else {
                0.0
            }
        }
        ModSeqStepShape::Exp => {
            (step.start_level * (1.0 - local_t).powf(2.6 * exp)).clamp(0.0, 1.0)
        }
        ModSeqStepShape::Flat => step.start_level.clamp(0.0, 1.0),
    }
}

/// Evaluates a continuous multi-segment `MSEG` curve at normalized phase in `[0.0, 1.0]`.
#[inline]
pub fn evaluate_mseg_curve(mseg: &MsegCurve, phase: f32) -> f32 {
    let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
    let p = phase.clamp(0.0, 1.0);
    if p <= mseg.nodes[0].time_x {
        return mseg.nodes[0].level_y.clamp(0.0, 1.0);
    }
    for i in 0..(n - 1) {
        let a = mseg.nodes[i];
        let b = mseg.nodes[i + 1];
        if p <= b.time_x || i + 2 == n {
            let span = (b.time_x - a.time_x).max(1e-5);
            let local_t = ((p - a.time_x) / span).clamp(0.0, 1.0);
            let exp = 2.0_f32.powf(a.tension.clamp(-1.0, 1.0) * 2.5);
            let curved_t = local_t.powf(exp);
            return (a.level_y + (b.level_y - a.level_y) * curved_t).clamp(0.0, 1.0);
        }
    }
    mseg.nodes[n - 1].level_y.clamp(0.0, 1.0)
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

        let raw = match params.mode {
            ModSeqMode::Performer => evaluate_modseq_curve(steps, params.num_steps, self.phase),
            ModSeqMode::Mseg => evaluate_mseg_curve(mseg, self.phase),
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
