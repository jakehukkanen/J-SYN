# Instructions for GitHub Copilot & Review Agents Working on J-SYN

Read `AGENTS.md` and `docs/FX_UNIT_SPEC.md` before reviewing or modifying code in this repository.
J-SYN prioritizes **free/FOSS maintainability, readability, lightweight resource usage, and real-time audio safety**.

Flag any violation of the four sections below as **Blocking**.

---

## 1. Real-Time Audio Thread Safety (`crates/j-syn-dsp/`)

- **Zero audio-thread allocations, drops, or blocking:** Reject any heap allocation (`Vec`, `String`, `Box`, `format!`, `.collect()`, `Arc::new`), **heap deallocation/drop** (e.g. dropping `Box`/`ActiveFxEngine` on the audio thread instead of retiring it to a wait-free garbage ring), blocking `Mutex`/`RwLock`, filesystem/network I/O, `Instant::now()`, or `println!`/`dbg!` reachable from `SynthEngine::process*`, `Voice::process*`, `TailVoice::process*`, or `FxRackState::process*`. Any lazy engine provisioning must also work headless via the plugin-owned `FxProvisionWorker` when no GUI is open (for DAW automation / `pluginval --strictness-level 8`).
- **Numerical & idle hygiene:** Verify denormal flushing (`flush_denormal`), idle sleep recovery (`track_idle`), finite/NaN/Inf guards on feedback paths and resonant filters, and smoothing on discontinuous parameter jumps (gain, pan, mix, pitch).
- **Sample-rate & block-size invariance:** Increments, one-pole coefficients, delay lengths, and LFO rates must derive from `sample_rate` and produce equivalent results whether processed in `1`-sample or `512`-sample chunks.
- **Self-contained FX DSP:** Never couple FX DSP behavior in `crates/j-syn-dsp/src/fx/` to global synth flags (like `hq_mode`/`EngineMode`), so standalone plugins in `crates/plugins/*` and `J-SYN` rack slots stay 1:1.

## 2. Preset & Parameter Stability (`crates/j-syn/src/params/`, `presets/`, `crates/j-syn-ui/src/fx_units/params.rs`)

- **Immutable parameter IDs & enum variants:** Never rename existing `#[id = "..."]` strings or reorder/remove existing `#[derive(Enum)]` variants without an explicit backward-compatibility migration or alias in `crates/j-syn/src/presets/migrate.rs` (`enum_name_alias` or legacy migration).
- **Deterministic defaults & roundtrip coverage:** Any new parameter or persisted field must preserve deterministic defaults when loading older v1/v2 presets and be covered in `presets/` / `tests/preset_and_hygiene.rs`.
- **Shared `nih_plug::Enum` parameters:** `j-syn-dsp` must keep zero dependency on `nih_plug`; shared `nih_plug::Enum` parameter types live in `crates/j-syn-ui/src/fx_units/params.rs` and are re-exported in `crates/j-syn/src/params/`.

## 3. Vizia GUI, Theme & FX Unit Hygiene (`crates/j-syn/src/editor/`, `crates/j-syn-ui/`, `crates/plugins/`)

- **Never use `cx.add_timer(...)`:** `vizia_baseview` never calls `cx.process_timers()`, so `cx.add_timer(...)` callbacks silently never fire in VST3/CLAP hosts. Extend `mount_ui_frame_sync` (`FrameSyncView` + `FrameSyncToken` `Binding`) in `crates/j-syn/src/editor/mod_bar.rs` instead.
- **Split structural vs. value lenses:** Never emit `UiEvent::ModUiChanged` (`mod_ui_rev` structural rebuild) during continuous knob/slider drags inside `Binding::new(..., UiData::mod_ui_rev, ...)`; emit `UiEvent::ModValChanged` so `cx.capture()` widgets are not destroyed mid-drag.
- **Vizia CSS, `femtovg` & widget traps:**
  - Never use unsupported Vizia CSS selectors (especially `:focus-within`, which crashes Ableton Live).
  - Never put Unicode arrows (`▲`, `▼`, `◀`, `▶`, `▾`) or child icon views inside `vizia::Button`; use `ArrowButton`, `CrossButton`, or `VectorArrow::caret`.
  - Never apply `.hoverable(false)` to a container wrapping interactive overlay buttons/dropdowns; apply it only to underlying canvas/spacer views.
  - **Never use `.z_index(...)` inside a `ScrollView`** or anywhere under `crates/j-syn/src/editor/fx_rack/` (Vizia hoists `z_index > 0` views above `ScrollView`'s clip rect; place top-level `SelfDirected` overlays at `crates/j-syn/src/editor/` root).
  - **Bind a font before `canvas.fill_text(...)`:** `vg::Paint::color(...)` has no default font (`NoFontFound`); always bind via `j_syn_ui::shell::ensure_canvas_bold_font`.
  - **Avoid sticky CSS `opacity`:** Always pair CSS `opacity < 1.0` modifier classes with an explicit `opacity: 1;` base rule and/or reactive `.opacity(...)` in Rust.
  - **No clipped bold labels or stale popover heights:** Give bold uppercase single-line headers `>= 18 px` height (or `Auto`), and compute floating overlay clamping heights deterministically from shared layout constants + safety cushion rather than stale magic numbers.
- **Theme-only colors:** Route all `femtovg` and CSS colors through `theme::current_palette(cx)` / `ThemePalette` (zero hardcoded hex/RGB color literals in `draw()` outside `theme.rs`).
- **Naming & tokens:** Reference `j_syn_ui::tokens::*` instead of magic layout numbers. Always name slot effect cards **`FX Unit`** (`fx_unit` / `FX_UNIT_*`), never `snapin`, and avoid third-party synth/plugin brand namedrops in comments, PRs, or docs.

## 4. Anti-Slopification & Code Quality

- **No duplicated boilerplate:** Extract shared helpers, loops, or lookup tables instead of copy-pasting FX, oscillator, or knob blocks.
- **No silent fallbacks:** Reject swallowed errors without safe defaults or normalization/clamping mismatches between UI display formatting and DSP math.
- **Module size discipline:** If a PR adds `>150` lines to a file already `>800` lines (`editor/mod.rs`, `mod_bar.rs`, `editor/modseq/bottom_bars.rs`, `engine.rs`, `destinations.rs`), flag it and suggest extracting a cohesive submodule.
- **Invariant-checking tests:** Every new DSP feature or bugfix must ship with unit/smoke tests asserting exact behavioral invariants (finite output, frequency/gain/decay bounds, sample-rate invariance), not merely "output is non-zero".

## 5. Known Intentional Idioms (Do Not Flag as Bugs)

- `nih_plug::util::permit_alloc(|| ...)` wrapping lock-free `ArcSwap::load()` in `crates/j-syn/src/lib.rs` (required by `nih_plug`'s `assert_process_allocs` instrumentation for `ArcSwap` thread-local debt tracking on first read).
- `unsafe` `ParamPtr` calls in `crates/j-syn/src/presets/mod.rs` when resolving/storing patch parameter values via `nih_plug`'s `ParamMap` API (provided safety comments remain intact).
- Zero-allocation `f32`/`f64` fast approximations (`fast_tanh`, `TanhAdaa2`, polyphase IIR half-band oversampler) documented in DSP comments.
