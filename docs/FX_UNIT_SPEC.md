# J-SYN FX Unit & Standalone Plugin Blueprint

Reference checklist for extracting or redesigning any `J-SYN` FX Unit in `crates/j-syn-ui/src/fx_units/<fx>.rs` and `crates/plugins/j-<fx>/`.

---

## 1. Dimensions & Design Tokens (`crates/j-syn-ui/src/tokens.rs`)
- **Card Width:** `FX_UNIT_W = 200.0 px` (`188.0 px` inner width after `CARD_PAD = 6.0` on left/right).
- **Standalone Window Width:** `standalone_fx_card_width(fx_type) + STANDALONE_FX_PAD * 2.0` = **`224 px`** (`200 + 24`).
- **Card Header (`J-SYN` & Standalone):**
  - `FX_UNIT_HEADER_H = 20.0 px` (matches `CONTROL_H` so dropdown labels never clip).
  - `FX_UNIT_ARROW_H = 18.0 px` (`LedToggleSwitch` and `ArrowButton`s centered with `.top(Stretch(1.0)).bottom(Stretch(1.0))`).
- **Standalone Header Bar (`224 px` window, `6 px` horizontal padding):**
  - Left: `[Brand Pill]` + `bold_label(cx, "J-<FX>", "header-title").font_size(11.5)` (fits full names like `J-DISPERSION` cleanly).
  - Right: `22 × 22 px` cogwheel button (`GearIcon`) toggling an in-card **Settings** view (`THEME` + `UI SCALE` backed by shared `standalone-fx.json`, synced on window reopen).
- **Scope Height (`fx_unit_scope_height`):**
  - Compact (`Utility`, `JSatClipper`, `Distortion`): `FX_UNIT_SCOPE_COMPACT_H = 44.0 px`
  - Medium (`Chorus`, `Flanger`, `Phaser`, `Delay`, `OTT`, `Diopser`): `FX_UNIT_SCOPE_H = 58.0 px`
  - Tall (`Filter`, `Limiter`, `Reverb`, `TubeResonator`, `MultibandDist`): `FX_UNIT_SCOPE_TALL_H = 80.0 px`

---

## 2. Scope Overlays & `ScrollView` Clipping
- Place `FxSlotVisualizer` first inside a `ZStack` with `.hoverable(false)`.
- Declare interactive overlays (corner pills, dropdowns, drag overlays) **after** the visualizer with `.position_type(PositionType::SelfDirected)`.
- **NEVER use `.z_index(...)` inside a `ScrollView`:** Vizia hoists `z_index > 0` elements into a global bucket above `ScrollView`'s clip rect, causing scrolled-out buttons to float over `BUS A/B/C` headers. `ZStack` declaration order already handles drawing and hit-testing.

---

## 3. Tab Bars, Context Strips & Constant Height
- **Tab Bar (if tabbed):** `HStack` of `Stretch(1.0)` `fx-mb-side-btn` pills, `height(Pixels(tokens::PILL_H))` (`18.0 px`).
- **Context / Mode Strip:** Fixed `height(Pixels(tokens::CONTROL_H))` (`20.0 px`) across **all** tabs so card and standalone window heights never jump when switching tabs (e.g., live parameter readout on the main tab, `[● ON/OFF] [Mode ▼]` on sub-tabs).

---

## 4. Equidistant `2×2` Knob Grid (and `2×1` Rows)
- Use `KNOB_FX_UNIT_2X2 = 46.0 px` knobs (`FX_UNIT_KNOB_ROW_2X2_H = 64.0 px` per row).
- Add `.class("fx-unit-2x2")` to the knob grid container so `.fx-full-card .fx-unit-2x2 .knob-title` renders at `9.5 pt` inside `height(Pixels(tokens::KNOB_TITLE_H))` (`14.0 px`) with `.row_between(Pixels(tokens::SPACE_XS))` (`2.0 px`).
- On each 2-knob `HStack` row:
  - `.col_between(Pixels(0.0))`
  - `.child_left(Pixels(tokens::FX_UNIT_2X2_INSET_X))` (`27.0 px`)
  - `.child_right(Pixels(tokens::FX_UNIT_2X2_INSET_X))` (`27.0 px`)
  - Knob centers land at `66.5 px` and `133.5 px` inside the `200 px` card (**equidistant `66.5 / 67.0 / 66.5 px`**).
- On the `2×2` `VStack`:
  - `.row_between(Pixels(tokens::SPACE_MD))` (`6.0 px`)
  - `.top(Pixels(tokens::FX_UNIT_2X2_TOP_PAD))` (`10.0 px`)
  - `.bottom(Pixels(tokens::FX_UNIT_2X2_BOTTOM_PAD))` (`6.0 px`)
  - Enforces visual vertical equidistance (`~17 px` each) between the bottom of the scope/tab/dropdown and Row 1 labels, between Row 1 knobs and Row 2 labels, and between Row 2 knobs and the bottom of the FX card.

---

## 5. Modulation Wiring
- **All continuous knobs across all tabs must be modulatable** in `J-SYN` via `ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 0..16)`.
- `RotaryKnob::drop` automatically calls `ModUiHost::unregister_drop_rect` on unmount so tab/sub-tab switches never leave stale drop rects.
- Ensure `slot_scope_snapshot` in `fx_scopes.rs` and `FxBusState::process` in `j-syn-dsp/src/fx/mod.rs` both apply modulation offsets for all active tab parameters.

---

## 6. Shared `nih_plug::Enum` Parameters (`crates/j-syn-ui/src/fx_units/params.rs`)
- `j-syn-dsp` intentionally has **zero `nih_plug` dependency**, while `j-syn-ui` depends on `nih_plug` and is shared by both `crates/j-syn/` and `crates/plugins/j-*/`.
- Consolidate all shared `nih_plug::Enum` parameter types, DSP `From` conversions, and dropdown item tables (`DistortionModeParam`, `DiopserStagesParam`, `FilterModeParam`, `FxFilterSlopeParam`) in `crates/j-syn-ui/src/fx_units/params.rs` (re-exported via `j_syn_ui::fx_units::*`).
- `crates/j-syn/src/params/` re-exports these enums verbatim so `J-SYN` presets remain 100% binary/JSON compatible while sharing a single source of truth with standalone FX plugins.

---

## 7. Verification Gate
1. Render headless PNG previews and inspect visually for zero clipping, alignment, and constant tab height:
   - `cargo run --offline -p j-<fx> --example preview_j_<fx> -- --out target/previews/j-<fx>-100.png`
   - `cargo run --offline -p j-syn --example preview_j_syn -- --out target/previews/j-syn-<fx>-100.png`
2. `cargo test --offline --workspace` (zero project warnings).
3. `cargo xtask bundle-all --release --offline` (zero project warnings).
