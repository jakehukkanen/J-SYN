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
   - Read `AGENTS.md`, `crates/j-syn-ui/src/tokens.rs`, and `crates/plugins/j-utility/` as the reference template.
2. **Build Shared FX Unit UI (`crates/j-syn-ui/`):**
   - Implement the reusable variable-height FX Unit view (`220 px` target card width) in `crates/j-syn-ui/` so both `crates/plugins/j-<fx>/` and `J-SYN`'s Right Wing share the exact same layout and `ThemePalette` / `tokens::*` styling.
   - Never call slot cards `snapin`; always use `fx_unit` / `FX Unit`.
3. **Wire Standalone Plugin (`crates/plugins/j-<fx>/`):**
   - Expose DSP parameters from `j-syn-dsp`, wrap the shared `j-syn-ui` FX Unit view, and register the crate in root `Cargo.toml`.
4. **Visual & Build Verification Gate:**
   - Render PNG previews via the headless preview harness (both standalone and inside `J-SYN`'s rack) and inspect for clipping, truncation, or alignment issues.
   - Run `cargo test --offline --workspace` and `cargo xtask bundle-all --release --offline` with zero project compiler warnings.
5. **Ship:**
   - Update `docs/USER_GUIDE.md` if controls/UX changed, commit, push `feat/standalone-<fx>`, and open a PR targeting `main`.
