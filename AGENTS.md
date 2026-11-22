# J-SYN Architecture & Coding Standards

## Core Priority
**Maintainability and readability for both humans and AI are top priorities.** Prefer small, single-responsibility modules (~150–500 lines), descriptive helper functions, and shared UI/DSP primitives over deeply nested closures or copy-pasted boilerplate.

---

## Crate & Module Map

- `crates/j-syn-dsp/`: Real-time audio-thread DSP engine (zero heap allocations in `process()`, shared phase-warp + PM/RM evaluation in `sample_with_warp_and_pm`):
  - `lib.rs` / `engine.rs` / `voice.rs` / `oscillator.rs` / `envelope.rs` / `wavetable.rs`: Polyphonic synth engine (`64` voices, `Dirty` vs `Clean` interpolation, voice `auto_sleep`, `f64`-precision `~2.5 Hz` DC blocker, optional `-0.1 dBFS` brickwall safety limiter, `WavetableBank` custom `.wav` `2048`-sample frame slicing + mip-maps), plus full **MPE** (`mpe_enabled`, `mpe_pitch_bend_range` `48 st` default, Master `Ch 1` broadcast vs Member `Ch 2–16` isolated Pitch Bend / Channel Pressure / `CC74` Slide, CLAP/VST3 `PolyPressure`/`PolyTuning`/`PolyBrightness`, and `1.5 ms` per-voice expression smoothing).
  - `adaa.rs`: 2nd-order ADAA `tanh` waveshaper (`TanhAdaa2`, ported from Cure Audio `Scream` / `ADAA` / `Polylogarithm`).
  - `noise.rs`: Per-voice stereo White/Pink/Brown noise generator (`NoiseType`, `NoiseParams`, `NoiseState`).
  - `filter.rs`: Dual per-voice ZDF multimode & multi-filter engine (`LP`, `BP`, `HP` with selectable `FilterSlope` `6/12/24/36/48 dB` via pre-allocated `[StereoZdfSvf; 4]` + 1-pole TPT, Cure-style `Scream`, `Daft`, pre-allocated `Comb` delay line, `HP + Peak`, `HP + Notch`, `LP + Notch`, `LP + Peak`, `Bandreject`, `Peak + Peak`, `Notch + Notch`, `evaluate_filter_response_db`).
  - `fx/`: 3-lane routable Master FX Rack (`Bus A`, `Bus B`, `Bus C` via 6 `FxBusTopology` modes, `8` slots per lane, `STACK 1..4` per slot, zero audio-thread heap allocations): `mod.rs` (`FxType`, `FxBusTopology`, `FxSlotParams`, `FxRackParams`, `FxRackState`, `flush_denormal`/`track_idle`), `dynamics.rs` (`OttStereo`, `DistortionStereo`, `JSatClipperStereo`), `limiter.rs` (`LimiterStereo` lookahead + True Peak), `modulation.rs` (`EqStageStereo`, `ChorusStereo`, `PhaserStereo`, `DelayStereo`, `ReverbStereo`, `FlangerStereo`), `diopser.rs` (128-stage RBJ allpass `DiopserStereo`), `utility.rs` (`UtilityStereo` M/S + phase invert), `tube_resonator.rs` (`TubeResonatorStereo` waveguide), and `multiband_dist.rs` (3-band LR4 crossover distortion).
  - `oversample.rs`: Zero-allocation 4x polyphase IIR half-band oversampler (`Oversampler4x`) and RBJ LP/HP/AP/Shelf `Biquad` / `BiquadCoeffs` ported from `crushclone-rs`.
  - `insert.rs`: Dual per-voice 18-mode insert waveshapers + `Serial`/`Parallel` filter routing, `DirectDest` (`Off`/`BusA`/`BusB`/`Sub`), `FilterBusDest` (with serial double-feed guards), and feedback bus state (`InsertParams`, `RoutingParams`, `InsertShaperState`, `VoiceFeedbackState`).
  - `mod_matrix.rs`: Lock-free 32-slot modulation matrix (`ModMatrixSource` with 21 active sources including per-voice `Aftertouch` & `Slide` `CC74`, `ModDestination` covering all 3 FX buses `FX1..8` + `Bus Level/Pan`, `ModSlot` with `enabled`, `bypassed`, `aux_source`, `curve`, `ModMatrixState`, `FxScopeRing`, `ModUiTelemetry`).
  - `modseq.rs`: 4 hybrid 16-step `MODSEQ` / breakpoint `LFO` curve modulators (`ModSeqMode`, `ModSeqStepShape` 18-tile `PALETTE`, `ModSeqStep`, `MsegNode`, `MsegCurve`, 9 `LfoPreset` presets, `ModSeqParams`, `ModSeqState`).
- `crates/j-syn/src/lib.rs` & `crates/j-syn/src/params/`: `nih-plug` VST3/CLAP plugin wrapper (`lib.rs`) + split parameter definitions (`params/mod.rs` `PersistedArcSwap`/`JSynPluginParams`, `params/osc.rs`, `params/fx.rs`, `params/mod_params.rs`).
- `crates/j-syn/src/presets.rs`: Local `JSynConfig` persistence, recursive folder scanning, patch JSON save/load (with legacy aliasing), and custom `.wav` wavetable loading.
- `crates/j-syn/src/theme.css`: Base Vizia stylesheet mapped dynamically by `editor/theme.rs`.
- `crates/j-syn/src/editor/`: Modular Vizia GUI (`1760 × 840` 3-Wing Widescreen Chassis: `Left Wing` Oscillators/Noise, `Center Wing` Dual Filters + Center Modulation Stage + Dual Inserts + `2×4` Macro/Source Dock, `Right Wing` 3-Lane Scrollable FX Rack):
  - `tokens.rs`: Single source of truth for design tokens (window/wing dimensions, knob/pill/header/scope heights, `FX_UNIT_*`, `fx_unit_scope_height`).
  - `mod.rs`: Top-level UI state (`UiData`, `MainViewMode` `Main`/`Matrix`/`Settings`, `BottomTab`, `CenterStageTab`, `UiEvent`, `ModContextMenuState`), `GearIcon`, macOS `baseview` resize notification, global tooltip & context-menu overlays, and 3-wing chassis assembly.
  - `voice_tab.rs`, `routing_tab.rs`, `macros_tab.rs`, `modseq_canvases.rs`, `modseq_tab.rs`, `matrix_tab.rs`, `settings_view.rs`, `theme.rs`, `scope.rs`, `osc_panel.rs`, `noise_panel.rs`, `filter_panel.rs`, `insert_panel.rs`, `fx_scopes.rs`, `fx_rack.rs`, `mod_bar.rs`.
  - `widgets/`: `knob.rs` (`RotaryKnob` + `KnobDimExt::enabled_by`), `switch.rs` (`LedToggleSwitch`), `pill.rs` (`ParamStepPill`, `PitchDragBox` `OCT`/`SEMI`/`FINE`, `PitchGroupModOverlay`), `ratio.rs` (`RatioDragBox`), `mod.rs` (shared drag/cursor/tooltip helpers).
  - `controls/`: `mod.rs` (`bold_label`, `build_param_readout_box`, `build_pitch_tuning_group`, `build_ratio_tuning_row`, knob/pill cells, `build_module_header`, `build_center_tab_button`), `icon_buttons.rs` (`ArrowButton`, `CrossButton`, `VectorArrow::caret`), `dropdowns.rs` (`ScrollDropdown`, cascading folder dropdown builders).
- **Shared UI Kit & Standalone FX Ecosystem (`crates/j-syn-ui/` & `crates/plugins/*`):**
  - `crates/j-syn-ui/`: Plugin-agnostic Vizia/femtovg UI primitives, themes, tokens (`tokens.rs`), scopes (`scopes/`), headless PNG preview harness (`preview.rs`), standalone plugin shell (`fx_units/standalone_shell.rs`), consolidated shared `nih_plug::Enum` parameters (`fx_units/params.rs`), and **shared variable-height `200 px` FX Unit views** (`fx_units/`).
  - `crates/plugins/j-<fx>/`: All 14 standalone FX VST3/CLAP plugins (`j-chorus`, `j-clipper`, `j-delay`, `j-dispersion`, `j-dist`, `j-filter`, `j-flanger`, `j-limiter`, `j-mbdist`, `j-ott`, `j-phaser`, `j-reverb`, `j-tuberes`, `j-utility`), bundled alongside `j-syn` via `cargo xtask bundle-all --release --offline`.
  - **Naming rule:** Always call slot effect cards **`FX Unit`** (`fx_unit` / `FX_UNIT_*`) — never `snapin`.
  - **One-source FX Unit UIs & Self-Contained FX DSP:** All 14 FX slots in `J-SYN`'s 3-lane rack render via shared `crates/j-syn-ui/src/fx_units/` views (zero legacy rack fallback). Keep FX DSP state self-contained on `FxSlotParams` (never couple FX DSP behavior to global synth flags like `hq_mode`/`EngineMode`) so standalone plugins and `J-SYN` rack slots stay 1:1.
  - **Shared `nih_plug::Enum` Parameters:** `j-syn-dsp` intentionally has zero `nih_plug` dependency; house all shared `nih_plug::Enum` parameter types and dropdown item tables (`DistortionModeParam`, `DiopserStagesParam`, `FilterModeParam`, `FxFilterSlopeParam`) in `crates/j-syn-ui/src/fx_units/params.rs` and re-export them in `crates/j-syn/src/params/` to preserve preset compatibility.

---

## GUI, Ecosystem & Stability Conventions

- **Design Tokens First (`tokens.rs`):** Reference `tokens::*` for dimensions, spacing, knob/pill/header sizes, and `fx_unit_scope_height` rather than scattering inline pixel magic numbers. Active FX units use variable `height(Auto)` cards inside `ScrollView` with explicit `FX_LANE_SCROLL_GUTTER` right clearance; `Off` (`Empty`) slots collapse to header-only height. All FX Units use `FX_UNIT_W = 200 px` (`224 px` standalone window), with `46 px` (`KNOB_FX_UNIT_2X2`) knobs in equidistant `2×2` grids (`FX_UNIT_2X2_INSET_X`) or `3-over-2` grids (`SPACE_SM` inset on Row 1 + `FX_UNIT_2X2_INSET_X` on Row 2 for 5-knob units like `Filter`, dimming mode-specific knobs via `.enabled_by(...)` instead of tearing down widgets).
- Always use `controls::bold_label(cx, text, class)` instead of repeating `.font_family(...).font_weight(...)`.
- **Self-Contained Vector Icon Buttons & Balanced Centering:** Never put Unicode arrows (`▲`, `▼`, `◀`, `▶`, `<`, `>`, `▾`) or child icon views inside `vizia::Button` (`default_layout.css` imposes `button > .inner { min-left: 8px; min-right: 8px; }`, pushing narrow buttons off-center, and `Button`'s `child-space: 1s` shrinks `Stretch(1.0)` children to 1/3). Always use `ArrowButton`, `CrossButton`, or `VectorArrow::caret` in `controls/icon_buttons.rs`, which draw both box and glyph in one `cx.bounds()` pass using balanced rectangular centering.
- **Clickable `ZStack` Scope Overlays:** Never place `.hoverable(false)` on a container wrapping interactive overlay buttons/dropdowns (`vizia_core`'s `hover_entity` immediately skips all descendants when `!hoverable`). Apply `.hoverable(false)` only on the underlying canvas view (`FxSlotVisualizer`, `PitchGroupModOverlay`) and layout spacers, declare overlays *after* the canvas with `.position_type(PositionType::SelfDirected)`, and **never use `.z_index(...)` inside a `ScrollView`** (Vizia hoists `z_index > 0` views above `ScrollView`'s clip rect; `ZStack` declaration order already handles stacking and hit-testing). Use `#rrggbbaa` ghost-translucent idle styling (`:hover` / `:checked` for focus).
- **FX Unit Blueprint (`docs/FX_UNIT_SPEC.md`):** Follow [FX_UNIT_SPEC.md](docs/FX_UNIT_SPEC.md) for all FX Unit redesigns and standalone FX extractions; all continuous knobs across all tabs of tabbed FX units must be modulatable via `ModDestination::fx_bus_slot_param(bus, slot, 0..16)`.
- Always use `controls::build_dropdown_trigger_labels` / `build_enum_dropdown` / `build_prefixed_enum_dropdown` / `build_bool_dropdown` / `ScrollDropdown` for dropdowns, and `controls::build_param_readout_box` for numeric readouts; wrap long/nested dropdown menus in bounded `ScrollView` + `build_directory_tree_items`.
- Route **all** `femtovg` colors through `theme::current_palette(cx)` (and CSS hex tokens—both `#rrggbb` and `#rrggbbaa`—through `ThemePalette::map_hex_token`), keeping zero hardcoded unmapped colors in `draw()` or `theme.css`.
- **Single 60 Hz Vizia Timer:** Never add additional `cx.add_timer` loops; extend the single timer in `mod_bar.rs` instead.
- **Split Structural vs. Value Lenses:** Emit `UiEvent::ModValChanged` (`mod_val_rev`) during continuous drags inside `Binding::new(..., UiData::mod_ui_rev, ...)` views so captured drag widgets (`cx.capture()`) are never destroyed mid-drag.
- **CSS & Verification Gate:** Avoid unsupported Vizia CSS selectors (e.g. `:focus-within`, which crashes Ableton Live); run `cargo test --offline --workspace` (verifying CSS parsing, unique `#[id]` strings, single timer, patch/dir/theme tests, MPE & DSP tests) plus `cargo xtask bundle-all --release --offline` with zero project compiler warnings before committing.

---

## Multi-Conversation Workflow (Antigravity)

- Keep this monorepo (`Cargo.toml` workspace) for all shared DSP (`j-syn-dsp`), shared UI (`j-syn-ui`), `J-SYN`, and standalone FX plugins (`crates/plugins/*`).
- **Start fresh Antigravity conversations at clean crate/plugin boundaries** rather than extending long compacted threads:
  - One focused conversation for the `j-syn-ui` + `xtask` + preview-harness extraction (Issue #17).
  - Separate focused conversations for individual standalone FX plugins / FX unit UX redesigns (or small related batches), plus dedicated conversations for standalone DSP features in `j-syn-dsp`.
  - Rely on `AGENTS.md`, `tokens.rs`, and the target GitHub issue as the complete handoff spec for each new conversation.
- **Plan-First & Handoff-Ready Specs (Individual FX Extraction & Layout Redesign):**
  - Before beginning implementation on any standalone FX extraction (`crates/plugins/j-<fx>/`) or individual FX Unit layout redesign (`crates/j-syn-ui/src/fx_units/`), **always write out a complete, explicit implementation plan first** (resolving any open UX/layout ambiguities before editing code).
  - Write the plan with enough concrete detail that a cheaper/faster model (e.g. Hydrogen) can execute the implementation directly without guesswork:
    1. **Exact File List & Scope:** Every file to create or modify (`j-syn-dsp`, `j-syn-ui/src/fx_units/<fx>.rs`, `j-syn-ui/src/tokens.rs`, `j-syn/src/editor/fx_rack.rs`, `crates/plugins/j-<fx>/`, workspace `Cargo.toml`).
    2. **DSP & Parameter Contract:** Exact DSP struct/methods to make `pub`, exact `#[id = "..."]` parameter IDs, defaults, ranges, formatters, VST3/CLAP IDs, and shared trait projection methods (`<Fx>UnitParams`).
    3. **Layout & Pixel Math:** Exact Vizia container hierarchy (`ZStack` overlays, tab/context bars, knob grid), token references (`tokens::*`), existing CSS classes from `theme.css`, `standalone_fx_window_size` height math, constant-height behavior across tabs, and confirmation that the shared FX Unit body fits cleanly at `210–220 px` narrow width as well as `STANDALONE_FX_CARD_W` (`260 px`).
    4. **Exact Verification Steps:** Headless PNG preview commands (standalone default width, narrow `210 px` card stress test, and `J-SYN` Right Wing rack view) for visual inspection, followed by `cargo test --offline --workspace` and `cargo xtask bundle-all --release --offline` with zero project warnings.

---

## Git Workflow & Pull Requests

- **Branch per Issue / Feature:** Always create a dedicated branch off `main` (e.g. `fix/<short-slug>` or `feat/<short-slug>`) when working on an issue or task rather than committing directly to `main`. If tackling multiple unrelated issues, use separate branches off `main` for each.
- **Verify Before Pushing:** Run `cargo test --offline --workspace` and `cargo xtask bundle-all --release --offline` (zero project compiler warnings) before opening or merging a PR.
- **Pull Requests:** Once verified on a feature branch, always commit, push the branch to `origin`, and open a Pull Request targeting `main` (`gh pr create`), referencing the issue (`Closes #<n>` when applicable), without waiting for a separate prompt. Only pushing or merging directly to `main` (`git push origin main` or `gh pr merge --merge --delete-branch`) requires explicit user permission; after merging, fast-forward local `main`.
