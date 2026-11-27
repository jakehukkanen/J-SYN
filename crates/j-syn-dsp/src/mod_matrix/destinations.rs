//! Modulation sources, compact structural destination indices, legacy-compatible serde,
//! and human-readable label formatting.

use std::borrow::Cow;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const NUM_MOD_SLOTS: usize = 256;
pub const NUM_ACTIVE_SOURCES: usize = 29;

pub const FX_MOD_BUSES: usize = 3;
pub const FX_MOD_SLOTS_PER_BUS: usize = 30;
pub const FX_MOD_PARAMS_PER_SLOT: usize = 16;

/// Non-FX-slot destinations (`Osc`, `Noise`, `Filter`, `Insert`, `Routing`, `Glide`, `ModSeqRate`, `ModSeqRow`, `Sub`, `Bus Level/Pan`).
pub const NUM_VOICE_MOD_DESTS: usize = 92;
/// FX-slot parameter destinations (`3 buses × 30 slots × 16 params = 1,440`).
pub const NUM_FX_MOD_DESTS: usize = FX_MOD_BUSES * FX_MOD_SLOTS_PER_BUS * FX_MOD_PARAMS_PER_SLOT;
/// Total modulatable destinations (`80 + 1,440 = 1,520`).
pub const NUM_MOD_DESTS: usize = NUM_VOICE_MOD_DESTS + NUM_FX_MOD_DESTS;

/// Modulation sources assignable in any matrix slot.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ModMatrixSource {
    #[default]
    None = 0,
    Env1 = 1,
    Env2 = 2,
    Env3 = 3,
    Env4 = 4,
    ModSeq1 = 5,
    ModSeq2 = 6,
    ModSeq3 = 7,
    ModSeq4 = 8,
    Velocity = 9,
    Keytrack = 10,
    ModWheel = 11,
    Aftertouch = 12,
    Macro1 = 13,
    Macro2 = 14,
    Macro3 = 15,
    Macro4 = 16,
    Macro5 = 17,
    Macro6 = 18,
    Macro7 = 19,
    Macro8 = 20,
    Slide = 21,
    Env5 = 22,
    Env6 = 23,
    Env7 = 24,
    Env8 = 25,
    ModSeq5 = 26,
    ModSeq6 = 27,
    ModSeq7 = 28,
    ModSeq8 = 29,
}

impl ModMatrixSource {
    pub const ALL_ACTIVE: [ModMatrixSource; NUM_ACTIVE_SOURCES] = [
        Self::Env1,
        Self::Env2,
        Self::Env3,
        Self::Env4,
        Self::Env5,
        Self::Env6,
        Self::Env7,
        Self::Env8,
        Self::ModSeq1,
        Self::ModSeq2,
        Self::ModSeq3,
        Self::ModSeq4,
        Self::ModSeq5,
        Self::ModSeq6,
        Self::ModSeq7,
        Self::ModSeq8,
        Self::Velocity,
        Self::Keytrack,
        Self::ModWheel,
        Self::Aftertouch,
        Self::Slide,
        Self::Macro1,
        Self::Macro2,
        Self::Macro3,
        Self::Macro4,
        Self::Macro5,
        Self::Macro6,
        Self::Macro7,
        Self::Macro8,
    ];

    pub const ENVS: [ModMatrixSource; 8] = [
        Self::Env1,
        Self::Env2,
        Self::Env3,
        Self::Env4,
        Self::Env5,
        Self::Env6,
        Self::Env7,
        Self::Env8,
    ];

    pub const MODSEQS: [ModMatrixSource; 8] = [
        Self::ModSeq1,
        Self::ModSeq2,
        Self::ModSeq3,
        Self::ModSeq4,
        Self::ModSeq5,
        Self::ModSeq6,
        Self::ModSeq7,
        Self::ModSeq8,
    ];

    pub const PERF: [ModMatrixSource; 5] = [
        Self::Velocity,
        Self::Keytrack,
        Self::ModWheel,
        Self::Aftertouch,
        Self::Slide,
    ];

    pub const MACROS: [ModMatrixSource; 8] = [
        Self::Macro1,
        Self::Macro2,
        Self::Macro3,
        Self::Macro4,
        Self::Macro5,
        Self::Macro6,
        Self::Macro7,
        Self::Macro8,
    ];

    #[inline]
    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => Self::Env1,
            2 => Self::Env2,
            3 => Self::Env3,
            4 => Self::Env4,
            5 => Self::ModSeq1,
            6 => Self::ModSeq2,
            7 => Self::ModSeq3,
            8 => Self::ModSeq4,
            9 => Self::Velocity,
            10 => Self::Keytrack,
            11 => Self::ModWheel,
            12 => Self::Aftertouch,
            13 => Self::Macro1,
            14 => Self::Macro2,
            15 => Self::Macro3,
            16 => Self::Macro4,
            17 => Self::Macro5,
            18 => Self::Macro6,
            19 => Self::Macro7,
            20 => Self::Macro8,
            21 => Self::Slide,
            22 => Self::Env5,
            23 => Self::Env6,
            24 => Self::Env7,
            25 => Self::Env8,
            26 => Self::ModSeq5,
            27 => Self::ModSeq6,
            28 => Self::ModSeq7,
            29 => Self::ModSeq8,
            _ => Self::None,
        }
    }

    #[inline]
    pub fn active_index(self) -> Option<usize> {
        let raw = self as u8;
        if raw >= 1 && (raw as usize) <= NUM_ACTIVE_SOURCES {
            Some((raw as usize) - 1)
        } else {
            None
        }
    }

    #[inline]
    pub fn short_label(self) -> &'static str {
        match self {
            Self::None => "--",
            Self::Env1 => "E1",
            Self::Env2 => "E2",
            Self::Env3 => "E3",
            Self::Env4 => "E4",
            Self::Env5 => "E5",
            Self::Env6 => "E6",
            Self::Env7 => "E7",
            Self::Env8 => "E8",
            Self::ModSeq1 => "MS1",
            Self::ModSeq2 => "MS2",
            Self::ModSeq3 => "MS3",
            Self::ModSeq4 => "MS4",
            Self::ModSeq5 => "MS5",
            Self::ModSeq6 => "MS6",
            Self::ModSeq7 => "MS7",
            Self::ModSeq8 => "MS8",
            Self::Velocity => "VEL",
            Self::Keytrack => "KEY",
            Self::ModWheel => "MW",
            Self::Aftertouch => "AT",
            Self::Slide => "SLD",
            Self::Macro1 => "M1",
            Self::Macro2 => "M2",
            Self::Macro3 => "M3",
            Self::Macro4 => "M4",
            Self::Macro5 => "M5",
            Self::Macro6 => "M6",
            Self::Macro7 => "M7",
            Self::Macro8 => "M8",
        }
    }

    #[inline]
    pub fn full_label(self) -> &'static str {
        match self {
            Self::None => "--",
            Self::Env1 => "ENV 1 (AMP)",
            Self::Env2 => "ENV 2",
            Self::Env3 => "ENV 3",
            Self::Env4 => "ENV 4",
            Self::Env5 => "ENV 5",
            Self::Env6 => "ENV 6",
            Self::Env7 => "ENV 7",
            Self::Env8 => "ENV 8",
            Self::ModSeq1 => "MODSEQ 1",
            Self::ModSeq2 => "MODSEQ 2",
            Self::ModSeq3 => "MODSEQ 3",
            Self::ModSeq4 => "MODSEQ 4",
            Self::ModSeq5 => "MODSEQ 5",
            Self::ModSeq6 => "MODSEQ 6",
            Self::ModSeq7 => "MODSEQ 7",
            Self::ModSeq8 => "MODSEQ 8",
            Self::Velocity => "VELOCITY",
            Self::Keytrack => "KEYTRACK",
            Self::ModWheel => "MOD WHEEL",
            Self::Aftertouch => "AFTERTOUCH",
            Self::Slide => "SLIDE (CC74)",
            Self::Macro1 => "MACRO 1",
            Self::Macro2 => "MACRO 2",
            Self::Macro3 => "MACRO 3",
            Self::Macro4 => "MACRO 4",
            Self::Macro5 => "MACRO 5",
            Self::Macro6 => "MACRO 6",
            Self::Macro7 => "MACRO 7",
            Self::Macro8 => "MACRO 8",
        }
    }

    #[inline]
    pub fn color_rgb(self) -> (u8, u8, u8) {
        match self {
            Self::None => (140, 125, 110),
            Self::Env1 => (0, 230, 118),
            Self::Env2 => (46, 242, 150),
            Self::Env3 => (0, 212, 142),
            Self::Env4 => (92, 248, 172),
            Self::Env5 => (24, 218, 128),
            Self::Env6 => (68, 236, 162),
            Self::Env7 => (18, 198, 134),
            Self::Env8 => (114, 244, 184),
            Self::ModSeq1 => (0, 229, 255),
            Self::ModSeq2 => (56, 206, 255),
            Self::ModSeq3 => (0, 245, 232),
            Self::ModSeq4 => (100, 220, 255),
            Self::ModSeq5 => (24, 214, 250),
            Self::ModSeq6 => (76, 196, 255),
            Self::ModSeq7 => (20, 234, 224),
            Self::ModSeq8 => (118, 212, 255),
            Self::Velocity => (255, 82, 82),
            Self::Keytrack => (255, 112, 96),
            Self::ModWheel => (255, 92, 130),
            Self::Aftertouch => (255, 126, 82),
            Self::Slide => (255, 152, 108),
            Self::Macro1 => (255, 200, 72),
            Self::Macro2 => (255, 178, 60),
            Self::Macro3 => (245, 152, 66),
            Self::Macro4 => (255, 214, 102),
            Self::Macro5 => (222, 148, 255),
            Self::Macro6 => (196, 136, 255),
            Self::Macro7 => (240, 128, 224),
            Self::Macro8 => (176, 158, 255),
        }
    }
}

/// All modulatable continuous & pitch destinations across `J-SYN`, including all
/// `3 × 30 × 16 = 1,440` Master FX Unit parameters via `FxSlotParam`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModDestination {
    #[default]
    Osc1WtPos,
    Osc2WtPos,
    Osc3WtPos,
    Osc4WtPos,
    Osc1Warp,
    Osc2Warp,
    Osc3Warp,
    Osc4Warp,
    Osc1ModDepth,
    Osc2ModDepth,
    Osc3ModDepth,
    Osc4ModDepth,
    Osc1Pan,
    Osc2Pan,
    Osc3Pan,
    Osc4Pan,
    Osc1Level,
    Osc2Level,
    Osc3Level,
    Osc4Level,
    Osc1Pitch,
    Osc2Pitch,
    Osc3Pitch,
    Osc4Pitch,
    Osc1Phase,
    Osc2Phase,
    Osc3Phase,
    Osc4Phase,
    Osc1Detune,
    Osc2Detune,
    Osc3Detune,
    Osc4Detune,
    Osc1Spread,
    Osc2Spread,
    Osc3Spread,
    Osc4Spread,
    NoiseColor,
    NoiseStereo,
    NoiseLevel,
    Filter1Cutoff,
    Filter1Res,
    Filter1Drive,
    Filter1Aux,
    Filter2Cutoff,
    Filter2Res,
    Filter2Drive,
    Filter2Aux,
    Insert1Drive,
    Insert1Mix,
    Insert2Drive,
    Insert2Mix,
    Osc1FilterSend,
    Osc2FilterSend,
    Osc3FilterSend,
    Osc4FilterSend,
    NoiseFilterSend,
    FilterMix,
    FeedbackAmount,
    GlideMs,
    ModSeq1Rate,
    ModSeq2Rate,
    ModSeq3Rate,
    ModSeq4Rate,
    Osc1Ratio,
    Osc2Ratio,
    Osc3Ratio,
    Osc4Ratio,
    Osc1HzOffset,
    Osc2HzOffset,
    Osc3HzOffset,
    Osc4HzOffset,
    SubCutoff,
    SubDrive,
    SubLevel,
    BusALevel,
    BusAPan,
    BusBLevel,
    BusBPan,
    BusCLevel,
    BusCPan,
    ModSeq5Rate,
    ModSeq6Rate,
    ModSeq7Rate,
    ModSeq8Rate,
    ModSeq1Row,
    ModSeq2Row,
    ModSeq3Row,
    ModSeq4Row,
    ModSeq5Row,
    ModSeq6Row,
    ModSeq7Row,
    ModSeq8Row,
    FxSlotParam {
        bus: u8,
        slot: u8,
        param: u8,
    },
}

/// All 92 non-FX-slot destinations in canonical index order `0..NUM_VOICE_MOD_DESTS`.
pub const BASE_MOD_DESTINATIONS: [ModDestination; NUM_VOICE_MOD_DESTS] = [
    ModDestination::Osc1WtPos,
    ModDestination::Osc2WtPos,
    ModDestination::Osc3WtPos,
    ModDestination::Osc4WtPos,
    ModDestination::Osc1Warp,
    ModDestination::Osc2Warp,
    ModDestination::Osc3Warp,
    ModDestination::Osc4Warp,
    ModDestination::Osc1ModDepth,
    ModDestination::Osc2ModDepth,
    ModDestination::Osc3ModDepth,
    ModDestination::Osc4ModDepth,
    ModDestination::Osc1Pan,
    ModDestination::Osc2Pan,
    ModDestination::Osc3Pan,
    ModDestination::Osc4Pan,
    ModDestination::Osc1Level,
    ModDestination::Osc2Level,
    ModDestination::Osc3Level,
    ModDestination::Osc4Level,
    ModDestination::Osc1Pitch,
    ModDestination::Osc2Pitch,
    ModDestination::Osc3Pitch,
    ModDestination::Osc4Pitch,
    ModDestination::Osc1Phase,
    ModDestination::Osc2Phase,
    ModDestination::Osc3Phase,
    ModDestination::Osc4Phase,
    ModDestination::Osc1Detune,
    ModDestination::Osc2Detune,
    ModDestination::Osc3Detune,
    ModDestination::Osc4Detune,
    ModDestination::Osc1Spread,
    ModDestination::Osc2Spread,
    ModDestination::Osc3Spread,
    ModDestination::Osc4Spread,
    ModDestination::NoiseColor,
    ModDestination::NoiseStereo,
    ModDestination::NoiseLevel,
    ModDestination::Filter1Cutoff,
    ModDestination::Filter1Res,
    ModDestination::Filter1Drive,
    ModDestination::Filter1Aux,
    ModDestination::Filter2Cutoff,
    ModDestination::Filter2Res,
    ModDestination::Filter2Drive,
    ModDestination::Filter2Aux,
    ModDestination::Insert1Drive,
    ModDestination::Insert1Mix,
    ModDestination::Insert2Drive,
    ModDestination::Insert2Mix,
    ModDestination::Osc1FilterSend,
    ModDestination::Osc2FilterSend,
    ModDestination::Osc3FilterSend,
    ModDestination::Osc4FilterSend,
    ModDestination::NoiseFilterSend,
    ModDestination::FilterMix,
    ModDestination::FeedbackAmount,
    ModDestination::GlideMs,
    ModDestination::ModSeq1Rate,
    ModDestination::ModSeq2Rate,
    ModDestination::ModSeq3Rate,
    ModDestination::ModSeq4Rate,
    ModDestination::Osc1Ratio,
    ModDestination::Osc2Ratio,
    ModDestination::Osc3Ratio,
    ModDestination::Osc4Ratio,
    ModDestination::Osc1HzOffset,
    ModDestination::Osc2HzOffset,
    ModDestination::Osc3HzOffset,
    ModDestination::Osc4HzOffset,
    ModDestination::SubCutoff,
    ModDestination::SubDrive,
    ModDestination::SubLevel,
    ModDestination::BusALevel,
    ModDestination::BusAPan,
    ModDestination::BusBLevel,
    ModDestination::BusBPan,
    ModDestination::BusCLevel,
    ModDestination::BusCPan,
    ModDestination::ModSeq5Rate,
    ModDestination::ModSeq6Rate,
    ModDestination::ModSeq7Rate,
    ModDestination::ModSeq8Rate,
    ModDestination::ModSeq1Row,
    ModDestination::ModSeq2Row,
    ModDestination::ModSeq3Row,
    ModDestination::ModSeq4Row,
    ModDestination::ModSeq5Row,
    ModDestination::ModSeq6Row,
    ModDestination::ModSeq7Row,
    ModDestination::ModSeq8Row,
];

const BASE_SERDE_NAMES: [&str; NUM_VOICE_MOD_DESTS] = [
    "Osc1WtPos",
    "Osc2WtPos",
    "Osc3WtPos",
    "Osc4WtPos",
    "Osc1Warp",
    "Osc2Warp",
    "Osc3Warp",
    "Osc4Warp",
    "Osc1ModDepth",
    "Osc2ModDepth",
    "Osc3ModDepth",
    "Osc4ModDepth",
    "Osc1Pan",
    "Osc2Pan",
    "Osc3Pan",
    "Osc4Pan",
    "Osc1Level",
    "Osc2Level",
    "Osc3Level",
    "Osc4Level",
    "Osc1Pitch",
    "Osc2Pitch",
    "Osc3Pitch",
    "Osc4Pitch",
    "Osc1Phase",
    "Osc2Phase",
    "Osc3Phase",
    "Osc4Phase",
    "Osc1Detune",
    "Osc2Detune",
    "Osc3Detune",
    "Osc4Detune",
    "Osc1Spread",
    "Osc2Spread",
    "Osc3Spread",
    "Osc4Spread",
    "NoiseColor",
    "NoiseStereo",
    "NoiseLevel",
    "Filter1Cutoff",
    "Filter1Res",
    "Filter1Drive",
    "Filter1Aux",
    "Filter2Cutoff",
    "Filter2Res",
    "Filter2Drive",
    "Filter2Aux",
    "Insert1Drive",
    "Insert1Mix",
    "Insert2Drive",
    "Insert2Mix",
    "Osc1FilterSend",
    "Osc2FilterSend",
    "Osc3FilterSend",
    "Osc4FilterSend",
    "NoiseFilterSend",
    "FilterMix",
    "FeedbackAmount",
    "GlideMs",
    "ModSeq1Rate",
    "ModSeq2Rate",
    "ModSeq3Rate",
    "ModSeq4Rate",
    "Osc1Ratio",
    "Osc2Ratio",
    "Osc3Ratio",
    "Osc4Ratio",
    "Osc1HzOffset",
    "Osc2HzOffset",
    "Osc3HzOffset",
    "Osc4HzOffset",
    "SubCutoff",
    "SubDrive",
    "SubLevel",
    "BusALevel",
    "BusAPan",
    "BusBLevel",
    "BusBPan",
    "BusCLevel",
    "BusCPan",
    "ModSeq5Rate",
    "ModSeq6Rate",
    "ModSeq7Rate",
    "ModSeq8Rate",
    "ModSeq1Row",
    "ModSeq2Row",
    "ModSeq3Row",
    "ModSeq4Row",
    "ModSeq5Row",
    "ModSeq6Row",
    "ModSeq7Row",
    "ModSeq8Row",
];

const BASE_DISPLAY_NAMES: [&str; NUM_VOICE_MOD_DESTS] = [
    "OSC1 WT",
    "OSC2 WT",
    "OSC3 WT",
    "OSC4 WT",
    "OSC1 WARP",
    "OSC2 WARP",
    "OSC3 WARP",
    "OSC4 WARP",
    "OSC1 MOD",
    "OSC2 MOD",
    "OSC3 MOD",
    "OSC4 MOD",
    "OSC1 PAN",
    "OSC2 PAN",
    "OSC3 PAN",
    "OSC4 PAN",
    "OSC1 LVL",
    "OSC2 LVL",
    "OSC3 LVL",
    "OSC4 LVL",
    "OSC1 PITCH",
    "OSC2 PITCH",
    "OSC3 PITCH",
    "OSC4 PITCH",
    "OSC1 PHS",
    "OSC2 PHS",
    "OSC3 PHS",
    "OSC4 PHS",
    "OSC1 DET",
    "OSC2 DET",
    "OSC3 DET",
    "OSC4 DET",
    "OSC1 SPR",
    "OSC2 SPR",
    "OSC3 SPR",
    "OSC4 SPR",
    "NSE CLR",
    "NSE ST",
    "NSE LVL",
    "F1 CUT",
    "F1 RES",
    "F1 DRV",
    "F1 AUX",
    "F2 CUT",
    "F2 RES",
    "F2 DRV",
    "F2 AUX",
    "INS1 DRV",
    "INS1 MIX",
    "INS2 DRV",
    "INS2 MIX",
    "O1 SEND",
    "O2 SEND",
    "O3 SEND",
    "O4 SEND",
    "NS SEND",
    "F1/2 MIX",
    "FEEDBACK",
    "GLIDE",
    "MS1 RATE",
    "MS2 RATE",
    "MS3 RATE",
    "MS4 RATE",
    "OSC1 RATIO",
    "OSC2 RATIO",
    "OSC3 RATIO",
    "OSC4 RATIO",
    "OSC1 HZ",
    "OSC2 HZ",
    "OSC3 HZ",
    "OSC4 HZ",
    "SUB CUT",
    "SUB DRV",
    "SUB LVL",
    "BUS A LVL",
    "BUS A PAN",
    "BUS B LVL",
    "BUS B PAN",
    "BUS C LVL",
    "BUS C PAN",
    "MS5 RATE",
    "MS6 RATE",
    "MS7 RATE",
    "MS8 RATE",
    "MS1 ROW",
    "MS2 ROW",
    "MS3 ROW",
    "MS4 ROW",
    "MS5 ROW",
    "MS6 ROW",
    "MS7 ROW",
    "MS8 ROW",
];

impl ModDestination {
    #[inline]
    pub const fn index(self) -> usize {
        match self {
            Self::Osc1WtPos => 0,
            Self::Osc2WtPos => 1,
            Self::Osc3WtPos => 2,
            Self::Osc4WtPos => 3,
            Self::Osc1Warp => 4,
            Self::Osc2Warp => 5,
            Self::Osc3Warp => 6,
            Self::Osc4Warp => 7,
            Self::Osc1ModDepth => 8,
            Self::Osc2ModDepth => 9,
            Self::Osc3ModDepth => 10,
            Self::Osc4ModDepth => 11,
            Self::Osc1Pan => 12,
            Self::Osc2Pan => 13,
            Self::Osc3Pan => 14,
            Self::Osc4Pan => 15,
            Self::Osc1Level => 16,
            Self::Osc2Level => 17,
            Self::Osc3Level => 18,
            Self::Osc4Level => 19,
            Self::Osc1Pitch => 20,
            Self::Osc2Pitch => 21,
            Self::Osc3Pitch => 22,
            Self::Osc4Pitch => 23,
            Self::Osc1Phase => 24,
            Self::Osc2Phase => 25,
            Self::Osc3Phase => 26,
            Self::Osc4Phase => 27,
            Self::Osc1Detune => 28,
            Self::Osc2Detune => 29,
            Self::Osc3Detune => 30,
            Self::Osc4Detune => 31,
            Self::Osc1Spread => 32,
            Self::Osc2Spread => 33,
            Self::Osc3Spread => 34,
            Self::Osc4Spread => 35,
            Self::NoiseColor => 36,
            Self::NoiseStereo => 37,
            Self::NoiseLevel => 38,
            Self::Filter1Cutoff => 39,
            Self::Filter1Res => 40,
            Self::Filter1Drive => 41,
            Self::Filter1Aux => 42,
            Self::Filter2Cutoff => 43,
            Self::Filter2Res => 44,
            Self::Filter2Drive => 45,
            Self::Filter2Aux => 46,
            Self::Insert1Drive => 47,
            Self::Insert1Mix => 48,
            Self::Insert2Drive => 49,
            Self::Insert2Mix => 50,
            Self::Osc1FilterSend => 51,
            Self::Osc2FilterSend => 52,
            Self::Osc3FilterSend => 53,
            Self::Osc4FilterSend => 54,
            Self::NoiseFilterSend => 55,
            Self::FilterMix => 56,
            Self::FeedbackAmount => 57,
            Self::GlideMs => 58,
            Self::ModSeq1Rate => 59,
            Self::ModSeq2Rate => 60,
            Self::ModSeq3Rate => 61,
            Self::ModSeq4Rate => 62,
            Self::Osc1Ratio => 63,
            Self::Osc2Ratio => 64,
            Self::Osc3Ratio => 65,
            Self::Osc4Ratio => 66,
            Self::Osc1HzOffset => 67,
            Self::Osc2HzOffset => 68,
            Self::Osc3HzOffset => 69,
            Self::Osc4HzOffset => 70,
            Self::SubCutoff => 71,
            Self::SubDrive => 72,
            Self::SubLevel => 73,
            Self::BusALevel => 74,
            Self::BusAPan => 75,
            Self::BusBLevel => 76,
            Self::BusBPan => 77,
            Self::BusCLevel => 78,
            Self::BusCPan => 79,
            Self::ModSeq5Rate => 80,
            Self::ModSeq6Rate => 81,
            Self::ModSeq7Rate => 82,
            Self::ModSeq8Rate => 83,
            Self::ModSeq1Row => 84,
            Self::ModSeq2Row => 85,
            Self::ModSeq3Row => 86,
            Self::ModSeq4Row => 87,
            Self::ModSeq5Row => 88,
            Self::ModSeq6Row => 89,
            Self::ModSeq7Row => 90,
            Self::ModSeq8Row => 91,
            Self::FxSlotParam { bus, slot, param } => {
                let b = if (bus as usize) < FX_MOD_BUSES {
                    bus as usize
                } else {
                    FX_MOD_BUSES - 1
                };
                let s = if (slot as usize) < FX_MOD_SLOTS_PER_BUS {
                    slot as usize
                } else {
                    FX_MOD_SLOTS_PER_BUS - 1
                };
                let p = if (param as usize) < FX_MOD_PARAMS_PER_SLOT {
                    param as usize
                } else {
                    FX_MOD_PARAMS_PER_SLOT - 1
                };
                NUM_VOICE_MOD_DESTS
                    + b * (FX_MOD_SLOTS_PER_BUS * FX_MOD_PARAMS_PER_SLOT)
                    + s * FX_MOD_PARAMS_PER_SLOT
                    + p
            }
        }
    }

    #[inline]
    pub const fn from_index(idx: usize) -> Option<Self> {
        if idx < NUM_VOICE_MOD_DESTS {
            Some(BASE_MOD_DESTINATIONS[idx])
        } else if idx < NUM_MOD_DESTS {
            let rel = idx - NUM_VOICE_MOD_DESTS;
            let per_bus = FX_MOD_SLOTS_PER_BUS * FX_MOD_PARAMS_PER_SLOT;
            let bus = rel / per_bus;
            let rem = rel % per_bus;
            let slot = rem / FX_MOD_PARAMS_PER_SLOT;
            let param = rem % FX_MOD_PARAMS_PER_SLOT;
            Some(Self::FxSlotParam {
                bus: bus as u8,
                slot: slot as u8,
                param: param as u8,
            })
        } else {
            None
        }
    }

    #[inline]
    pub const fn osc_wt_pos(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1WtPos,
            1 => Self::Osc2WtPos,
            2 => Self::Osc3WtPos,
            _ => Self::Osc4WtPos,
        }
    }

    #[inline]
    pub const fn osc_warp(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1Warp,
            1 => Self::Osc2Warp,
            2 => Self::Osc3Warp,
            _ => Self::Osc4Warp,
        }
    }

    #[inline]
    pub const fn osc_mod_depth(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1ModDepth,
            1 => Self::Osc2ModDepth,
            2 => Self::Osc3ModDepth,
            _ => Self::Osc4ModDepth,
        }
    }

    #[inline]
    pub const fn osc_pan(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1Pan,
            1 => Self::Osc2Pan,
            2 => Self::Osc3Pan,
            _ => Self::Osc4Pan,
        }
    }

    #[inline]
    pub const fn osc_level(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1Level,
            1 => Self::Osc2Level,
            2 => Self::Osc3Level,
            _ => Self::Osc4Level,
        }
    }

    #[inline]
    pub const fn osc_pitch(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1Pitch,
            1 => Self::Osc2Pitch,
            2 => Self::Osc3Pitch,
            _ => Self::Osc4Pitch,
        }
    }

    #[inline]
    pub const fn osc_ratio(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1Ratio,
            1 => Self::Osc2Ratio,
            2 => Self::Osc3Ratio,
            _ => Self::Osc4Ratio,
        }
    }

    #[inline]
    pub const fn osc_hz_offset(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1HzOffset,
            1 => Self::Osc2HzOffset,
            2 => Self::Osc3HzOffset,
            _ => Self::Osc4HzOffset,
        }
    }

    #[inline]
    pub const fn osc_phase(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1Phase,
            1 => Self::Osc2Phase,
            2 => Self::Osc3Phase,
            _ => Self::Osc4Phase,
        }
    }

    #[inline]
    pub const fn osc_detune(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1Detune,
            1 => Self::Osc2Detune,
            2 => Self::Osc3Detune,
            _ => Self::Osc4Detune,
        }
    }

    #[inline]
    pub const fn osc_spread(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1Spread,
            1 => Self::Osc2Spread,
            2 => Self::Osc3Spread,
            _ => Self::Osc4Spread,
        }
    }

    #[inline]
    pub const fn filter_cutoff(idx: usize) -> Self {
        if idx == 0 {
            Self::Filter1Cutoff
        } else {
            Self::Filter2Cutoff
        }
    }

    #[inline]
    pub const fn filter_res(idx: usize) -> Self {
        if idx == 0 {
            Self::Filter1Res
        } else {
            Self::Filter2Res
        }
    }

    #[inline]
    pub const fn filter_drive(idx: usize) -> Self {
        if idx == 0 {
            Self::Filter1Drive
        } else {
            Self::Filter2Drive
        }
    }

    #[inline]
    pub const fn filter_aux(idx: usize) -> Self {
        if idx == 0 {
            Self::Filter1Aux
        } else {
            Self::Filter2Aux
        }
    }

    #[inline]
    pub const fn insert_drive(idx: usize) -> Self {
        if idx == 0 {
            Self::Insert1Drive
        } else {
            Self::Insert2Drive
        }
    }

    #[inline]
    pub const fn insert_mix(idx: usize) -> Self {
        if idx == 0 {
            Self::Insert1Mix
        } else {
            Self::Insert2Mix
        }
    }

    #[inline]
    pub const fn osc_filter_send(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1FilterSend,
            1 => Self::Osc2FilterSend,
            2 => Self::Osc3FilterSend,
            _ => Self::Osc4FilterSend,
        }
    }

    #[inline]
    pub const fn modseq_rate(idx: usize) -> Self {
        match idx {
            0 => Self::ModSeq1Rate,
            1 => Self::ModSeq2Rate,
            2 => Self::ModSeq3Rate,
            3 => Self::ModSeq4Rate,
            4 => Self::ModSeq5Rate,
            5 => Self::ModSeq6Rate,
            6 => Self::ModSeq7Rate,
            _ => Self::ModSeq8Rate,
        }
    }

    #[inline]
    pub const fn modseq_row(idx: usize) -> Self {
        match idx {
            0 => Self::ModSeq1Row,
            1 => Self::ModSeq2Row,
            2 => Self::ModSeq3Row,
            3 => Self::ModSeq4Row,
            4 => Self::ModSeq5Row,
            5 => Self::ModSeq6Row,
            6 => Self::ModSeq7Row,
            _ => Self::ModSeq8Row,
        }
    }

    /// Returns destination for `bus` (`0..3`), `slot` (`0..30`), and `param` (`0..16`).
    #[inline]
    pub const fn fx_bus_slot_param(bus: usize, slot: usize, param: usize) -> Self {
        let b = if bus < FX_MOD_BUSES {
            bus as u8
        } else {
            (FX_MOD_BUSES - 1) as u8
        };
        let s = if slot < FX_MOD_SLOTS_PER_BUS {
            slot as u8
        } else {
            (FX_MOD_SLOTS_PER_BUS - 1) as u8
        };
        let p = if param < FX_MOD_PARAMS_PER_SLOT {
            param as u8
        } else {
            (FX_MOD_PARAMS_PER_SLOT - 1) as u8
        };
        Self::FxSlotParam {
            bus: b,
            slot: s,
            param: p,
        }
    }

    #[inline]
    pub const fn fx_param(slot: usize, param: usize) -> Self {
        Self::fx_bus_slot_param(0, slot, param)
    }

    #[inline]
    pub const fn fx_slot_param_coords(self) -> Option<(usize, usize, usize)> {
        match self {
            Self::FxSlotParam { bus, slot, param } => {
                Some((bus as usize, slot as usize, param as usize))
            }
            _ => None,
        }
    }

    #[inline]
    pub const fn is_fx_bus_slot(self, bus: usize, slot: usize) -> bool {
        match self {
            Self::FxSlotParam {
                bus: b,
                slot: s,
                ..
            } => (b as usize) == bus && (s as usize) == slot,
            _ => false,
        }
    }

    #[inline]
    pub const fn is_fx_slot(self, slot: usize) -> bool {
        self.is_fx_bus_slot(0, slot)
    }

    /// Human-readable short label for UI menus and `MATRIX` view (non-RT).
    pub fn name(self) -> Cow<'static, str> {
        match self {
            Self::FxSlotParam { bus, slot, param } => {
                let bus_ch = match bus {
                    0 => 'A',
                    1 => 'B',
                    _ => 'C',
                };
                Cow::Owned(format!("BUS {} FX{} P{}", bus_ch, slot + 1, param + 1))
            }
            _ => Cow::Borrowed(BASE_DISPLAY_NAMES[self.index()]),
        }
    }

    fn parse_serde_name(s: &str) -> Option<Self> {
        if let Some(idx) = BASE_SERDE_NAMES.iter().position(|&n| n == s) {
            return Some(BASE_MOD_DESTINATIONS[idx]);
        }
        // Accept legacy & extended FX names:
        // "Fx1P1".."Fx30P16", "BusAFx1P1".."BusAFx30P16", "BusBFx...", "BusCFx..."
        let (bus, rest) = if let Some(r) = s.strip_prefix("Fx") {
            (0usize, r)
        } else if let Some(r) = s.strip_prefix("BusAFx") {
            (0usize, r)
        } else if let Some(r) = s.strip_prefix("BusBFx") {
            (1usize, r)
        } else {
            let r = s.strip_prefix("BusCFx")?;
            (2usize, r)
        };
        let (slot_str, param_str) = rest.split_once('P')?;
        let slot_1: usize = slot_str.parse().ok()?;
        let param_1: usize = param_str.parse().ok()?;
        if (1..=FX_MOD_SLOTS_PER_BUS).contains(&slot_1)
            && (1..=FX_MOD_PARAMS_PER_SLOT).contains(&param_1)
        {
            Some(Self::fx_bus_slot_param(bus, slot_1 - 1, param_1 - 1))
        } else {
            None
        }
    }
}

impl Serialize for ModDestination {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match *self {
            Self::FxSlotParam { bus, slot, param } => {
                // Emit exact v0.4 variant names for slots 1..=8 so saved patches stay backward-friendly.
                let s = if bus == 0 && slot < 8 && !(4..8).contains(&param) {
                    format!("Fx{}P{}", slot + 1, param + 1)
                } else {
                    let bus_ch = match bus {
                        0 => 'A',
                        1 => 'B',
                        _ => 'C',
                    };
                    format!("Bus{bus_ch}Fx{}P{}", slot + 1, param + 1)
                };
                serializer.serialize_str(&s)
            }
            other => serializer.serialize_str(BASE_SERDE_NAMES[other.index()]),
        }
    }
}

impl<'de> Deserialize<'de> for ModDestination {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = Cow::<'de, str>::deserialize(deserializer)?;
        Self::parse_serde_name(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown ModDestination '{s}'")))
    }
}
