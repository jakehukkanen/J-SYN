//! Lock-free 32-slot Modulation Matrix and real-time UI telemetry primitives.

use std::sync::atomic::{AtomicU32, Ordering};

use serde::{Deserialize, Serialize};

pub const NUM_MOD_SLOTS: usize = 32;
pub const NUM_ACTIVE_SOURCES: usize = 20;
pub const NUM_MOD_DESTS: usize = 95;

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
}

impl ModMatrixSource {
    pub const ALL_ACTIVE: [ModMatrixSource; NUM_ACTIVE_SOURCES] = [
        Self::Env1,
        Self::Env2,
        Self::Env3,
        Self::Env4,
        Self::ModSeq1,
        Self::ModSeq2,
        Self::ModSeq3,
        Self::ModSeq4,
        Self::Velocity,
        Self::Keytrack,
        Self::ModWheel,
        Self::Aftertouch,
        Self::Macro1,
        Self::Macro2,
        Self::Macro3,
        Self::Macro4,
        Self::Macro5,
        Self::Macro6,
        Self::Macro7,
        Self::Macro8,
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
            Self::ModSeq1 => "MS1",
            Self::ModSeq2 => "MS2",
            Self::ModSeq3 => "MS3",
            Self::ModSeq4 => "MS4",
            Self::Velocity => "VEL",
            Self::Keytrack => "KEY",
            Self::ModWheel => "MW",
            Self::Aftertouch => "AT",
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
            Self::ModSeq1 => "MODSEQ 1",
            Self::ModSeq2 => "MODSEQ 2",
            Self::ModSeq3 => "MODSEQ 3",
            Self::ModSeq4 => "MODSEQ 4",
            Self::Velocity => "VELOCITY",
            Self::Keytrack => "KEYTRACK",
            Self::ModWheel => "MOD WHEEL",
            Self::Aftertouch => "AFTERTOUCH",
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
            // Envelopes: Mint (#00E676) high-contrast family
            Self::Env1 => (0, 230, 118),
            Self::Env2 => (46, 242, 150),
            Self::Env3 => (0, 212, 142),
            Self::Env4 => (92, 248, 172),
            // ModSeqs: Cyan (#00E5FF) high-contrast family
            Self::ModSeq1 => (0, 229, 255),
            Self::ModSeq2 => (56, 206, 255),
            Self::ModSeq3 => (0, 245, 232),
            Self::ModSeq4 => (100, 220, 255),
            // MIDI / performance sources: Coral (#FF5252) high-contrast family
            Self::Velocity => (255, 82, 82),
            Self::Keytrack => (255, 112, 96),
            Self::ModWheel => (255, 92, 130),
            Self::Aftertouch => (255, 126, 82),
            // Macros: Warm Amber/Violet high-contrast family
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

/// All modulatable continuous & pitch destinations across `J-SYN`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ModDestination {
    #[default]
    Osc1WtPos = 0,
    Osc2WtPos = 1,
    Osc3WtPos = 2,
    Osc4WtPos = 3,
    Osc1Warp = 4,
    Osc2Warp = 5,
    Osc3Warp = 6,
    Osc4Warp = 7,
    Osc1ModDepth = 8,
    Osc2ModDepth = 9,
    Osc3ModDepth = 10,
    Osc4ModDepth = 11,
    Osc1Pan = 12,
    Osc2Pan = 13,
    Osc3Pan = 14,
    Osc4Pan = 15,
    Osc1Level = 16,
    Osc2Level = 17,
    Osc3Level = 18,
    Osc4Level = 19,
    Osc1Pitch = 20,
    Osc2Pitch = 21,
    Osc3Pitch = 22,
    Osc4Pitch = 23,
    Osc1Phase = 24,
    Osc2Phase = 25,
    Osc3Phase = 26,
    Osc4Phase = 27,
    Osc1Detune = 28,
    Osc2Detune = 29,
    Osc3Detune = 30,
    Osc4Detune = 31,
    Osc1Spread = 32,
    Osc2Spread = 33,
    Osc3Spread = 34,
    Osc4Spread = 35,
    NoiseColor = 36,
    NoiseStereo = 37,
    NoiseLevel = 38,
    Filter1Cutoff = 39,
    Filter1Res = 40,
    Filter1Drive = 41,
    Filter1Aux = 42,
    Filter2Cutoff = 43,
    Filter2Res = 44,
    Filter2Drive = 45,
    Filter2Aux = 46,
    Insert1Drive = 47,
    Insert1Mix = 48,
    Insert2Drive = 49,
    Insert2Mix = 50,
    Osc1FilterSend = 51,
    Osc2FilterSend = 52,
    Osc3FilterSend = 53,
    Osc4FilterSend = 54,
    NoiseFilterSend = 55,
    FilterMix = 56,
    FeedbackAmount = 57,
    GlideMs = 58,
    ModSeq1Rate = 59,
    ModSeq2Rate = 60,
    ModSeq3Rate = 61,
    ModSeq4Rate = 62,
    Fx1P1 = 63,
    Fx1P2 = 64,
    Fx1P3 = 65,
    Fx1P4 = 66,
    Fx2P1 = 67,
    Fx2P2 = 68,
    Fx2P3 = 69,
    Fx2P4 = 70,
    Fx3P1 = 71,
    Fx3P2 = 72,
    Fx3P3 = 73,
    Fx3P4 = 74,
    Fx4P1 = 75,
    Fx4P2 = 76,
    Fx4P3 = 77,
    Fx4P4 = 78,
    Fx5P1 = 79,
    Fx5P2 = 80,
    Fx5P3 = 81,
    Fx5P4 = 82,
    Fx6P1 = 83,
    Fx6P2 = 84,
    Fx6P3 = 85,
    Fx6P4 = 86,
    Fx7P1 = 87,
    Fx7P2 = 88,
    Fx7P3 = 89,
    Fx7P4 = 90,
    Fx8P1 = 91,
    Fx8P2 = 92,
    Fx8P3 = 93,
    Fx8P4 = 94,
}

impl ModDestination {
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[inline]
    pub fn from_index(idx: usize) -> Option<Self> {
        if idx >= NUM_MOD_DESTS {
            return None;
        }
        // Safe exhaustive match over 0..NUM_MOD_DESTS
        Some(match idx {
            0 => Self::Osc1WtPos,
            1 => Self::Osc2WtPos,
            2 => Self::Osc3WtPos,
            3 => Self::Osc4WtPos,
            4 => Self::Osc1Warp,
            5 => Self::Osc2Warp,
            6 => Self::Osc3Warp,
            7 => Self::Osc4Warp,
            8 => Self::Osc1ModDepth,
            9 => Self::Osc2ModDepth,
            10 => Self::Osc3ModDepth,
            11 => Self::Osc4ModDepth,
            12 => Self::Osc1Pan,
            13 => Self::Osc2Pan,
            14 => Self::Osc3Pan,
            15 => Self::Osc4Pan,
            16 => Self::Osc1Level,
            17 => Self::Osc2Level,
            18 => Self::Osc3Level,
            19 => Self::Osc4Level,
            20 => Self::Osc1Pitch,
            21 => Self::Osc2Pitch,
            22 => Self::Osc3Pitch,
            23 => Self::Osc4Pitch,
            24 => Self::Osc1Phase,
            25 => Self::Osc2Phase,
            26 => Self::Osc3Phase,
            27 => Self::Osc4Phase,
            28 => Self::Osc1Detune,
            29 => Self::Osc2Detune,
            30 => Self::Osc3Detune,
            31 => Self::Osc4Detune,
            32 => Self::Osc1Spread,
            33 => Self::Osc2Spread,
            34 => Self::Osc3Spread,
            35 => Self::Osc4Spread,
            36 => Self::NoiseColor,
            37 => Self::NoiseStereo,
            38 => Self::NoiseLevel,
            39 => Self::Filter1Cutoff,
            40 => Self::Filter1Res,
            41 => Self::Filter1Drive,
            42 => Self::Filter1Aux,
            43 => Self::Filter2Cutoff,
            44 => Self::Filter2Res,
            45 => Self::Filter2Drive,
            46 => Self::Filter2Aux,
            47 => Self::Insert1Drive,
            48 => Self::Insert1Mix,
            49 => Self::Insert2Drive,
            50 => Self::Insert2Mix,
            51 => Self::Osc1FilterSend,
            52 => Self::Osc2FilterSend,
            53 => Self::Osc3FilterSend,
            54 => Self::Osc4FilterSend,
            55 => Self::NoiseFilterSend,
            56 => Self::FilterMix,
            57 => Self::FeedbackAmount,
            58 => Self::GlideMs,
            59 => Self::ModSeq1Rate,
            60 => Self::ModSeq2Rate,
            61 => Self::ModSeq3Rate,
            62 => Self::ModSeq4Rate,
            63 => Self::Fx1P1,
            64 => Self::Fx1P2,
            65 => Self::Fx1P3,
            66 => Self::Fx1P4,
            67 => Self::Fx2P1,
            68 => Self::Fx2P2,
            69 => Self::Fx2P3,
            70 => Self::Fx2P4,
            71 => Self::Fx3P1,
            72 => Self::Fx3P2,
            73 => Self::Fx3P3,
            74 => Self::Fx3P4,
            75 => Self::Fx4P1,
            76 => Self::Fx4P2,
            77 => Self::Fx4P3,
            78 => Self::Fx4P4,
            79 => Self::Fx5P1,
            80 => Self::Fx5P2,
            81 => Self::Fx5P3,
            82 => Self::Fx5P4,
            83 => Self::Fx6P1,
            84 => Self::Fx6P2,
            85 => Self::Fx6P3,
            86 => Self::Fx6P4,
            87 => Self::Fx7P1,
            88 => Self::Fx7P2,
            89 => Self::Fx7P3,
            90 => Self::Fx7P4,
            91 => Self::Fx8P1,
            92 => Self::Fx8P2,
            93 => Self::Fx8P3,
            _ => Self::Fx8P4,
        })
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
            _ => Self::ModSeq4Rate,
        }
    }

    #[inline]
    pub const fn fx_param(slot: usize, param: usize) -> Self {
        match (slot, param) {
            (0, 0) => Self::Fx1P1,
            (0, 1) => Self::Fx1P2,
            (0, 2) => Self::Fx1P3,
            (0, 3) => Self::Fx1P4,
            (1, 0) => Self::Fx2P1,
            (1, 1) => Self::Fx2P2,
            (1, 2) => Self::Fx2P3,
            (1, 3) => Self::Fx2P4,
            (2, 0) => Self::Fx3P1,
            (2, 1) => Self::Fx3P2,
            (2, 2) => Self::Fx3P3,
            (2, 3) => Self::Fx3P4,
            (3, 0) => Self::Fx4P1,
            (3, 1) => Self::Fx4P2,
            (3, 2) => Self::Fx4P3,
            (3, 3) => Self::Fx4P4,
            (4, 0) => Self::Fx5P1,
            (4, 1) => Self::Fx5P2,
            (4, 2) => Self::Fx5P3,
            (4, 3) => Self::Fx5P4,
            (5, 0) => Self::Fx6P1,
            (5, 1) => Self::Fx6P2,
            (5, 2) => Self::Fx6P3,
            (5, 3) => Self::Fx6P4,
            (6, 0) => Self::Fx7P1,
            (6, 1) => Self::Fx7P2,
            (6, 2) => Self::Fx7P3,
            (6, 3) => Self::Fx7P4,
            (7, 0) => Self::Fx8P1,
            (7, 1) => Self::Fx8P2,
            (7, 2) => Self::Fx8P3,
            _ => Self::Fx8P4,
        }
    }

    #[inline]
    pub const fn is_fx_slot(self, slot: usize) -> bool {
        let idx = self as usize;
        let base = (Self::Fx1P1 as usize) + slot * 4;
        slot < 8 && idx >= base && idx < base + 4
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Osc1WtPos => "OSC1 WT",
            Self::Osc2WtPos => "OSC2 WT",
            Self::Osc3WtPos => "OSC3 WT",
            Self::Osc4WtPos => "OSC4 WT",
            Self::Osc1Warp => "OSC1 WARP",
            Self::Osc2Warp => "OSC2 WARP",
            Self::Osc3Warp => "OSC3 WARP",
            Self::Osc4Warp => "OSC4 WARP",
            Self::Osc1ModDepth => "OSC1 MOD",
            Self::Osc2ModDepth => "OSC2 MOD",
            Self::Osc3ModDepth => "OSC3 MOD",
            Self::Osc4ModDepth => "OSC4 MOD",
            Self::Osc1Pan => "OSC1 PAN",
            Self::Osc2Pan => "OSC2 PAN",
            Self::Osc3Pan => "OSC3 PAN",
            Self::Osc4Pan => "OSC4 PAN",
            Self::Osc1Level => "OSC1 LVL",
            Self::Osc2Level => "OSC2 LVL",
            Self::Osc3Level => "OSC3 LVL",
            Self::Osc4Level => "OSC4 LVL",
            Self::Osc1Pitch => "OSC1 PITCH",
            Self::Osc2Pitch => "OSC2 PITCH",
            Self::Osc3Pitch => "OSC3 PITCH",
            Self::Osc4Pitch => "OSC4 PITCH",
            Self::Osc1Phase => "OSC1 PHS",
            Self::Osc2Phase => "OSC2 PHS",
            Self::Osc3Phase => "OSC3 PHS",
            Self::Osc4Phase => "OSC4 PHS",
            Self::Osc1Detune => "OSC1 DET",
            Self::Osc2Detune => "OSC2 DET",
            Self::Osc3Detune => "OSC3 DET",
            Self::Osc4Detune => "OSC4 DET",
            Self::Osc1Spread => "OSC1 SPR",
            Self::Osc2Spread => "OSC2 SPR",
            Self::Osc3Spread => "OSC3 SPR",
            Self::Osc4Spread => "OSC4 SPR",
            Self::NoiseColor => "NSE CLR",
            Self::NoiseStereo => "NSE ST",
            Self::NoiseLevel => "NSE LVL",
            Self::Filter1Cutoff => "F1 CUT",
            Self::Filter1Res => "F1 RES",
            Self::Filter1Drive => "F1 DRV",
            Self::Filter1Aux => "F1 AUX",
            Self::Filter2Cutoff => "F2 CUT",
            Self::Filter2Res => "F2 RES",
            Self::Filter2Drive => "F2 DRV",
            Self::Filter2Aux => "F2 AUX",
            Self::Insert1Drive => "INS1 DRV",
            Self::Insert1Mix => "INS1 MIX",
            Self::Insert2Drive => "INS2 DRV",
            Self::Insert2Mix => "INS2 MIX",
            Self::Osc1FilterSend => "O1 SEND",
            Self::Osc2FilterSend => "O2 SEND",
            Self::Osc3FilterSend => "O3 SEND",
            Self::Osc4FilterSend => "O4 SEND",
            Self::NoiseFilterSend => "NS SEND",
            Self::FilterMix => "F1/2 MIX",
            Self::FeedbackAmount => "FEEDBACK",
            Self::GlideMs => "GLIDE",
            Self::ModSeq1Rate => "MS1 RATE",
            Self::ModSeq2Rate => "MS2 RATE",
            Self::ModSeq3Rate => "MS3 RATE",
            Self::ModSeq4Rate => "MS4 RATE",
            Self::Fx1P1 => "FX1 P1",
            Self::Fx1P2 => "FX1 P2",
            Self::Fx1P3 => "FX1 P3",
            Self::Fx1P4 => "FX1 P4",
            Self::Fx2P1 => "FX2 P1",
            Self::Fx2P2 => "FX2 P2",
            Self::Fx2P3 => "FX2 P3",
            Self::Fx2P4 => "FX2 P4",
            Self::Fx3P1 => "FX3 P1",
            Self::Fx3P2 => "FX3 P2",
            Self::Fx3P3 => "FX3 P3",
            Self::Fx3P4 => "FX3 P4",
            Self::Fx4P1 => "FX4 P1",
            Self::Fx4P2 => "FX4 P2",
            Self::Fx4P3 => "FX4 P3",
            Self::Fx4P4 => "FX4 P4",
            Self::Fx5P1 => "FX5 P1",
            Self::Fx5P2 => "FX5 P2",
            Self::Fx5P3 => "FX5 P3",
            Self::Fx5P4 => "FX5 P4",
            Self::Fx6P1 => "FX6 P1",
            Self::Fx6P2 => "FX6 P2",
            Self::Fx6P3 => "FX6 P3",
            Self::Fx6P4 => "FX6 P4",
            Self::Fx7P1 => "FX7 P1",
            Self::Fx7P2 => "FX7 P2",
            Self::Fx7P3 => "FX7 P3",
            Self::Fx7P4 => "FX7 P4",
            Self::Fx8P1 => "FX8 P1",
            Self::Fx8P2 => "FX8 P2",
            Self::Fx8P3 => "FX8 P3",
            Self::Fx8P4 => "FX8 P4",
        }
    }
}

/// Single modulation routing slot inside the fixed 32-slot matrix.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ModSlot {
    pub source: ModMatrixSource,
    pub destination: ModDestination,
    pub depth: f32,
    pub bipolar: bool,
    pub enabled: bool,
    #[serde(default)]
    pub aux_source: ModMatrixSource,
    #[serde(default)]
    pub curve: f32,
}

/// Pre-allocated fixed-size 32-slot modulation matrix (`Copy`, lock-free swap & persistence).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ModMatrixState {
    pub slots: [ModSlot; NUM_MOD_SLOTS],
}

impl Default for ModMatrixState {
    fn default() -> Self {
        Self {
            slots: [ModSlot::default(); NUM_MOD_SLOTS],
        }
    }
}

impl ModMatrixState {
    /// Finds an active slot matching `(source, destination)` or allocates the first free slot.
    pub fn assign_or_update(
        &mut self,
        source: ModMatrixSource,
        destination: ModDestination,
        default_depth: f32,
    ) -> Option<usize> {
        if source == ModMatrixSource::None {
            return None;
        }
        if let Some(idx) = self
            .slots
            .iter()
            .position(|s| s.enabled && s.source == source && s.destination == destination)
        {
            return Some(idx);
        }
        if let Some(free_idx) = self
            .slots
            .iter()
            .position(|s| !s.enabled || s.source == ModMatrixSource::None)
        {
            self.slots[free_idx] = ModSlot {
                source,
                destination,
                depth: default_depth.clamp(-1.0, 1.0),
                bipolar: false,
                enabled: true,
                aux_source: ModMatrixSource::None,
                curve: 0.0,
            };
            return Some(free_idx);
        }
        None
    }

    pub fn clear_slot(&mut self, slot_idx: usize) {
        if slot_idx < NUM_MOD_SLOTS {
            self.slots[slot_idx] = ModSlot::default();
        }
    }

    pub fn swap_slots(&mut self, a: usize, b: usize) {
        if a < NUM_MOD_SLOTS && b < NUM_MOD_SLOTS && a != b {
            self.slots.swap(a, b);
        }
    }

    /// Evaluates all 32 slots for a voice's current `[0..1]` source values and accumulates
    /// per-destination normalized offsets into `out_deltas`. Zero heap allocations.
    #[inline]
    pub fn evaluate_destinations(
        &self,
        source_values: &[f32; NUM_ACTIVE_SOURCES],
        out_deltas: &mut [f32; NUM_MOD_DESTS],
    ) {
        out_deltas.fill(0.0);
        for slot in &self.slots {
            if !slot.enabled {
                continue;
            }
            if let Some(src_idx) = slot.source.active_index() {
                let mut raw = source_values[src_idx].clamp(0.0, 1.0);
                let curve = slot.curve.clamp(-1.0, 1.0);
                if curve.abs() > 1e-4 {
                    let exp = (curve * 2.5).exp();
                    raw = raw.powf(exp);
                }
                let mut sig = if slot.bipolar {
                    raw * 2.0 - 1.0
                } else {
                    raw
                };
                if let Some(aux_idx) = slot.aux_source.active_index() {
                    sig *= source_values[aux_idx].clamp(0.0, 1.0);
                }
                let dest_idx = slot.destination.index();
                if dest_idx < NUM_MOD_DESTS {
                    out_deltas[dest_idx] += sig * slot.depth.clamp(-1.0, 1.0);
                }
            }
        }
    }
}

pub const FX_SCOPE_LEN: usize = 128;

/// Lock-free atomic ring buffer storing downsampled pre/post peak waveforms & activity for an FX slot.
pub struct FxScopeRing {
    pre_peaks: [AtomicU32; FX_SCOPE_LEN],
    post_peaks: [AtomicU32; FX_SCOPE_LEN],
    write_idx: AtomicU32,
    activity_bits: AtomicU32,
}

impl Default for FxScopeRing {
    fn default() -> Self {
        Self::new()
    }
}

impl FxScopeRing {
    pub fn new() -> Self {
        Self {
            pre_peaks: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            post_peaks: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            write_idx: AtomicU32::new(0),
            activity_bits: AtomicU32::new(0.0_f32.to_bits()),
        }
    }

    #[inline]
    pub fn push(&self, pre_peak: f32, post_peak: f32, activity: f32) {
        let idx = (self.write_idx.load(Ordering::Relaxed) as usize) % FX_SCOPE_LEN;
        self.pre_peaks[idx].store(pre_peak.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
        self.post_peaks[idx].store(post_peak.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
        self.write_idx
            .store(((idx + 1) % FX_SCOPE_LEN) as u32, Ordering::Relaxed);
        self.activity_bits
            .store(activity.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    #[inline]
    pub fn clear(&self) {
        for v in &self.pre_peaks {
            v.store(0.0_f32.to_bits(), Ordering::Relaxed);
        }
        for v in &self.post_peaks {
            v.store(0.0_f32.to_bits(), Ordering::Relaxed);
        }
        self.activity_bits
            .store(0.0_f32.to_bits(), Ordering::Relaxed);
    }

    pub fn read_snapshot(
        &self,
        pre_out: &mut [f32; FX_SCOPE_LEN],
        post_out: &mut [f32; FX_SCOPE_LEN],
    ) -> f32 {
        let head = (self.write_idx.load(Ordering::Relaxed) as usize) % FX_SCOPE_LEN;
        for i in 0..FX_SCOPE_LEN {
            let idx = (head + i) % FX_SCOPE_LEN;
            pre_out[i] = f32::from_bits(self.pre_peaks[idx].load(Ordering::Relaxed));
            post_out[i] = f32::from_bits(self.post_peaks[idx].load(Ordering::Relaxed));
        }
        f32::from_bits(self.activity_bits.load(Ordering::Relaxed))
    }
}

/// Lock-free atomic telemetry published by the audio thread for 60 Hz UI mod-ring animations.
pub struct ModUiTelemetry {
    pub dest_offsets: [AtomicU32; NUM_MOD_DESTS],
    pub source_levels: [AtomicU32; NUM_ACTIVE_SOURCES],
    pub modseq_phases: [AtomicU32; 4],
    pub env_levels: [AtomicU32; 4],
    /// Post-modulation `[cutoff_hz, resonance, drive, aux]` for `FILTER 1` and `FILTER 2`.
    pub filter_live: [[AtomicU32; 4]; 2],
    pub fx_scopes: [FxScopeRing; 8],
    pub master_peak_l: AtomicU32,
    pub master_peak_r: AtomicU32,
    pub master_rms_l: AtomicU32,
    pub master_rms_r: AtomicU32,
    pub master_clip_l: AtomicU32,
    pub master_clip_r: AtomicU32,
}

impl Default for ModUiTelemetry {
    fn default() -> Self {
        Self::new()
    }
}

impl ModUiTelemetry {
    pub fn new() -> Self {
        Self {
            dest_offsets: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            source_levels: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            modseq_phases: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            env_levels: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            filter_live: std::array::from_fn(|_| {
                [
                    AtomicU32::new(0.0_f32.to_bits()),
                    AtomicU32::new(0.0_f32.to_bits()),
                    AtomicU32::new(0.0_f32.to_bits()),
                    AtomicU32::new(0.0_f32.to_bits()),
                ]
            }),
            fx_scopes: std::array::from_fn(|_| FxScopeRing::new()),
            master_peak_l: AtomicU32::new(0.0_f32.to_bits()),
            master_peak_r: AtomicU32::new(0.0_f32.to_bits()),
            master_rms_l: AtomicU32::new(0.0_f32.to_bits()),
            master_rms_r: AtomicU32::new(0.0_f32.to_bits()),
            master_clip_l: AtomicU32::new(0),
            master_clip_r: AtomicU32::new(0),
        }
    }

    #[inline]
    pub fn store_master_meters(
        &self,
        peak_l: f32,
        peak_r: f32,
        rms_l: f32,
        rms_r: f32,
        clip_l: bool,
        clip_r: bool,
    ) {
        self.master_peak_l.store(peak_l.to_bits(), Ordering::Relaxed);
        self.master_peak_r.store(peak_r.to_bits(), Ordering::Relaxed);
        self.master_rms_l.store(rms_l.to_bits(), Ordering::Relaxed);
        self.master_rms_r.store(rms_r.to_bits(), Ordering::Relaxed);
        if clip_l {
            self.master_clip_l.store(1, Ordering::Relaxed);
        }
        if clip_r {
            self.master_clip_r.store(1, Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn master_meters(&self) -> (f32, f32, f32, f32, bool, bool) {
        (
            f32::from_bits(self.master_peak_l.load(Ordering::Relaxed)),
            f32::from_bits(self.master_peak_r.load(Ordering::Relaxed)),
            f32::from_bits(self.master_rms_l.load(Ordering::Relaxed)),
            f32::from_bits(self.master_rms_r.load(Ordering::Relaxed)),
            self.master_clip_l.load(Ordering::Relaxed) != 0,
            self.master_clip_r.load(Ordering::Relaxed) != 0,
        )
    }

    #[inline]
    pub fn reset_master_clip(&self) {
        self.master_clip_l.store(0, Ordering::Relaxed);
        self.master_clip_r.store(0, Ordering::Relaxed);
    }

    #[inline]
    pub fn store_filter_live(
        &self,
        idx: usize,
        cutoff_hz: f32,
        resonance: f32,
        drive: f32,
        aux: f32,
    ) {
        if idx < 2 {
            self.filter_live[idx][0].store(cutoff_hz.to_bits(), Ordering::Relaxed);
            self.filter_live[idx][1].store(resonance.to_bits(), Ordering::Relaxed);
            self.filter_live[idx][2].store(drive.to_bits(), Ordering::Relaxed);
            self.filter_live[idx][3].store(aux.to_bits(), Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn filter_live(&self, idx: usize) -> [f32; 4] {
        if idx < 2 {
            [
                f32::from_bits(self.filter_live[idx][0].load(Ordering::Relaxed)),
                f32::from_bits(self.filter_live[idx][1].load(Ordering::Relaxed)),
                f32::from_bits(self.filter_live[idx][2].load(Ordering::Relaxed)),
                f32::from_bits(self.filter_live[idx][3].load(Ordering::Relaxed)),
            ]
        } else {
            [0.0; 4]
        }
    }

    #[inline]
    pub fn store_voice_snapshot(
        &self,
        dest_offsets: &[f32; NUM_MOD_DESTS],
        source_levels: &[f32; NUM_ACTIVE_SOURCES],
        modseq_phases: &[f32; 4],
        env_levels: &[f32; 4],
    ) {
        for (dst, &val) in self.dest_offsets.iter().zip(dest_offsets.iter()) {
            dst.store(val.to_bits(), Ordering::Relaxed);
        }
        for (dst, &val) in self.source_levels.iter().zip(source_levels.iter()) {
            dst.store(val.to_bits(), Ordering::Relaxed);
        }
        for (dst, &val) in self.modseq_phases.iter().zip(modseq_phases.iter()) {
            dst.store(val.to_bits(), Ordering::Relaxed);
        }
        for (dst, &val) in self.env_levels.iter().zip(env_levels.iter()) {
            dst.store(val.to_bits(), Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn dest_offset(&self, dest: ModDestination) -> f32 {
        f32::from_bits(self.dest_offsets[dest.index()].load(Ordering::Relaxed))
    }

    #[inline]
    pub fn source_level(&self, src: ModMatrixSource) -> f32 {
        if let Some(idx) = src.active_index() {
            f32::from_bits(self.source_levels[idx].load(Ordering::Relaxed))
        } else {
            0.0
        }
    }

    #[inline]
    pub fn modseq_phase(&self, idx: usize) -> f32 {
        if idx < 4 {
            f32::from_bits(self.modseq_phases[idx].load(Ordering::Relaxed))
        } else {
            0.0
        }
    }

    #[inline]
    pub fn env_level(&self, idx: usize) -> f32 {
        if idx < 4 {
            f32::from_bits(self.env_levels[idx].load(Ordering::Relaxed))
        } else {
            0.0
        }
    }
}
