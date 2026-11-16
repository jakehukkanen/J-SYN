# J-SYN User Guide

Welcome to **J-SYN**, a 64-voice polyphonic wavetable synthesizer VST3 and CLAP plugin.

---

## 1. Top Header Bar
- **View Switcher (`MAIN | FX | MATRIX | ⚙`):**
  - `MAIN`: Core synth chassis (4 Oscillators, Noise, Dual Filters, Insert FX, FX 1–4, and Bottom Drawer).
  - `FX`: Full 2×4 Master FX Rack (`FX 1`–`FX 8`).
  - `MATRIX`: Full 32-slot interactive Modulation Matrix.
  - `⚙` (Settings): UI Theme, Engine Quality (`Dirty` / `Clean`), `DC Blocker`, `Safety Limiter`, and Presets/Wavetables directories.
- **Presets (`< [Preset v] > INIT SAVE LOAD`):**
  - Click the preset dropdown to browse `.json` presets organized into collapsible subfolder categories.
  - `<` / `>` steps through all presets across subfolders.
  - `INIT` resets the patch; `SAVE` / `LOAD` writes or opens `.json` patch files.
- **Global Controls:** `UI SCALE` (`85%`–`200%`), `MASTER` output volume (`0.0 dB` default), and live stereo Peak/RMS meter.

---

## 2. Oscillators (`OSC 1` – `OSC 4`) & Noise
- **Enable & Pitch:** Toggle each oscillator with its LED switch. Drag the `PITCH` box (`±48 st`):
  - **Normal Drag:** Semitones (`1.00 st`)
  - **`Opt/Alt + Drag`:** Octaves (`±12.00 st`)
  - **`Shift + Drag`:** Fine cents (`0.01 st`)
- **Wavetables:** Use `<` / `>` or the wavetable dropdown to choose factory tables, `"Load .WAV..."`, or `.wav` files in your configured `Wavetables Directory` (sliced automatically into up to `256` × `2048`-sample frames). Subfolders appear as collapsible categories.
- **Unison & Tone:** `PHASE`, `UNI` (`1`–`8`), `DETUNE`, `SPREAD`, `WT POS`, `WARP` (`None`, `B-`, `B+`, `SP`), `>`/`<` routing order toggle (`Warp -> Mod` vs `Mod -> Warp`), `MOD` (`Off`, `PM 1..4`, `RM 1..4`), and `LEVEL`. Both `WARP` and `MOD` dropdowns support mouse-wheel scrolling.
- **Noise:** Stereo `White`, `Pink`, or `Brown` noise with `COLOR`, `STEREO`, and `LEVEL`.

---

## 3. Filters & Insert FX
- **Dual Multimode Filters (`FILTER 1` & `FILTER 2`):**
  - Modes: `LP`, `BP`, `HP` (`6/12/24/36/48 dB` slopes), `Scream`, `Daft`, `Comb`, `HP+Peak`, `HP+Notch`, `LP+Notch`, `LP+Peak`, with a live post-modulation frequency response scope.
- **`INSERT FX` (`INS 1` & `INS 2`):**
  - Per-voice waveshapers (`Tube`, `Mech`, `Tube -> Mech`, `Mech -> Tube`, `Sine`, `Parabolic`, `SoftClip`, `Bitcrush`) selectable pre-F1, between F1→F2, or post-F2.

---

## 4. Master FX Rack (`FX 1` – `FX 8`)
- Available slots: `Empty`, `OTT`, `Distortion`, `Chorus`, `Delay`, `Reverb`, `Clipper`, `Filter`.
- Each slot has an `ON/OFF` toggle, `STACK 1..4` multiplier, live `150 Hz` pre/post scope, and `▲`/`▼` slot-swapping buttons in the `FX` view.
- Empty slots default to `ON` with `Mix = 100%`; cards only dim when toggled `OFF`.

---

## 5. Bottom Drawer & Modulation System
- **Modulation Pills (`E1`–`E4`, `MS1`–`MS4`, `VEL`, `KEY`, `MW`, `AT`):**
  - **Drag** any pill onto a knob or value pill to assign modulation.
  - **Click** `E1`–`E4` or `MS1`–`MS4` to open its editor in the `MODSEQ` / `ENV` view.
  - **Collar Handle:** Hover/drag the radial modulation arc around a knob (`RowResize` cursor) to adjust depth.
  - **Right-Click** any modulated control to open a quick menu to **Bypass (`ON`/`BYP`)**, toggle **Polarity (`UNI`/`BIP`)**, or **Remove (`x`)** each routing.
- **`VOICE` Tab:** `F1/F2 KEYTRK`, `OSC 1`–`OSC 4` keytrack toggles, `VOICING` (`Poly`/`Mono`, `PB RANGE`, `GLIDE`), and `OSCILLATOR PAN`.
- **`MODSEQ` Tab:** Switch between `MODSEQ` (16-step bar/shape stamping + `2×9` shape palette; right-click tile to fill) and `LFO` (breakpoint MSEG curve with `SNAP 1/16` and presets), or edit `ENV 1`–`ENV 4` ADSR envelopes.
- **`ROUTING` Tab:** Filter send balances, `DIRECT` bypasses, `Serial`/`Parallel` filter routing, and feedback bus.
- **`MACROS` Tab:** 8 macros (`M1`–`M8`) with click-to-rename custom labels reflected in the `MATRIX` and modulation menus.

---

## 6. Settings & Configuration Persistence
- Preferences (`presets_dir`, `wavetables_dir`, `theme`, `ui_scale`) are saved automatically to:
  - **macOS:** `~/Library/Application Support/j-syn/config.json`
  - **Windows:** `%APPDATA%\j-syn\config.json`
  - **Linux:** `~/.config/j-syn/config.json`
- Custom `.json` themes placed in `<config_dir>/themes/` are scanned on startup and via `Rescan` in the Settings view.

---

## 7. Mouse & Keyboard Shortcuts
- **Double-Click / `Cmd+Click` / `Ctrl+Click`:** Reset knob or pill to default.
- **`Shift + Drag`:** Fine precision adjustment.
- **`Opt/Alt + Drag` (Oscillator `PITCH`):** Step by octaves (`±12.00 st`).
- **`Shift + Right-Click` (Modulated control):** Quick-remove primary modulation slot.
- **Mouse Wheel over Dropdowns:** Step through options without opening the popup.
