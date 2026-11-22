//! Parallel Dual-Bus FX, Sub Bus, and FX slot parameters.

use std::sync::Arc;

use j_syn_dsp::{
    FxBusTopology, MbDistBandParams, SubBusParams, MB_BAND_LEVEL_DEFAULT_NORM,
};
pub use j_syn_ui::fx_units::{
    DiopserStagesParam, DistortionModeParam, FilterModeParam, FxFilterSlopeParam,
};
use nih_plug::prelude::*;

use super::{bipolar_percentage_param, fx_level_param};

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

    pub fn new_bus_c() -> Self {
        Self {
            level: fx_level_param("Bus C Level"),
            pan: bipolar_percentage_param("Bus C Pan", 0.0),
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
            enabled: BoolParam::new("Sub Bus", false),
            cutoff: FloatParam::new(
                "Sub LP Cutoff",
                2_000.0,
                FloatRange::Skewed {
                    min: 20.0,
                    max: 2_000.0,
                    factor: FloatRange::skew_factor(-1.8),
                },
            )
            .with_value_to_string(Arc::new(|hz| {
                if j_syn_dsp::sub_cutoff_hz_to_norm(hz) >= 0.995 {
                    "Off".to_string()
                } else if hz >= 1000.0 {
                    format!("{:.2} kHz", hz / 1000.0)
                } else {
                    format!("{:.0} Hz", hz)
                }
            }))
            .with_string_to_value(Arc::new(|s| {
                let t = s.trim();
                if t.eq_ignore_ascii_case("off") || t.eq_ignore_ascii_case("lp off") {
                    Some(2_000.0)
                } else {
                    formatters::s2v_f32_hz_then_khz()(t)
                }
            })),
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
    #[name = "Dispersion"]
    Diopser,
    #[name = "Tube Res."]
    TubeResonator,
    #[name = "Utility"]
    Utility,
    #[name = "Limiter"]
    Limiter,
    #[name = "MB Dist"]
    MultibandDist,
    #[name = "Flanger"]
    Flanger,
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
            j_syn_dsp::FxType::TubeResonator => Self::TubeResonator,
            j_syn_dsp::FxType::Utility => Self::Utility,
            j_syn_dsp::FxType::Limiter => Self::Limiter,
            j_syn_dsp::FxType::MultibandDist => Self::MultibandDist,
            j_syn_dsp::FxType::Flanger => Self::Flanger,
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
            FxTypeParam::TubeResonator => Self::TubeResonator,
            FxTypeParam::Utility => Self::Utility,
            FxTypeParam::Limiter => Self::Limiter,
            FxTypeParam::MultibandDist => Self::MultibandDist,
            FxTypeParam::Flanger => Self::Flanger,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxBusTopologyParam {
    #[name = "Parallel (A | B | C)"]
    Parallel,
    #[name = "Serial (A -> B | C)"]
    Serial,
    #[name = "Serial All (A -> B -> C)"]
    SerialAll,
    #[name = "A+B -> C"]
    ABToC,
    #[name = "A -> C | B"]
    AToC,
    #[name = "A | B -> C"]
    BToC,
}

impl From<FxBusTopologyParam> for FxBusTopology {
    fn from(v: FxBusTopologyParam) -> Self {
        match v {
            FxBusTopologyParam::Parallel => Self::Parallel,
            FxBusTopologyParam::Serial => Self::Serial,
            FxBusTopologyParam::SerialAll => Self::SerialAll,
            FxBusTopologyParam::ABToC => Self::ABToC,
            FxBusTopologyParam::AToC => Self::AToC,
            FxBusTopologyParam::BToC => Self::BToC,
        }
    }
}

impl From<FxBusTopology> for FxBusTopologyParam {
    fn from(v: FxBusTopology) -> Self {
        match v {
            FxBusTopology::Parallel => Self::Parallel,
            FxBusTopology::Serial => Self::Serial,
            FxBusTopology::SerialAll => Self::SerialAll,
            FxBusTopology::ABToC => Self::ABToC,
            FxBusTopology::AToC => Self::AToC,
            FxBusTopology::BToC => Self::BToC,
        }
    }
}

#[derive(Params)]
pub struct FxMbBandPluginParams {
    #[id = "mb_en"]
    pub enabled: BoolParam,
    #[id = "mb_solo"]
    pub solo: BoolParam,
    #[id = "mb_m"]
    pub mode: EnumParam<DistortionModeParam>,
    #[id = "mb_drv"]
    pub drive: FloatParam,
    #[id = "mb_bias"]
    pub bias: FloatParam,
    #[id = "mb_lvl"]
    pub level: FloatParam,
    #[id = "mb_mix"]
    pub mix: FloatParam,
}

impl FxMbBandPluginParams {
    pub fn new_band(_band_idx: usize) -> Self {
        Self {
            enabled: BoolParam::new("Band Enabled", true),
            solo: BoolParam::new("Band Solo", false),
            mode: EnumParam::new("Band Mode", DistortionModeParam::Tube),
            drive: FloatParam::new("Band Drive", 0.30, FloatRange::Linear { min: 0.0, max: 1.0 }),
            bias: FloatParam::new("Band Bias", 0.50, FloatRange::Linear { min: 0.0, max: 1.0 }),
            level: FloatParam::new(
                "Band Level",
                MB_BAND_LEVEL_DEFAULT_NORM,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
            mix: FloatParam::new("Band Mix", 1.00, FloatRange::Linear { min: 0.0, max: 1.0 }),
        }
    }

    #[inline]
    pub fn param_at(&self, idx: usize) -> &FloatParam {
        match idx {
            0 => &self.drive,
            1 => &self.bias,
            2 => &self.level,
            _ => &self.mix,
        }
    }

    #[inline]
    pub fn to_dsp(&self) -> MbDistBandParams {
        MbDistBandParams {
            mode: self.mode.value().into(),
            enabled: self.enabled.value(),
            solo: self.solo.value(),
            drive: self.drive.value(),
            bias: self.bias.value(),
            level: self.level.value(),
            mix: self.mix.value(),
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
    #[id = "fx_tr_flt"]
    pub tube_res_filter_enabled: BoolParam,
    #[id = "fx_tr_dirty"]
    pub tube_res_dirty_lfo: BoolParam,
    #[id = "fx_util_inv_l"]
    pub utility_inv_l: BoolParam,
    #[id = "fx_util_inv_r"]
    pub utility_inv_r: BoolParam,
    #[id = "fx_lim_tp"]
    pub limiter_true_peak: BoolParam,
    #[id = "fx_clip_pcm"]
    pub clipper_pcm: BoolParam,
    #[id = "fx_dist_m"]
    pub distortion_mode: EnumParam<DistortionModeParam>,
    #[nested(array, group = "MB Bands")]
    pub mb_bands: [Box<FxMbBandPluginParams>; 3],
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
            diopser_stages: EnumParam::new("Dispersion Stages", DiopserStagesParam::S32),
            tube_res_filter_enabled: BoolParam::new("Tube Res. Filter", init.tube_res_filter_enabled),
            tube_res_dirty_lfo: BoolParam::new("Tube Res. Dirty LFO", init.tube_res_dirty_lfo),
            utility_inv_l: BoolParam::new("Utility Inv L", init.utility_inv_l),
            utility_inv_r: BoolParam::new("Utility Inv R", init.utility_inv_r),
            limiter_true_peak: BoolParam::new("Limiter True Peak", init.limiter_true_peak),
            clipper_pcm: BoolParam::new("Clipper PCM", init.clipper_pcm),
            distortion_mode: EnumParam::new("Distortion Mode", DistortionModeParam::Tube),
            mb_bands: std::array::from_fn(|b| Box::new(FxMbBandPluginParams::new_band(b))),
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
            tube_res_filter_enabled: self.tube_res_filter_enabled.value(),
            tube_res_dirty_lfo: self.tube_res_dirty_lfo.value(),
            utility_inv_l: self.utility_inv_l.value(),
            utility_inv_r: self.utility_inv_r.value(),
            limiter_true_peak: self.limiter_true_peak.value(),
            clipper_pcm: self.clipper_pcm.value(),
            distortion_mode: self.distortion_mode.value().into(),
            mb_bands: [
                self.mb_bands[0].to_dsp(),
                self.mb_bands[1].to_dsp(),
                self.mb_bands[2].to_dsp(),
            ],
        }
    }
}
