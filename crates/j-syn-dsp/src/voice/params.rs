//! Voice polyphony mode, bus outputs, modulation overrides, and top-level `SynthParams`.

use crate::envelope::AdsrParams;
use crate::filter::FilterParams;
use crate::fx::{FxBusParams, FxBusTopology, FxRackOrder, FxSlotParams, SubBusParams};
use crate::insert::RoutingParams;
use crate::mod_matrix::ModMatrixState;
use crate::modseq::{ModSeqParams, ModSeqStepBank, NUM_MOD_ENVS, NUM_MODSEQS};
use crate::noise::NoiseParams;
use crate::oscillator::{EngineMode, OscParams};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolyMode {
    Poly = 0,
    MonoLegato = 1,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct VoiceBusOutput {
    pub bus_a_l: f32,
    pub bus_a_r: f32,
    pub bus_b_l: f32,
    pub bus_b_r: f32,
    pub sub_l: f32,
    pub sub_r: f32,
}

impl VoiceBusOutput {
    #[inline]
    pub fn scale(self, gain: f32) -> Self {
        Self {
            bus_a_l: self.bus_a_l * gain,
            bus_a_r: self.bus_a_r * gain,
            bus_b_l: self.bus_b_l * gain,
            bus_b_r: self.bus_b_r * gain,
            sub_l: self.sub_l * gain,
            sub_r: self.sub_r * gain,
        }
    }
}

/// Lightweight per-sample smoothed overrides passed into `Voice` / `TailVoice` so `SynthEngine`
/// never copies the ~18 KB `SynthParams` struct per audio sample.
#[derive(Debug, Clone, Copy)]
pub struct VoiceControlOverrides {
    pub osc_wt_pos: [f32; 4],
    pub osc_mod_depth: [f32; 4],
    pub osc_warp_amount: [f32; 4],
    pub filter_cutoff_hz: [f32; 2],
    pub filter_res: [f32; 2],
    pub filter_drive: [f32; 2],
}

impl VoiceControlOverrides {
    #[inline]
    pub fn from_params(params: &SynthParams) -> Self {
        Self {
            osc_wt_pos: std::array::from_fn(|i| params.oscs[i].wt_pos),
            osc_mod_depth: std::array::from_fn(|i| params.oscs[i].mod_depth),
            osc_warp_amount: std::array::from_fn(|i| params.oscs[i].warp_amount),
            filter_cutoff_hz: std::array::from_fn(|i| params.filters[i].cutoff_hz),
            filter_res: std::array::from_fn(|i| params.filters[i].resonance),
            filter_drive: std::array::from_fn(|i| params.filters[i].drive),
        }
    }
}

/// Snapshot of all synthesizer DSP parameters for a processing block.
#[derive(Debug, Clone, Copy)]
pub struct SynthParams {
    pub master_gain: f32,
    pub engine_mode: EngineMode,
    pub poly_mode: PolyMode,
    pub glide_ms: f32,
    pub pitch_bend_range: f32,
    pub mpe_enabled: bool,
    pub mpe_pitch_bend_range: f32,
    pub amp_attack_s: f32,
    pub amp_decay_s: f32,
    pub amp_sustain: f32,
    pub amp_release_s: f32,
    pub oscs: [OscParams; 4],
    pub noise: NoiseParams,
    pub filters: [FilterParams; 2],
    pub routing: RoutingParams,
    pub mod_envs: [AdsrParams; NUM_MOD_ENVS],
    pub modseqs: [ModSeqParams; NUM_MODSEQS],
    pub modseq_steps: ModSeqStepBank,
    pub mod_matrix: ModMatrixState,
    pub macros: [f32; 8],
    pub fx_bus_a: FxBusParams,
    pub fx_bus_b: FxBusParams,
    pub fx_bus_c: FxBusParams,
    pub fx_bus_topology: FxBusTopology,
    pub fx_rack_order: FxRackOrder,
    pub sub_bus: SubBusParams,
    pub tempo_bpm: f32,
    pub dc_blocker: bool,
    pub master_limiter: bool,
}

impl Default for SynthParams {
    fn default() -> Self {
        let mut oscs = [OscParams::default(); 4];
        oscs[0].level = 0.75;
        let init_slots = std::array::from_fn(FxSlotParams::init_patch_slot);
        Self {
            master_gain: 0.707_106_77, // -3 dB
            engine_mode: EngineMode::Vintage2007,
            poly_mode: PolyMode::Poly,
            glide_ms: 0.0,
            pitch_bend_range: 2.0,
            mpe_enabled: false,
            mpe_pitch_bend_range: 48.0,
            amp_attack_s: 0.005,
            amp_decay_s: 0.25,
            amp_sustain: 1.0,
            amp_release_s: 0.20,
            oscs,
            noise: NoiseParams::default(),
            filters: [
                FilterParams::default_filter_1(),
                FilterParams::default_filter_2(),
            ],
            routing: RoutingParams::default(),
            mod_envs: [AdsrParams::default(); NUM_MOD_ENVS],
            modseqs: [ModSeqParams::default(); NUM_MODSEQS],
            modseq_steps: ModSeqStepBank::default(),
            mod_matrix: ModMatrixState::default(),
            macros: [0.0; 8],
            fx_bus_a: FxBusParams {
                slots: init_slots,
                enabled: true,
                muted: false,
                level: 1.0,
                pan: 0.0,
            },
            fx_bus_b: FxBusParams {
                slots: init_slots,
                enabled: true,
                muted: false,
                level: 1.0,
                pan: 0.0,
            },
            fx_bus_c: FxBusParams {
                slots: init_slots,
                enabled: true,
                muted: false,
                level: 1.0,
                pan: 0.0,
            },
            fx_bus_topology: FxBusTopology::Parallel,
            fx_rack_order: FxRackOrder::default(),
            sub_bus: SubBusParams::default(),
            tempo_bpm: 120.0,
            dc_blocker: true,
            master_limiter: false,
        }
    }
}
