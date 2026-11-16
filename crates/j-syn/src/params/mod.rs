//! `nih-plug` parameter definitions, nested racks, and persisted plugin state.

use std::sync::{Arc, Mutex};

use arc_swap::ArcSwap;
use j_syn_dsp::{ModMatrixState, ModSeqStepBank};
use nih_plug::params::persist::PersistentField;
use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;
use serde::{de::DeserializeOwned, Serialize};

use crate::editor;

pub mod fx;
pub mod mod_params;
pub mod osc;

pub use fx::*;
pub use mod_params::*;
pub use osc::*;


/// Lock-free `ArcSwap<T>` wrapper persisted by `nih-plug` via JSON (`#[persist = "..."]`).
pub struct PersistedArcSwap<T>(pub Arc<ArcSwap<T>>);

impl<T> Clone for PersistedArcSwap<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T: Default> Default for PersistedArcSwap<T> {
    fn default() -> Self {
        Self(Arc::new(ArcSwap::from_pointee(T::default())))
    }
}

impl<T: Copy> PersistedArcSwap<T> {
    #[inline]
    pub fn snapshot(&self) -> T {
        let guard = self.0.load();
        **guard
    }

    #[inline]
    pub fn store_value(&self, val: T) {
        self.0.store(Arc::new(val));
    }
}

impl<'a, T> PersistentField<'a, T> for PersistedArcSwap<T>
where
    T: Copy + Default + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    fn set(&self, new_value: T) {
        self.0.store(Arc::new(new_value));
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&T) -> R,
    {
        let guard = self.0.load();
        f(&**guard)
    }
}

pub(super) fn env_time_param(name: &str, default_val: f32, min: f32, max: f32) -> FloatParam {
    FloatParam::new(
        name,
        default_val,
        FloatRange::Skewed {
            min,
            max,
            factor: FloatRange::skew_factor(-2.0),
        },
    )
    .with_unit(" s")
    .with_value_to_string(formatters::v2s_f32_rounded(3))
}

pub(super) fn percentage_param(name: &str, default_val: f32) -> FloatParam {
    FloatParam::new(name, default_val, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_unit(" %")
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
}

pub(super) fn bipolar_percentage_param(name: &str, default_val: f32) -> FloatParam {
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

pub(super) fn fx_level_param(name: &str) -> FloatParam {
    FloatParam::new(name, 1.0, FloatRange::Linear { min: 0.0, max: 2.0 })
        .with_unit(" %")
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
}

#[derive(Params)]
pub struct JSynPluginParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    #[persist = "wavetable_paths"]
    pub wavetable_paths: Arc<Mutex<[String; 4]>>,

    #[persist = "mod_matrix"]
    pub mod_matrix: PersistedArcSwap<ModMatrixState>,

    #[persist = "modseq_steps"]
    pub modseq_steps: PersistedArcSwap<ModSeqStepBank>,

    #[id = "master_vol"]
    pub master_volume_db: FloatParam,

    #[id = "dc_blocker"]
    pub dc_blocker: BoolParam,

    #[id = "master_limiter"]
    pub master_limiter: BoolParam,

    #[id = "modseq_swing"]
    pub modseq_swing: BoolParam,

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

    #[nested(array, group = "Mod Envelopes")]
    pub mod_envs: [ModEnvPluginParams; 3],

    #[nested(array, group = "Mod Sequencers")]
    pub modseqs: [ModSeqPluginParams; 4],

    #[nested(array, group = "Master FX")]
    pub fx_slots: [FxSlotPluginParams; 8],

    #[id = "bus_a_lvl"]
    pub bus_a_level: FloatParam,

    #[id = "bus_a_pan"]
    pub bus_a_pan: FloatParam,

    #[nested(id_prefix = "bus_b", group = "Bus B FX")]
    pub fx_bus_b: FxBusPluginParams,

    #[id = "fx_bus_topo"]
    pub fx_bus_topology: EnumParam<FxBusTopologyParam>,

    #[nested(id_prefix = "sub", group = "Sub Bus")]
    pub sub_bus: SubBusPluginParams,

    #[nested(array, group = "Macros")]
    pub macros: [MacroPluginParams; 8],

    #[persist = "macro_names"]
    pub macro_names: Arc<Mutex<[String; 8]>>,
}

impl JSynPluginParams {
    #[inline]
    pub fn fx_bus_slots(&self, bus: usize) -> &[FxSlotPluginParams; 8] {
        if bus == 0 {
            &self.fx_slots
        } else {
            &self.fx_bus_b.slots
        }
    }

    #[inline]
    pub fn fx_bus_level_param(&self, bus: usize) -> &FloatParam {
        if bus == 0 {
            &self.bus_a_level
        } else {
            &self.fx_bus_b.level
        }
    }

    #[inline]
    pub fn fx_bus_pan_param(&self, bus: usize) -> &FloatParam {
        if bus == 0 {
            &self.bus_a_pan
        } else {
            &self.fx_bus_b.pan
        }
    }
}

#[derive(Params)]
pub struct MacroPluginParams {
    #[id = "val"]
    pub value: FloatParam,
}

impl MacroPluginParams {
    pub fn new(idx: usize) -> Self {
        Self {
            value: FloatParam::new(
                format!("Macro {}", idx + 1),
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),
        }
    }
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
            mod_matrix: PersistedArcSwap::default(),
            modseq_steps: PersistedArcSwap::default(),
            master_volume_db: FloatParam::new(
                "Master Vol",
                0.0,
                FloatRange::Linear {
                    min: -24.0,
                    max: 6.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            dc_blocker: BoolParam::new("DC Blocker", true),
            master_limiter: BoolParam::new("Safety Limiter", false),
            modseq_swing: BoolParam::new("ModSeq Groove Swing", false),

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

            amp_attack: env_time_param("Attack", 0.005, 0.001, 5.0),
            amp_decay: env_time_param("Decay", 0.25, 0.002, 5.0),
            amp_sustain: percentage_param("Sustain", 1.0),
            amp_release: env_time_param("Release", 0.20, 0.002, 8.0),

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
            mod_envs: std::array::from_fn(|_| ModEnvPluginParams::default()),
            modseqs: std::array::from_fn(|_| ModSeqPluginParams::default()),
            fx_slots: std::array::from_fn(FxSlotPluginParams::new_slot),
            bus_a_level: fx_level_param("Bus A Level"),
            bus_a_pan: bipolar_percentage_param("Bus A Pan", 0.0),
            fx_bus_b: FxBusPluginParams::new_bus_b(),
            fx_bus_topology: EnumParam::new("FX Bus Routing", FxBusTopologyParam::Parallel),
            sub_bus: SubBusPluginParams::default(),
            macros: std::array::from_fn(MacroPluginParams::new),
            macro_names: Arc::new(Mutex::new(std::array::from_fn(|i| {
                format!("MACRO {}", i + 1)
            }))),
        }
    }
}
