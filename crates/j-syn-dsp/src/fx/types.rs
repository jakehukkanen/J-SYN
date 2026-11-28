//! Parameter and topology types for the 3-bus 30-slot Master FX Rack.

use crate::filter::{FilterMode, FilterSlope};
use crate::oscillator::EngineMode;
use crate::waveshaper::DistortionMode;

use super::engine_pool::FxRackOrder;
use super::multiband_dist::{
    MB_BAND_LEVEL_DEFAULT_NORM, MB_XOVER_HIGH_DEFAULT_NORM, MB_XOVER_LOW_DEFAULT_NORM,
    MbDistBandParams,
};
use super::utility::UTILITY_WIDTH_DEFAULT_NORM;
use super::{NUM_FX_BUSES, NUM_FX_SLOTS};

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
    TubeResonator = 10,
    Utility = 11,
    Limiter = 12,
    MultibandDist = 13,
    Flanger = 14,
}

impl FxType {
    pub const ALL: [Self; 15] = [
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
        Self::TubeResonator,
        Self::Utility,
        Self::Limiter,
        Self::MultibandDist,
        Self::Flanger,
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
            10 => Self::TubeResonator,
            11 => Self::Utility,
            12 => Self::Limiter,
            13 => Self::MultibandDist,
            14 => Self::Flanger,
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
            Self::Diopser => "Dispersion",
            Self::TubeResonator => "Tube Res.",
            Self::Utility => "Utility",
            Self::Limiter => "Limiter",
            Self::MultibandDist => "MB Dist",
            Self::Flanger => "Flanger",
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
            Self::Diopser => "DISP",
            Self::TubeResonator => "TUBE",
            Self::Utility => "UTIL",
            Self::Limiter => "LIM",
            Self::MultibandDist => "MBDST",
            Self::Flanger => "FLANG",
        }
    }

    #[inline]
    pub const fn subtab_label(self) -> Option<&'static str> {
        match self {
            Self::Chorus | Self::Delay | Self::Reverb => Some("EQ"),
            Self::Flanger => Some("MOD"),
            Self::TubeResonator => Some("LFO"),
            Self::Limiter => Some("ADV"),
            _ => None,
        }
    }

    #[inline]
    pub const fn has_eq_subtab(self) -> bool {
        self.subtab_label().is_some()
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
    /// Enable the pre-resonator softening bandpass filter in `TUBE RES.` (default `true`).
    pub tube_res_filter_enabled: bool,
    /// Enable 32-sample stepped ("Dirty") Pitch LFO in `TUBE RES.` (default `true`).
    pub tube_res_dirty_lfo: bool,
    /// Invert left channel phase in `UTILITY` (default `false`).
    pub utility_inv_l: bool,
    /// Invert right channel phase in `UTILITY` (default `false`).
    pub utility_inv_r: bool,
    /// Enable 4x True Peak detection in `LIMITER` (default `false`).
    pub limiter_true_peak: bool,
    /// Enable secondary 1x non-oversampled PCM peak-catching clipper stage (default `true`).
    pub clipper_pcm: bool,
    /// Master Distortion waveshaping algorithm (default `DistortionMode::Tube`).
    pub distortion_mode: DistortionMode,
    /// Per-band parameters (`[Low, Mid, High]`) for `FxType::MultibandDist`.
    pub mb_bands: [MbDistBandParams; 3],
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
            tube_res_filter_enabled: true,
            tube_res_dirty_lfo: true,
            utility_inv_l: false,
            utility_inv_r: false,
            limiter_true_peak: false,
            clipper_pcm: true,
            distortion_mode: DistortionMode::Tube,
            mb_bands: [MbDistBandParams {
                mode: DistortionMode::Tube,
                enabled: true,
                solo: false,
                drive: 0.30,
                bias: 0.50,
                level: MB_BAND_LEVEL_DEFAULT_NORM,
                mix: 1.00,
            }; 3],
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
            // Distortion: Bias Neutral (0.50), Drive 30%, Tone Neutral (0.50), Mix 100%
            FxType::Distortion => [0.50, 0.30, 0.50, 1.00],
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
            // Tube Res.: Tune C1 (12/72), Flt Freq 67 Hz (0.2676), Flt Width 2.0 oct (1.5/8.5), Mix 100%
            FxType::TubeResonator => [12.0 / 72.0, 0.267_604, 1.5 / 8.5, 1.00],
            // Utility: Width 100% (0.40), Pan C (0.50), Gain 0.0 dB (0.50), Mix 100%
            FxType::Utility => [UTILITY_WIDTH_DEFAULT_NORM, 0.50, 0.50, 1.00],
            // Limiter: Gain 0.0 dB (0.0), Ceil -0.1 dB (17.9/18.0), Lookahead 1.5 ms (0.50), Mix 100%
            FxType::Limiter => [0.00, 17.9 / 18.0, 0.50, 1.00],
            // MultibandDist: Low Xov 200 Hz (0.50), High Xov ~2.5 kHz, Out Gain 0 dB (0.50), Mix 100%
            FxType::MultibandDist => [
                MB_XOVER_LOW_DEFAULT_NORM,
                MB_XOVER_HIGH_DEFAULT_NORM,
                0.50,
                1.00,
            ],
            // Flanger: Rate ~0.20 Hz (0.35), Depth 65%, Manual Delay ~0.95 ms (0.45), Mix 100%
            FxType::Flanger => [0.35, 0.65, 0.45, 1.00],
        }
    }

    /// Default normalized secondary `[P5, P6, P7, P8]` (`EQ` / `MOD` / `LFO` / `ADV`) knob positions for `fx_type`.
    pub const fn default_eq_params_for_type(fx_type: FxType) -> [f32; 4] {
        match fx_type {
            // Chorus EQ: Low Cut 20 Hz (0.0), High Cut 20 kHz (1.0), Spread 100% (0.5), Feedback 0% (0.0)
            FxType::Chorus => [0.0, 1.0, 0.50, 0.0],
            // Delay EQ: Low Cut 20 Hz (0.0), High Cut ~8.5 kHz (0.77), Ping-Pong 0% (0.0), Width 100% (0.5)
            FxType::Delay => [0.0, 0.77, 0.0, 0.50],
            // Reverb EQ: Low Cut ~60 Hz (0.24), High Cut 20 kHz (1.0), Pre-Dly 0 ms (0.0), Width 100% (0.5)
            FxType::Reverb => [0.24, 1.0, 0.0, 0.50],
            // Flanger MOD: Feedbk +50% (0.75), Stereo Phase 90° (0.50), Low Cut 20 Hz (0.0), Tone ~11.5 kHz (0.85)
            FxType::Flanger => [0.75, 0.50, 0.0, 0.85],
            // Tube Res. LFO: LFO Amt 50%, LFO Spd 1.00 Hz (0.42625), Feedbk/Decay 50 ms (0.29922), Mix shared on P4
            FxType::TubeResonator => [0.50, 0.426_25, 0.299_22, 1.00],
            // Limiter ADV: Release ~80 ms (0.53), Knee 0% (0.0), Link 100% (1.0), Punch 100% (1.0)
            FxType::Limiter => [0.53, 0.00, 1.00, 1.00],
            // Filter: Freq 2 ~4.1 kHz (0.72) for multi-filter modes
            FxType::Filter => [0.72, 1.0, 0.50, 0.50],
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
            tube_res_filter_enabled: true,
            tube_res_dirty_lfo: true,
            utility_inv_l: false,
            utility_inv_r: false,
            limiter_true_peak: false,
            clipper_pcm: true,
            distortion_mode: DistortionMode::Tube,
            mb_bands: [MbDistBandParams {
                mode: DistortionMode::Tube,
                enabled: true,
                solo: false,
                drive: 0.30,
                bias: 0.50,
                level: MB_BAND_LEVEL_DEFAULT_NORM,
                mix: 1.00,
            }; 3],
        }
    }
}

/// Parameters for one parallel 30-slot serial FX Bus (`Bus A`, `Bus B`, or `Bus C`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FxBusParams {
    pub slots: [FxSlotParams; NUM_FX_SLOTS],
    pub enabled: bool,
    pub muted: bool,
    pub level: f32,
    pub pan: f32,
}

impl Default for FxBusParams {
    fn default() -> Self {
        Self {
            slots: [FxSlotParams::default(); NUM_FX_SLOTS],
            enabled: true,
            muted: false,
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
            enabled: false,
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

/// How `Bus A`, `Bus B`, and `Bus C` connect inside the Master FX Rack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum FxBusTopology {
    /// Parallel: `Bus A`, `Bus B`, and `Bus C` process independently and sum into Master (`A | B | C`).
    Parallel = 0,
    /// Serial: `Bus A` output feeds directly into `Bus B`, while `Bus C` stays parallel (`A -> B | C`).
    #[default]
    Serial = 1,
    /// Cascade all 3 buses (`A -> B -> C -> Master`).
    SerialAll = 2,
    /// Bus A and Bus B both feed Bus C (`(A | B) -> C`).
    ABToC = 3,
    /// Bus A feeds Bus C while Bus B goes to Master (`A -> C | B`).
    AToC = 4,
    /// Bus B feeds Bus C while Bus A goes to Master (`A | B -> C`).
    BToC = 5,
}

impl FxBusTopology {
    /// Output target of Bus A: `0 = Master`, `1 = Bus B`, `2 = Bus C`.
    #[inline]
    pub const fn bus_a_target(self) -> u8 {
        match self {
            Self::Parallel | Self::BToC => 0,
            Self::Serial | Self::SerialAll => 1,
            Self::ABToC | Self::AToC => 2,
        }
    }

    /// Output target of Bus B: `0 = Master`, `2 = Bus C`.
    #[inline]
    pub const fn bus_b_target(self) -> u8 {
        match self {
            Self::Parallel | Self::Serial | Self::AToC => 0,
            Self::SerialAll | Self::ABToC | Self::BToC => 2,
        }
    }

    #[inline]
    pub const fn from_lane_targets(a_target: u8, b_target: u8) -> Self {
        match (a_target, b_target) {
            (0, 0) => Self::Parallel,
            (1, 0) => Self::Serial,
            (1, 2) => Self::SerialAll,
            (2, 2) => Self::ABToC,
            (2, 0) => Self::AToC,
            _ => Self::BToC,
        }
    }
}

/// Full FX Rack (`Bus A` + `Bus B` + `Bus C` + `Sub Bus` + `FxBusTopology` + `FxRackOrder` + `EngineMode`).
#[derive(Debug, Clone, Copy)]
pub struct FxRackParams {
    pub buses: [FxBusParams; NUM_FX_BUSES],
    pub sub: SubBusParams,
    pub topology: FxBusTopology,
    pub order: FxRackOrder,
    pub engine_mode: EngineMode,
}

impl Default for FxRackParams {
    fn default() -> Self {
        Self {
            buses: [FxBusParams::default(); NUM_FX_BUSES],
            sub: SubBusParams::default(),
            topology: FxBusTopology::default(),
            order: FxRackOrder::default(),
            engine_mode: EngineMode::Vintage2007,
        }
    }
}
