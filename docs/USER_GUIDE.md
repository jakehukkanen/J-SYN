# J-SYN User Guide

Welcome to **J-SYN**, a 64-voice polyphonic wavetable synthesizer VST3 and CLAP plugin.

---

## 1. Top Header Bar
- **View Switcher (`MAIN | FX | MATRIX | ⚙`):**
  - `MAIN`: Core synth chassis (4 Oscillators, Noise, Dual Filters, Insert FX, `BUS A | BUS B` FX 1–4 switcher, and Bottom Drawer).
  - `FX`: Full 4×4 Dual-Bus Master FX Rack (`Bus A` & `Bus B` `FX 1–8` in column-major order) plus the `SUB / DIRECT` & `BUS ROUTING` top banner.
  - `MATRIX`: Full 32-slot interactive Modulation Matrix.
  - `⚙` (Settings): UI Theme, Engine Quality (`Dirty` / `Clean`), `DC Blocker`, `Safety Limiter`, `MODSEQ SWING`, `GEAR SIZE` (`0.10–0.60`), and Presets/Wavetables directories.
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
  - Modes: `LP`, `BP`, `HP`, `Notch`, `Peak`, `HP + Peak`, `HP + Notch`, `LP + Notch`, `LP + Peak`, `Peak + Peak`, and `Notch + Notch` (`6/12/24/36/48 dB` slopes), plus fixed `12 dB` `Bandreject`, `Scream`, `Daft`, and `Comb` modes (slope selector automatically locks/dims to `12 dB`), with a live post-modulation frequency response scope.
- **`INSERT FX` (`INS 1` & `INS 2`):**
  - Per-voice waveshapers (`Tube`, `Mech`, `Tube -> Mech`, `Mech -> Tube`, `Sine`, `Parabolic`, `SoftClip`, `Bitcrush`) selectable pre-F1, between F1→F2, or post-F2.

---

## 4. Dual-Bus Master FX Rack (`Bus A` & `Bus B`) + `Sub / Direct Bus`
- **Bus Topology (`PARALLEL (A | B)` vs `SERIAL (A -> B)`):**
  - **`PARALLEL (A | B)`:** `Bus A` and `Bus B` process in parallel and sum into the Master stage alongside the Sub Bus.
  - **`SERIAL (A -> B)`:** `Bus A` (after `Bus A LEVEL/PAN`) feeds directly into `Bus B` for up to 16 serial FX slots.
- **`SUB / DIRECT` Bus:** `[• SUB FX]` toggle, `LP CUTOFF` (`20 Hz–2 kHz`), `CLIP DRIVE` (`0–24 dB`), and `LEVEL`. When `SUB FX` is turned off (`SUB OFF`), LP and Clipper DSP are bypassed and signal routed to `Sub` passes straight through at `SUB LEVEL` as a clean Direct Out.
- **Available Slot Effects:** `Empty`, `OTT`, `Distortion`, `Chorus`, `Phaser`, `Delay`, `Reverb`, `Clipper`, `Filter` (all 15 modes + `12/24/48` slopes via floating scope dropdowns), and `Dispersion` (up to `128`-stage cascaded allpass bank with floating `STG` selector).
- **Floating Scope Controls (`ZStack`):** `Reverb`, `Delay`, and `Chorus` feature a translucent `MAIN | EQ` toggle at the top-right of their scope to switch between main and EQ/spatial parameters while keeping all 8 parameters active and modulatable.
- Each slot has an `ON/OFF` toggle, `STK 1..4` multiplier, live `150 Hz` pre/post scope, and `▲`/`▼` slot-swapping buttons in the `FX` view. Empty slots default to `ON` with `Mix = 100%`; cards only dim when toggled `OFF`.

---

## 5. Bottom Drawer, MPE & Modulation System
- **Modulation Pills (`E1`–`E4`, `MS1`–`MS4`, `VEL`, `KEY`, `MW`, `AT`, `SLD`):**
  - **Drag** any pill onto a knob or value pill to assign modulation.
  - **Click** `E1`–`E4` or `MS1`–`MS4` to open its editor in the `MODSEQ` / `ENV` view.
  - **Collar Handle:** Hover/drag the radial modulation arc around a knob to adjust depth.
  - **Right-Click** any modulated control to open a quick menu to **Bypass (`ON`/`BYP`)**, toggle **Polarity (`UNI`/`BIP`)**, or **Remove (`×`)** each routing.
- **`VOICE` Tab & MPE:**
  - `KEYTRACKING` (`F1/F2 KEYTRK`, `OSC 1`–`OSC 4` keytrack toggles), `VOICING` (`Poly`/`Mono`, `PB RANGE`, **`[• MPE]`**, **`MPE PB`** `1–96 st` [default `48 st`], `GLIDE`), and `OSCILLATOR PAN`.
  - When **`MPE`** is enabled, Master `Ch 1` broadcasts global bend (`±PB RANGE`), `CC 1`, and pressure, while Member Channels `Ch 2–16` route per-note Pitch Bend (`±MPE PB`), Pressure (`AT`), and `CC 74` Slide (`SLD`) exclusively to each active voice with `1.5 ms` smoothing (alongside CLAP/VST3 `PolyPressure`, `PolyTuning`, and `PolyBrightness`).
- **`MODSEQ` Tab:**
  - **`MODSEQ` (Performer) Mode:**
    - **Left-Drag on Grid:** Stamps the selected `2×9` palette shape at the cursor's vertical depth.
    - **Right-Click a Palette Tile:** Fills the entire sequence with that shape (`FILL` fills with the currently highlighted tile).
    - **`Opt/Alt + Left-Drag` or `Right-Drag` (Vertical on a Step):** Adjusts that step's curve **tension** (`-1.0..=1.0`, warping its timing/concavity).
    - **`Shift + Left-Drag` (Vertical on a Step):** Adjusts the step's **end level** (linear ramp from start level to end level).
  - **`LFO` (MSEG) Mode:**
    - **Drag Breakpoint Nodes:** Move nodes (`Shift` or `SNAP 1/16` snaps `X` to `1/16` and `Y` to `1/8`).
    - **Double-Click:** Add a new breakpoint node on empty canvas, or delete an interior node.
    - **Drag Midpoint Handle / `Opt/Alt + Left-Drag` / `Right-Drag`:** Bend the enclosing segment's curve **tension**.
  - **`ENV 1`–`ENV 4`:** Click `E1`–`E4` in the mod bar to edit ADSR envelopes with live curve preview.
- **`ROUTING` Tab:** Filter send balances, 4-way per-source `DIRECT` sends (`Off | Bus A | Bus B | Sub`), per-filter output bus destinations (`Bus A | Bus B | A+B | Sub`), `Serial`/`Parallel` filter routing, and feedback bus.
- **`MACROS` Tab:** 8 macros (`M1`–`M8`) with click-to-rename custom labels reflected in the `MATRIX` and modulation menus.

---

## 6. Settings & Configuration Persistence
- Preferences (`presets_dir`, `wavetables_dir`, `theme`, `ui_scale`) are saved automatically to:
  - **macOS:** `~/Library/Application Support/j-syn/config.json`
  - **Windows:** `%APPDATA%\j-syn\config.json`
  - **Linux:** `~/.config/j-syn/config.json`
- Custom `.json` themes placed in `<config_dir>/themes/` are scanned on startup and via `Rescan` in the Settings view.
- **Experimental (`MODSEQ SWING`):** Disabled by default for straight step modulation. When enabled in Settings, applies alternating step-phase tension warping (`-0.4 / 0.0 / ...`) to `MODSEQ` sequences and preserves per-step tension when stamping.

---

## 7. Mouse & Keyboard Shortcuts
- **Double-Click / `Cmd+Click` / `Ctrl+Click`:** Reset knob or pill to default.
- **`Shift + Drag`:** Fine precision adjustment.
- **`Opt/Alt + Drag` (Oscillator `PITCH`):** Step by octaves (`±12.00 st`).
- **`Shift + Right-Click` (Modulated control):** Quick-remove primary modulation slot.
- **Mouse Wheel over Dropdowns:** Step through options without opening the popup.
