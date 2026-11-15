//! `j-syn-dsp`: Zero-allocation 16-voice polyphonic wavetable synthesizer DSP engine.

pub mod engine;
pub mod envelope;
pub mod oscillator;
pub mod voice;
pub mod wavetable;

pub use engine::{SynthEngine, MAX_TAIL_VOICES, MAX_VOICES};
pub use envelope::{AdsrStage, AmpAdsr};
pub use oscillator::{
    warp_phase_bend, EngineMode, ModSource, ModType, OscParams, OscillatorState, WarpMode,
    MAX_UNISON,
};
pub use voice::{PolyMode, SynthParams, TailVoice, Voice};
pub use wavetable::{
    FactoryWavetable, Wavetable, WavetableBank, FRAME_LEN, MAX_FRAMES, NUM_FACTORY_WAVETABLES,
    NUM_MIP_LEVELS,
};
