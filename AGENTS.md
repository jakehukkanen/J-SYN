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
- **Planned Ecosystem Expansion (Issue #17):**
  - Extract plugin-agnostic UI primitives, themes, tokens, scopes, and **shared variable-height FX Unit views** into `crates/j-syn-ui/` (`crates/plugins/j-<fx>/` standalone VST3/CLAP plugins will consume `j-syn-dsp` + `j-syn-ui` directly, and `xtask` will support `bundle-all`).
  - **Naming rule:** Always call slot effect cards **`FX Unit`** (`fx_unit` / `FX_UNIT_*`) — never `snapin`.
  - **One-source FX Unit UIs:** When building standalone FX plugins (`crates/plugins/*`), house reusable unit layouts in `crates/j-syn-ui/` so standalone FX UX improvements immediately propagate back into `J-SYN`'s 3-lane rack.

---

## GUI, Ecosystem & Stability Conventions

- **Design Tokens First (`tokens.rs`):** Reference `tokens::*` for dimensions, spacing, knob/pill/header sizes, and `fx_unit_scope_height` rather than scattering inline pixel magic numbers. Active FX units use variable `height(Auto)` cards inside `ScrollView` with explicit `FX_LANE_SCROLL_GUTTER` right clearance; `Off` (`Empty`) slots collapse to header-only height.
- Always use `controls::bold_label(cx, text, class)` instead of repeating `.font_family(...).font_weight(...)`.
- **Self-Contained Vector Icon Buttons & Balanced Centering:** Never put Unicode arrows (`▲`, `▼`, `◀`, `▶`, `<`, `>`, `▾`) or child icon views inside `vizia::Button` (`default_layout.css` imposes `button > .inner { min-left: 8px; min-right: 8px; }`, pushing narrow buttons off-center, and `Button`'s `child-space: 1s` shrinks `Stretch(1.0)` children to 1/3). Always use `ArrowButton`, `CrossButton`, or `VectorArrow::caret` in `controls/icon_buttons.rs`, which draw both box and glyph in one `cx.bounds()` pass using balanced rectangular centering.
- **Clickable `ZStack` Scope Overlays:** Never place `.hoverable(false)` on a container wrapping interactive overlay buttons/dropdowns (`vizia_core`'s `hover_entity` immediately skips all descendants when `!hoverable`). Apply `.hoverable(false)` only on the underlying canvas view (`FxSlotVisualizer`, `PitchGroupModOverlay`) and layout spacers, declare overlays *after* the canvas with `.position_type(PositionType::SelfDirected).z_index(10)`, and use `#rrggbbaa` ghost-translucent idle styling (`:hover` / `:checked` for focus).
- Always use `controls::build_dropdown_trigger_labels` / `build_enum_dropdown` / `build_prefixed_enum_dropdown` / `build_bool_dropdown` / `ScrollDropdown` for dropdowns, and `controls::build_param_readout_box` for numeric readouts; wrap long/nested dropdown menus in bounded `ScrollView` + `build_directory_tree_items`.
- Route **all** `femtovg` colors through `theme::current_palette(cx)` (and CSS hex tokens—both `#rrggbb` and `#rrggbbaa`—through `ThemePalette::map_hex_token`), keeping zero hardcoded unmapped colors in `draw()` or `theme.css`.
- **Single 60 Hz Vizia Timer:** Never add additional `cx.add_timer` loops; extend the single timer in `mod_bar.rs` instead.
- **Split Structural vs. Value Lenses:** Emit `UiEvent::ModValChanged` (`mod_val_rev`) during continuous drags inside `Binding::new(..., UiData::mod_ui_rev, ...)` views so captured drag widgets (`cx.capture()`) are never destroyed mid-drag.
- **CSS & Verification Gate:** Avoid unsupported Vizia CSS selectors (e.g. `:focus-within`, which crashes Ableton Live); run `cargo test --offline --workspace` (verifying CSS parsing, unique `#[id]` strings, single timer, patch/dir/theme tests, MPE & DSP tests) plus `cargo xtask bundle j-syn --release --offline` (or `bundle-all` once extended) with zero project compiler warnings before committing.

---

## Multi-Conversation Workflow (Antigravity)

- Keep this monorepo (`Cargo.toml` workspace) for all shared DSP (`j-syn-dsp`), shared UI (`j-syn-ui`), `J-SYN`, and standalone FX plugins (`crates/plugins/*`).
- **Start fresh Antigravity conversations at clean crate/plugin boundaries** rather than extending long compacted threads:
  - One focused conversation for the `j-syn-ui` + `xtask` + preview-harness extraction (Issue #17).
  - Separate focused conversations for individual standalone FX plugins / FX unit UX redesigns (or small related batches), plus dedicated conversations for standalone DSP features in `j-syn-dsp`.
  - Rely on `AGENTS.md`, `tokens.rs`, and the target GitHub issue as the complete handoff spec for each new conversation.

---

## Git Workflow & Pull Requests

- **Branch per Issue / Feature:** Always create a dedicated branch off `main` (e.g. `fix/<short-slug>` or `feat/<short-slug>`) when working on an issue or task rather than committing directly to `main`. If tackling multiple unrelated issues, use separate branches off `main` for each.
- **Verify Before Pushing:** Run `cargo test --offline --workspace` and `cargo xtask bundle j-syn --release --offline` (zero project compiler warnings) before opening or merging a PR.
- **Pull Requests:** Once verified on a feature branch, always commit, push the branch to `origin`, and open a Pull Request targeting `main` (`gh pr create`), referencing the issue (`Closes #<n>` when applicable), without waiting for a separate prompt. Only pushing or merging directly to `main` (`git push origin main` or `gh pr merge --merge --delete-branch`) requires explicit user permission; after merging, fast-forward local `main`.
