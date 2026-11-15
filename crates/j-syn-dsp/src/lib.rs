//! `j-syn-dsp`: Zero-allocation 16-voice polyphonic wavetable synthesizer DSP engine.

pub mod adaa;
pub mod engine;
pub mod envelope;
pub mod filter;
pub mod insert;
pub mod mod_matrix;
pub mod modseq;
pub mod noise;
pub mod oscillator;
pub mod oversample;
pub mod voice;
pub mod wavetable;

pub use engine::{SynthEngine, MAX_TAIL_VOICES, MAX_VOICES};
pub use envelope::{evaluate_adsr_preview, AdsrParams, AdsrStage, AmpAdsr};
pub use filter::{
    evaluate_filter_response_db, fast_tanh, FilterMode, FilterParams, FilterSlope, FilterState,
    FilterSvfCore, StereoZdfSvf, COMB_BUF_LEN,
};
pub use insert::{
    FilterRouting, InsertMode, InsertParams, InsertPosition, InsertShaperState, InsertState,
    RoutingParams, VoiceFeedbackState,
};
pub use mod_matrix::{
    ModDestination, ModMatrixSource, ModMatrixState, ModSlot, ModUiTelemetry, NUM_ACTIVE_SOURCES,
    NUM_MOD_DESTS, NUM_MOD_SLOTS,
};
pub use modseq::{
    evaluate_modseq_curve, evaluate_mseg_curve, evaluate_step_shape, LfoPreset, ModSeqMode,
    ModSeqParams, ModSeqPlayMode, ModSeqRate, ModSeqRetrig, ModSeqShapePreset, ModSeqState,
    ModSeqStep, ModSeqStepBank, ModSeqStepShape, ModSeqSyncMode, MsegCurve, MsegNode,
    MAX_MODSEQ_STEPS, MAX_MSEG_NODES,
};
pub use noise::{NoiseParams, NoiseState, NoiseType};
pub use oscillator::{
    sample_with_warp_and_pm, warp_phase_bend, EngineMode, ModOrder, ModSource, ModType, OscParams,
    OscillatorState, WarpMode, MAX_UNISON,
};
pub use voice::{PolyMode, SynthParams, TailVoice, Voice};
pub use wavetable::{
    FactoryWavetable, Wavetable, WavetableBank, FRAME_LEN, MAX_FRAMES, NUM_FACTORY_WAVETABLES,
    NUM_MIP_LEVELS,
};
