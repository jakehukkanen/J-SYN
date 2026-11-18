//! Oscillator, wavetable, noise, filter, insert, and routing parameters.

use std::sync::Arc;

use j_syn_dsp::{
    DirectDest, FactoryWavetable, FilterBusDest, FilterMode, FilterParams, FilterRouting,
    FilterSlope, InsertMode, InsertParams, InsertPosition, ModOrder, ModSource, ModType,
    NoiseParams, NoiseType, OscParams, OscPitchMode, RoutingParams, WarpMode,
};
use nih_plug::prelude::*;

use super::{bipolar_percentage_param, percentage_param};

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PitchModeParam {
    #[name = "Semitones"]
    Semitones,
    #[name = "Ratio"]
    Ratio,
}

impl From<PitchModeParam> for OscPitchMode {
    fn from(val: PitchModeParam) -> Self {
        match val {
            PitchModeParam::Semitones => OscPitchMode::Semitones,
            PitchModeParam::Ratio => OscPitchMode::Ratio,
        }
    }
}

impl From<OscPitchMode> for PitchModeParam {
    fn from(val: OscPitchMode) -> Self {
        match val {
            OscPitchMode::Semitones => PitchModeParam::Semitones,
            OscPitchMode::Ratio => PitchModeParam::Ratio,
        }
    }
}

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
    #[name = "B-"]
    BendMinus,
    #[name = "B+"]
    BendPlus,
    #[name = "SP"]
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
            InsertModeParam::ClassicA => InsertMode::ClassicA,
            InsertModeParam::ClassicB => InsertMode::ClassicB,
            InsertModeParam::ClassicC => InsertMode::ClassicC,
            InsertModeParam::DiodeA => InsertMode::DiodeA,
            InsertModeParam::DiodeB => InsertMode::DiodeB,
            InsertModeParam::LinearFold => InsertMode::LinearFold,
            InsertModeParam::SineFold => InsertMode::SineFold,
            InsertModeParam::HardClip => InsertMode::HardClip,
            InsertModeParam::HalfRectify => InsertMode::HalfRectify,
            InsertModeParam::FullRectify => InsertMode::FullRectify,
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
    pub(crate) fn new(idx: usize) -> Self {
        let default_cutoff = if idx == 0 { 20_000.0 } else { 2_000.0 };
        Self {
            enabled: BoolParam::new("Filter Enabled", false),
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
    pub(crate) fn new(idx: usize) -> Self {
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

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectRouteParam {
    #[name = "Off"]
    Off,
    #[name = "Bus A"]
    BusA,
    #[name = "Bus B"]
    BusB,
    #[name = "Bus C"]
    Sub,
}

impl From<DirectRouteParam> for DirectDest {
    fn from(v: DirectRouteParam) -> Self {
        match v {
            DirectRouteParam::Off => Self::Off,
            DirectRouteParam::BusA => Self::BusA,
            DirectRouteParam::BusB => Self::BusB,
            DirectRouteParam::Sub => Self::Sub,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterBusParam {
    #[name = "Bus A"]
    BusA,
    #[name = "Bus B"]
    BusB,
    #[name = "A+B"]
    Both,
    #[name = "Bus C"]
    Sub,
    #[name = "A+C"]
    AC,
    #[name = "B+C"]
    BC,
    #[name = "A+B+C"]
    All,
}

impl From<FilterBusParam> for FilterBusDest {
    fn from(v: FilterBusParam) -> Self {
        match v {
            FilterBusParam::BusA => Self::BusA,
            FilterBusParam::BusB => Self::BusB,
            FilterBusParam::Both => Self::Both,
            FilterBusParam::Sub => Self::Sub,
            FilterBusParam::AC => Self::AC,
            FilterBusParam::BC => Self::BC,
            FilterBusParam::All => Self::All,
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
    pub osc1_direct: EnumParam<DirectRouteParam>,

    #[id = "osc2_direct"]
    pub osc2_direct: EnumParam<DirectRouteParam>,

    #[id = "osc3_direct"]
    pub osc3_direct: EnumParam<DirectRouteParam>,

    #[id = "osc4_direct"]
    pub osc4_direct: EnumParam<DirectRouteParam>,

    #[id = "noise_direct"]
    pub noise_direct: EnumParam<DirectRouteParam>,

    #[id = "f1_out_bus"]
    pub f1_out_bus: EnumParam<FilterBusParam>,

    #[id = "f2_out_bus"]
    pub f2_out_bus: EnumParam<FilterBusParam>,

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
            osc1_direct: EnumParam::new("OSC 1 Direct", DirectRouteParam::Off),
            osc2_direct: EnumParam::new("OSC 2 Direct", DirectRouteParam::Off),
            osc3_direct: EnumParam::new("OSC 3 Direct", DirectRouteParam::Off),
            osc4_direct: EnumParam::new("OSC 4 Direct", DirectRouteParam::Off),
            noise_direct: EnumParam::new("Noise Direct", DirectRouteParam::Off),
            f1_out_bus: EnumParam::new("Filter 1 Out", FilterBusParam::BusA),
            f2_out_bus: EnumParam::new("Filter 2 Out", FilterBusParam::BusA),
            filter_routing: EnumParam::new("Filter Routing", FilterRoutingParam::Serial),
            filter_mix: bipolar_percentage_param("Filter Mix", 0.0),
            inserts: [InsertSlotPluginParams::new(0), InsertSlotPluginParams::new(1)],
            feedback_amount: percentage_param("Feedback", 0.0),
        }
    }
}

impl RoutingPluginParams {
    #[inline]
    pub fn osc_filter_send_param(&self, idx: usize) -> &FloatParam {
        match idx {
            0 => &self.osc1_flt_bal,
            1 => &self.osc2_flt_bal,
            2 => &self.osc3_flt_bal,
            _ => &self.osc4_flt_bal,
        }
    }

    #[inline]
    pub fn osc_direct_param(&self, idx: usize) -> &EnumParam<DirectRouteParam> {
        match idx {
            0 => &self.osc1_direct,
            1 => &self.osc2_direct,
            2 => &self.osc3_direct,
            _ => &self.osc4_direct,
        }
    }

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
                self.osc1_direct.value().into(),
                self.osc2_direct.value().into(),
                self.osc3_direct.value().into(),
                self.osc4_direct.value().into(),
            ],
            noise_direct: self.noise_direct.value().into(),
            filter_out_bus: [
                self.f1_out_bus.value().into(),
                self.f2_out_bus.value().into(),
            ],
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

    #[id = "pitch"]
    pub pitch: FloatParam,

    #[id = "pitch_mode"]
    pub pitch_mode: EnumParam<PitchModeParam>,

    #[id = "ratio"]
    pub ratio: FloatParam,

    #[id = "hz_offset"]
    pub hz_offset: FloatParam,

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
    pub(crate) fn new(osc_idx: usize) -> Self {
        let default_wt = match osc_idx {
            0 => WavetableChoice::BasicShapes,
            1 => WavetableChoice::DigitalFormant,
            2 => WavetableChoice::HarmonicSync,
            _ => WavetableChoice::AggressivePwm,
        };
        let default_level = if osc_idx == 0 { 0.75 } else { 0.5 };

        Self {
            enabled: BoolParam::new("Enabled", osc_idx == 0),
            wt_select: EnumParam::new("Wavetable", default_wt),
            wt_pos: FloatParam::new("WT Pos", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit(" %")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            pitch: FloatParam::new(
                "Pitch",
                0.0,
                FloatRange::Linear {
                    min: -48.0,
                    max: 48.0,
                },
            )
            .with_unit(" st")
            .with_value_to_string(Arc::new(|val| {
                let clean = if val.abs() < 0.0005 { 0.0 } else { val };
                format!("{clean:.2}")
            }))
            .with_string_to_value(Arc::new(|s| {
                s.trim().trim_end_matches("st").trim().parse::<f32>().ok()
            })),
            pitch_mode: EnumParam::new("Pitch Mode", PitchModeParam::Semitones),
            ratio: FloatParam::new(
                "Ratio",
                1.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 32.0,
                },
            )
            .with_unit(" ×")
            .with_value_to_string(Arc::new(|val| {
                let clean = if val.abs() < 0.00005 {
                    0.0
                } else {
                    val.clamp(0.0, 32.0)
                };
                format!("{clean:.3}")
            }))
            .with_string_to_value(Arc::new(|s| {
                s.trim()
                    .trim_end_matches('×')
                    .trim_end_matches('x')
                    .trim()
                    .parse::<f32>()
                    .ok()
            })),
            hz_offset: FloatParam::new(
                "Hz Offset",
                0.0,
                FloatRange::SymmetricalSkewed {
                    min: -1000.0,
                    max: 1000.0,
                    factor: FloatRange::skew_factor(-2.0),
                    center: 0.0,
                },
            )
            .with_unit(" Hz")
            .with_value_to_string(Arc::new(|val| {
                let clean = if val.abs() < 0.05 { 0.0 } else { val };
                if clean == 0.0 {
                    "0.0".to_string()
                } else if clean.abs() < 10.0 {
                    format!("{clean:+.2}")
                } else if clean.abs() < 100.0 {
                    format!("{clean:+.1}")
                } else {
                    format!("{clean:+.0}")
                }
            }))
            .with_string_to_value(Arc::new(|s| {
                s.trim()
                    .trim_end_matches("Hz")
                    .trim_end_matches("hz")
                    .trim()
                    .parse::<f32>()
                    .ok()
            })),
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
            level: FloatParam::new("Level", default_level, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit(" %")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            keytrack: BoolParam::new("Keytrack", true),
        }
    }

    pub(crate) fn to_dsp(&self) -> OscParams {
        OscParams {
            enabled: self.enabled.value(),
            wt_pos: self.wt_pos.value(),
            pitch_mode: self.pitch_mode.value().into(),
            pitch_semitones: self.pitch.value(),
            ratio: self.ratio.value().clamp(0.0, 32.0),
            hz_offset: self.hz_offset.value(),
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
