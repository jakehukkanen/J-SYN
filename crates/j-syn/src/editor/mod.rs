//! Warm dark-mode skeuomorphic Vizia GUI for `J-SYN`.
//!
//! Maintainability and readability for both humans and AI are top priorities in this layout:
//! - [`mod@self`] holds top-level UI state (`UiData`, `BottomTab`, `CenterStageTab`, `UiEvent`),
//!   macOS `baseview` resize notification, and the 3-Wing `1568 × 882` layout builders
//!   (`create`, `build_header_bar`, `build_left_wing`, `build_center_wing`, `build_right_wing`).
//! - [`controls`] centralizes shared label typography, dropdowns, readouts, knob/pill cells, and
//!   module chrome (`bold_label`, `build_dropdown_trigger_labels`, `build_param_readout_box`, etc.).
//! - [`widgets`] implements custom `femtovg` / `ParamWidgetBase` controls (`RotaryKnob`,
//!   `LedToggleSwitch`, `ParamStepPill`).
//! - [`scope`] draws the live single-cycle `WavetableScope` preview canvas.
//! - [`osc_panel`] builds each `OSC 1` – `OSC 4` hardware strip.
//! - [`noise_panel`] builds the per-voice stereo `NOISE` strip below `OSC 4`.
//! - [`filter_panel`] builds `FILTER 1` and `FILTER 2` with live frequency response curves.
//! - [`mod_bar`] provides draggable modulation source pills, 60 Hz redraw ticker,
//!   and the inline modulation slot inspector (`UNI/BIP`, remove, cycle).
//! - [`modseq_tab`] builds the `MODSEQ` bottom tab (`MSEQ 1`–`MSEQ 4` curves + `ENV 2`–`ENV 4`).
//! - [`routing_tab`] builds the `ROUTING` bottom tab (Filter sends, Topology, Feedback, Insert).

use std::sync::Arc;

use j_syn_dsp::mod_matrix::{ModMatrixSource, ModUiTelemetry};
use j_syn_dsp::{FxEngineSwapQueue, WavetableBank};
use nih_plug::prelude::{Editor, GuiContext};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};

pub(crate) use j_syn_ui::{theme, tokens, widgets, UiKitEvent, UiShellState};

use crate::JSynPluginParams;

mod controls;
mod context_menu;
mod filter_panel;
mod fx_drag_overlay;
mod fx_picker_menu;
mod fx_rack;
mod fx_scopes;
mod header_bar;
mod insert_panel;
mod macros_tab;
mod matrix_tab;
mod mod_bar;
mod modseq_canvases;
mod modseq_tab;
mod noise_panel;
mod osc_panel;
mod routing_tab;
mod scope;
mod settings_view;
mod voice_tab;


use context_menu::build_mod_context_menu_overlay;
use controls::build_center_tab_button;
use header_bar::build_header_bar;
use filter_panel::build_dual_filter_section;
use fx_drag_overlay::FxDragOverlay;
use fx_picker_menu::{
    build_fx_overlay_menu, FxOverlayMenuKind, FxOverlayMenuState, FxPickerTarget,
};
use fx_rack::build_three_lane_fx_rack;
use insert_panel::build_insert_panel;
use macros_tab::build_macros_tab;
use mod_bar::{ModSourcePill, ModUiState};
use modseq_tab::{build_modseq_tab, ModSeqSubTab};
use noise_panel::build_noise_panel;
use osc_panel::build_osc_panel;
use routing_tab::build_routing_tab;

pub const BASE_WIDTH: u32 = tokens::WINDOW_W;
pub const BASE_HEIGHT: u32 = tokens::WINDOW_H;

pub(super) const AMBER_RGB: (u8, u8, u8) = (245, 179, 73);
pub(super) const CORAL_RGB: (u8, u8, u8) = (235, 140, 85);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainViewMode {
    Main,
    Matrix,
    Settings,
}

impl Data for MainViewMode {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BottomTab {
    VoiceEnv,
    ModSeq,
    Routing,
}

impl Data for BottomTab {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CenterStageTab {
    Mod(ModSeqSubTab),
    Voice,
    Routing,
}

impl Data for CenterStageTab {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

#[derive(Clone, PartialEq, Debug, Default)]
pub(crate) struct ModContextMenuState {
    pub visible: bool,
    pub dest_idx: usize,
    pub osc_pitch_idx: Option<usize>,
    pub always_show_actions: bool,
    pub show_mod_picker: bool,
    pub anchor_x: f32,
    pub anchor_y: f32,
    pub rev: u64,
}

impl Data for ModContextMenuState {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

fn format_dir_display(dir: Option<&str>) -> String {
    match dir.filter(|s| !s.trim().is_empty()) {
        Some(s) => s.to_string(),
        None => "(Not Set)".to_string(),
    }
}

pub(crate) enum UiEvent {
    SelectViewMode(MainViewMode),
    SelectTab(BottomTab),
    SelectModSeqSubTab(ModSeqSubTab),
    ModUiChanged,
    ModValChanged,
    SetGearRadius(f32),
    CommitGearRadius,
    SetMacroName(usize, String),
    InitPatch,
    SavePatch,
    LoadPatchDialog,
    LoadPresetPath(std::path::PathBuf, String),
    StepPreset(isize),
    PickPresetsDir,
    PickWavetablesDir,
    ClearPresetsDir,
    ClearWavetablesDir,
    RescanDirectories,
    ToggleWtFolder(String),
    TogglePresetFolder(String),
    OpenWtFolder(String),
    OpenPresetFolder(String),
    SyncExternalState,
    ToggleModMenuPicker,
    HideModContextMenu,
    ModMenuRefresh,
    ShowFxPicker(usize, FxPickerTarget, f32, f32),
    ShowFxUnitMenu(usize, usize, usize, f32, f32),
    ShowFxLaneMenu(usize, f32, f32),
    HideFxOverlayMenu,
}

#[derive(Lens)]
pub(crate) struct UiData {
    pub params: Arc<JSynPluginParams>,
    pub mod_ui: Arc<ModUiState>,
    #[lens(ignore)]
    pub gui_context: Arc<dyn GuiContext>,
    #[lens(ignore)]
    pub bank: Arc<WavetableBank>,
    pub view_mode: MainViewMode,
    pub active_tab: BottomTab,
    pub modseq_subtab: ModSeqSubTab,
    pub center_tab: CenterStageTab,
    pub mod_ui_rev: u64,
    pub mod_val_rev: u64,
    pub gear_radius: f32,
    pub macro_names: [String; 8],
    pub current_preset_name: String,
    pub presets_dir_rev: u64,
    pub wavetables_dir_rev: u64,
    pub expanded_preset_folder: Option<String>,
    pub expanded_wt_folder: Option<String>,
    pub presets_dir_display: String,
    pub wavetables_dir_display: String,
    pub mod_menu: ModContextMenuState,
    pub fx_menu: FxOverlayMenuState,
}

impl Model for UiData {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|kit_event, meta| match kit_event {
            UiKitEvent::ModUiChanged => {
                self.mod_ui_rev = self.mod_ui_rev.wrapping_add(1);
                self.mod_val_rev = self.mod_val_rev.wrapping_add(1);
                meta.consume();
            }
            UiKitEvent::ModValChanged => {
                self.mod_val_rev = self.mod_val_rev.wrapping_add(1);
                meta.consume();
            }
            UiKitEvent::ShowModContextMenu(dest, x, y) => {
                cx.emit(UiKitEvent::HideTooltip);
                self.fx_menu.kind = FxOverlayMenuKind::None;
                self.mod_menu = ModContextMenuState {
                    visible: true,
                    dest_idx: dest.index(),
                    osc_pitch_idx: None,
                    always_show_actions: false,
                    show_mod_picker: false,
                    anchor_x: *x,
                    anchor_y: *y,
                    rev: self.mod_menu.rev.wrapping_add(1),
                };
                meta.consume();
            }
            UiKitEvent::ShowActionModContextMenu(dest, x, y) => {
                cx.emit(UiKitEvent::HideTooltip);
                self.fx_menu.kind = FxOverlayMenuKind::None;
                self.mod_menu = ModContextMenuState {
                    visible: true,
                    dest_idx: dest.index(),
                    osc_pitch_idx: None,
                    always_show_actions: true,
                    show_mod_picker: false,
                    anchor_x: *x,
                    anchor_y: *y,
                    rev: self.mod_menu.rev.wrapping_add(1),
                };
                meta.consume();
            }
            UiKitEvent::ShowPitchContextMenu(osc_idx, dest, x, y) => {
                cx.emit(UiKitEvent::HideTooltip);
                self.fx_menu.kind = FxOverlayMenuKind::None;
                self.mod_menu = ModContextMenuState {
                    visible: true,
                    dest_idx: dest.index(),
                    osc_pitch_idx: Some(*osc_idx),
                    always_show_actions: true,
                    show_mod_picker: false,
                    anchor_x: *x,
                    anchor_y: *y,
                    rev: self.mod_menu.rev.wrapping_add(1),
                };
                meta.consume();
            }
            _ => {}
        });

        event.map(|ui_event, meta| match ui_event {
            UiEvent::SelectViewMode(mode) => {
                self.view_mode = *mode;
                self.mod_ui.set_view_mode(match mode {
                    MainViewMode::Main => 0,
                    MainViewMode::Matrix => 2,
                    MainViewMode::Settings => 3,
                });
                cx.emit(UiKitEvent::HideTooltip);
                self.fx_menu.kind = FxOverlayMenuKind::None;
                self.mod_ui.clear_drag();
                self.mod_ui.fx_drag.cancel_drag();
                meta.consume();
            }
            UiEvent::SelectTab(tab) => {
                self.active_tab = *tab;
                self.mod_ui.set_active_tab(*tab);
                self.center_tab = match *tab {
                    BottomTab::VoiceEnv => CenterStageTab::Voice,
                    BottomTab::Routing => CenterStageTab::Routing,
                    BottomTab::ModSeq => CenterStageTab::Mod(self.modseq_subtab),
                };
                cx.emit(UiKitEvent::HideTooltip);
                meta.consume();
            }
            UiEvent::SelectModSeqSubTab(sub) => {
                self.modseq_subtab = *sub;
                self.mod_ui.set_modseq_subtab(*sub);
                if self.active_tab == BottomTab::ModSeq {
                    self.center_tab = CenterStageTab::Mod(*sub);
                }
                cx.emit(UiKitEvent::HideTooltip);
                meta.consume();
            }
            UiEvent::ModUiChanged => {
                self.mod_ui_rev = self.mod_ui_rev.wrapping_add(1);
                self.mod_val_rev = self.mod_val_rev.wrapping_add(1);
                meta.consume();
            }
            UiEvent::ModValChanged => {
                self.mod_val_rev = self.mod_val_rev.wrapping_add(1);
                meta.consume();
            }
            UiEvent::SetGearRadius(r) => {
                self.gear_radius = r.clamp(0.10, 0.60);
                meta.consume();
            }
            UiEvent::CommitGearRadius => {
                let mut cfg = crate::presets::JSynConfig::load();
                cfg.gear_radius = self.gear_radius.clamp(0.10, 0.60);
                let _ = cfg.save();
                meta.consume();
            }
            UiEvent::SetMacroName(idx, name) => {
                if *idx < 8 {
                    let trimmed = if name.trim().is_empty() {
                        format!("MACRO {}", idx + 1)
                    } else {
                        name.clone()
                    };
                    self.macro_names[*idx] = name.clone();
                    if let Ok(mut guard) = self.params.macro_names.lock() {
                        guard[*idx] = trimmed;
                    }
                    self.mod_ui_rev = self.mod_ui_rev.wrapping_add(1);
                }
                meta.consume();
            }
            UiEvent::InitPatch => {
                self.macro_names = crate::presets::init_patch(
                    &self.params,
                    &self.gui_context,
                    &self.bank,
                );
                crate::presets::provision_active_fx(&self.params, &self.mod_ui.fx_swap_queue);
                self.fx_menu.kind = FxOverlayMenuKind::None;
                self.current_preset_name = "INIT".to_string();
                self.mod_ui_rev = self.mod_ui_rev.wrapping_add(1);
                meta.consume();
            }
            UiEvent::SavePatch => {
                let params = self.params.clone();
                let mod_ui = self.mod_ui.clone();
                let default_name = if self.current_preset_name == "INIT" {
                    "New Patch.json".to_string()
                } else {
                    format!("{}.json", self.current_preset_name)
                };
                let start_dir = crate::presets::JSynConfig::load().presets_dir;
                std::thread::spawn(move || {
                    let mut dlg = rfd::FileDialog::new()
                        .add_filter("J-SYN Patch", &["json"])
                        .set_file_name(&default_name);
                    if let Some(dir) = start_dir.filter(|s| !s.trim().is_empty()) {
                        dlg = dlg.set_directory(dir);
                    }
                    if let Some(path) = dlg.save_file() {
                        let stem = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Patch")
                            .to_string();
                        let patch = crate::presets::JSynPatch::capture(&stem, &params);
                        if patch.save_to_file(&path).is_ok() {
                            mod_ui.notify_external_sync(Some(stem));
                        }
                    }
                });
                meta.consume();
            }
            UiEvent::LoadPatchDialog => {
                let params = self.params.clone();
                let gui_context = self.gui_context.clone();
                let bank = self.bank.clone();
                let mod_ui = self.mod_ui.clone();
                let start_dir = crate::presets::JSynConfig::load().presets_dir;
                std::thread::spawn(move || {
                    let mut dlg = rfd::FileDialog::new().add_filter("J-SYN Patch", &["json"]);
                    if let Some(dir) = start_dir.filter(|s| !s.trim().is_empty()) {
                        dlg = dlg.set_directory(dir);
                    }
                    if let Some(path) = dlg.pick_file() {
                        if let Ok(patch) = crate::presets::JSynPatch::load_from_file(&path) {
                            let name = if patch.name.trim().is_empty() {
                                path.file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("Patch")
                                    .to_string()
                            } else {
                                patch.name.clone()
                            };
                            let _ = patch.apply(&params, &gui_context, &bank);
                            crate::presets::provision_active_fx(&params, &mod_ui.fx_swap_queue);
                            mod_ui.notify_external_sync(Some(name));
                        }
                    }
                });
                meta.consume();
            }
            UiEvent::LoadPresetPath(path, name) => {
                if let Ok(patch) = crate::presets::JSynPatch::load_from_file(path) {
                    self.macro_names = patch.apply(&self.params, &self.gui_context, &self.bank);
                    crate::presets::provision_active_fx(&self.params, &self.mod_ui.fx_swap_queue);
                    self.fx_menu.kind = FxOverlayMenuKind::None;
                    self.current_preset_name = name.clone();
                    self.mod_ui_rev = self.mod_ui_rev.wrapping_add(1);
                }
                meta.consume();
            }
            UiEvent::StepPreset(dir) => {
                let cfg = crate::presets::JSynConfig::load();
                let entries = crate::presets::scan_presets(cfg.presets_dir.as_deref());
                if !entries.is_empty() {
                    let cur_pos = entries
                        .iter()
                        .position(|e| e.name == self.current_preset_name);
                    let next_idx = match cur_pos {
                        Some(idx) => {
                            let len = entries.len() as isize;
                            (((idx as isize + *dir) % len) + len) as usize % entries.len()
                        }
                        None => {
                            if *dir >= 0 {
                                0
                            } else {
                                entries.len() - 1
                            }
                        }
                    };
                    let target = &entries[next_idx];
                    if let Ok(patch) = crate::presets::JSynPatch::load_from_file(&target.path) {
                        self.macro_names =
                            patch.apply(&self.params, &self.gui_context, &self.bank);
                        crate::presets::provision_active_fx(
                            &self.params,
                            &self.mod_ui.fx_swap_queue,
                        );
                        self.fx_menu.kind = FxOverlayMenuKind::None;
                        self.current_preset_name = target.name.clone();
                        self.mod_ui_rev = self.mod_ui_rev.wrapping_add(1);
                    }
                }
                meta.consume();
            }
            UiEvent::PickPresetsDir => {
                let mod_ui = self.mod_ui.clone();
                let start_dir = crate::presets::JSynConfig::load().presets_dir;
                std::thread::spawn(move || {
                    let mut dlg = rfd::FileDialog::new();
                    if let Some(dir) = start_dir.filter(|s| !s.trim().is_empty()) {
                        dlg = dlg.set_directory(dir);
                    }
                    if let Some(folder) = dlg.pick_folder() {
                        let mut cfg = crate::presets::JSynConfig::load();
                        cfg.presets_dir = Some(folder.to_string_lossy().into_owned());
                        let _ = cfg.save();
                        mod_ui.notify_external_sync(None);
                    }
                });
                meta.consume();
            }
            UiEvent::PickWavetablesDir => {
                let mod_ui = self.mod_ui.clone();
                let start_dir = crate::presets::JSynConfig::load().wavetables_dir;
                std::thread::spawn(move || {
                    let mut dlg = rfd::FileDialog::new();
                    if let Some(dir) = start_dir.filter(|s| !s.trim().is_empty()) {
                        dlg = dlg.set_directory(dir);
                    }
                    if let Some(folder) = dlg.pick_folder() {
                        let mut cfg = crate::presets::JSynConfig::load();
                        cfg.wavetables_dir = Some(folder.to_string_lossy().into_owned());
                        let _ = cfg.save();
                        mod_ui.notify_external_sync(None);
                    }
                });
                meta.consume();
            }
            UiEvent::ClearPresetsDir => {
                let mut cfg = crate::presets::JSynConfig::load();
                cfg.presets_dir = None;
                let _ = cfg.save();
                self.presets_dir_display = "(Not Set)".to_string();
                self.expanded_preset_folder = None;
                self.presets_dir_rev = self.presets_dir_rev.wrapping_add(1);
                meta.consume();
            }
            UiEvent::ClearWavetablesDir => {
                let mut cfg = crate::presets::JSynConfig::load();
                cfg.wavetables_dir = None;
                let _ = cfg.save();
                self.wavetables_dir_display = "(Not Set)".to_string();
                self.expanded_wt_folder = None;
                self.wavetables_dir_rev = self.wavetables_dir_rev.wrapping_add(1);
                meta.consume();
            }
            UiEvent::RescanDirectories => {
                let cfg = crate::presets::JSynConfig::load();
                self.presets_dir_display = format_dir_display(cfg.presets_dir.as_deref());
                self.wavetables_dir_display = format_dir_display(cfg.wavetables_dir.as_deref());
                self.expanded_preset_folder = None;
                self.expanded_wt_folder = None;
                self.presets_dir_rev = self.presets_dir_rev.wrapping_add(1);
                self.wavetables_dir_rev = self.wavetables_dir_rev.wrapping_add(1);
                cx.emit(UiKitEvent::ReloadThemes);
                meta.consume();
            }
            UiEvent::ToggleWtFolder(folder) => {
                if self.expanded_wt_folder.as_deref() == Some(folder.as_str()) {
                    self.expanded_wt_folder = folder
                        .rsplit_once('/')
                        .map(|(parent, _)| parent.to_string());
                } else {
                    self.expanded_wt_folder = Some(folder.clone());
                }
                self.wavetables_dir_rev = self.wavetables_dir_rev.wrapping_add(1);
                meta.consume();
            }
            UiEvent::OpenWtFolder(folder) => {
                if self.expanded_wt_folder.as_deref() != Some(folder.as_str()) {
                    self.expanded_wt_folder = Some(folder.clone());
                    self.wavetables_dir_rev = self.wavetables_dir_rev.wrapping_add(1);
                }
                meta.consume();
            }
            UiEvent::TogglePresetFolder(folder) => {
                if self.expanded_preset_folder.as_deref() == Some(folder.as_str()) {
                    self.expanded_preset_folder = folder
                        .rsplit_once('/')
                        .map(|(parent, _)| parent.to_string());
                } else {
                    self.expanded_preset_folder = Some(folder.clone());
                }
                self.presets_dir_rev = self.presets_dir_rev.wrapping_add(1);
                meta.consume();
            }
            UiEvent::OpenPresetFolder(folder) => {
                if self.expanded_preset_folder.as_deref() != Some(folder.as_str()) {
                    self.expanded_preset_folder = Some(folder.clone());
                    self.presets_dir_rev = self.presets_dir_rev.wrapping_add(1);
                }
                meta.consume();
            }
            UiEvent::SyncExternalState => {
                if let Some(name) = self.mod_ui.take_pending_preset_name() {
                    self.current_preset_name = name;
                }
                if let Ok(guard) = self.params.macro_names.lock() {
                    self.macro_names = guard.clone();
                }
                let cfg = crate::presets::JSynConfig::load();
                self.presets_dir_display = format_dir_display(cfg.presets_dir.as_deref());
                self.wavetables_dir_display = format_dir_display(cfg.wavetables_dir.as_deref());
                self.presets_dir_rev = self.presets_dir_rev.wrapping_add(1);
                self.wavetables_dir_rev = self.wavetables_dir_rev.wrapping_add(1);
                self.mod_ui_rev = self.mod_ui_rev.wrapping_add(1);
                meta.consume();
            }
            UiEvent::ToggleModMenuPicker => {
                self.mod_menu.show_mod_picker = !self.mod_menu.show_mod_picker;
                self.mod_menu.rev = self.mod_menu.rev.wrapping_add(1);
                meta.consume();
            }
            UiEvent::HideModContextMenu => {
                self.mod_menu.visible = false;
                self.mod_menu.show_mod_picker = false;
                meta.consume();
            }
            UiEvent::ModMenuRefresh => {
                self.mod_menu.rev = self.mod_menu.rev.wrapping_add(1);
                meta.consume();
            }
            UiEvent::ShowFxPicker(bus, target, x, y) => {
                cx.emit(UiKitEvent::HideTooltip);
                self.mod_menu.visible = false;
                self.fx_menu = FxOverlayMenuState {
                    kind: FxOverlayMenuKind::Picker {
                        bus: *bus,
                        target: *target,
                    },
                    anchor_x: *x,
                    anchor_y: *y,
                    rev: self.fx_menu.rev.wrapping_add(1),
                };
                meta.consume();
            }
            UiEvent::ShowFxUnitMenu(bus, pos, slot, x, y) => {
                cx.emit(UiKitEvent::HideTooltip);
                self.mod_menu.visible = false;
                self.fx_menu = FxOverlayMenuState {
                    kind: FxOverlayMenuKind::UnitContext {
                        bus: *bus,
                        pos: *pos,
                        slot: *slot,
                    },
                    anchor_x: *x,
                    anchor_y: *y,
                    rev: self.fx_menu.rev.wrapping_add(1),
                };
                meta.consume();
            }
            UiEvent::ShowFxLaneMenu(bus, x, y) => {
                cx.emit(UiKitEvent::HideTooltip);
                self.mod_menu.visible = false;
                self.fx_menu = FxOverlayMenuState {
                    kind: FxOverlayMenuKind::LaneContext { bus: *bus },
                    anchor_x: *x,
                    anchor_y: *y,
                    rev: self.fx_menu.rev.wrapping_add(1),
                };
                meta.consume();
            }
            UiEvent::HideFxOverlayMenu => {
                self.fx_menu.kind = FxOverlayMenuKind::None;
                meta.consume();
            }
        });
    }
}

pub(crate) fn default_state() -> Arc<ViziaState> {
    let cfg = crate::presets::JSynConfig::load();
    let scale = (cfg.ui_scale.clamp(85, 200) as f64) / 100.0;
    ViziaState::new_with_default_scale_factor(|| (BASE_WIDTH, BASE_HEIGHT), scale)
}


fn build_left_wing(
    cx: &mut Context,
    bank: Arc<WavetableBank>,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    VStack::new(cx, move |cx| {
        for idx in 0..4 {
            build_osc_panel(
                cx,
                idx,
                bank.clone(),
                params.clone(),
                gui_context.clone(),
                mod_ui.clone(),
            );
        }

        // Per-voice stereo Noise generator strip below OSC 4
        build_noise_panel(cx, params, gui_context, mod_ui);
    })
    .row_between(Pixels(tokens::SPACE_SM))
    .width(Pixels(tokens::LEFT_WING_W))
    .height(Stretch(1.0));
}

fn build_center_wing(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    VStack::new(cx, move |cx| {
        build_dual_filter_section(
            cx,
            params.clone(),
            gui_context.clone(),
            mod_ui.clone(),
        );

        let p_stage = params.clone();
        let g_stage = gui_context.clone();
        let u_stage = mod_ui.clone();
        let u_tabs = mod_ui.clone();
        VStack::new(cx, move |cx| {
            HStack::new(cx, move |cx| {
                for &src in &ModMatrixSource::ALL_ACTIVE[0..4] {
                    ModSourcePill::new(cx, src, u_tabs.clone())
                        .width(Stretch(0.9))
                        .height(Stretch(1.0));
                }
                for &src in &ModMatrixSource::ALL_ACTIVE[4..8] {
                    ModSourcePill::new(cx, src, u_tabs.clone())
                        .width(Stretch(1.0))
                        .height(Stretch(1.0));
                }
                build_center_tab_button(cx, CenterStageTab::Voice, "VOICE", 1.2);
                build_center_tab_button(cx, CenterStageTab::Routing, "ROUTE", 1.2);
            })
            .class("tab-bar")
            .col_between(Pixels(tokens::SPACE_XS))
            .height(Pixels(24.0))
            .width(Stretch(1.0));

            Binding::new(cx, UiData::active_tab, move |cx, active| {
                match active.get(cx) {
                    BottomTab::VoiceEnv => {
                        voice_tab::build_voice_tab(
                            cx,
                            p_stage.clone(),
                            g_stage.clone(),
                            u_stage.clone(),
                        );
                    }
                    BottomTab::ModSeq => {
                        build_modseq_tab(
                            cx,
                            p_stage.clone(),
                            g_stage.clone(),
                            u_stage.clone(),
                        );
                    }
                    BottomTab::Routing => {
                        build_routing_tab(
                            cx,
                            p_stage.clone(),
                            g_stage.clone(),
                            u_stage.clone(),
                        );
                    }
                }
            });
        })
        .row_between(Pixels(tokens::SPACE_XS))
        .width(Stretch(1.0))
        .height(Stretch(1.0));

        build_insert_panel(
            cx,
            params.clone(),
            gui_context.clone(),
            mod_ui.clone(),
        );

        build_macros_tab(cx, mod_ui);
    })
    .row_between(Pixels(tokens::SPACE_SM))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}

fn build_right_wing(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    VStack::new(cx, move |cx| {
        build_three_lane_fx_rack(cx, params, gui_context, mod_ui);
    })
    .width(Pixels(tokens::RIGHT_WING_W))
    .height(Stretch(1.0));
}

pub fn build_j_syn_ui(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    bank: Arc<WavetableBank>,
    telemetry: Arc<ModUiTelemetry>,
    fx_swap_queue: Arc<FxEngineSwapQueue>,
    initial_theme: theme::ThemePalette,
    initial_scale_pct: u32,
    persist_settings: bool,
) {
    let initial_cfg = crate::presets::JSynConfig::load();

    let mut shell = UiShellState::new(initial_theme, initial_scale_pct);
    if persist_settings {
        shell = shell
            .with_theme_callback(|name| {
                let mut cfg = crate::presets::JSynConfig::load();
                cfg.theme = name.to_string();
                let _ = cfg.save();
            })
            .with_scale_callback(|pct| {
                let mut cfg = crate::presets::JSynConfig::load();
                cfg.ui_scale = pct;
                let _ = cfg.save();
            });
    }
    shell.install(cx);

    let mod_ui = Arc::new(ModUiState::new(params.clone(), telemetry, fx_swap_queue));
    let initial_macro_names = params
        .macro_names
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| std::array::from_fn(|i| format!("MACRO {}", i + 1)));
    UiData {
        params: params.clone(),
        mod_ui: mod_ui.clone(),
        gui_context: gui_context.clone(),
        bank: bank.clone(),
        view_mode: MainViewMode::Main,
        active_tab: BottomTab::ModSeq,
        modseq_subtab: ModSeqSubTab::MSeq(0),
        center_tab: CenterStageTab::Mod(ModSeqSubTab::MSeq(0)),
        mod_ui_rev: 0,
        mod_val_rev: 0,
        gear_radius: initial_cfg.gear_radius.clamp(0.10, 0.60),
        macro_names: initial_macro_names,
        current_preset_name: "INIT".to_string(),
        presets_dir_rev: 0,
        wavetables_dir_rev: 0,
        expanded_preset_folder: None,
        expanded_wt_folder: None,
        presets_dir_display: format_dir_display(initial_cfg.presets_dir.as_deref()),
        wavetables_dir_display: format_dir_display(initial_cfg.wavetables_dir.as_deref()),
        mod_menu: ModContextMenuState::default(),
        fx_menu: FxOverlayMenuState::default(),
    }
    .build(cx);

    let bank_for_ui = bank;
    let params_for_ui = params;
    let gui_ctx_for_ui = gui_context;
    let params_menu = params_for_ui.clone();
    let gui_ctx_menu = gui_ctx_for_ui.clone();
    let params_fx_menu = params_for_ui.clone();
    let gui_ctx_fx_menu = gui_ctx_for_ui.clone();
    let mod_ui_for_ui = mod_ui.clone();
    let mod_ui_fx_menu = mod_ui.clone();
    let mod_ui_esc = mod_ui.clone();
    let fx_drag_overlay_state = mod_ui.fx_drag.clone();

    VStack::new(cx, move |cx| {
        // Prevent Spacebar (e.g. DAW transport play/stop) from triggering focused buttons,
        // and let Esc cancel active FX drags or close floating menus.
        cx.add_listener(move |_view: &mut VStack, cx, event| {
            event.map(|window_event, meta| match window_event {
                WindowEvent::KeyDown(Code::Space, _)
                | WindowEvent::KeyUp(Code::Space, _)
                | WindowEvent::Press { mouse: false }
                | WindowEvent::PressDown { mouse: false } => {
                    meta.consume();
                }
                WindowEvent::KeyDown(Code::Escape, _) => {
                    if mod_ui_esc.fx_drag.cancel_drag() {
                        cx.emit(UiEvent::ModValChanged);
                        cx.needs_redraw();
                    }
                    cx.emit(UiEvent::HideFxOverlayMenu);
                    cx.emit(UiEvent::HideModContextMenu);
                    meta.consume();
                }
                _ => {}
            });
        });

        mod_bar::start_ui_redraw_timer(cx, mod_ui_for_ui.clone());

        build_header_bar(cx, mod_ui_for_ui.clone());

        // Main chassis: 3-Wing Widescreen Layout (Left Oscs + Noise, Center Synth Core, Right 3-Lane FX),
        // MATRIX Overview, or Settings.
        VStack::new(cx, move |cx| {
            VStack::new(cx, move |cx| {
                let bank_m = bank_for_ui.clone();
                let params_m = params_for_ui.clone();
                let gui_m = gui_ctx_for_ui.clone();
                let mod_ui_m = mod_ui_for_ui.clone();

                Binding::new(cx, UiData::view_mode, move |cx, mode| {
                    match mode.get(cx) {
                        MainViewMode::Main => {
                            let bank_c = bank_m.clone();
                            let params_c = params_m.clone();
                            let gui_c = gui_m.clone();
                            let mod_ui_c = mod_ui_m.clone();
                            HStack::new(cx, move |cx| {
                                build_left_wing(
                                    cx,
                                    bank_c,
                                    params_c.clone(),
                                    gui_c.clone(),
                                    mod_ui_c.clone(),
                                );
                                build_center_wing(
                                    cx,
                                    params_c.clone(),
                                    gui_c.clone(),
                                    mod_ui_c.clone(),
                                );
                                build_right_wing(cx, params_c, gui_c, mod_ui_c);
                            })
                            .col_between(Pixels(tokens::SPACE_MD))
                            .width(Stretch(1.0))
                            .height(Stretch(1.0));
                        }
                        MainViewMode::Matrix => {
                            matrix_tab::build_matrix_view(cx, mod_ui_m.clone());
                        }
                        MainViewMode::Settings => {
                            settings_view::build_settings_view(
                                cx,
                                params_m.clone(),
                                gui_m.clone(),
                            );
                        }
                    }
                });
            })
            .class("section-panel")
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        })
        .class("main-chassis")
        .width(Stretch(1.0))
        .height(Stretch(1.0));

        // Global floating value tooltip overlay
        j_syn_ui::build_tooltip_overlay(cx, BASE_WIDTH as f32, BASE_HEIGHT as f32);

        build_mod_context_menu_overlay(cx, mod_ui, params_menu, gui_ctx_menu);
        build_fx_overlay_menu(cx, mod_ui_fx_menu, params_fx_menu, gui_ctx_fx_menu);
        FxDragOverlay::new(cx, fx_drag_overlay_state);
        })
        .class("plugin-root")
        .width(Stretch(1.0))
        .height(Stretch(1.0));
}

pub(crate) fn create(
    params: Arc<JSynPluginParams>,
    editor_state: Arc<ViziaState>,
    bank: Arc<WavetableBank>,
    telemetry: Arc<ModUiTelemetry>,
    fx_swap_queue: Arc<FxEngineSwapQueue>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Builtin, move |cx, gui_context| {
        let initial_cfg = crate::presets::JSynConfig::load();
        let initial_scale_pct = initial_cfg.ui_scale.clamp(85, 200);
        let initial_theme = theme::find_theme_by_name(&initial_cfg.theme);
        build_j_syn_ui(
            cx,
            params.clone(),
            gui_context,
            bank.clone(),
            telemetry.clone(),
            fx_swap_queue.clone(),
            initial_theme,
            initial_scale_pct,
            true,
        );
    })
}

