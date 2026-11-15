//! `j-syn`: Zero-allocation 16-voice polyphonic wavetable synthesizer (VST3 + CLAP).

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use j_syn_dsp::{
    EngineMode, FactoryWavetable, FilterMode, FilterParams, FilterRouting, FilterSlope,
    InsertMode, InsertParams, InsertPosition, ModOrder, ModSource, ModType, NoiseParams,
    NoiseType, OscParams, PolyMode, RoutingParams, SynthEngine, SynthParams, WarpMode, Wavetable,
    WavetableBank,
};
use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;

mod editor;

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavetableChoice {
    #[name = "Basic Shapes"]
    BasicShapes,
    #[name = "Digital Formant"]
    DigitalFormant,
    #[name = "Harmonic Sync"]
    HarmonicSync,
    #[name = "Aggressive PWM"]
    AggressivePwm,
    #[name = "Custom .WAV"]
    Custom,
}

impl WavetableChoice {
    #[inline]
    pub fn factory_preset(self) -> Option<FactoryWavetable> {
        match self {
            Self::BasicShapes => Some(FactoryWavetable::BasicShapes),
            Self::DigitalFormant => Some(FactoryWavetable::DigitalFormant),
            Self::HarmonicSync => Some(FactoryWavetable::HarmonicSync),
            Self::AggressivePwm => Some(FactoryWavetable::AggressivePwm),
            Self::Custom => None,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModOrderParam {
    #[name = "PM>W"]
    PmThenWarp,
    #[name = "W>PM"]
    WarpThenPm,
}

impl From<ModOrderParam> for ModOrder {
    fn from(val: ModOrderParam) -> Self {
        match val {
            ModOrderParam::PmThenWarp => ModOrder::PmThenWarp,
            ModOrderParam::WarpThenPm => ModOrder::WarpThenPm,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarpModeParam {
    #[name = "None"]
    None,
    #[name = "Bend-"]
    BendMinus,
    #[name = "Bend+"]
    BendPlus,
    #[name = "Spectrum"]
    Spectrum,
}

impl From<WarpModeParam> for WarpMode {
    fn from(val: WarpModeParam) -> Self {
        match val {
            WarpModeParam::None => WarpMode::None,
            WarpModeParam::BendMinus => WarpMode::BendMinus,
            WarpModeParam::BendPlus => WarpMode::BendPlus,
            WarpModeParam::Spectrum => WarpMode::Spectrum,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSourceParam {
    #[name = "Off"]
    Off,
    #[name = "Osc 1"]
    Osc1,
    #[name = "Osc 2"]
    Osc2,
    #[name = "Osc 3"]
    Osc3,
    #[name = "Osc 4"]
    Osc4,
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModTypeParam {
    #[name = "PM"]
    Pm,
    #[name = "RM"]
    Rm,
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseTypeParam {
    #[name = "White"]
    White,
    #[name = "Pink"]
    Pink,
    #[name = "Brown"]
    Brown,
}

impl From<NoiseTypeParam> for NoiseType {
    fn from(val: NoiseTypeParam) -> Self {
        match val {
            NoiseTypeParam::White => NoiseType::White,
            NoiseTypeParam::Pink => NoiseType::Pink,
            NoiseTypeParam::Brown => NoiseType::Brown,
        }
    }
}

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
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterSlopeParam {
    #[name = "6 dB"]
    Db6,
    #[name = "12 dB"]
    Db12,
    #[name = "24 dB"]
    Db24,
    #[name = "36 dB"]
    Db36,
    #[name = "48 dB"]
    Db48,
}

impl From<FilterSlopeParam> for FilterSlope {
    fn from(val: FilterSlopeParam) -> Self {
        match val {
            FilterSlopeParam::Db6 => FilterSlope::Db6,
            FilterSlopeParam::Db12 => FilterSlope::Db12,
            FilterSlopeParam::Db24 => FilterSlope::Db24,
            FilterSlopeParam::Db36 => FilterSlope::Db36,
            FilterSlopeParam::Db48 => FilterSlope::Db48,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterRoutingParam {
    #[name = "Serial (F1->F2)"]
    Serial,
    #[name = "Parallel (F1+F2)"]
    Parallel,
}

impl From<FilterRoutingParam> for FilterRouting {
    fn from(val: FilterRoutingParam) -> Self {
        match val {
            FilterRoutingParam::Serial => FilterRouting::Serial,
            FilterRoutingParam::Parallel => FilterRouting::Parallel,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertModeParam {
    #[name = "Off"]
    Off,
    #[name = "Tube"]
    Tube,
    #[name = "Mech"]
    Mech,
    #[name = "Tube -> Mech"]
    TubeToMech,
    #[name = "Mech -> Tube"]
    MechToTube,
    #[name = "Sine Shaper"]
    SineShaper,
    #[name = "Parabolic"]
    Parabolic,
    #[name = "Soft Clip"]
    SoftClip,
    #[name = "Bitcrush"]
    Bitcrush,
}

impl From<InsertModeParam> for InsertMode {
    fn from(val: InsertModeParam) -> Self {
        match val {
            InsertModeParam::Off => InsertMode::Off,
            InsertModeParam::Tube => InsertMode::Tube,
            InsertModeParam::Mech => InsertMode::Mech,
            InsertModeParam::TubeToMech => InsertMode::TubeToMech,
            InsertModeParam::MechToTube => InsertMode::MechToTube,
            InsertModeParam::SineShaper => InsertMode::SineShaper,
            InsertModeParam::Parabolic => InsertMode::Parabolic,
            InsertModeParam::SoftClip => InsertMode::SoftClip,
            InsertModeParam::Bitcrush => InsertMode::Bitcrush,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertPosParam {
    #[name = "Pre-F1"]
    PreFilter,
    #[name = "Between F1->F2"]
    BetweenFilters,
    #[name = "Post-F2"]
    PostFilter,
}

impl From<InsertPosParam> for InsertPosition {
    fn from(val: InsertPosParam) -> Self {
        match val {
            InsertPosParam::PreFilter => InsertPosition::PreFilter,
            InsertPosParam::BetweenFilters => InsertPosition::BetweenFilters,
            InsertPosParam::PostFilter => InsertPosition::PostFilter,
        }
    }
}

fn percentage_param(name: &str, default_val: f32) -> FloatParam {
    FloatParam::new(name, default_val, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_unit(" %")
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
}

fn bipolar_percentage_param(name: &str, default_val: f32) -> FloatParam {
    FloatParam::new(
        name,
        default_val,
        FloatRange::Linear {
            min: -1.0,
            max: 1.0,
        },
    )
    .with_unit(" %")
    .with_value_to_string(formatters::v2s_f32_percentage(0))
    .with_string_to_value(formatters::s2v_f32_percentage())
}

#[derive(Params)]
pub struct NoisePluginParams {
    #[id = "noise_enabled"]
    pub enabled: BoolParam,

    #[id = "noise_type"]
    pub noise_type: EnumParam<NoiseTypeParam>,

    #[id = "noise_color"]
    pub color: FloatParam,

    #[id = "noise_stereo"]
    pub stereo: FloatParam,

    #[id = "noise_level"]
    pub level: FloatParam,
}

impl Default for NoisePluginParams {
    fn default() -> Self {
        Self {
            enabled: BoolParam::new("Noise Enabled", false),
            noise_type: EnumParam::new("Noise Type", NoiseTypeParam::White),
            color: bipolar_percentage_param("Noise Color", 0.0),
            stereo: percentage_param("Noise Stereo", 0.5),
            level: percentage_param("Noise Level", 0.5),
        }
    }
}

impl NoisePluginParams {
    pub fn to_dsp(&self) -> NoiseParams {
        NoiseParams {
            enabled: self.enabled.value(),
            noise_type: self.noise_type.value().into(),
            color: self.color.value(),
            stereo: self.stereo.value(),
            level: self.level.value(),
        }
    }
}

#[derive(Params)]
pub struct FilterPluginParams {
    #[id = "flt_enabled"]
    pub enabled: BoolParam,

    #[id = "flt_mode"]
    pub mode: EnumParam<FilterModeParam>,

    #[id = "flt_slope"]
    pub slope: EnumParam<FilterSlopeParam>,

    #[id = "flt_cutoff"]
    pub cutoff: FloatParam,

    #[id = "flt_res"]
    pub resonance: FloatParam,

    #[id = "flt_drive"]
    pub drive: FloatParam,

    #[id = "flt_keytrack"]
    pub keytrack: FloatParam,

    #[id = "flt_aux"]
    pub aux: FloatParam,
}

impl FilterPluginParams {
    fn new(idx: usize) -> Self {
        let default_cutoff = if idx == 0 { 20_000.0 } else { 2_000.0 };
        Self {
            enabled: BoolParam::new("Filter Enabled", idx == 0),
            mode: EnumParam::new("Filter Mode", FilterModeParam::Lp),
            slope: EnumParam::new("Filter Slope", FilterSlopeParam::Db12),
            cutoff: FloatParam::new(
                "Cutoff",
                default_cutoff,
                FloatRange::Skewed {
                    min: 20.0,
                    max: 20_000.0,
                    factor: FloatRange::skew_factor(-2.2),
                },
            )
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
            .with_string_to_value(formatters::s2v_f32_hz_then_khz()),
            resonance: percentage_param("Resonance", 0.0),
            drive: percentage_param("Drive", 0.0),
            keytrack: percentage_param("Keytrack", 0.0),
            aux: percentage_param("Aux", 1.0),
        }
    }

    pub fn to_dsp(&self) -> FilterParams {
        FilterParams {
            enabled: self.enabled.value(),
            mode: self.mode.value().into(),
            slope: self.slope.value().into(),
            cutoff_hz: self.cutoff.value(),
            resonance: self.resonance.value(),
            drive: self.drive.value(),
            keytrack: self.keytrack.value(),
            aux: self.aux.value(),
        }
    }
}

#[derive(Params)]
pub struct InsertSlotPluginParams {
    #[id = "ins_mode"]
    pub mode: EnumParam<InsertModeParam>,

    #[id = "ins_pos"]
    pub position: EnumParam<InsertPosParam>,

    #[id = "ins_drive"]
    pub drive: FloatParam,

    #[id = "ins_mix"]
    pub mix: FloatParam,
}

impl InsertSlotPluginParams {
    fn new(idx: usize) -> Self {
        let default_pos = if idx == 0 {
            InsertPosParam::BetweenFilters
        } else {
            InsertPosParam::PostFilter
        };
        Self {
            mode: EnumParam::new("Insert Mode", InsertModeParam::Off),
            position: EnumParam::new("Insert Position", default_pos),
            drive: percentage_param("Insert Drive", 0.25),
            mix: percentage_param("Insert Mix", 1.0),
        }
    }

    pub fn to_dsp(&self) -> InsertParams {
        InsertParams {
            mode: self.mode.value().into(),
            position: self.position.value().into(),
            drive: self.drive.value(),
            mix: self.mix.value(),
        }
    }
}

#[derive(Params)]
pub struct RoutingPluginParams {
    #[id = "osc1_flt_bal"]
    pub osc1_flt_bal: FloatParam,

    #[id = "osc2_flt_bal"]
    pub osc2_flt_bal: FloatParam,

    #[id = "osc3_flt_bal"]
    pub osc3_flt_bal: FloatParam,

    #[id = "osc4_flt_bal"]
    pub osc4_flt_bal: FloatParam,

    #[id = "noise_flt_bal"]
    pub noise_flt_bal: FloatParam,

    #[id = "osc1_direct"]
    pub osc1_direct: BoolParam,

    #[id = "osc2_direct"]
    pub osc2_direct: BoolParam,

    #[id = "osc3_direct"]
    pub osc3_direct: BoolParam,

    #[id = "osc4_direct"]
    pub osc4_direct: BoolParam,

    #[id = "noise_direct"]
    pub noise_direct: BoolParam,

    #[id = "flt_routing"]
    pub filter_routing: EnumParam<FilterRoutingParam>,

    #[id = "flt_mix"]
    pub filter_mix: FloatParam,

    #[nested(array, group = "Inserts")]
    pub inserts: [InsertSlotPluginParams; 2],

    #[id = "fb_amt"]
    pub feedback_amount: FloatParam,
}

impl Default for RoutingPluginParams {
    fn default() -> Self {
        Self {
            osc1_flt_bal: bipolar_percentage_param("OSC 1 Send", -1.0),
            osc2_flt_bal: bipolar_percentage_param("OSC 2 Send", -1.0),
            osc3_flt_bal: bipolar_percentage_param("OSC 3 Send", -1.0),
            osc4_flt_bal: bipolar_percentage_param("OSC 4 Send", -1.0),
            noise_flt_bal: bipolar_percentage_param("Noise Send", -1.0),
            osc1_direct: BoolParam::new("OSC 1 Direct", false),
            osc2_direct: BoolParam::new("OSC 2 Direct", false),
            osc3_direct: BoolParam::new("OSC 3 Direct", false),
            osc4_direct: BoolParam::new("OSC 4 Direct", false),
            noise_direct: BoolParam::new("Noise Direct", false),
            filter_routing: EnumParam::new("Filter Routing", FilterRoutingParam::Serial),
            filter_mix: bipolar_percentage_param("Filter Mix", 0.0),
            inserts: [InsertSlotPluginParams::new(0), InsertSlotPluginParams::new(1)],
            feedback_amount: percentage_param("Feedback", 0.0),
        }
    }
}

impl RoutingPluginParams {
    pub fn to_dsp(&self) -> RoutingParams {
        RoutingParams {
            osc_filter_balance: [
                self.osc1_flt_bal.value(),
                self.osc2_flt_bal.value(),
                self.osc3_flt_bal.value(),
                self.osc4_flt_bal.value(),
            ],
            noise_filter_balance: self.noise_flt_bal.value(),
            osc_direct: [
                self.osc1_direct.value(),
                self.osc2_direct.value(),
                self.osc3_direct.value(),
                self.osc4_direct.value(),
            ],
            noise_direct: self.noise_direct.value(),
            filter_routing: self.filter_routing.value().into(),
            filter_mix: self.filter_mix.value(),
            inserts: [self.inserts[0].to_dsp(), self.inserts[1].to_dsp()],
            feedback_amount: self.feedback_amount.value(),
        }
    }
}

#[derive(Params)]
pub struct OscPluginParams {
    #[id = "enabled"]
    pub enabled: BoolParam,

    #[id = "wt_select"]
    pub wt_select: EnumParam<WavetableChoice>,

    #[id = "wt_pos"]
    pub wt_pos: FloatParam,

    #[id = "octave"]
    pub octave: IntParam,

    #[id = "semi"]
    pub semi: IntParam,

    #[id = "fine"]
    pub fine: FloatParam,

    #[id = "phase"]
    pub phase: FloatParam,

    #[id = "retrig"]
    pub retrig: BoolParam,

    #[id = "warp_mode"]
    pub warp_mode: EnumParam<WarpModeParam>,

    #[id = "warp_amt"]
    pub warp_amount: FloatParam,

    #[id = "mod_src"]
    pub mod_source: EnumParam<ModSourceParam>,

    #[id = "mod_type"]
    pub mod_type: EnumParam<ModTypeParam>,

    #[id = "mod_depth"]
    pub mod_depth: FloatParam,

    #[id = "mod_order"]
    pub mod_order: EnumParam<ModOrderParam>,

    #[id = "unison"]
    pub unison: IntParam,

    #[id = "detune"]
    pub detune: FloatParam,

    #[id = "spread"]
    pub spread: FloatParam,

    #[id = "pan"]
    pub pan: FloatParam,

    #[id = "level"]
    pub level: FloatParam,

    #[id = "keytrack"]
    pub keytrack: BoolParam,
}

impl OscPluginParams {
    fn new(osc_idx: usize) -> Self {
        let default_wt = match osc_idx {
            0 => WavetableChoice::BasicShapes,
            1 => WavetableChoice::DigitalFormant,
            2 => WavetableChoice::HarmonicSync,
            _ => WavetableChoice::AggressivePwm,
        };

        Self {
            enabled: BoolParam::new("Enabled", osc_idx == 0),
            wt_select: EnumParam::new("Wavetable", default_wt),
            wt_pos: FloatParam::new("WT Pos", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit(" %")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            octave: IntParam::new("Octave", 0, IntRange::Linear { min: -3, max: 3 }),
            semi: IntParam::new("Semi", 0, IntRange::Linear { min: -12, max: 12 }),
            fine: FloatParam::new(
                "Fine",
                0.0,
                FloatRange::Linear {
                    min: -100.0,
                    max: 100.0,
                },
            )
            .with_unit(" ct")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            phase: FloatParam::new(
                "Phase",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 360.0,
                },
            )
            .with_unit("°")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            retrig: BoolParam::new("Retrig", true),
            warp_mode: EnumParam::new("Warp Mode", WarpModeParam::None),
            warp_amount: FloatParam::new(
                "Warp Amt",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),
            mod_source: EnumParam::new("Mod Src", ModSourceParam::Off),
            mod_type: EnumParam::new("Mod Type", ModTypeParam::Pm),
            mod_depth: FloatParam::new(
                "Mod Depth",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),
            mod_order: EnumParam::new("Mod Order", ModOrderParam::PmThenWarp),
            unison: IntParam::new("Unison", 1, IntRange::Linear { min: 1, max: 8 }),
            detune: FloatParam::new("Detune", 0.2, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit(" %")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            spread: FloatParam::new("Spread", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit(" %")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            pan: FloatParam::new(
                "Pan",
                0.0,
                FloatRange::Linear {
                    min: -1.0,
                    max: 1.0,
                },
            )
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),
            level: FloatParam::new("Level", 0.75, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit(" %")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            keytrack: BoolParam::new("Keytrack", true),
        }
    }

    fn to_dsp(&self) -> OscParams {
        OscParams {
            enabled: self.enabled.value(),
            wt_pos: self.wt_pos.value(),
            octave: self.octave.value(),
            semi: self.semi.value(),
            fine_cents: self.fine.value(),
            phase_offset: (self.phase.value() / 360.0).rem_euclid(1.0).fract(),
            retrig: self.retrig.value(),
            warp_mode: match self.warp_mode.value() {
                WarpModeParam::None => WarpMode::None,
                WarpModeParam::BendMinus => WarpMode::BendMinus,
                WarpModeParam::BendPlus => WarpMode::BendPlus,
                WarpModeParam::Spectrum => WarpMode::Spectrum,
            },
            warp_amount: self.warp_amount.value(),
            mod_source: match self.mod_source.value() {
                ModSourceParam::Off => ModSource::Off,
                ModSourceParam::Osc1 => ModSource::Osc1,
                ModSourceParam::Osc2 => ModSource::Osc2,
                ModSourceParam::Osc3 => ModSource::Osc3,
                ModSourceParam::Osc4 => ModSource::Osc4,
            },
            mod_type: match self.mod_type.value() {
                ModTypeParam::Pm => ModType::Pm,
                ModTypeParam::Rm => ModType::Rm,
            },
            mod_depth: self.mod_depth.value(),
            mod_order: self.mod_order.value().into(),
            unison: self.unison.value().clamp(1, 8) as usize,
            detune: self.detune.value(),
            spread: self.spread.value(),
            pan: self.pan.value(),
            level: self.level.value(),
            keytrack: self.keytrack.value(),
        }
    }
}

#[derive(Params)]
pub struct JSynPluginParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    #[persist = "wavetable_paths"]
    pub wavetable_paths: Arc<Mutex<[String; 4]>>,

    #[id = "master_vol"]
    pub master_volume_db: FloatParam,

    #[id = "hq_mode"]
    pub hq_mode: BoolParam,

    #[id = "mono_legato"]
    pub mono_legato: BoolParam,

    #[id = "glide_ms"]
    pub glide_ms: FloatParam,

    #[id = "pb_range"]
    pub pitch_bend_range: IntParam,

    #[id = "amp_atk"]
    pub amp_attack: FloatParam,

    #[id = "amp_dec"]
    pub amp_decay: FloatParam,

    #[id = "amp_sus"]
    pub amp_sustain: FloatParam,

    #[id = "amp_rel"]
    pub amp_release: FloatParam,

    #[nested(array, group = "Oscillators")]
    pub oscs: [OscPluginParams; 4],

    #[nested(group = "Noise")]
    pub noise: NoisePluginParams,

    #[nested(array, group = "Filters")]
    pub filters: [FilterPluginParams; 2],

    #[nested(group = "Routing")]
    pub routing: RoutingPluginParams,
}

impl Default for JSynPluginParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),
            wavetable_paths: Arc::new(Mutex::new([
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ])),
            master_volume_db: FloatParam::new(
                "Master Vol",
                -3.0,
                FloatRange::Linear {
                    min: -24.0,
                    max: 6.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            hq_mode: BoolParam::new("HQ Mode", false),
            mono_legato: BoolParam::new("Mono Legato", false),

            glide_ms: FloatParam::new(
                "Glide",
                0.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 500.0,
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            pitch_bend_range: IntParam::new(
                "PB Range",
                2,
                IntRange::Linear { min: 1, max: 24 },
            )
            .with_unit(" st"),

            amp_attack: FloatParam::new(
                "Attack",
                0.005,
                FloatRange::Skewed {
                    min: 0.001,
                    max: 5.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_unit(" s")
            .with_value_to_string(formatters::v2s_f32_rounded(3)),

            amp_decay: FloatParam::new(
                "Decay",
                0.25,
                FloatRange::Skewed {
                    min: 0.002,
                    max: 5.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_unit(" s")
            .with_value_to_string(formatters::v2s_f32_rounded(3)),

            amp_sustain: FloatParam::new(
                "Sustain",
                1.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            amp_release: FloatParam::new(
                "Release",
                0.20,
                FloatRange::Skewed {
                    min: 0.002,
                    max: 8.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_unit(" s")
            .with_value_to_string(formatters::v2s_f32_rounded(3)),

            oscs: [
                OscPluginParams::new(0),
                OscPluginParams::new(1),
                OscPluginParams::new(2),
                OscPluginParams::new(3),
            ],
            noise: NoisePluginParams::default(),
            filters: [
                FilterPluginParams::new(0),
                FilterPluginParams::new(1),
            ],
            routing: RoutingPluginParams::default(),
        }
    }
}

pub struct JSynPlugin {
    params: Arc<JSynPluginParams>,
    engine: SynthEngine,
    wavetable_bank: Arc<WavetableBank>,
}

impl Default for JSynPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(JSynPluginParams::default()),
            engine: SynthEngine::new(44100.0),
            wavetable_bank: Arc::new(WavetableBank::new()),
        }
    }
}

impl JSynPlugin {
    fn reload_persisted_wavetables(&self) {
        let paths = {
            let guard = self.params.wavetable_paths.lock().unwrap();
            guard.clone()
        };
        for (idx, path_str) in paths.into_iter().enumerate() {
            if !path_str.is_empty() && self.params.oscs[idx].wt_select.value() == WavetableChoice::Custom {
                let bank = self.wavetable_bank.clone();
                let path = PathBuf::from(path_str);
                std::thread::spawn(move || {
                    if let Ok(wt) = Wavetable::load_from_wav(&path) {
                        bank.swap_custom(idx, Arc::new(wt));
                    }
                });
            }
        }
    }
}

impl Plugin for JSynPlugin {
    const NAME: &'static str = "J-SYN";
    const VENDOR: &'static str = "Jake Hukkanen";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(
            self.params.clone(),
            self.params.editor_state.clone(),
            self.wavetable_bank.clone(),
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.engine.set_sample_rate(buffer_config.sample_rate);
        self.engine.reset();
        self.reload_persisted_wavetables();
        true
    }

    fn reset(&mut self) {
        self.engine.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let num_samples = buffer.samples();
        if num_samples == 0 {
            return ProcessStatus::Normal;
        }

        let synth_params = SynthParams {
            master_gain: util::db_to_gain(self.params.master_volume_db.value()),
            engine_mode: if self.params.hq_mode.value() {
                EngineMode::Hq
            } else {
                EngineMode::Vintage2007
            },
            poly_mode: if self.params.mono_legato.value() {
                PolyMode::MonoLegato
            } else {
                PolyMode::Poly
            },
            glide_ms: self.params.glide_ms.value(),
            pitch_bend_range: self.params.pitch_bend_range.value() as f32,
            amp_attack_s: self.params.amp_attack.value(),
            amp_decay_s: self.params.amp_decay.value(),
            amp_sustain: self.params.amp_sustain.value(),
            amp_release_s: self.params.amp_release.value(),
            oscs: [
                self.params.oscs[0].to_dsp(),
                self.params.oscs[1].to_dsp(),
                self.params.oscs[2].to_dsp(),
                self.params.oscs[3].to_dsp(),
            ],
            noise: self.params.noise.to_dsp(),
            filters: [
                self.params.filters[0].to_dsp(),
                self.params.filters[1].to_dsp(),
            ],
            routing: self.params.routing.to_dsp(),
        };

        let choices = [
            self.params.oscs[0].wt_select.value(),
            self.params.oscs[1].wt_select.value(),
            self.params.oscs[2].wt_select.value(),
            self.params.oscs[3].wt_select.value(),
        ];
        let g0 = if choices[0].factory_preset().is_none() {
            Some(nih_plug::util::permit_alloc(|| {
                self.wavetable_bank.osc_tables[0].load()
            }))
        } else {
            None
        };
        let g1 = if choices[1].factory_preset().is_none() {
            Some(nih_plug::util::permit_alloc(|| {
                self.wavetable_bank.osc_tables[1].load()
            }))
        } else {
            None
        };
        let g2 = if choices[2].factory_preset().is_none() {
            Some(nih_plug::util::permit_alloc(|| {
                self.wavetable_bank.osc_tables[2].load()
            }))
        } else {
            None
        };
        let g3 = if choices[3].factory_preset().is_none() {
            Some(nih_plug::util::permit_alloc(|| {
                self.wavetable_bank.osc_tables[3].load()
            }))
        } else {
            None
        };
        let tables: [&Wavetable; 4] = [
            match choices[0].factory_preset() {
                Some(p) => self.wavetable_bank.factory_table(p),
                None => g0.as_deref().unwrap(),
            },
            match choices[1].factory_preset() {
                Some(p) => self.wavetable_bank.factory_table(p),
                None => g1.as_deref().unwrap(),
            },
            match choices[2].factory_preset() {
                Some(p) => self.wavetable_bank.factory_table(p),
                None => g2.as_deref().unwrap(),
            },
            match choices[3].factory_preset() {
                Some(p) => self.wavetable_bank.factory_table(p),
                None => g3.as_deref().unwrap(),
            },
        ];

        let channels = buffer.as_slice();
        if channels.len() < 2 {
            return ProcessStatus::Normal;
        }
        let (left_slice, right_rest) = channels.split_at_mut(1);
        let left = &mut left_slice[0][..num_samples];
        let right = &mut right_rest[0][..num_samples];

        let mut next_event = context.next_event();
        for sample_idx in 0..num_samples {
            while let Some(event) = next_event {
                if event.timing() as usize > sample_idx {
                    break;
                }

                match event {
                    NoteEvent::NoteOn {
                        channel,
                        note,
                        velocity,
                        voice_id,
                        ..
                    } => {
                        self.engine
                            .note_on(channel, note, velocity, voice_id, &synth_params);
                    }
                    NoteEvent::NoteOff {
                        channel,
                        note,
                        voice_id,
                        ..
                    } => {
                        self.engine
                            .note_off(channel, note, voice_id, &synth_params);
                    }
                    NoteEvent::Choke {
                        channel,
                        note,
                        voice_id,
                        ..
                    } => {
                        self.engine.choke(channel, note, voice_id);
                    }
                    NoteEvent::MidiPitchBend { value, .. } => {
                        self.engine.set_pitch_bend(value);
                    }
                    NoteEvent::MidiCC { cc, value, .. } => {
                        // CC 120: All Sound Off, CC 123: All Notes Off
                        if cc == 120 && value >= 0.0 {
                            self.engine.all_sound_off();
                        } else if cc == 123 && value >= 0.0 {
                            self.engine.all_notes_off();
                        }
                    }
                    _ => {}
                }

                next_event = context.next_event();
            }

            let (l, r) = self.engine.process_sample_with_tables(&synth_params, tables);
            left[sample_idx] = l;
            right[sample_idx] = r;
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for JSynPlugin {
    const CLAP_ID: &'static str = "com.jakehukkanen.j-syn";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("16-voice polyphonic wavetable synthesizer with 4 oscillators and cross-PM");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for JSynPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"JH_JSynWavetable";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Instrument,
        Vst3SubCategory::Synth,
    ];
}

nih_export_clap!(JSynPlugin);
nih_export_vst3!(JSynPlugin);
