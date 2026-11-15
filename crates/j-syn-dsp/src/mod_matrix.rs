//! Lock-free 32-slot Modulation Matrix and real-time UI telemetry primitives.

use std::sync::atomic::{AtomicU32, Ordering};

use serde::{Deserialize, Serialize};

pub const NUM_MOD_SLOTS: usize = 32;
pub const NUM_ACTIVE_SOURCES: usize = 12;
pub const NUM_MOD_DESTS: usize = 63;

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
        }
    }

    #[inline]
    pub fn color_rgb(self) -> (u8, u8, u8) {
        match self {
            Self::None => (140, 125, 110),
            Self::Env1 => (245, 179, 73),     // Warm amber
            Self::Env2 => (108, 212, 255),    // Cyan
            Self::Env3 => (134, 232, 146),    // Mint green
            Self::Env4 => (235, 140, 215),    // Orchid
            Self::ModSeq1 => (255, 206, 84),  // Gold
            Self::ModSeq2 => (240, 128, 96),  // Coral
            Self::ModSeq3 => (162, 152, 255), // Periwinkle
            Self::ModSeq4 => (86, 224, 198),  // Teal
            Self::Velocity => (255, 112, 112),
            Self::Keytrack => (190, 224, 96),
            Self::ModWheel => (255, 156, 200),
            Self::Aftertouch => (255, 168, 76),
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
    Osc1Semi = 20,
    Osc2Semi = 21,
    Osc3Semi = 22,
    Osc4Semi = 23,
    Osc1Fine = 24,
    Osc2Fine = 25,
    Osc3Fine = 26,
    Osc4Fine = 27,
    Osc1Phase = 28,
    Osc2Phase = 29,
    Osc3Phase = 30,
    Osc4Phase = 31,
    Osc1Detune = 32,
    Osc2Detune = 33,
    Osc3Detune = 34,
    Osc4Detune = 35,
    Osc1Spread = 36,
    Osc2Spread = 37,
    Osc3Spread = 38,
    Osc4Spread = 39,
    NoiseColor = 40,
    NoiseStereo = 41,
    NoiseLevel = 42,
    Filter1Cutoff = 43,
    Filter1Res = 44,
    Filter1Drive = 45,
    Filter1Aux = 46,
    Filter2Cutoff = 47,
    Filter2Res = 48,
    Filter2Drive = 49,
    Filter2Aux = 50,
    Insert1Drive = 51,
    Insert1Mix = 52,
    Insert2Drive = 53,
    Insert2Mix = 54,
    Osc1FilterSend = 55,
    Osc2FilterSend = 56,
    Osc3FilterSend = 57,
    Osc4FilterSend = 58,
    NoiseFilterSend = 59,
    FilterMix = 60,
    FeedbackAmount = 61,
    GlideMs = 62,
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
        // Safe exhaustive match over 0..63
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
            20 => Self::Osc1Semi,
            21 => Self::Osc2Semi,
            22 => Self::Osc3Semi,
            23 => Self::Osc4Semi,
            24 => Self::Osc1Fine,
            25 => Self::Osc2Fine,
            26 => Self::Osc3Fine,
            27 => Self::Osc4Fine,
            28 => Self::Osc1Phase,
            29 => Self::Osc2Phase,
            30 => Self::Osc3Phase,
            31 => Self::Osc4Phase,
            32 => Self::Osc1Detune,
            33 => Self::Osc2Detune,
            34 => Self::Osc3Detune,
            35 => Self::Osc4Detune,
            36 => Self::Osc1Spread,
            37 => Self::Osc2Spread,
            38 => Self::Osc3Spread,
            39 => Self::Osc4Spread,
            40 => Self::NoiseColor,
            41 => Self::NoiseStereo,
            42 => Self::NoiseLevel,
            43 => Self::Filter1Cutoff,
            44 => Self::Filter1Res,
            45 => Self::Filter1Drive,
            46 => Self::Filter1Aux,
            47 => Self::Filter2Cutoff,
            48 => Self::Filter2Res,
            49 => Self::Filter2Drive,
            50 => Self::Filter2Aux,
            51 => Self::Insert1Drive,
            52 => Self::Insert1Mix,
            53 => Self::Insert2Drive,
            54 => Self::Insert2Mix,
            55 => Self::Osc1FilterSend,
            56 => Self::Osc2FilterSend,
            57 => Self::Osc3FilterSend,
            58 => Self::Osc4FilterSend,
            59 => Self::NoiseFilterSend,
            60 => Self::FilterMix,
            61 => Self::FeedbackAmount,
            _ => Self::GlideMs,
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
    pub const fn osc_semi(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1Semi,
            1 => Self::Osc2Semi,
            2 => Self::Osc3Semi,
            _ => Self::Osc4Semi,
        }
    }

    #[inline]
    pub const fn osc_fine(idx: usize) -> Self {
        match idx {
            0 => Self::Osc1Fine,
            1 => Self::Osc2Fine,
            2 => Self::Osc3Fine,
            _ => Self::Osc4Fine,
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
            Self::Osc1Semi => "OSC1 SEMI",
            Self::Osc2Semi => "OSC2 SEMI",
            Self::Osc3Semi => "OSC3 SEMI",
            Self::Osc4Semi => "OSC4 SEMI",
            Self::Osc1Fine => "OSC1 FINE",
            Self::Osc2Fine => "OSC2 FINE",
            Self::Osc3Fine => "OSC3 FINE",
            Self::Osc4Fine => "OSC4 FINE",
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
                let raw = source_values[src_idx].clamp(0.0, 1.0);
                let sig = if slot.bipolar {
                    raw * 2.0 - 1.0
                } else {
                    raw
                };
                let dest_idx = slot.destination.index();
                if dest_idx < NUM_MOD_DESTS {
                    out_deltas[dest_idx] += sig * slot.depth.clamp(-1.0, 1.0);
                }
            }
        }
    }
}

/// Lock-free atomic telemetry published by the audio thread for 60 Hz UI mod-ring animations.
pub struct ModUiTelemetry {
    pub dest_offsets: [AtomicU32; NUM_MOD_DESTS],
    pub source_levels: [AtomicU32; NUM_ACTIVE_SOURCES],
    pub modseq_phases: [AtomicU32; 4],
    pub env_levels: [AtomicU32; 4],
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
