//! Performer step shapes, 4×9 stamp palette (`1×`, `2×`/`3×`, `½×` 2-step & half pieces),
//! presets, and step-shape evaluation.

use std::f32::consts::PI;

use serde::{Deserialize, Serialize};

use super::{MAX_MODSEQ_STEPS, MODSEQ_ROW_STEPS};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ModSeqStepSpan {
    #[default]
    Full = 0,
    HalfFirst = 1,
    HalfSecond = 2,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ModSeqStepShape {
    #[default]
    Linear = 0,
    Flat = 1,
    RampUp = 2,
    RampDown = 3,
    ExpUp = 4,
    ExpDown = 5,
    LogUp = 6,
    LogDown = 7,
    SineArchUp = 8,
    SineArchDown = 9,
    Triangle = 10,
    Pulse = 11,
    SCurve = 12,
    RampDown2x = 13,
    TriSine2x = 14,
    Square2x = 15,
    RampDown3x = 16,
    TriSine3x = 17,
    Square3x = 18,
}

impl ModSeqStepShape {
    /// Ordered 2x9 base shape list (Row 1: 9 single curves, Row 2: 3 single + 6 subdivided 2x/3x bursts).
    pub const PALETTE: [Self; 18] = [
        Self::Flat,
        Self::RampUp,
        Self::RampDown,
        Self::ExpUp,
        Self::ExpDown,
        Self::LogUp,
        Self::LogDown,
        Self::SineArchUp,
        Self::SineArchDown,
        Self::Triangle,
        Self::Pulse,
        Self::SCurve,
        Self::RampDown2x,
        Self::TriSine2x,
        Self::Square2x,
        Self::RampDown3x,
        Self::TriSine3x,
        Self::Square3x,
    ];

    pub const STAMPS: [Self; 18] = Self::PALETTE;

    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Flat,
            2 => Self::RampUp,
            3 => Self::RampDown,
            4 => Self::ExpUp,
            5 => Self::ExpDown,
            6 => Self::LogUp,
            7 => Self::LogDown,
            8 => Self::SineArchUp,
            9 => Self::SineArchDown,
            10 => Self::Triangle,
            11 => Self::Pulse,
            12 => Self::SCurve,
            13 => Self::RampDown2x,
            14 => Self::TriSine2x,
            15 => Self::Square2x,
            16 => Self::RampDown3x,
            17 => Self::TriSine3x,
            18 => Self::Square3x,
            _ => Self::Linear,
        }
    }

    #[inline]
    pub fn subdivision(self) -> u8 {
        match self {
            Self::RampDown2x | Self::TriSine2x | Self::Square2x => 2,
            Self::RampDown3x | Self::TriSine3x | Self::Square3x => 3,
            _ => 1,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Linear => "LIN",
            Self::Flat => "FLAT",
            Self::RampUp => "RAMP UP",
            Self::RampDown => "RAMP DN",
            Self::ExpUp => "EXP UP",
            Self::ExpDown => "EXP DN",
            Self::LogUp => "LOG UP",
            Self::LogDown => "LOG DN",
            Self::SineArchUp => "ARCH UP",
            Self::SineArchDown => "ARCH DN",
            Self::Triangle => "TRI",
            Self::Pulse => "SQUARE",
            Self::SCurve => "S-CURVE",
            Self::RampDown2x => "2x RAMP",
            Self::TriSine2x => "2x SINE",
            Self::Square2x => "2x SQR",
            Self::RampDown3x => "3x RAMP",
            Self::TriSine3x => "3x SINE",
            Self::Square3x => "3x SQR",
        }
    }
}

/// Single palette tile in the 4×9 `MODSEQ` stamp grid (supports 1×, 2×/3×, auto-2-step ½×, and single-half pieces).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModSeqPaletteTile {
    pub shape: ModSeqStepShape,
    pub span: ModSeqStepSpan,
    pub two_step: bool,
}

impl ModSeqPaletteTile {
    pub const fn full(shape: ModSeqStepShape) -> Self {
        Self {
            shape,
            span: ModSeqStepSpan::Full,
            two_step: false,
        }
    }

    pub const fn half_2step(shape: ModSeqStepShape) -> Self {
        Self {
            shape,
            span: ModSeqStepSpan::HalfFirst,
            two_step: true,
        }
    }

    pub const fn half_piece(shape: ModSeqStepShape, span: ModSeqStepSpan) -> Self {
        Self {
            shape,
            span,
            two_step: false,
        }
    }
}

/// Ordered 4×9 palette tiles (36 total):
/// - Row 1: 9 primary 1× shapes
/// - Row 2: 3 single 1× + 6 subdivided 2×/3× bursts
/// - Row 3: 9 auto-2-step ½× (50%-speed) curves
/// - Row 4: 8 single-step half-curve pieces (½A / ½B) + ½× S-Curve auto-2-step
pub const MODSEQ_PALETTE_TILES: [ModSeqPaletteTile; 36] = [
    // Row 1
    ModSeqPaletteTile::full(ModSeqStepShape::Flat),
    ModSeqPaletteTile::full(ModSeqStepShape::RampUp),
    ModSeqPaletteTile::full(ModSeqStepShape::RampDown),
    ModSeqPaletteTile::full(ModSeqStepShape::ExpUp),
    ModSeqPaletteTile::full(ModSeqStepShape::ExpDown),
    ModSeqPaletteTile::full(ModSeqStepShape::LogUp),
    ModSeqPaletteTile::full(ModSeqStepShape::LogDown),
    ModSeqPaletteTile::full(ModSeqStepShape::SineArchUp),
    ModSeqPaletteTile::full(ModSeqStepShape::SineArchDown),
    // Row 2
    ModSeqPaletteTile::full(ModSeqStepShape::Triangle),
    ModSeqPaletteTile::full(ModSeqStepShape::Pulse),
    ModSeqPaletteTile::full(ModSeqStepShape::SCurve),
    ModSeqPaletteTile::full(ModSeqStepShape::RampDown2x),
    ModSeqPaletteTile::full(ModSeqStepShape::TriSine2x),
    ModSeqPaletteTile::full(ModSeqStepShape::Square2x),
    ModSeqPaletteTile::full(ModSeqStepShape::RampDown3x),
    ModSeqPaletteTile::full(ModSeqStepShape::TriSine3x),
    ModSeqPaletteTile::full(ModSeqStepShape::Square3x),
    // Row 3 (auto-2-step ½×)
    ModSeqPaletteTile::half_2step(ModSeqStepShape::RampUp),
    ModSeqPaletteTile::half_2step(ModSeqStepShape::RampDown),
    ModSeqPaletteTile::half_2step(ModSeqStepShape::ExpUp),
    ModSeqPaletteTile::half_2step(ModSeqStepShape::ExpDown),
    ModSeqPaletteTile::half_2step(ModSeqStepShape::LogUp),
    ModSeqPaletteTile::half_2step(ModSeqStepShape::LogDown),
    ModSeqPaletteTile::half_2step(ModSeqStepShape::SineArchUp),
    ModSeqPaletteTile::half_2step(ModSeqStepShape::SineArchDown),
    ModSeqPaletteTile::half_2step(ModSeqStepShape::Triangle),
    // Row 4 (single-step half pieces + ½× S-Curve)
    ModSeqPaletteTile::half_piece(ModSeqStepShape::RampUp, ModSeqStepSpan::HalfFirst),
    ModSeqPaletteTile::half_piece(ModSeqStepShape::RampUp, ModSeqStepSpan::HalfSecond),
    ModSeqPaletteTile::half_piece(ModSeqStepShape::RampDown, ModSeqStepSpan::HalfFirst),
    ModSeqPaletteTile::half_piece(ModSeqStepShape::RampDown, ModSeqStepSpan::HalfSecond),
    ModSeqPaletteTile::half_piece(ModSeqStepShape::SineArchUp, ModSeqStepSpan::HalfFirst),
    ModSeqPaletteTile::half_piece(ModSeqStepShape::SineArchUp, ModSeqStepSpan::HalfSecond),
    ModSeqPaletteTile::half_piece(ModSeqStepShape::SCurve, ModSeqStepSpan::HalfFirst),
    ModSeqPaletteTile::half_piece(ModSeqStepShape::SCurve, ModSeqStepSpan::HalfSecond),
    ModSeqPaletteTile::half_2step(ModSeqStepShape::SCurve),
];

/// Single step segment in a `MODSEQ` Performer curve (`0.0..=1.0` levels, `-1.0..=1.0` tension).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ModSeqStep {
    pub start_level: f32,
    pub end_level: f32,
    pub tension: f32,
    pub shape: ModSeqStepShape,
    #[serde(default)]
    pub span: ModSeqStepSpan,
}

#[derive(Deserialize)]
#[serde(default)]
struct ModSeqStepWire {
    start_level: f32,
    end_level: f32,
    tension: f32,
    #[serde(deserialize_with = "crate::serde_lenient::known_variant")]
    shape: Option<ModSeqStepShape>,
    #[serde(deserialize_with = "crate::serde_lenient::known_variant")]
    span: Option<ModSeqStepSpan>,
}

impl Default for ModSeqStepWire {
    fn default() -> Self {
        let d = ModSeqStep::default();
        Self {
            start_level: d.start_level,
            end_level: d.end_level,
            tension: d.tension,
            shape: Some(d.shape),
            span: Some(d.span),
        }
    }
}

impl<'de> Deserialize<'de> for ModSeqStep {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Some(w) = Option::<ModSeqStepWire>::deserialize(deserializer)? else {
            return Ok(Self::default());
        };
        Ok(Self {
            start_level: crate::serde_lenient::finite_clamped(w.start_level, 0.5, 0.0, 1.0),
            end_level: crate::serde_lenient::finite_clamped(w.end_level, 0.5, 0.0, 1.0),
            tension: crate::serde_lenient::finite_clamped(w.tension, 0.0, -1.0, 1.0),
            shape: w.shape.unwrap_or(ModSeqStepShape::Linear),
            span: w.span.unwrap_or(ModSeqStepSpan::Full),
        })
    }
}

impl Default for ModSeqStep {
    fn default() -> Self {
        Self {
            start_level: 0.5,
            end_level: 0.5,
            tension: 0.0,
            shape: ModSeqStepShape::Linear,
            span: ModSeqStepSpan::Full,
        }
    }
}

impl ModSeqStep {
    #[inline]
    pub fn stamped_with_span(
        shape: ModSeqStepShape,
        depth: f32,
        tension: f32,
        span: ModSeqStepSpan,
    ) -> Self {
        let d = depth.clamp(0.0, 1.0);
        let end = match shape {
            ModSeqStepShape::Flat
            | ModSeqStepShape::RampUp
            | ModSeqStepShape::ExpUp
            | ModSeqStepShape::LogUp
            | ModSeqStepShape::SineArchDown
            | ModSeqStepShape::SCurve
            | ModSeqStepShape::Linear => d,
            _ => 0.0,
        };
        Self {
            start_level: d,
            end_level: end,
            tension: tension.clamp(-1.0, 1.0),
            shape,
            span,
        }
    }

    #[inline]
    pub fn stamped(shape: ModSeqStepShape, depth: f32, tension: f32) -> Self {
        Self::stamped_with_span(shape, depth, tension, ModSeqStepSpan::Full)
    }
}

/// Fills one 16-step row (`0` or `1`) of a `MODSEQ` sequence with `tile`.
#[inline]
pub fn fill_modseq_row_with_tile(
    seq: &mut [ModSeqStep; MAX_MODSEQ_STEPS],
    row: usize,
    tile: ModSeqPaletteTile,
) {
    let row_start = row.min(1) * MODSEQ_ROW_STEPS;
    for col in 0..MODSEQ_ROW_STEPS {
        let span = if tile.two_step {
            if col % 2 == 0 {
                ModSeqStepSpan::HalfFirst
            } else {
                ModSeqStepSpan::HalfSecond
            }
        } else {
            tile.span
        };
        seq[row_start + col] = ModSeqStep::stamped_with_span(tile.shape, 1.0, 0.0, span);
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
                    span: ModSeqStepSpan::Full,
                },
                Self::RampDown => ModSeqStep {
                    start_level: 1.0 - t0,
                    end_level: 1.0 - t1,
                    tension: 0.0,
                    shape: ModSeqStepShape::Linear,
                    span: ModSeqStepSpan::Full,
                },
                Self::Pulse => {
                    let lvl = if idx < n / 2 { 1.0 } else { 0.0 };
                    ModSeqStep {
                        start_level: lvl,
                        end_level: lvl,
                        tension: 0.0,
                        shape: ModSeqStepShape::Linear,
                        span: ModSeqStepSpan::Full,
                    }
                }
                Self::Triangle => {
                    let tri = |t: f32| {
                        if t < 0.5 { t * 2.0 } else { (1.0 - t) * 2.0 }
                    };
                    ModSeqStep {
                        start_level: tri(t0),
                        end_level: tri(t1),
                        tension: 0.0,
                        shape: ModSeqStepShape::Linear,
                        span: ModSeqStepSpan::Full,
                    }
                }
                Self::Sine => {
                    let s = |t: f32| 0.5 - 0.5 * (2.0 * PI * t).cos();
                    ModSeqStep {
                        start_level: s(t0),
                        end_level: s(t1),
                        tension: 0.0,
                        shape: ModSeqStepShape::Linear,
                        span: ModSeqStepSpan::Full,
                    }
                }
                Self::SampleAndHold => {
                    let v = ((idx.wrapping_mul(73) + 19) % 13) as f32 / 12.0;
                    ModSeqStep {
                        start_level: v,
                        end_level: v,
                        tension: 0.0,
                        shape: ModSeqStepShape::Linear,
                        span: ModSeqStepSpan::Full,
                    }
                }
            };
        }
    }
}

/// Evaluates a single `ModSeqStep` at normalized step-local phase `local_t` in `[0.0, 1.0]`. Zero allocations.
#[inline]
pub fn evaluate_step_shape(step: ModSeqStep, local_t: f32) -> f32 {
    let raw_t = local_t.clamp(0.0, 1.0);
    let t = match step.span {
        ModSeqStepSpan::Full => raw_t,
        ModSeqStepSpan::HalfFirst => raw_t * 0.5,
        ModSeqStepSpan::HalfSecond => 0.5 + raw_t * 0.5,
    };
    let d = step.start_level.clamp(0.0, 1.0);
    let exp = 2.0_f32.powf(step.tension.clamp(-1.0, 1.0) * 2.2);
    let ct = t.powf(exp);

    match step.shape {
        ModSeqStepShape::Linear => {
            (step.start_level + (step.end_level - step.start_level) * ct).clamp(0.0, 1.0)
        }
        ModSeqStepShape::Flat => d,
        ModSeqStepShape::RampUp => (d * ct).clamp(0.0, 1.0),
        ModSeqStepShape::RampDown => (d * (1.0 - ct)).clamp(0.0, 1.0),
        ModSeqStepShape::ExpUp => (d * t.powf(2.6 * exp)).clamp(0.0, 1.0),
        ModSeqStepShape::ExpDown => (d * (1.0 - t).powf(2.6 * exp)).clamp(0.0, 1.0),
        ModSeqStepShape::LogUp => {
            let p = (2.6 / exp).clamp(0.15, 16.0);
            (d * (1.0 - (1.0 - t).powf(p))).clamp(0.0, 1.0)
        }
        ModSeqStepShape::LogDown => {
            let p = (2.6 / exp).clamp(0.15, 16.0);
            (d * (1.0 - t.powf(p))).clamp(0.0, 1.0)
        }
        ModSeqStepShape::SineArchUp => (d * (PI * ct).sin().clamp(0.0, 1.0)).clamp(0.0, 1.0),
        ModSeqStepShape::SineArchDown => {
            (d * (1.0 - (PI * ct).sin().clamp(0.0, 1.0))).clamp(0.0, 1.0)
        }
        ModSeqStepShape::Triangle => {
            let tri = if ct < 0.5 { ct * 2.0 } else { (1.0 - ct) * 2.0 };
            (d * tri).clamp(0.0, 1.0)
        }
        ModSeqStepShape::Pulse => {
            if ct < 0.5 {
                d
            } else {
                0.0
            }
        }
        ModSeqStepShape::SCurve => (d * (0.5 - 0.5 * (PI * ct).cos())).clamp(0.0, 1.0),
        ModSeqStepShape::RampDown2x => {
            let sub_ct = (t.min(0.999_999) * 2.0).fract().powf(exp);
            (d * (1.0 - sub_ct)).clamp(0.0, 1.0)
        }
        ModSeqStepShape::TriSine2x => {
            let sub_ct = (t.min(0.999_999) * 2.0).fract().powf(exp);
            (d * (0.5 - 0.5 * (2.0 * PI * sub_ct).cos())).clamp(0.0, 1.0)
        }
        ModSeqStepShape::Square2x => {
            let sub_ct = (t.min(0.999_999) * 2.0).fract().powf(exp);
            if sub_ct < 0.5 { d } else { 0.0 }
        }
        ModSeqStepShape::RampDown3x => {
            let sub_ct = (t.min(0.999_999) * 3.0).fract().powf(exp);
            (d * (1.0 - sub_ct)).clamp(0.0, 1.0)
        }
        ModSeqStepShape::TriSine3x => {
            let sub_ct = (t.min(0.999_999) * 3.0).fract().powf(exp);
            (d * (0.5 - 0.5 * (2.0 * PI * sub_ct).cos())).clamp(0.0, 1.0)
        }
        ModSeqStepShape::Square3x => {
            let sub_ct = (t.min(0.999_999) * 3.0).fract().powf(exp);
            if sub_ct < 0.5 { d } else { 0.0 }
        }
    }
}
