# J-SYN Architecture & Coding Standards

## Core Priority
**Maintainability and readability for both humans and AI are top priorities.** Prefer small, single-responsibility modules (~150–500 lines), descriptive helper functions, and shared UI/DSP primitives over deeply nested closures or copy-pasted boilerplate.

---

## Crate & Module Map

- `crates/j-syn-dsp/`: Real-time audio-thread DSP engine (zero heap allocations in `process()`, shared phase-warp + PM/RM evaluation in `sample_with_warp_and_pm`):
  - `adaa.rs`: 2nd-order ADAA `tanh` waveshaper (`TanhAdaa2`, ported from Cure Audio `Scream` / `ADAA` / `Polylogarithm`).
  - `noise.rs`: Per-voice stereo White/Pink/Brown noise generator (`NoiseType`, `NoiseParams`, `NoiseState`).
  - `filter.rs`: Dual per-voice ZDF multimode & multi-filter engine (`LP`, `BP`, `HP` with selectable `FilterSlope` `6/12/24/36/48 dB` via pre-allocated `[StereoZdfSvf; 4]` + 1-pole TPT, Cure-style `Scream`, `Daft`, pre-allocated `Comb` delay line, `HP + Peak`, `HP + Notch`, `LP + Notch`, `LP + Peak`, `evaluate_filter_response_db`).
  - `oversample.rs`: Zero-allocation 4x polyphase IIR half-band oversampler (`Oversampler4x`) and RBJ shelf `Biquad` / `BiquadCoeffs` ported from `crushclone-rs`.
  - `insert.rs`: Dual per-voice insert waveshapers (`Tube`, `Mech`, `Tube -> Mech`, `Mech -> Tube`, `SineShaper`, `Parabolic`, `SoftClip`, `Bitcrush`) + `Serial`/`Parallel` filter routing, `Direct Out` bypass, and feedback bus state (`InsertParams`, `RoutingParams`, `InsertShaperState`, `VoiceFeedbackState`).
  - `mod_matrix.rs`: Lock-free 32-slot modulation matrix (`ModMatrixSource`, `ModDestination`, `ModSlot`, `ModMatrixState`, `ModUiTelemetry`).
  - `modseq.rs`: 4 hybrid 16-step Performer / MSEG curve sequencers (`ModSeqStep`, `ModSeqStepBank`, `ModSeqShapePreset`, `ModSeqSyncMode`, `ModSeqRate`, `ModSeqPlayMode`, `ModSeqRetrig`, `ModSeqParams`, `ModSeqState`, `evaluate_modseq_curve`).
- `crates/j-syn/src/lib.rs`: `nih-plug` VST3/CLAP plugin wrapper and parameter definitions.
- `crates/j-syn/src/theme.css`: Vizia stylesheet defining colors, spacing, and hardware panel styling.
- `crates/j-syn/src/editor/`: Modular Vizia GUI:
  - `mod.rs`: Top-level UI state (`UiData`, `BottomTab`, `UiEvent`), macOS `baseview` resize notification, and high-level window section assembly (`create`, `build_header_bar`, `build_osc_column`, `build_filter_column`, `build_fx_column`, `build_bottom_tabs`).
  - `widgets.rs`: Custom `femtovg` / `ParamWidgetBase` widgets (`RotaryKnob`, `LedToggleSwitch`, `ParamStepPill`).
  - `scope.rs`: Real-time single-cycle `WavetableScope` waveform preview canvas.
  - `controls.rs`: Shared label, dropdown, readout, knob/pill cell, and module chrome helpers (`bold_label`, `build_dropdown_trigger_labels`, `build_enum_dropdown`, `build_bool_dropdown`, `build_wavetable_dropdown`, `build_ui_scale_dropdown`, `build_param_readout_box`, `build_header_knob_cell`, `build_header_knob_cell_mod`, `build_knob_cell`, `build_flex_knob_cell_mod`, `build_mini_pill_cell`, `build_flex_pill_cell`, `build_flex_pill_cell_mod`, `build_module_header`, `build_placeholder_bay`, `build_tab_button`).
  - `osc_panel.rs`: Oscillator strip (`OSC 1` – `OSC 4`) builder (`build_osc_panel`).
  - `noise_panel.rs`: Per-voice stereo Noise generator strip builder (`build_noise_panel`).
  - `filter_panel.rs`: Dual multimode filter cards (`FILTER 1` / `FILTER 2`) with live `FilterResponseScope` frequency curve canvas and context-sensitive 4th knob (`build_filter_panel`).
  - `insert_panel.rs`: Compact `INSERT SHAPERS` card (`INS 1` and `INS 2` mode, position, and drive controls) below `FILTER 2` (`build_insert_panel`).
  - `routing_tab.rs`: `ROUTING` bottom tab (`SOURCE FILTER SEND` + `DIRECT` bypasses, `FILTER BUS & FEEDBACK`, `INSERT MIX`).
  - `mod_bar.rs`: Lock-free modulation UI state (`ModUiState`), draggable `ModSourcePill` strip (`E1..E4`, `MS1..MS4`, `VEL`, `KEY`, `MW`, `AT`), 60 Hz telemetry animation timer, and inline slot inspector (`build_mod_source_bar`).
  - `modseq_tab.rs`: `MODSEQ` bottom drawer (`MSEQ 1`–`MSEQ 4` interactive `ModSeqCurveCanvas` + shape presets and `ENV 2`–`ENV 4` `AdsrPreviewScope` + ADSR knobs, `build_modseq_tab`).

---

## GUI Conventions

- Always use `controls::bold_label(cx, text, class)` instead of repeating `.font_family(...).font_weight(...)`.
- Always use `controls::build_dropdown_trigger_labels` / `build_enum_dropdown` / `build_bool_dropdown` for dropdowns, and `controls::build_param_readout_box` for numeric parameter readouts.
- Place new Milestone 2–4 panels (e.g., Filters, Routing, ModSeq/Performer, FX Rack) in their own dedicated submodules under `crates/j-syn/src/editor/`.
