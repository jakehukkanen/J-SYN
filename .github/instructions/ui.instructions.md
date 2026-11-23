---
applyTo: "crates/j-syn/src/editor/**,crates/j-syn-ui/**,crates/plugins/**"
---

# Vizia UI & FX Unit Review Checklist

1. **Single Timer Rule:** Never introduce a new `cx.add_timer(...)` in `crates/j-syn/src/editor/` — extend the single 60 Hz timer in `mod_bar.rs`.
2. **Drag Safety:** Never emit `UiEvent::ModUiChanged` / `UiKitEvent::ModUiChanged` during continuous pointer drags inside a `mod_ui_rev` `Binding`; use `ModValChanged` so `cx.capture()` widgets survive drags.
3. **ScrollView & Overlay Rules:**
   - Never use `.z_index(...)` inside a `ScrollView` or anywhere under `crates/j-syn/src/editor/fx_rack/` (place top-level `SelfDirected` overlays at `crates/j-syn/src/editor/` root).
   - Never apply `.hoverable(false)` to a container holding interactive overlay buttons/dropdowns.
   - Set `.min_height(Percentage(100.0))` on `ScrollView` inner content stacks when a trailing `Stretch(1.0)` drop/insert zone must span remaining viewport height.
   - Compute floating menu/popover clamping heights deterministically from shared layout constants + safety cushion (never stale magic numbers) and cover with a bounds unit test.
   - Follow `docs/FX_UNIT_SPEC.md` for constant-height tabbed FX Units (`FX_UNIT_W = 200 px`) and equidistant `2x2` / `3-over-2` knob grids using `j_syn_ui::tokens::*`.
4. **Vector Buttons, `femtovg` Text & CSS Hygiene:**
   - Use `ArrowButton`, `CrossButton`, `CollapseButton`, or `VectorArrow::caret` instead of Unicode arrows or child views inside `vizia::Button`.
   - Always bind a `FontId` via `j_syn_ui::shell::ensure_canvas_bold_font` before calling `canvas.fill_text(...)` (otherwise `femtovg` silently returns `NoFontFound`).
   - Pair any CSS `opacity < 1.0` modifier class with an explicit `opacity: 1;` base rule and/or reactive `.opacity(...)` in Rust to avoid Vizia's sticky opacity bug.
   - Give bold uppercase single-line headers `>= 18 px` height (or `Auto`) so Plus Jakarta Sans Bold ascenders never clip against `bounds.y`.
   - Avoid unsupported Vizia CSS selectors (especially `:focus-within`).
5. **Theme Compliance:** All `femtovg` colors in `draw()` must flow through `theme::current_palette(cx)` (no unmapped hex or hardcoded RGB palette literals outside `theme.rs`).
6. **Naming:** Always use `FX Unit` (`fx_unit` / `FX_UNIT_*`), never `snapin`, and avoid third-party synth/plugin brand names in comments or docs.
