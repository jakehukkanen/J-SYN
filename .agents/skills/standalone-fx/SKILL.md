---
name: standalone-fx
description: >-
  Extract or redesign a J-SYN FX effect as a standalone VST3/CLAP plugin in
  crates/plugins/ with a shared variable-height FX Unit view in crates/j-syn-ui/.
  Use when spinning out a new standalone FX plugin or iterating on an FX Unit's UX.
---

# Standalone FX Plugin & Shared FX Unit Workflow

1. **Branch & Read Reference:**
   - Create `feat/standalone-<fx>` off `main`.
   - Read `AGENTS.md`, `docs/FX_UNIT_SPEC.md`, `crates/j-syn-ui/src/tokens.rs`, and reference plugins (`crates/plugins/j-flanger/`, `crates/plugins/j-mbdist/`).
2. **Build Shared FX Unit UI (`crates/j-syn-ui/src/fx_units/`):**
   - Implement the reusable variable-height FX Unit view (`FX_UNIT_W = 200 px` card width; `224 px` standalone window via `standalone_shell.rs`) using `KNOB_FX_UNIT_2X2 = 46 px` equidistant `2×2` knob grids (`docs/FX_UNIT_SPEC.md` §4).
   - Place any shared `nih_plug::Enum` parameter types and dropdown item tables in `crates/j-syn-ui/src/fx_units/params.rs` (re-exported in `j-syn/src/params/` to preserve preset compatibility).
   - Never call slot cards `snapin`; always use `fx_unit` / `FX Unit`. Never use `.z_index(...)` inside a `ScrollView`.
3. **Wire Standalone Plugin (`crates/plugins/j-<fx>/`):**
   - Expose DSP structs/methods from `j-syn-dsp`, wrap `build_standalone_fx_shell` + the shared `j-syn-ui` FX Unit view, and register the crate in root `Cargo.toml`.
4. **Visual & Build Verification Gate:**
   - Render PNG previews via the headless preview harness for every tab (both standalone and inside `J-SYN`'s Right Wing) and visually inspect for clipping, alignment, and constant tab height.
   - Run `cargo test --offline --workspace` and `cargo xtask bundle-all --release --offline` with zero project compiler warnings.
5. **Ship:**
   - Update `docs/USER_GUIDE.md` if controls/UX changed, commit, push `feat/standalone-<fx>`, and open a PR targeting `main`.
