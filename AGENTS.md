# J-SYN Architecture & Coding Standards

## Core Priority
**Maintainability and readability for both humans and AI are top priorities.** Prefer small, single-responsibility modules (~150–500 lines), descriptive helper functions, and shared UI/DSP primitives over deeply nested closures or copy-pasted boilerplate.

---

## Crate & Module Map

- `crates/j-syn-dsp/`: Real-time audio-thread DSP engine (zero heap allocations in `process()`, shared phase-warp + PM/RM evaluation in `sample_with_warp_and_pm`):
  - `lib.rs` / `engine.rs` / `voice.rs` / `oscillator.rs` / `envelope.rs` / `wavetable.rs`: Polyphonic synth engine (`64` voices, `Dirty` vs `Clean` interpolation, voice `auto_sleep`, `f64`-precision `~2.5 Hz` DC blocker, optional `-0.1 dBFS` brickwall safety limiter, `WavetableBank` custom `.wav` `2048`-sample frame slicing + mip-maps).
  - `adaa.rs`: 2nd-order ADAA `tanh` waveshaper (`TanhAdaa2`, ported from Cure Audio `Scream` / `ADAA` / `Polylogarithm`).
  - `noise.rs`: Per-voice stereo White/Pink/Brown noise generator (`NoiseType`, `NoiseParams`, `NoiseState`).
  - `filter.rs`: Dual per-voice ZDF multimode & multi-filter engine (`LP`, `BP`, `HP` with selectable `FilterSlope` `6/12/24/36/48 dB` via pre-allocated `[StereoZdfSvf; 4]` + 1-pole TPT, Cure-style `Scream`, `Daft`, pre-allocated `Comb` delay line, `HP + Peak`, `HP + Notch`, `LP + Notch`, `LP + Peak`, `evaluate_filter_response_db`).
  - `fx/`: Parallel dual-bus 8-slot serial Master FX Rack (`Bus A` & `Bus B` + `Sub Bus`) with per-slot stacking (`STACK 1..4`) and zero audio-thread heap allocations (`mod.rs`: `FxType` including `Diopser` & `Phaser`, `FxFilterSlope`, `FxSlotParams`, `FxBusParams`, `SubBusParams`, `SubBusProcessor`, `FxRackParams`, `FxRackState`; `modulation.rs`: `EqStageStereo`, `ChorusStereo`, `PhaserStereo`, `DelayStereo`, `ReverbStereo`; `diopser.rs`: pre-allocated 128-stage cascaded RBJ allpass bank `DiopserStereo` & `evaluate_diopser_ripple`).
  - `oversample.rs`: Zero-allocation 4x polyphase IIR half-band oversampler (`Oversampler4x`) and RBJ LP/HP/AP/Shelf `Biquad` / `BiquadCoeffs` ported from `crushclone-rs`.
  - `insert.rs`: Dual per-voice insert waveshapers (`Tube`, `Mech`, `Tube -> Mech`, `Mech -> Tube`, `SineShaper`, `Parabolic`, `SoftClip`, `Bitcrush`) + `Serial`/`Parallel` filter routing, `DirectDest` (`Off`/`BusA`/`BusB`/`Sub`), `FilterBusDest` (`BusA`/`BusB`/`Both`/`Sub`), and feedback bus state (`InsertParams`, `RoutingParams`, `InsertShaperState`, `VoiceFeedbackState`).
  - `mod_matrix.rs`: Lock-free 32-slot modulation matrix (`ModMatrixSource`, `ModDestination` with 198 destinations covering `Bus A` & `Bus B` `FX1..8 P1..8` + `Sub Bus` + `Bus Level/Pan`, `ModSlot` with `enabled`, `bypassed`, `aux_source`, `curve`, `ModMatrixState`, `FxScopeRing`, `ModUiTelemetry`).
  - `modseq.rs`: 4 hybrid 16-step `MODSEQ` / breakpoint `LFO` curve modulators (`ModSeqMode`, `ModSeqStepShape` with 18-tile `PALETTE` including 2x/3x subdivisions, `ModSeqStep`, `MsegNode`, `MsegCurve`, 9 `LfoPreset` presets including `Sine`, `ModSeqStepBank`, `ModSeqShapePreset`, `ModSeqSyncMode`, `ModSeqRate`, `ModSeqPlayMode`, `ModSeqRetrig`, `ModSeqParams`, `ModSeqState`, `evaluate_step_shape`, `evaluate_modseq_curve`, `evaluate_mseg_curve`).
- `crates/j-syn/src/lib.rs`: `nih-plug` VST3/CLAP plugin wrapper, unique `#[id = "..."]` parameter definitions, and `PersistedArcSwap` structured state.
- `crates/j-syn/src/presets.rs`: Local `JSynConfig` persistence (`presets_dir`, `wavetables_dir`, `theme`, `ui_scale`), recursive folder scanning (`DirectoryTree`, `DirectoryFolder`, `scan_preset_tree`, `scan_wavetable_tree`), patch JSON save/load, and custom `.wav` wavetable loading.
- `crates/j-syn/src/theme.css`: Base Vizia stylesheet defining layout, spacing, disabled-module dimming, inline readouts, and dropdown/overlay styling (mapped dynamically by `editor/theme.rs`).
- `crates/j-syn/src/editor/`: Modular Vizia GUI:
  - `mod.rs`: Top-level UI state (`UiData`, `MainViewMode` `Main`/`Fx`/`Matrix`/`Settings`, `BottomTab`, `UiEvent`, `ModContextMenuState`), `GearIcon`, macOS `baseview` resize notification, global tooltip & right-click modulation context menu overlays, and window section assembly (`create`, `build_header_bar`, `VOICE` bottom drawer with `KEYTRACKING`/`VOICING`/`OSCILLATOR PAN`).
  - `theme.rs`: `ThemePalette` (`Serialize`/`Deserialize`), 5 built-in themes (`Studio Amber`, `Arctic Slate`, `Midnight Violet`, `Phosphor`, `Daylight Studio`), `<config_dir>/themes/*.json` scanning, single-pass `render_css()`, `DynamicCssSource`, and `current_palette(cx)` / `mod_source_rgb` / `vg_lerp` helpers.
  - `widgets.rs`: Custom `femtovg` / `ParamWidgetBase` widgets (`RotaryKnob` with `CursorIcon::RowResize` mod-collar handle & dimmed bypassed arcs, `LedToggleSwitch`, `ParamStepPill`, `PitchDragBox` with `Alt+Drag` octaves / normal semitones / `Shift+Drag` cents).
  - `scope.rs`: Real-time single-cycle `WavetableScope` and header stereo `MasterMeter` canvases.
  - `controls.rs`: Shared label, dropdown, readout, knob/pill cell, and tree menu helpers (`bold_label`, `ScrollDropdown`, `build_dropdown_trigger_labels`, `build_directory_tree_items`, `build_enum_dropdown`, `build_bool_dropdown`, `build_wavetable_dropdown`, `build_ui_scale_dropdown`, `build_param_readout_box`, knob/pill cells, `build_module_header`, `build_tab_button`).
  - `osc_panel.rs`: Oscillator strip (`OSC 1` – `OSC 4`) builder with custom `.wav` filename label, stretched `WavetableScope`, `WARP` (`None`, `B-`, `B+`, `SP`) + `<`/`>` routing order toggle + scrollable unified `MOD` (`Off`, `PM 1..4`, `RM 1..4`) row (`build_osc_panel`).
  - `noise_panel.rs`: Per-voice stereo Noise generator strip builder (`build_noise_panel`).
  - `filter_panel.rs`: Dual multimode filter cards (`FILTER 1` / `FILTER 2`) with live `FilterResponseScope` frequency curve canvas and context-sensitive 4th knob (`build_filter_panel`).
  - `insert_panel.rs`: Compact `INSERT FX` card (`INS 1` and `INS 2` mode, position, and drive controls) with per-slot `POS`/`DRIVE` disabled dimming when `Off` (`build_insert_panel`).
  - `fx_scopes.rs`: Shared `femtovg` FX scope visualizers (`FxSlotVisualizer`, `fx_accent_rgb`, Clipper ceiling grey-out, 15-mode Filter curve, Diopser dispersion ripple curve, OTT/Distortion/Delay/Reverb/Chorus/Phaser scopes).
  - `fx_rack.rs`: Parallel Dual-Bus FX UI (`BUS A | BUS B` switcher on `MAIN`, full `4×4` `Bus A` + `Bus B` view + `SUB BUS` banner with `LP CUTOFF`/`CLIP DRIVE`/`LEVEL`, `▲`/`▼` slot swapping, `STACK 1..4`, translucent `ZStack` overlays for `Filter` Type/Slope, `Diopser` `STAGES`, and `MAIN | EQ` sub-tabs).
  - `routing_tab.rs`: `ROUTING` bottom tab (`SOURCE FILTER SEND` + 4-way `Off | Bus A | Bus B | Sub` Direct selectors, `FILTER BUS & FEEDBACK` with `F1 OUT`/`F2 OUT` bus selectors, `INSERT MIX`).
  - `macros_tab.rs`: `MACROS` bottom drawer tab with inline-editable `M1–M8` labels and macro knobs (`build_macros_tab`).
  - `settings_view.rs`: Gearwheel `Settings` view (`UI Theme` selector, `Dirty`/`Clean` quality, `DC Blocker`, `Safety Limiter`, `Presets`/`Wavetables` directories + `Rescan`).
  - `mod_bar.rs`: Lock-free `ModUiState`, single 60 Hz UI timer, dual-purpose draggable/clickable `ModSourcePill` strip (`E1..E4`, `MS1..MS4`, `VEL`, `KEY`, `MW`, `AT`), inline slot inspector, and interactive `MATRIX` table (`SOURCE`/`CURVE`/`AUX` dropdowns, `DEPTH` drag box, `POL`, `ORD`, `DEL`).
  - `modseq_tab.rs`: `MODSEQ` bottom drawer (`MSEQ 1`–`MSEQ 4` dedicated `MODSEQ` `ModSeqPaletteGrid` `2x9` shape palette + step depth bars vs. `LFO` `LfoPresetBar` + `SNAP 1/16` grid snapping, and `ENV 1`–`ENV 4` `AdsrPreviewScope` + ADSR knobs, `build_modseq_tab`).

---

## GUI & Stability Conventions

- Always use `controls::bold_label(cx, text, class)` instead of repeating `.font_family(...).font_weight(...)`.
- Always use `controls::build_dropdown_trigger_labels` / `build_enum_dropdown` / `build_bool_dropdown` / `ScrollDropdown` for dropdowns, and `controls::build_param_readout_box` for numeric readouts; wrap long/nested dropdown menus in bounded `ScrollView` + `build_directory_tree_items`.
- Route **all** `femtovg` colors through `theme::current_palette(cx)` (and CSS hex tokens through `ThemePalette::map_hex_token`), keeping zero hardcoded unmapped colors in `draw()` or `theme.css`.
- **Single 60 Hz Vizia Timer:** Never add additional `cx.add_timer` loops; extend the single timer in `mod_bar.rs` instead.
- **Split Structural vs. Value Lenses:** Emit `UiEvent::ModValChanged` (`mod_val_rev`) during continuous drags inside `Binding::new(..., UiData::mod_ui_rev, ...)` views so captured drag widgets (`cx.capture()`) are never destroyed mid-drag.
- **CSS & Verification Gate:** Avoid unsupported Vizia CSS selectors (e.g. `:focus-within`, which crashes Ableton Live); run `cargo test --offline --workspace` (verifying CSS parsing, unique `#[id]` strings, single timer, patch/dir/theme tests, and DSP tests) plus `cargo xtask bundle j-syn --release` with zero project compiler warnings before committing.
