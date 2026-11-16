//! Envelope and MODSEQ / LFO modulator parameters.

use j_syn_dsp::{
    AdsrParams, ModSeqMode, ModSeqParams, ModSeqPlayMode, ModSeqRetrig, ModSeqSyncMode,
};
use nih_plug::prelude::*;

use super::{env_time_param, percentage_param};

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSeqSyncParam {
    #[name = "Free"]
    Free,
    #[name = "Straight"]
    Straight,
    #[name = "Triplet"]
    Triplet,
    #[name = "Dotted"]
    Dotted,
}

impl From<ModSeqSyncParam> for ModSeqSyncMode {
    fn from(val: ModSeqSyncParam) -> Self {
        match val {
            ModSeqSyncParam::Free => ModSeqSyncMode::Free,
            ModSeqSyncParam::Straight => ModSeqSyncMode::Straight,
            ModSeqSyncParam::Triplet => ModSeqSyncMode::Triplet,
            ModSeqSyncParam::Dotted => ModSeqSyncMode::Dotted,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSeqPlayParam {
    #[name = "Loop"]
    Loop,
    #[name = "1-Shot"]
    OneShot,
    #[name = "PingPong"]
    PingPong,
}

impl From<ModSeqPlayParam> for ModSeqPlayMode {
    fn from(val: ModSeqPlayParam) -> Self {
        match val {
            ModSeqPlayParam::Loop => ModSeqPlayMode::Loop,
            ModSeqPlayParam::OneShot => ModSeqPlayMode::OneShot,
            ModSeqPlayParam::PingPong => ModSeqPlayMode::PingPong,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSeqModeParam {
    #[name = "MODSEQ"]
    Performer,
    #[name = "LFO"]
    Mseg,
}

impl From<ModSeqModeParam> for ModSeqMode {
    fn from(val: ModSeqModeParam) -> Self {
        match val {
            ModSeqModeParam::Performer => ModSeqMode::Performer,
            ModSeqModeParam::Mseg => ModSeqMode::Mseg,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSeqRetrigParam {
    #[name = "Poly"]
    PolyVoice,
    #[name = "Global"]
    MonoGlobal,
}

impl From<ModSeqRetrigParam> for ModSeqRetrig {
    fn from(val: ModSeqRetrigParam) -> Self {
        match val {
            ModSeqRetrigParam::PolyVoice => ModSeqRetrig::PolyVoice,
            ModSeqRetrigParam::MonoGlobal => ModSeqRetrig::MonoGlobal,
        }
    }
}

#[derive(Params)]
pub struct ModEnvPluginParams {
    #[id = "atk"]
    pub attack: FloatParam,

    #[id = "dec"]
    pub decay: FloatParam,

    #[id = "sus"]
    pub sustain: FloatParam,

    #[id = "rel"]
    pub release: FloatParam,
}

impl Default for ModEnvPluginParams {
    fn default() -> Self {
        let d = AdsrParams::default();
        Self {
            attack: env_time_param("Attack", d.attack_s, 0.001, 5.0),
            decay: env_time_param("Decay", d.decay_s, 0.002, 5.0),
            sustain: percentage_param("Sustain", d.sustain),
            release: env_time_param("Release", d.release_s, 0.002, 8.0),
        }
    }
}

impl ModEnvPluginParams {
    pub fn to_dsp(&self) -> AdsrParams {
        AdsrParams {
            attack_s: self.attack.value(),
            decay_s: self.decay.value(),
            sustain: self.sustain.value(),
            release_s: self.release.value(),
        }
    }
}

#[derive(Params)]
pub struct ModSeqPluginParams {
    #[id = "ms_mode"]
    pub mode: EnumParam<ModSeqModeParam>,

    #[id = "ms_sync"]
    pub sync_mode: EnumParam<ModSeqSyncParam>,

    #[id = "ms_rate"]
    pub rate: FloatParam,

    #[id = "ms_play"]
    pub play_mode: EnumParam<ModSeqPlayParam>,

    #[id = "ms_retrig"]
    pub retrig: EnumParam<ModSeqRetrigParam>,

    #[id = "ms_steps"]
    pub num_steps: IntParam,

    #[id = "ms_smooth"]
    pub smooth: FloatParam,
}

impl Default for ModSeqPluginParams {
    fn default() -> Self {
        Self {
            mode: EnumParam::new("Mode", ModSeqModeParam::Performer),
            sync_mode: EnumParam::new("Sync", ModSeqSyncParam::Straight),
            rate: percentage_param("Rate", 0.35),
            play_mode: EnumParam::new("Play Mode", ModSeqPlayParam::Loop),
            retrig: EnumParam::new("Retrig", ModSeqRetrigParam::PolyVoice),
            num_steps: IntParam::new("Steps", 8, IntRange::Linear { min: 2, max: 16 }),
            smooth: percentage_param("Smooth", 0.05),
        }
    }
}

impl ModSeqPluginParams {
    pub fn to_dsp(&self, groove_swing: bool) -> ModSeqParams {
        ModSeqParams {
            mode: self.mode.value().into(),
            sync_mode: self.sync_mode.value().into(),
            rate_norm: self.rate.value(),
            play_mode: self.play_mode.value().into(),
            retrig: self.retrig.value().into(),
            num_steps: self.num_steps.value().clamp(2, 16) as usize,
            smooth: self.smooth.value(),
            rate_mod: 0.0,
            groove_swing,
        }
    }
}
