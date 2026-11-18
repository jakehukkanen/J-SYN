# J-SYN User Guide

Welcome to **J-SYN**, a 64-voice polyphonic wavetable synthesizer VST3 and CLAP plugin featuring a `920 × 840` 2-Wing `MAIN` workspace (`Left Wing`: Oscillators & Noise, `Center Wing`: Dual Filters, Center Modulation Stage, Insert Shapers & Permanent `2×4` Macro/Source Dock) plus a dedicated **`FX`** view hosting the 3-Lane Master FX Rack (`BUS A | BUS B | BUS C`).

---

## 1. Top Header Bar
- **View Switcher (`MAIN | FX | MATRIX | ⚙`):**
  - `MAIN`: 2-Wing synth chassis (Oscillators & Noise, Dual Filters, Insert 1/2, Center Stage Tabs, and Permanent Macro & Source Dock).
  - `FX`: Full-width 3-Lane `BUS A | BUS B | BUS C` Snapin FX Rack with a permanent draggable modulation source pill strip (`E1–E4`, `MS1–MS4`, `VEL–SLD`, `M1–M8`) across the top.
  - `MATRIX`: Full 32-slot interactive Modulation Matrix.
  - `⚙` (Settings): UI Theme, Engine Quality (`Dirty` / `Clean`), `DC Blocker`, `Safety Limiter`, `MODSEQ SWING`, `GEAR SIZE` (`0.10–0.60`), and Presets/Wavetables directories.
    - > **Note on `0 dBFS` Headroom & `DC Blocker`:** The `~2.5 Hz` `DC Blocker` runs **after** the Master FX Rack and `MASTER` gain. Even with a `Limiter` or `Clipper` set to `0.0 dB` ceiling at the end of a bus, highpass phase-tilt on flat-topped or asymmetric waveforms can push output above `0.0 dBFS` unless `Safety Limiter` is enabled (or `DC Blocker` is disabled).
- **Presets (`< [Preset v] > INIT SAVE LOAD`):**
  - Click the preset dropdown to browse `.json` presets organized into collapsible subfolder categories.
  - `<` / `>` steps through all presets across subfolders.
  - `INIT` resets the patch; `SAVE` / `LOAD` writes or opens `.json` patch files.
- **Global Controls:** `UI SCALE` (`85%`–`200%`), `MASTER` output volume (`0.0 dB` default), and live stereo Peak/RMS meter.

---

## 2. Left Wing — Oscillators (`OSC 1` – `OSC 4`) & `NOISE`
- **Wavetables & Enable:** Toggle each oscillator with its header LED switch, and use `<` / `>` or the wavetable dropdown (spanning the header bar) to choose factory tables, `"Load .WAV..."`, or `.wav` files in your configured `Wavetables Directory` (sliced automatically into up to `256` × `2048`-sample frames). Subfolders appear as collapsible categories.
- **Pitch / Ratio:** Right-click the **`PITCH`** / **`RATIO`** box to switch between **`Pitch (Semitones)`** and **`Ratio (FM Harmonic)`** modes, assign modulation, clear modulation, or reset:
  - **Pitch (`Semitones`) Mode (`±48 st`):** Normal drag = `1.00 st`, `Opt/Alt + Drag` = `±12.00 st`, `Shift + Drag` = `0.01 st`.
  - **Ratio (`FM Harmonic`) Mode (`0.000–32.000 ×`):** Dragging the **Left Half** steps through whole/sub-harmonic ratios (`0, 0.25, 0.5, 1..32`); dragging the **Right Half** adjusts fractions (`0.10`, or `0.01` with `Shift`). Includes a companion **`Hz Offset`** pill (`±1000 Hz` skewed toward `0.1–20 Hz`) beneath `RATIO` with its own right-click modulation menu.
- **Main Osc Knobs & Bottom Dock:** `WT POS`, `WARP` (with `WARP` mode dropdown `None`, `B-`, `B+`, `SP` directly underneath), `>`/`<` routing order toggle (`Warp -> Mod` vs `Mod -> Warp`) centered between the dropdowns, `MOD` (with `MOD` dropdown `Off`, `PM 1..4`, `RM 1..4` directly underneath), and `LEVEL`. Both `WARP` and `MOD` dropdowns support mouse-wheel scrolling. Below the waveform scope and knobs, each card docks 5 balanced controls: **`RETRIG`**, **`PHS`** (`Phase`), **`UNI`** (`1`–`8`), **`DET`** (`Detune`), and **`SPD`** (`Spread`). Per-oscillator `PAN` and `KT` remain in the **`VOICE`** Center Stage tab.
- **Noise:** Stereo `White`, `Pink`, or `Brown` noise with `COLOR`, `STEREO`, and `LEVEL`. Per-source filter send balances and Direct Bus routing live in the **`ROUTE`** tab.

---

## 3. Center Wing — Dual Filters, Insert Shapers & Center Stage
- **Dual Multimode Filters (`FILTER 1` & `FILTER 2`):**
  - Modes: `LP`, `BP`, `HP`, `Notch`, `Peak`, `HP + Peak`, `HP + Notch`, `LP + Notch`, `LP + Peak`, `Peak + Peak`, and `Notch + Notch` (`6/12/24/36/48 dB` slopes), plus fixed `12 dB` `Bandreject`, `Scream`, `Daft`, and `Comb` modes (slope selector automatically locks/dims to `12 dB`), with a live post-modulation frequency response scope.
  - Filter topology (`Serial (F1->F2)` / `Parallel (F1+F2)`), `F1 OUT BUS` / `F2 OUT BUS`, `F1/F2 MIX`, and `FEEDBACK` live in the **`ROUTE`** Center Stage tab.
- **Center Modulation Stage (`E1–4 | MS1–4 | VOICE | ROUTE`):**
  - The **`E1`–`E4`** and **`MS1`–`MS4`** tab pills are dual-purpose: **Click** to switch tabs, or **Drag** onto any knob/pill to assign modulation directly from the tab bar.
- **Side-by-Side `INS 1` & `INS 2`:**
  - Header controls: **`[INS 1]` / `[INS 2]`** LED bypass switch (greys out the card when toggled `OFF`), compact waveshaper algorithm dropdown (`Off`, `Tube`, `Mech`, `T -> M`, `M -> T`, `Sine`, `Parabolic`, `Soft Clip`, `Crush`, `Classic A/B/C`, `Diode A/B`, `Lin Fold`, `Sin Fold`, `Hard Clip`, `Half Rect`, `Full Rect`), and position selector (**`| F1`**, **`F1 | F2`**, **`| F2`**).
  - Body: Real-time waveshaper transfer curve visualizer alongside `DRIVE` and dry/wet **`MIX`** knobs.

---

## 4. `FX` View — 3-Lane Snapin FX Rack (`BUS A | BUS B | BUS C`)
- **Three Parallel / Routable 8-Slot FX Lanes (`BUS A`, `BUS B`, `BUS C`):**
  - Each lane features an 8-slot scrollable stack of Snapin cards (`ON/OFF`, effect selector, `↑`/`↓` slot reordering, `STK 1..4` stacking, and live `150 Hz` pre/post visualizer scope) plus lane header `L` (level) and `P` (pan) mini-knobs.
  - **Lane-to-Lane Routing:** Click the target badge in the `BUS A` or `BUS B` lane header (or use **`FX LANE MATRIX`** in the **`ROUTE`** tab) to configure 6 routing topologies (`Bus C` always outputs `-> MST`):
    - **`Parallel (A | B | C)`:** All 3 lanes sum independently to Master.
    - **`Serial (A -> B | C)`:** `Bus A` feeds `Bus B`; `Bus C` runs in parallel.
    - **`Serial All (A -> B -> C)`:** Cascades all 3 lanes (`24` total serial FX slots).
    - **`A+B -> C`:** Both `Bus A` and `Bus B` sum into `Bus C`.
    - **`A -> C | B`:** `Bus A` feeds `Bus C`; `Bus B` goes direct to Master.
    - **`A | B -> C`:** `Bus A` goes to Master; `Bus B` feeds `Bus C`.
  - Filter outputs (`A+B`, `A+C`, `B+C`, `A+B+C`) and direct sends automatically guard against double-feeding when routed into a downstream serial lane.
- **Available Slot Effects:** `Empty`, `Utility` (`0%–500%` M/S `WIDTH`, `PAN`, `±60 dB` `GAIN`, floating `[Ø L]`/`[Ø R]` phase invert), `OTT`, `Distortion`, `MB Dist`, `Clipper`, `Limiter`, `Chorus`, `Flanger`, `Phaser`, `Delay`, `Reverb`, `Filter` (all 15 modes + `12/24/48` slopes via floating scope dropdowns), `Dispersion` (up to `128`-stage cascaded allpass bank with floating `STG` selector), and `Tube Res.` (waveguide tube resonator with note-snapping `TUNE`, floating `[• FLT]` toggle, and `MAIN | LFO` sub-tab):
  - **`Flanger` (`MAIN` & `MOD`):** Cubic Hermite interpolated stereo short-delay (`0.1–15 ms`) jet/comb flanger with `MAIN` (`RATE` `0.02–15 Hz`, `DEPTH`, `DELAY`, `MIX`) and `MOD` (`FEEDBK` `-99%..+99%` bipolar hollow vs. resonant comb feedback, `PHASE` `0°–180°` L/R LFO offset, `LOW CUT`, `TONE`), plus a live L/R comb response & LFO phase orbit scope.
  - **`Distortion`:** Selectable via floating scope dropdown (`Tube`, `Mech`, `Sine`, `Crush`, `Classic A/B/C`, `Diode A/B`, `Linear Fold`, `Sine Fold`, `Hard Clip`, `Half Rect`, `Full Rect`), with `BIAS` (`-100%..+100%` DC-compensated symmetry shift), `DRIVE`, `TONE`, and `MIX`, plus a live transfer-curve visualizer.
  - **`MB Dist` (3-Band Multiband Distortion):** Phase-aligned `LR4` 3-band crossover (`LOW` / `MID` / `HIGH`) with upper-crossover allpass compensation for flat unity summing when dry/bypassed. Each band's algorithm dropdown (`Lin Fold` / `Sin Fold` short labels alongside `Tube`, `Mech`, `Sine`, `Crush`, `Classic A/B/C`, `Diode A/B`, `Hard Clip`, `Half Rect`, `Full Rect`) and solo pill `[S]` sit at the top-left/center/right of the scope: click a band's dropdown or its shaded region on the scope to open that band's knobs (`DRIVE`, `BIAS`, `LEVEL` `-24..+12 dB`, `MIX`) and `[ON]` bypass toggle (stacked below `[XOV]` on the left of the knobs). Plain-clicking `[S]` solos that band exclusively (or un-solos if it is the sole soloed band); hold **`Shift + Click`** on `[S]` to solo multiple bands. Click `[XOV]` to view global controls (`LOW XOV` `40–1000 Hz`, `HIGH XOV` `500–16000 Hz`, `OUT GN` `-18..+18 dB`, `MIX`), or drag the vertical crossover markers directly on the scope (displays a live Hz tooltip while holding/dragging; double-click to reset).
  - **`Limiter` (`MAIN` & `ADV`):** `0.0 ms` lookahead acts as a zero-lookahead transient-clipping hybrid; `1.0–3.0 ms` lookahead uses an S-curve attack ramp and dual-stage program-dependent release for low-IMD limiting. `MAIN` controls: `GAIN` (`0..+24 dB`), `CEIL` (`-18..0 dB`, `-0.1 dB` default), `LOOKAHD` (`0.0–3.0 ms`), `MIX`, and floating `[TP]` (4x True Peak detection, off by default). `ADV` controls: `RELEASE` (`10–500 ms`), `KNEE` (`0%` hard clamp [default] to `100%` soft `TanhAdaa2` saturation on the final safety clipper), `LINK` (`0%` dual-mono to `100%` stereo-linked), and `PUNCH` (`0%–100%`, `100%` default — slows envelope attack so sharp transients feed harder into the final clipper rather than ducking sustain/bass).
- **Floating Scope Controls (`ZStack`):** `Reverb`, `Delay`, and `Chorus` feature a translucent `MAIN | EQ` toggle (`Tube Res.` uses `MAIN | LFO`, `Limiter` uses `MAIN | ADV`) at the top-right of their scope to switch between sub-pages while keeping all slot parameters active and modulatable.

---

## 5. Permanent Macro & Source Dock, Center Stage Tabs & MPE
- **Permanent `2×4` Macro & Performance Source Dock:** Always visible at bottom-center: houses a vertical column of performance source pills (`VEL`, `KEY`, `MW`, `AT`, `SLD`) on the left alongside all 8 renameable **`M1`–`M8`** macro cards (with draggable badge, editable name textbox, knob, and readout).
  - **Drag** `E1`–`E4` / `MS1`–`MS4` directly from the Center Stage tab strip (or the top strip in **`FX`** view), or `VEL`–`SLD` / `M1`–`M8` from the dock onto any knob or value pill to assign modulation.
  - **Click** `E1`–`E4` or `MS1`–`MS4` (including from the **`FX`** source strip) to jump directly to its editor in the Center Stage.
  - **Collar Handle:** Hover/drag the radial modulation arc around a knob to adjust depth.
  - **Right-Click** any modulated control to open a quick menu to **Bypass (`ON`/`BYP`)**, toggle **Polarity (`UNI`/`BIP`)**, or **Remove (`×`)** each routing.
- **`VOICE` Center Tab & MPE:**
  - `OSCILLATOR PAN & KEYTRACK` (4 side-by-side `OSC 1`–`OSC 4` columns for `PAN` and `KT`) + `VOICING, MPE & FILTER KEYTRACKING` (stacked **`MODE`** / **`PB RANGE`**, stacked **`[• MPE]`** / **`MPE PB`** `1–96 st` [default `48 st`], alongside `GLIDE`, `F1 KT`, `F2 KT`).
  - When **`MPE`** is enabled, Master `Ch 1` broadcasts global bend (`±PB RANGE`), `CC 1`, and pressure, while Member Channels `Ch 2–16` route per-note Pitch Bend (`±MPE PB`), Pressure (`AT`), and `CC 74` Slide (`SLD`) exclusively to each active voice with `1.5 ms` smoothing (alongside CLAP/VST3 `PolyPressure`, `PolyTuning`, and `PolyBrightness`).
- **`E1`–`E4` / `MS1`–`MS4` Center Stage Tabs (`MODSEQ` & `LFO`):**
  - **`MODSEQ` (Performer) Mode:**
    - **Left-Drag on Grid:** Stamps the selected `2×9` palette shape at the cursor's vertical depth.
    - **Right-Click a Palette Tile:** Fills the entire sequence with that shape (`FILL` fills with the currently highlighted tile; `FILL` and `CLEAR` sit beneath the palette).
    - **`Opt/Alt + Left-Drag` or `Right-Drag` (Vertical on a Step):** Adjusts that step's curve **tension** (`-1.0..=1.0`, warping its timing/concavity).
    - **`Shift + Left-Drag` (Vertical on a Step):** Adjusts the step's **end level** (linear ramp from start level to end level).
  - **`LFO` (MSEG) Mode:**
    - **Drag Breakpoint Nodes:** Move nodes (`Shift` or `SNAP` snaps `X` to `1/16` and `Y` to `1/8`). Preset bar and `RESET CURVE` sit in the lower-left control strip.
    - **Double-Click:** Add a new breakpoint node on empty canvas, or delete an interior node.
    - **Drag Midpoint Handle / `Opt/Alt + Left-Drag` / `Right-Drag`:** Bend the enclosing segment's curve **tension**.
  - **`E1`–`E4`:** Edit ADSR envelopes with live curve preview and numeric readouts.
- **`ROUTE` Center Tab:** Per-source Filter Send balance (`F1 <-> F2`, automatically dimmed with an explanatory hover tooltip when Direct Bus routing is active) and 4-way per-source `DIRECT` sends (`Off | Bus A | Bus B | Bus C`), `FX LANE MATRIX`, `FILTER MODE`, `F1 OUT BUS` / `F2 OUT BUS`, `F1/F2 MIX`, and `FEEDBACK`.

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
