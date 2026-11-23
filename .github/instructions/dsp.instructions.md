---
applyTo: "crates/j-syn-dsp/**"
---

# DSP Review Checklist (`crates/j-syn-dsp/`)

1. **Zero Audio-Thread Allocations or Drops:** No `Vec`, `String`, `Box`, `format!`, `.collect()`, `Arc::new`, heap deallocation/drop, or blocking locks reachable from `process*`. All delay lines, filter stages, and voice states must be pre-allocated — or lazily provisioned via a wait-free command/garbage ring (`FxEngineSwapQueue`) with both allocation and drop happening strictly on the UI or `JSynBgTask` background thread (never on the audio thread).
2. **Hot-Path Data Locality:** Keep per-voice modulation tables split from global FX destinations (`NUM_VOICE_MOD_DESTS` vs. `NUM_MOD_DESTS`) and pass compact control override structs (`VoiceControlOverrides`) instead of copying full `SynthParams` per sample.
3. **Denormal & Feedback Safety:** Every feedback loop, IIR state, and delay/reverb line must flush denormals (`flush_denormal`), clamp non-finite inputs/outputs, and cleanly decay to idle (`track_idle` / voice `auto_sleep`).
4. **Sample-Rate & Chunk-Size Invariance:** Filter coefficients, slew times, LFO phase increments, and delay lengths must derive from `sample_rate`. Changes should behave identically across `1`, `7`, `64`, and `513`-sample block splits and `44.1 / 48 / 96 / 192 kHz` (as validated by `pluginval --strictness-level 8` and `clap-validator`).
5. **No `nih_plug` Dependency:** `crates/j-syn-dsp` must stay completely independent of `nih_plug` and Vizia.
6. **Test Coverage:** New DSP behavior or bugfixes must include targeted invariant tests in `crates/j-syn-dsp/tests/dsp_tests.rs` or `crates/j-syn-dsp/tests/smoke_ci.rs` (and stay clean under `tests/rt_no_alloc.rs`).
