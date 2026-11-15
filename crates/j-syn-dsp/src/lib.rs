//! `j-syn-dsp`: Zero-allocation 16-voice polyphonic wavetable synthesizer DSP engine.

pub mod adaa;
pub mod engine;
pub mod envelope;
pub mod filter;
pub mod insert;
pub mod noise;
pub mod oscillator;
pub mod oversample;
pub mod voice;
pub mod wavetable;

pub use engine::{SynthEngine, MAX_TAIL_VOICES, MAX_VOICES};
pub use envelope::{AdsrStage, AmpAdsr};
pub use filter::{
    evaluate_filter_response_db, fast_tanh, FilterMode, FilterParams, FilterSlope, FilterState,
    FilterSvfCore, StereoZdfSvf, COMB_BUF_LEN,
};
pub use insert::{
    FilterRouting, InsertMode, InsertParams, InsertPosition, InsertShaperState, InsertState,
    RoutingParams, VoiceFeedbackState,
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
