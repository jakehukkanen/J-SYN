//! Parallel Dual-Bus FX, Sub Bus, and FX slot parameters.

use j_syn_dsp::{FxFilterSlope, SubBusParams};
use nih_plug::prelude::*;

use super::{bipolar_percentage_param, fx_level_param, FilterModeParam};

#[derive(Params)]
pub struct FxBusPluginParams {
    #[id = "lvl"]
    pub level: FloatParam,

    #[id = "pan"]
    pub pan: FloatParam,

    #[nested(array, group = "Slots")]
    pub slots: [FxSlotPluginParams; 8],
}

impl FxBusPluginParams {
    pub fn new_bus_b() -> Self {
        Self {
            level: fx_level_param("Bus B Level"),
            pan: bipolar_percentage_param("Bus B Pan", 0.0),
            slots: std::array::from_fn(FxSlotPluginParams::new_slot),
        }
    }
}

#[derive(Params)]
pub struct SubBusPluginParams {
    #[id = "en"]
    pub enabled: BoolParam,

    #[id = "cutoff"]
    pub cutoff: FloatParam,

    #[id = "drive"]
    pub drive: FloatParam,

    #[id = "lvl"]
    pub level: FloatParam,
}

impl Default for SubBusPluginParams {
    fn default() -> Self {
        Self {
            enabled: BoolParam::new("Sub Bus", true),
            cutoff: FloatParam::new(
                "Sub LP Cutoff",
                2_000.0,
                FloatRange::Skewed {
                    min: 20.0,
                    max: 2_000.0,
                    factor: FloatRange::skew_factor(-1.8),
                },
            )
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
            .with_string_to_value(formatters::s2v_f32_hz_then_khz()),
            drive: FloatParam::new("Sub Drive", 0.0, FloatRange::Linear { min: 0.0, max: 24.0 })
                .with_unit(" dB")
                .with_value_to_string(formatters::v2s_f32_rounded(1)),
            level: fx_level_param("Sub Level"),
        }
    }
}

impl SubBusPluginParams {
    pub fn to_dsp(&self) -> SubBusParams {
        SubBusParams {
            enabled: self.enabled.value(),
            lp_cutoff_hz: self.cutoff.value(),
            clip_drive_db: self.drive.value(),
            level: self.level.value(),
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxTypeParam {
    #[name = "Empty"]
    Off,
    #[name = "OTT"]
    Ott,
    #[name = "Distortion"]
    Distortion,
    #[name = "Chorus"]
    Chorus,
    #[name = "Delay"]
    Delay,
    #[name = "Reverb"]
    Reverb,
    #[name = "Clipper"]
    JSatClipper,
    #[name = "Filter"]
    Filter,
    #[name = "Phaser"]
    Phaser,
    #[name = "Diopser"]
    Diopser,
}

impl From<j_syn_dsp::FxType> for FxTypeParam {
    fn from(v: j_syn_dsp::FxType) -> Self {
        match v {
            j_syn_dsp::FxType::Off => Self::Off,
            j_syn_dsp::FxType::Ott => Self::Ott,
            j_syn_dsp::FxType::Distortion => Self::Distortion,
            j_syn_dsp::FxType::Chorus => Self::Chorus,
            j_syn_dsp::FxType::Delay => Self::Delay,
            j_syn_dsp::FxType::Reverb => Self::Reverb,
            j_syn_dsp::FxType::JSatClipper => Self::JSatClipper,
            j_syn_dsp::FxType::Filter => Self::Filter,
            j_syn_dsp::FxType::Phaser => Self::Phaser,
            j_syn_dsp::FxType::Diopser => Self::Diopser,
        }
    }
}

impl From<FxTypeParam> for j_syn_dsp::FxType {
    fn from(v: FxTypeParam) -> Self {
        match v {
            FxTypeParam::Off => Self::Off,
            FxTypeParam::Ott => Self::Ott,
            FxTypeParam::Distortion => Self::Distortion,
            FxTypeParam::Chorus => Self::Chorus,
            FxTypeParam::Delay => Self::Delay,
            FxTypeParam::Reverb => Self::Reverb,
            FxTypeParam::JSatClipper => Self::JSatClipper,
            FxTypeParam::Filter => Self::Filter,
            FxTypeParam::Phaser => Self::Phaser,
            FxTypeParam::Diopser => Self::Diopser,
        }
    }
}

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

#[derive(Params)]
pub struct FxSlotPluginParams {
    #[id = "fx_en"]
    pub enabled: BoolParam,
    #[id = "fx_type"]
    pub fx_type: EnumParam<FxTypeParam>,
    #[id = "fx_stack"]
    pub stack: IntParam,
    #[id = "fx_p1"]
    pub p1: FloatParam,
    #[id = "fx_p2"]
    pub p2: FloatParam,
    #[id = "fx_p3"]
    pub p3: FloatParam,
    #[id = "fx_p4"]
    pub p4: FloatParam,
    #[id = "fx_p5"]
    pub p5: FloatParam,
    #[id = "fx_p6"]
    pub p6: FloatParam,
    #[id = "fx_p7"]
    pub p7: FloatParam,
    #[id = "fx_p8"]
    pub p8: FloatParam,
    #[id = "fx_flt_m"]
    pub filter_mode: EnumParam<FilterModeParam>,
    #[id = "fx_flt_s"]
    pub filter_slope: EnumParam<FxFilterSlopeParam>,
    #[id = "fx_diop_s"]
    pub diopser_stages: EnumParam<DiopserStagesParam>,
}

impl FxSlotPluginParams {
    pub fn new_slot(slot_idx: usize) -> Self {
        let init = j_syn_dsp::FxSlotParams::init_patch_slot(slot_idx);
        Self {
            enabled: BoolParam::new("Enabled", init.enabled),
            fx_type: EnumParam::new("Type", FxTypeParam::from(init.fx_type)),
            stack: IntParam::new(
                "Stack",
                init.stack as i32,
                IntRange::Linear {
                    min: 1,
                    max: j_syn_dsp::MAX_FX_STACK as i32,
                },
            ),
            p1: FloatParam::new("Param 1", init.params[0], FloatRange::Linear { min: 0.0, max: 1.0 }),
            p2: FloatParam::new("Param 2", init.params[1], FloatRange::Linear { min: 0.0, max: 1.0 }),
            p3: FloatParam::new("Param 3", init.params[2], FloatRange::Linear { min: 0.0, max: 1.0 }),
            p4: FloatParam::new("Param 4", init.params[3], FloatRange::Linear { min: 0.0, max: 1.0 }),
            p5: FloatParam::new("Param 5", init.eq_params[0], FloatRange::Linear { min: 0.0, max: 1.0 }),
            p6: FloatParam::new("Param 6", init.eq_params[1], FloatRange::Linear { min: 0.0, max: 1.0 }),
            p7: FloatParam::new("Param 7", init.eq_params[2], FloatRange::Linear { min: 0.0, max: 1.0 }),
            p8: FloatParam::new("Param 8", init.eq_params[3], FloatRange::Linear { min: 0.0, max: 1.0 }),
            filter_mode: EnumParam::new("Filter Mode", FilterModeParam::Lp),
            filter_slope: EnumParam::new("Filter Slope", FxFilterSlopeParam::Db12),
            diopser_stages: EnumParam::new("Diopser Stages", DiopserStagesParam::S32),
        }
    }

    pub fn params_array(&self) -> [&FloatParam; 4] {
        [&self.p1, &self.p2, &self.p3, &self.p4]
    }

    pub fn eq_params_array(&self) -> [&FloatParam; 4] {
        [&self.p5, &self.p6, &self.p7, &self.p8]
    }

    pub fn param_at(&self, idx: usize) -> &FloatParam {
        match idx {
            0 => &self.p1,
            1 => &self.p2,
            2 => &self.p3,
            3 => &self.p4,
            4 => &self.p5,
            5 => &self.p6,
            6 => &self.p7,
            _ => &self.p8,
        }
    }

    pub fn to_dsp(&self) -> j_syn_dsp::FxSlotParams {
        j_syn_dsp::FxSlotParams {
            fx_type: self.fx_type.value().into(),
            enabled: self.enabled.value(),
            stack: self.stack.value().clamp(1, 4) as u8,
            params: [
                self.p1.value(),
                self.p2.value(),
                self.p3.value(),
                self.p4.value(),
            ],
            eq_params: [
                self.p5.value(),
                self.p6.value(),
                self.p7.value(),
                self.p8.value(),
            ],
            filter_mode: self.filter_mode.value().into(),
            filter_slope: self.filter_slope.value().into(),
            diopser_stages: self.diopser_stages.value().to_index(),
        }
    }
}
