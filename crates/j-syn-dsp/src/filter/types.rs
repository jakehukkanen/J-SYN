//! Filter mode, slope, parameter definitions, and cutoff/saturation helpers.

pub const COMB_BUF_LEN: usize = 2048;

/// Skew factor matching `nih_plug::prelude::FloatRange::skew_factor(-2.2)` (`2.0_f32.powf(-2.2)`).
pub const CUTOFF_SKEW_FACTOR: f32 = 0.217_637_64;

/// Maps a filter cutoff frequency (`20.0..=20_000.0 Hz`) to normalized `0.0..=1.0` space.
#[inline]
pub fn cutoff_hz_to_norm(hz: f32) -> f32 {
    let t = ((hz.clamp(20.0, 20_000.0) - 20.0) / (20_000.0 - 20.0)).clamp(0.0, 1.0);
    t.powf(CUTOFF_SKEW_FACTOR)
}

/// Maps normalized `0.0..=1.0` cutoff position back to `20.0..=20_000.0 Hz`.
#[inline]
pub fn cutoff_norm_to_hz(norm: f32) -> f32 {
    let t = norm.clamp(0.0, 1.0).powf(1.0 / CUTOFF_SKEW_FACTOR);
    (20.0 + (20_000.0 - 20.0) * t).clamp(20.0, 20_000.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    Lp = 0,
    Bp = 1,
    Hp = 2,
    Scream = 3,
    Daft = 4,
    Comb = 5,
    HpPeak = 6,
    HpNotch = 7,
    LpNotch = 8,
    LpPeak = 9,
    Notch = 10,
    Peak = 11,
    Bandreject = 12,
    PeakPeak = 13,
    NotchNotch = 14,
}

impl FilterMode {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::Bp,
            2 => Self::Hp,
            3 => Self::Scream,
            4 => Self::Daft,
            5 => Self::Comb,
            6 => Self::HpPeak,
            7 => Self::HpNotch,
            8 => Self::LpNotch,
            9 => Self::LpPeak,
            10 => Self::Notch,
            11 => Self::Peak,
            12 => Self::Bandreject,
            13 => Self::PeakPeak,
            14 => Self::NotchNotch,
            _ => Self::Lp,
        }
    }

    #[inline]
    pub fn is_multi(self) -> bool {
        matches!(
            self,
            Self::HpPeak
                | Self::HpNotch
                | Self::LpNotch
                | Self::LpPeak
                | Self::PeakPeak
                | Self::NotchNotch
        )
    }

    /// True for fixed 2-pole / 12 dB filter topologies whose Slope control is disabled.
    #[inline]
    pub fn has_fixed_12db_slope(self) -> bool {
        matches!(
            self,
            Self::Scream | Self::Daft | Self::Comb | Self::Bandreject
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterSlope {
    Db6 = 0,
    Db12 = 1,
    Db24 = 2,
    Db36 = 3,
    Db48 = 4,
}

impl FilterSlope {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::Db6,
            2 => Self::Db24,
            3 => Self::Db36,
            4 => Self::Db48,
            _ => Self::Db12,
        }
    }

    #[inline]
    pub fn num_2pole_stages(self) -> usize {
        match self {
            Self::Db6 => 0,
            Self::Db12 => 1,
            Self::Db24 => 2,
            Self::Db36 => 3,
            Self::Db48 => 4,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FilterParams {
    pub enabled: bool,
    pub mode: FilterMode,
    pub slope: FilterSlope,
    pub cutoff_hz: f32,
    pub resonance: f32,
    pub drive: f32,
    pub keytrack: f32,
    /// Context-sensitive 4th parameter (`0.0..1.0`):
    /// - `Scream`: feedback scream amount
    /// - Multi-filter (`HP+Peak`, `HP+Notch`, `LP+Notch`, `LP+Peak`, `Peak+Peak`, `Notch+Notch`): secondary `FREQ 2` (`20 Hz`..`20 kHz`)
    /// - `Comb`: high-frequency feedback damping
    /// - Other single modes: wet/dry `MIX`
    pub aux: f32,
}

impl FilterParams {
    pub fn default_filter_1() -> Self {
        Self {
            enabled: true,
            mode: FilterMode::Lp,
            slope: FilterSlope::Db12,
            cutoff_hz: 20_000.0,
            resonance: 0.0,
            drive: 0.0,
            keytrack: 0.0,
            aux: 1.0,
        }
    }

    pub fn default_filter_2() -> Self {
        Self {
            enabled: false,
            mode: FilterMode::Lp,
            slope: FilterSlope::Db12,
            cutoff_hz: 2_000.0,
            resonance: 0.0,
            drive: 0.0,
            keytrack: 0.0,
            aux: 1.0,
        }
    }
}

impl Default for FilterParams {
    fn default() -> Self {
        Self::default_filter_1()
    }
}

#[inline]
pub fn aux_to_freq2_hz(aux: f32) -> f32 {
    20.0 * 1000.0_f32.powf(aux.clamp(0.0, 1.0))
}

/// Fast rational Pade approximation of `tanh(x)` suitable for real-time audio saturation.
#[inline]
pub fn fast_tanh(x: f32) -> f32 {
    let x = x.clamp(-4.5, 4.5);
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

#[inline]
pub(super) fn soft_limit_state(x: f32) -> f32 {
    if x.abs() <= 1.25 {
        x
    } else {
        fast_tanh(x * 0.25) * 4.0
    }
}
