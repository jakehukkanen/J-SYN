# Instructions for GitHub Copilot & Review Agents Working on J-SYN

Read `AGENTS.md` and `docs/FX_UNIT_SPEC.md` before reviewing or modifying code in this repository.
J-SYN prioritizes **free/FOSS maintainability, readability, lightweight resource usage, and real-time audio safety**.

Flag any violation of the four sections below as **Blocking**.

---

## 1. Real-Time Audio Thread Safety (`crates/j-syn-dsp/`)

- **Zero audio-thread allocations or blocking:** Reject any heap allocation (`Vec`, `String`, `Box`, `format!`, `.collect()`, `Arc::new`), blocking `Mutex`/`RwLock`, filesystem/network I/O, `Instant::now()`, or `println!`/`dbg!` reachable from `SynthEngine::process*`, `Voice::process*`, `TailVoice::process*`, or `FxRackState::process*`.
- **Numerical & idle hygiene:** Verify denormal flushing (`flush_denormal`), idle sleep recovery (`track_idle`), finite/NaN/Inf guards on feedback paths and resonant filters, and smoothing on discontinuous parameter jumps (gain, pan, mix, pitch).
- **Sample-rate & block-size invariance:** Increments, one-pole coefficients, delay lengths, and LFO rates must derive from `sample_rate` and produce equivalent results whether processed in `1`-sample or `512`-sample chunks.
- **Self-contained FX DSP:** Never couple FX DSP behavior in `crates/j-syn-dsp/src/fx/` to global synth flags (like `hq_mode`/`EngineMode`), so standalone plugins in `crates/plugins/*` and `J-SYN` rack slots stay 1:1.

## 2. Preset & Parameter Stability (`crates/j-syn/src/params/`, `presets.rs`, `crates/j-syn-ui/src/fx_units/params.rs`)

- **Immutable parameter IDs & enum variants:** Never rename existing `#[id = "..."]` strings or reorder/remove existing `#[derive(Enum)]` variants without an explicit backward-compatibility migration or alias in `crates/j-syn/src/presets.rs` (`enum_name_alias` or legacy migration).
- **Deterministic defaults & roundtrip coverage:** Any new parameter or persisted field must preserve deterministic defaults when loading older v1/v2 presets and be covered in `presets.rs` / `tests/preset_and_hygiene.rs`.
- **Shared `nih_plug::Enum` parameters:** `j-syn-dsp` must keep zero dependency on `nih_plug`; shared `nih_plug::Enum` parameter types live in `crates/j-syn-ui/src/fx_units/params.rs` and are re-exported in `crates/j-syn/src/params/`.

## 3. Vizia GUI, Theme & FX Unit Hygiene (`crates/j-syn/src/editor/`, `crates/j-syn-ui/`, `crates/plugins/`)

- **Single 60 Hz timer:** Do not add any new `cx.add_timer(...)` loop; extend the single 60 Hz timer in `crates/j-syn/src/editor/mod_bar.rs`.
- **Split structural vs. value lenses:** Never emit `UiEvent::ModUiChanged` (`mod_ui_rev` structural rebuild) during continuous knob/slider drags inside `Binding::new(..., UiData::mod_ui_rev, ...)`; emit `UiEvent::ModValChanged` so `cx.capture()` widgets are not destroyed mid-drag.
- **Vizia CSS & widget traps:**
  - Never use unsupported Vizia CSS selectors (especially `:focus-within`, which crashes Ableton Live).
  - Never put Unicode arrows (`▲`, `▼`, `◀`, `▶`, `▾`) or child icon views inside `vizia::Button`; use `ArrowButton`, `CrossButton`, or `VectorArrow::caret`.
  - Never apply `.hoverable(false)` to a container wrapping interactive overlay buttons/dropdowns; apply it only to underlying canvas/spacer views.
  - **Never use `.z_index(...)` inside a `ScrollView`** (Vizia hoists `z_index > 0` views above `ScrollView`'s clip rect).
- **Theme-only colors:** Route all `femtovg` and CSS colors through `theme::current_palette(cx)` / `ThemePalette` (zero hardcoded hex/RGB color literals in `draw()` outside `theme.rs`).
- **Naming & tokens:** Reference `j_syn_ui::tokens::*` instead of magic layout numbers. Always name slot effect cards **`FX Unit`** (`fx_unit` / `FX_UNIT_*`), never `snapin`.

## 4. Anti-Slopification & Code Quality

- **No duplicated boilerplate:** Extract shared helpers, loops, or lookup tables instead of copy-pasting FX, oscillator, or knob blocks.
- **No silent fallbacks:** Reject swallowed errors without safe defaults or normalization/clamping mismatches between UI display formatting and DSP math.
- **Module size discipline:** If a PR adds `>150` lines to a file already `>800` lines (`editor/mod.rs`, `mod_bar.rs`, `modseq_canvases.rs`, `engine.rs`, `voice.rs`, `filter.rs`, `fx/mod.rs`, `destinations.rs`), flag it and suggest extracting a cohesive submodule.
- **Invariant-checking tests:** Every new DSP feature or bugfix must ship with unit/smoke tests asserting exact behavioral invariants (finite output, frequency/gain/decay bounds, sample-rate invariance), not merely "output is non-zero".

## 5. Known Intentional Idioms (Do Not Flag as Bugs)

- `nih_plug::util::permit_alloc(|| ...)` wrapping lock-free `ArcSwap::load()` in `crates/j-syn/src/lib.rs` (required by `nih_plug`'s `assert_process_allocs` instrumentation for `ArcSwap` thread-local debt tracking on first read).
- `unsafe` `ParamPtr` calls in `crates/j-syn/src/presets.rs` when resolving/storing patch parameter values via `nih_plug`'s `ParamMap` API (provided safety comments remain intact).
- Zero-allocation `f32`/`f64` fast approximations (`fast_tanh`, `TanhAdaa2`, polyphase IIR half-band oversampler) documented in DSP comments.
