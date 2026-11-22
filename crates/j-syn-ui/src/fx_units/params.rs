//! Shared `nih_plug::Enum` parameter types, DSP conversions, and dropdown item tables
//! consumed by both `J-SYN` and standalone FX plugins (`crates/plugins/*`).

use j_syn_dsp::{DistortionMode, FilterMode, FxFilterSlope};
use nih_plug::prelude::Enum;

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistortionModeParam {
    #[name = "Tube"]
    Tube,
    #[name = "Mech"]
    Mech,
    #[name = "Sine"]
    Sine,
    #[name = "Crush"]
    Crush,
    #[name = "Classic A"]
    ClassicA,
    #[name = "Classic B"]
    ClassicB,
    #[name = "Classic C"]
    ClassicC,
    #[name = "Diode A"]
    DiodeA,
    #[name = "Diode B"]
    DiodeB,
    #[name = "Linear Fold"]
    LinearFold,
    #[name = "Sine Fold"]
    SineFold,
    #[name = "Hard Clip"]
    HardClip,
    #[name = "Half Rect"]
    HalfRectify,
    #[name = "Full Rect"]
    FullRectify,
}

impl From<DistortionModeParam> for DistortionMode {
    fn from(v: DistortionModeParam) -> Self {
        match v {
            DistortionModeParam::Tube => Self::Tube,
            DistortionModeParam::Mech => Self::Mech,
            DistortionModeParam::Sine => Self::Sine,
            DistortionModeParam::Crush => Self::Crush,
            DistortionModeParam::ClassicA => Self::ClassicA,
            DistortionModeParam::ClassicB => Self::ClassicB,
            DistortionModeParam::ClassicC => Self::ClassicC,
            DistortionModeParam::DiodeA => Self::DiodeA,
            DistortionModeParam::DiodeB => Self::DiodeB,
            DistortionModeParam::LinearFold => Self::LinearFold,
            DistortionModeParam::SineFold => Self::SineFold,
            DistortionModeParam::HardClip => Self::HardClip,
            DistortionModeParam::HalfRectify => Self::HalfRectify,
            DistortionModeParam::FullRectify => Self::FullRectify,
        }
    }
}

pub const MB_DIST_MODE_ITEMS: &[(&str, DistortionModeParam)] = &[
    ("Tube", DistortionModeParam::Tube),
    ("Mech", DistortionModeParam::Mech),
    ("Sine", DistortionModeParam::Sine),
    ("Crush", DistortionModeParam::Crush),
    ("Classic A", DistortionModeParam::ClassicA),
    ("Classic B", DistortionModeParam::ClassicB),
    ("Classic C", DistortionModeParam::ClassicC),
    ("Diode A", DistortionModeParam::DiodeA),
    ("Diode B", DistortionModeParam::DiodeB),
    ("Linear Fold", DistortionModeParam::LinearFold),
    ("Sine Fold", DistortionModeParam::SineFold),
    ("Hard Clip", DistortionModeParam::HardClip),
    ("Half Rect", DistortionModeParam::HalfRectify),
    ("Full Rect", DistortionModeParam::FullRectify),
];

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiopserStagesParam {
    #[name = "8"]
    S8,
    #[name = "16"]
    S16,
    #[name = "32"]
    S32,
    #[name = "64"]
    S64,
    #[name = "128"]
    S128,
}

impl DiopserStagesParam {
    #[inline]
    pub const fn to_index(self) -> u8 {
        match self {
            Self::S8 => 0,
            Self::S16 => 1,
            Self::S32 => 2,
            Self::S64 => 3,
            Self::S128 => 4,
        }
    }
}

pub const DIOPSER_STAGE_ITEMS: &[(&str, DiopserStagesParam)] = &[
    ("8", DiopserStagesParam::S8),
    ("16", DiopserStagesParam::S16),
    ("32", DiopserStagesParam::S32),
    ("64", DiopserStagesParam::S64),
    ("128", DiopserStagesParam::S128),
];

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterModeParam {
    #[name = "LP"]
    Lp,
    #[name = "BP"]
    Bp,
    #[name = "HP"]
    Hp,
    #[name = "Scream"]
    Scream,
    #[name = "Daft"]
    Daft,
    #[name = "Comb"]
    Comb,
    #[name = "HP + Peak"]
    HpPeak,
    #[name = "HP + Notch"]
    HpNotch,
    #[name = "LP + Notch"]
    LpNotch,
    #[name = "LP + Peak"]
    LpPeak,
    #[name = "Notch"]
    Notch,
    #[name = "Peak"]
    Peak,
    #[name = "Bandreject"]
    Bandreject,
    #[name = "Peak + Peak"]
    PeakPeak,
    #[name = "Notch + Notch"]
    NotchNotch,
}

impl From<FilterModeParam> for FilterMode {
    fn from(val: FilterModeParam) -> Self {
        match val {
            FilterModeParam::Lp => FilterMode::Lp,
            FilterModeParam::Bp => FilterMode::Bp,
            FilterModeParam::Hp => FilterMode::Hp,
            FilterModeParam::Scream => FilterMode::Scream,
            FilterModeParam::Daft => FilterMode::Daft,
            FilterModeParam::Comb => FilterMode::Comb,
            FilterModeParam::HpPeak => FilterMode::HpPeak,
            FilterModeParam::HpNotch => FilterMode::HpNotch,
            FilterModeParam::LpNotch => FilterMode::LpNotch,
            FilterModeParam::LpPeak => FilterMode::LpPeak,
            FilterModeParam::Notch => FilterMode::Notch,
            FilterModeParam::Peak => FilterMode::Peak,
            FilterModeParam::Bandreject => FilterMode::Bandreject,
            FilterModeParam::PeakPeak => FilterMode::PeakPeak,
            FilterModeParam::NotchNotch => FilterMode::NotchNotch,
        }
    }
}

pub const FX_FILTER_MODE_ITEMS: &[(&str, FilterModeParam)] = &[
    ("LP", FilterModeParam::Lp),
    ("BP", FilterModeParam::Bp),
    ("HP", FilterModeParam::Hp),
    ("Notch", FilterModeParam::Notch),
    ("Bandreject", FilterModeParam::Bandreject),
    ("Peak", FilterModeParam::Peak),
    ("Scream", FilterModeParam::Scream),
    ("Daft", FilterModeParam::Daft),
    ("Comb", FilterModeParam::Comb),
    ("HP + Peak", FilterModeParam::HpPeak),
    ("HP + Notch", FilterModeParam::HpNotch),
    ("LP + Notch", FilterModeParam::LpNotch),
    ("LP + Peak", FilterModeParam::LpPeak),
    ("Peak + Peak", FilterModeParam::PeakPeak),
    ("Notch + Notch", FilterModeParam::NotchNotch),
];

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxFilterSlopeParam {
    #[name = "12 dB"]
    Db12,
    #[name = "24 dB"]
    Db24,
    #[name = "48 dB"]
    Db48,
}

impl From<FxFilterSlopeParam> for FxFilterSlope {
    fn from(v: FxFilterSlopeParam) -> Self {
        match v {
            FxFilterSlopeParam::Db12 => Self::Db12,
            FxFilterSlopeParam::Db24 => Self::Db24,
            FxFilterSlopeParam::Db48 => Self::Db48,
        }
    }
}

impl From<FxFilterSlope> for FxFilterSlopeParam {
    fn from(v: FxFilterSlope) -> Self {
        match v {
            FxFilterSlope::Db12 => Self::Db12,
            FxFilterSlope::Db24 => Self::Db24,
            FxFilterSlope::Db48 => Self::Db48,
        }
    }
}

pub const FX_FILTER_SLOPE_ITEMS: &[(&str, FxFilterSlopeParam)] = &[
    ("12 dB", FxFilterSlopeParam::Db12),
    ("24 dB", FxFilterSlopeParam::Db24),
    ("48 dB", FxFilterSlopeParam::Db48),
];
