---
applyTo: "crates/j-syn/src/editor/**,crates/j-syn-ui/**,crates/plugins/**"
---

# Vizia UI & FX Unit Review Checklist

1. **Single Timer Rule:** Never introduce a new `cx.add_timer(...)` in `crates/j-syn/src/editor/` — extend the single 60 Hz timer in `mod_bar.rs`.
2. **Drag Safety:** Never emit `UiEvent::ModUiChanged` / `UiKitEvent::ModUiChanged` during continuous pointer drags inside a `mod_ui_rev` `Binding`; use `ModValChanged` so `cx.capture()` widgets survive drags.
3. **ScrollView & Overlay Rules:**
   - Never use `.z_index(...)` inside a `ScrollView`.
   - Never apply `.hoverable(false)` to a container holding interactive overlay buttons/dropdowns.
   - Follow `docs/FX_UNIT_SPEC.md` for constant-height tabbed FX Units (`FX_UNIT_W = 200 px`) and equidistant `2x2` / `3-over-2` knob grids using `j_syn_ui::tokens::*`.
4. **Vector Buttons & CSS Hygiene:**
   - Use `ArrowButton`, `CrossButton`, `CollapseButton`, or `VectorArrow::caret` instead of Unicode arrows or child views inside `vizia::Button`.
   - Avoid unsupported Vizia CSS selectors (especially `:focus-within`).
5. **Theme Compliance:** All `femtovg` colors in `draw()` must flow through `theme::current_palette(cx)` (no unmapped hex or hardcoded RGB palette literals outside `theme.rs`).
6. **Naming:** Always use `FX Unit` (`fx_unit` / `FX_UNIT_*`), never `snapin`.
