# J-SYN Architecture & Coding Standards

## Core Priority
**Maintainability and readability for both humans and AI are top priorities.** Prefer small, single-responsibility modules (~150–500 lines), descriptive helper functions, and shared UI/DSP primitives over deeply nested closures or copy-pasted boilerplate.

---

## Crate & Module Map

- `crates/j-syn-dsp/`: Real-time audio-thread DSP engine (zero heap allocations in `process()`, shared phase-warp + PM/RM evaluation in `sample_with_warp_and_pm`).
- `crates/j-syn/src/lib.rs`: `nih-plug` VST3/CLAP plugin wrapper and parameter definitions.
- `crates/j-syn/src/theme.css`: Vizia stylesheet defining colors, spacing, and hardware panel styling.
- `crates/j-syn/src/editor/`: Modular Vizia GUI:
  - `mod.rs`: Top-level UI state (`UiData`, `BottomTab`, `UiEvent`), macOS `baseview` resize notification, and high-level window section assembly (`create`, `build_header_bar`, `build_osc_column`, `build_filter_column`, `build_fx_column`, `build_bottom_tabs`).
  - `widgets.rs`: Custom `femtovg` / `ParamWidgetBase` widgets (`RotaryKnob`, `LedToggleSwitch`, `ParamStepPill`).
  - `scope.rs`: Real-time single-cycle `WavetableScope` waveform preview canvas.
  - `controls.rs`: Shared label, dropdown, readout, knob/pill cell, and module chrome helpers (`bold_label`, `build_dropdown_trigger_labels`, `build_enum_dropdown`, `build_bool_dropdown`, `build_wavetable_dropdown`, `build_ui_scale_dropdown`, `build_param_readout_box`, `build_header_knob_cell`, `build_knob_cell`, `build_flex_knob_cell`, `build_mini_pill_cell`, `build_flex_pill_cell`, `build_module_header`, `build_placeholder_bay`, `build_tab_button`).
  - `osc_panel.rs`: Oscillator strip (`OSC 1` – `OSC 4`) builder (`build_osc_panel`).

---

## GUI Conventions

- Always use `controls::bold_label(cx, text, class)` instead of repeating `.font_family(...).font_weight(...)`.
- Always use `controls::build_dropdown_trigger_labels` / `build_enum_dropdown` / `build_bool_dropdown` for dropdowns, and `controls::build_param_readout_box` for numeric parameter readouts.
- Place new Milestone 2–4 panels (e.g., Filters, Routing, ModSeq/Performer, FX Rack) in their own dedicated submodules under `crates/j-syn/src/editor/`.
