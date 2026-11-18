# J-SYN User Guide

Welcome to **J-SYN**, a 64-voice polyphonic wavetable synthesizer VST3 and CLAP plugin.

---

## 1. Top Header Bar
- **View Switcher (`MAIN | FX | MATRIX | ⚙`):**
  - `MAIN`: Core synth chassis (4 Oscillators, Noise, Dual Filters, Insert FX, `BUS A | BUS B` FX 1–4 switcher, and Bottom Drawer).
  - `FX`: Full 4×4 Dual-Bus Master FX Rack (`Bus A` & `Bus B` `FX 1–8` in column-major order) plus the `SUB / DIRECT` & `BUS ROUTING` top banner.
  - `MATRIX`: Full 32-slot interactive Modulation Matrix.
  - `⚙` (Settings): UI Theme, Engine Quality (`Dirty` / `Clean`), `DC Blocker`, `Safety Limiter`, `MODSEQ SWING`, `GEAR SIZE` (`0.10–0.60`), and Presets/Wavetables directories.
    - > **Note on `0 dBFS` Headroom & `DC Blocker`:** The `~2.5 Hz` `DC Blocker` runs **after** the Master FX Rack and `MASTER` gain. Even with a `Limiter` or `Clipper` set to `0.0 dB` ceiling at the end of a bus, highpass phase-tilt on flat-topped or asymmetric waveforms can push output above `0.0 dBFS` unless `Safety Limiter` is enabled (or `DC Blocker` is disabled).
- **Presets (`< [Preset v] > INIT SAVE LOAD`):**
  - Click the preset dropdown to browse `.json` presets organized into collapsible subfolder categories.
  - `<` / `>` steps through all presets across subfolders.
  - `INIT` resets the patch; `SAVE` / `LOAD` writes or opens `.json` patch files.
- **Global Controls:** `UI SCALE` (`85%`–`200%`), `MASTER` output volume (`0.0 dB` default), and live stereo Peak/RMS meter.

---

## 2. Oscillators (`OSC 1` – `OSC 4`) & Noise
- **Enable & Pitch / Ratio:** Toggle each oscillator with its LED switch. Right-click the **`PITCH`** / **`RATIO`** box to switch between **`Pitch (Semitones)`** and **`Ratio (FM Harmonic)`** modes, assign modulation, clear modulation, or reset:
  - **Pitch (`Semitones`) Mode (`±48 st`):** Normal drag = `1.00 st`, `Opt/Alt + Drag` = `±12.00 st`, `Shift + Drag` = `0.01 st`.
  - **Ratio (`FM Harmonic`) Mode (`0.000–32.000 ×`):** Dragging the **Left Half** steps through whole/sub-harmonic ratios (`0, 0.25, 0.5, 1..32`); dragging the **Right Half** adjusts fractions (`0.10`, or `0.01` with `Shift`). Includes a companion **`Hz Offset`** pill (`±1000 Hz` skewed toward `0.1–20 Hz`) beneath `RATIO` with its own right-click modulation menu.
- **Wavetables:** Use `<` / `>` or the wavetable dropdown to choose factory tables, `"Load .WAV..."`, or `.wav` files in your configured `Wavetables Directory` (sliced automatically into up to `256` × `2048`-sample frames). Subfolders appear as collapsible categories.
- **Unison & Tone:** `PHASE`, `UNI` (`1`–`8`), `DETUNE`, `SPREAD`, `WT POS`, `WARP` (`None`, `B-`, `B+`, `SP`), `>`/`<` routing order toggle (`Warp -> Mod` vs `Mod -> Warp`), `MOD` (`Off`, `PM 1..4`, `RM 1..4`), and `LEVEL`. Both `WARP` and `MOD` dropdowns support mouse-wheel scrolling.
- **Noise:** Stereo `White`, `Pink`, or `Brown` noise with `COLOR`, `STEREO`, and `LEVEL`.

---

## 3. Filters & Insert FX
- **Dual Multimode Filters (`FILTER 1` & `FILTER 2`):**
  - Modes: `LP`, `BP`, `HP`, `Notch`, `Peak`, `HP + Peak`, `HP + Notch`, `LP + Notch`, `LP + Peak`, `Peak + Peak`, and `Notch + Notch` (`6/12/24/36/48 dB` slopes), plus fixed `12 dB` `Bandreject`, `Scream`, `Daft`, and `Comb` modes (slope selector automatically locks/dims to `12 dB`), with a live post-modulation frequency response scope.
- **`INSERT FX` (`INS 1` & `INS 2`):**
  - 4x-oversampled per-voice waveshapers (`Tube`, `Mech`, `Tube -> Mech`, `Mech -> Tube`, `Sine Shaper`, `Parabolic`, `Soft Clip`, `Bitcrush`, `Classic A`, `Classic B`, `Classic C`, `Diode A`, `Diode B`, `Linear Fold`, `Sine Fold`, `Hard Clip`, `Half Rect`, `Full Rect`) selectable pre-F1, between F1→F2, or post-F2.

---

## 4. Dual-Bus Master FX Rack (`Bus A` & `Bus B`) + `Sub / Direct Bus`
- **Bus Topology (`PARALLEL (A | B)` vs `SERIAL (A -> B)`):**
  - **`PARALLEL (A | B)`:** `Bus A` and `Bus B` process in parallel and sum into the Master stage alongside the Sub Bus.
  - **`SERIAL (A -> B)`:** `Bus A` (after `Bus A LEVEL/PAN`) feeds directly into `Bus B` for up to 16 serial FX slots.
- **`SUB / DIRECT` Bus:** `[• SUB FX]` toggle, `LP CUTOFF` (`20 Hz–2 kHz`), `CLIP DRIVE` (`0–24 dB`), and `LEVEL`. When `SUB FX` is turned off (`SUB OFF`), LP and Clipper DSP are bypassed and signal routed to `Sub` passes straight through at `SUB LEVEL` as a clean Direct Out.
- **Available Slot Effects:** `Empty`, `Utility` (`0%–500%` M/S `WIDTH`, `PAN`, `±60 dB` `GAIN`, floating `[Ø L]`/`[Ø R]` phase invert), `OTT`, `Distortion`, `MB Dist`, `Clipper`, `Limiter`, `Chorus`, `Phaser`, `Delay`, `Reverb`, `Filter` (all 15 modes + `12/24/48` slopes via floating scope dropdowns), `Dispersion` (up to `128`-stage cascaded allpass bank with floating `STG` selector), and `Tube Res.` (waveguide tube resonator with note-snapping `TUNE`, floating `[• FLT]` toggle, and `MAIN | LFO` sub-tab):
  - **`Distortion`:** Selectable via floating scope dropdown (`Tube`, `Mech`, `Sine`, `Crush`, `Classic A/B/C`, `Diode A/B`, `Linear Fold`, `Sine Fold`, `Hard Clip`, `Half Rect`, `Full Rect`), with `BIAS` (`-100%..+100%` DC-compensated symmetry shift), `DRIVE`, `TONE`, and `MIX`, plus a live transfer-curve visualizer.
  - **`MB Dist` (3-Band Multiband Distortion):** Phase-aligned `LR4` 3-band crossover (`LOW` / `MID` / `HIGH`) with upper-crossover allpass compensation for flat unity summing when dry/bypassed. Each band's algorithm dropdown (`Lin Fold` / `Sin Fold` short labels alongside `Tube`, `Mech`, `Sine`, `Crush`, `Classic A/B/C`, `Diode A/B`, `Hard Clip`, `Half Rect`, `Full Rect`) and solo pill `[S]` sit at the top-left/center/right of the scope: click a band's dropdown or its shaded region on the scope to open that band's knobs (`DRIVE`, `BIAS`, `LEVEL` `-24..+12 dB`, `MIX`) and `[ON]` bypass toggle (stacked below `[XOV]` on the left of the knobs). Plain-clicking `[S]` solos that band exclusively (or un-solos if it is the sole soloed band); hold **`Shift + Click`** on `[S]` to solo multiple bands. Click `[XOV]` to view global controls (`LOW XOV` `40–1000 Hz`, `HIGH XOV` `500–16000 Hz`, `OUT GN` `-18..+18 dB`, `MIX`), or drag the vertical crossover markers directly on the scope (displays a live Hz tooltip while holding/dragging; double-click to reset).
  - **`Limiter` (`MAIN` & `ADV`):** `0.0 ms` lookahead acts as a zero-lookahead transient-clipping hybrid; `1.0–3.0 ms` lookahead uses an S-curve attack ramp and dual-stage program-dependent release for low-IMD limiting. `MAIN` controls: `GAIN` (`0..+24 dB`), `CEIL` (`-18..0 dB`, `-0.1 dB` default), `LOOKAHD` (`0.0–3.0 ms`), `MIX`, and floating `[TP]` (4x True Peak detection, off by default). `ADV` controls: `RELEASE` (`10–500 ms`), `KNEE` (`0%` hard clamp [default] to `100%` soft `TanhAdaa2` saturation on the final safety clipper), `LINK` (`0%` dual-mono to `100%` stereo-linked), and `PUNCH` (`0%–100%`, `100%` default — slows envelope attack so sharp transients feed harder into the final clipper rather than ducking sustain/bass).
- **Floating Scope Controls (`ZStack`):** `Reverb`, `Delay`, and `Chorus` feature a translucent `MAIN | EQ` toggle (`Tube Res.` uses `MAIN | LFO`, `Limiter` uses `MAIN | ADV`) at the top-right of their scope to switch between sub-pages while keeping all slot parameters active and modulatable.
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
- **`ROUTING` Tab:** Filter send balances (`F1 <-> F2`), 4-way per-source `DIRECT` sends (`Off | Bus A | Bus B | Sub` — selecting `Bus A`, `Bus B`, or `Sub` sends that source straight to the chosen bus, **skipping `FILTER 1`, `FILTER 2`, and `INSERT FX`**), per-filter output bus destinations (`Bus A | Bus B | A+B | Sub`), `Serial`/`Parallel` filter routing, and feedback bus.
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
