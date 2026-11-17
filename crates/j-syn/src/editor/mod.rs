//! Warm dark-mode skeuomorphic Vizia GUI for `J-SYN`.
//!
//! Maintainability and readability for both humans and AI are top priorities in this layout:
//! - [`mod@self`] holds top-level UI state (`UiData`, `BottomTab`, `UiEvent`), macOS `baseview`
//!   resize notification, and the high-level window section builders (`create`, `build_header_bar`,
//!   `build_osc_column`, `build_filter_column`, `build_fx_column`, `build_bottom_tabs`).
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

use j_syn_dsp::mod_matrix::{ModDestination, ModMatrixSource, ModUiTelemetry};
use j_syn_dsp::WavetableBank;
use nih_plug::prelude::{Editor, GuiContext, Param, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};

use crate::{JSynPluginParams, PitchModeParam};

mod controls;
mod filter_panel;
mod fx_rack;
mod fx_scopes;
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
mod theme;
mod voice_tab;
mod widgets;

use controls::{
    bold_label, build_dropdown_trigger_labels, build_header_knob_cell, build_tab_button,
    build_ui_scale_dropdown, ArrowButton, ArrowDir, CheckIcon, CrossButton,
};
use filter_panel::build_filter_panel;
use insert_panel::build_insert_panel;
use macros_tab::build_macros_tab;
use mod_bar::{build_mod_source_bar, ModUiState};
use modseq_tab::{build_modseq_tab, ModSeqSubTab};
use noise_panel::build_noise_panel;
use osc_panel::build_osc_panel;
use routing_tab::build_routing_tab;

const BASE_WIDTH: u32 = 1160;
const BASE_HEIGHT: u32 = 778;

pub(super) const AMBER_RGB: (u8, u8, u8) = (245, 179, 73);
pub(super) const GOLD_RGB: (u8, u8, u8) = (242, 203, 97);
pub(super) const CORAL_RGB: (u8, u8, u8) = (235, 140, 85);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainViewMode {
    Main,
    Fx,
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
    MacrosGlobal,
}

impl Data for BottomTab {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

#[derive(Clone, PartialEq, Debug, Default)]
pub(crate) struct TooltipState {
    pub visible: bool,
    pub text: String,
    pub center_x: f32,
    pub anchor_top: f32,
    pub anchor_bottom: f32,
}

impl Data for TooltipState {
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
    SelectMainFxBus(usize),
    ModUiChanged,
    ModValChanged,
    SetUiScale(u32),
    SetGearRadius(f32),
    CommitGearRadius,
    SelectTheme(String),
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
    ShowTooltip(TooltipState),
    HideTooltip,
    ShowModContextMenu(ModDestination, f32, f32),
    ShowActionModContextMenu(ModDestination, f32, f32),
    ShowPitchContextMenu(usize, ModDestination, f32, f32),
    ToggleModMenuPicker,
    HideModContextMenu,
    ModMenuRefresh,
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
    pub main_fx_bus: usize,
    pub mod_ui_rev: u64,
    pub mod_val_rev: u64,
    pub ui_scale_pct: u32,
    pub gear_radius: f32,
    pub theme_name: String,
    pub themes_rev: u64,
    #[lens(ignore)]
    pub active_theme: theme::ThemePalette,
    #[lens(ignore)]
    pub dynamic_css: Arc<std::sync::RwLock<String>>,
    pub macro_names: [String; 8],
    pub current_preset_name: String,
    pub presets_dir_rev: u64,
    pub wavetables_dir_rev: u64,
    pub expanded_preset_folder: Option<String>,
    pub expanded_wt_folder: Option<String>,
    pub presets_dir_display: String,
    pub wavetables_dir_display: String,
    pub tooltip: TooltipState,
    pub mod_menu: ModContextMenuState,
}

impl Model for UiData {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|ui_event, meta| match ui_event {
            UiEvent::SelectViewMode(mode) => {
                self.view_mode = *mode;
                self.mod_ui.set_view_mode(match mode {
                    MainViewMode::Main => 0,
                    MainViewMode::Fx => 1,
                    MainViewMode::Matrix => 2,
                    MainViewMode::Settings => 3,
                });
                self.tooltip.visible = false;
                self.mod_ui.clear_drag();
                meta.consume();
            }
            UiEvent::SelectTab(tab) => {
                self.active_tab = *tab;
                self.mod_ui.set_active_tab(*tab);
                self.tooltip.visible = false;
                meta.consume();
            }
            UiEvent::SelectModSeqSubTab(sub) => {
                self.modseq_subtab = *sub;
                self.mod_ui.set_modseq_subtab(*sub);
                self.tooltip.visible = false;
                meta.consume();
            }
            UiEvent::SelectMainFxBus(bus) => {
                let next = (*bus).min(1);
                self.main_fx_bus = next;
                self.mod_ui.set_main_fx_bus(next);
                self.tooltip.visible = false;
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
            UiEvent::SetUiScale(pct) => {
                self.ui_scale_pct = *pct;
                let mut cfg = crate::presets::JSynConfig::load();
                cfg.ui_scale = *pct;
                let _ = cfg.save();
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
            UiEvent::SelectTheme(name) => {
                let pal = theme::find_theme_by_name(name);
                if let Ok(mut guard) = self.dynamic_css.write() {
                    *guard = pal.render_css();
                }
                let mut cfg = crate::presets::JSynConfig::load();
                cfg.theme = pal.name.clone();
                let _ = cfg.save();
                self.theme_name = pal.name.clone();
                self.active_theme = pal;
                let _ = _cx.reload_styles();
                _cx.needs_redraw();
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
                            mod_ui.notify_external_sync(Some(name));
                        }
                    }
                });
                meta.consume();
            }
            UiEvent::LoadPresetPath(path, name) => {
                if let Ok(patch) = crate::presets::JSynPatch::load_from_file(path) {
                    self.macro_names = patch.apply(&self.params, &self.gui_context, &self.bank);
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
                self.themes_rev = self.themes_rev.wrapping_add(1);
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
            UiEvent::ShowTooltip(state) => {
                self.tooltip = state.clone();
                meta.consume();
            }
            UiEvent::HideTooltip => {
                self.tooltip.visible = false;
                meta.consume();
            }
            UiEvent::ShowModContextMenu(dest, x, y) => {
                self.tooltip.visible = false;
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
            UiEvent::ShowActionModContextMenu(dest, x, y) => {
                self.tooltip.visible = false;
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
            UiEvent::ShowPitchContextMenu(osc_idx, dest, x, y) => {
                self.tooltip.visible = false;
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
        });
    }
}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
pub(super) fn notify_baseview_resize_macos() {
    use objc::runtime::{Class, Object};
    use objc::{class, msg_send, sel, sel_impl};

    unsafe fn visit_subviews(view: *mut Object) {
        if view.is_null() {
            return;
        }
        unsafe {
            let cls: *const Class = msg_send![view, class];
            if !cls.is_null() && (*cls).instance_variable("baseview_state").is_some() {
                let sel = sel!(viewDidChangeBackingProperties:);
                let nil: *mut Object = std::ptr::null_mut();
                let delay: f64 = 0.025;
                let _: () = msg_send![
                    view,
                    performSelector: sel
                    withObject: nil
                    afterDelay: delay
                ];
            }
            let subviews: *mut Object = msg_send![view, subviews];
            if subviews.is_null() {
                return;
            }
            let count: usize = msg_send![subviews, count];
            for i in 0..count {
                let child: *mut Object = msg_send![subviews, objectAtIndex: i];
                visit_subviews(child);
            }
        }
    }

    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        if app.is_null() {
            return;
        }
        let windows: *mut Object = msg_send![app, windows];
        if windows.is_null() {
            return;
        }
        let count: usize = msg_send![windows, count];
        for i in 0..count {
            let win: *mut Object = msg_send![windows, objectAtIndex: i];
            if win.is_null() {
                continue;
            }
            let content_view: *mut Object = msg_send![win, contentView];
            visit_subviews(content_view);
        }
    }
}

pub(crate) fn default_state() -> Arc<ViziaState> {
    let cfg = crate::presets::JSynConfig::load();
    let scale = (cfg.ui_scale.clamp(85, 200) as f64) / 100.0;
    ViziaState::new_with_default_scale_factor(|| (BASE_WIDTH, BASE_HEIGHT), scale)
}

struct GearIcon;

impl GearIcon {
    fn new<'a>(cx: &'a mut Context) -> Handle<'a, Self> {
        Self.build(cx, |_| {})
            .bind(UiData::gear_radius, |mut handle, _| handle.needs_redraw())
    }
}

impl View for GearIcon {
    fn element(&self) -> Option<&'static str> {
        Some("gear-icon")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        use nih_plug_vizia::vizia::vg;

        let bounds = cx.bounds();
        if bounds.w <= 2.0 || bounds.h <= 2.0 {
            return;
        }
        let active = UiData::view_mode.get(cx) == MainViewMode::Settings;
        let pal = theme::current_palette(cx);
        let color = if active {
            theme::vg_color(&pal.accent_primary, 255)
        } else {
            theme::vg_color(&pal.text_muted, 255)
        };

        let cx_p = bounds.x + bounds.w * 0.5;
        let cy_p = bounds.y + bounds.h * 0.5;
        let radius_ratio = UiData::gear_radius.get(cx).clamp(0.10, 0.60);
        let r_outer = bounds.w.min(bounds.h) * radius_ratio;
        let r_inner = r_outer * 0.68;
        let r_hole = r_outer * 0.31;
        let teeth = 6usize;

        let mut gear = vg::Path::new();
        for i in 0..teeth {
            let base = (i as f32 / teeth as f32) * std::f32::consts::TAU;
            let step = std::f32::consts::TAU / (teeth as f32 * 4.0);
            let a0 = base - step * 1.15;
            let a1 = base - step * 0.60;
            let a2 = base + step * 0.60;
            let a3 = base + step * 1.15;
            let a4 = base + step * 2.00;

            let p0 = (cx_p + r_inner * a0.cos(), cy_p + r_inner * a0.sin());
            let p1 = (cx_p + r_outer * a1.cos(), cy_p + r_outer * a1.sin());
            let p2 = (cx_p + r_outer * a2.cos(), cy_p + r_outer * a2.sin());
            let p3 = (cx_p + r_inner * a3.cos(), cy_p + r_inner * a3.sin());
            let p4 = (cx_p + r_inner * a4.cos(), cy_p + r_inner * a4.sin());
            if i == 0 {
                gear.move_to(p0.0, p0.1);
            } else {
                gear.line_to(p0.0, p0.1);
            }
            gear.line_to(p1.0, p1.1);
            gear.line_to(p2.0, p2.1);
            gear.line_to(p3.0, p3.1);
            gear.line_to(p4.0, p4.1);
        }
        gear.close();
        gear.circle(cx_p, cy_p, r_hole);
        gear.solidity(vg::Solidity::Hole);

        canvas.fill_path(&gear, &vg::Paint::color(color));
    }
}

fn build_header_bar(cx: &mut Context, mod_ui: Arc<ModUiState>) {
    HStack::new(cx, move |cx| {
        Element::new(cx).class("rack-screw");
        HStack::new(cx, move |cx| {
            Element::new(cx).class("brand-pill");
            bold_label(cx, "J-SYN", "header-title");
        })
        .col_between(Pixels(6.0))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .width(Auto);

        HStack::new(cx, |cx| {
            Button::new(
                cx,
                |cx| cx.emit(UiEvent::SelectViewMode(MainViewMode::Main)),
                |cx| bold_label(cx, "MAIN", "view-switch-label").hoverable(false),
            )
            .class("view-switch-btn")
            .checked(UiData::view_mode.map(|m| *m == MainViewMode::Main))
            .width(Pixels(48.0))
            .height(Pixels(24.0));

            Button::new(
                cx,
                |cx| cx.emit(UiEvent::SelectViewMode(MainViewMode::Fx)),
                |cx| bold_label(cx, "FX", "view-switch-label").hoverable(false),
            )
            .class("view-switch-btn")
            .checked(UiData::view_mode.map(|m| *m == MainViewMode::Fx))
            .width(Pixels(38.0))
            .height(Pixels(24.0));

            Button::new(
                cx,
                |cx| cx.emit(UiEvent::SelectViewMode(MainViewMode::Matrix)),
                |cx| bold_label(cx, "MATRIX", "view-switch-label").hoverable(false),
            )
            .class("view-switch-btn")
            .checked(UiData::view_mode.map(|m| *m == MainViewMode::Matrix))
            .width(Pixels(56.0))
            .height(Pixels(24.0));

            Button::new(
                cx,
                |cx| cx.emit(UiEvent::SelectViewMode(MainViewMode::Settings)),
                |cx| {
                    GearIcon::new(cx)
                        .width(Stretch(1.0))
                        .height(Stretch(1.0))
                        .hoverable(false)
                },
            )
            .class("view-switch-btn")
            .checked(UiData::view_mode.map(|m| *m == MainViewMode::Settings))
            .width(Pixels(36.0))
            .height(Pixels(24.0))
            .child_space(Pixels(0.0));
        })
        .class("view-switch-group")
        .col_between(Pixels(3.0))
        .width(Auto)
        .height(Pixels(30.0))
        .top(Stretch(1.0))
        .bottom(Stretch(1.0));

        // Preset management bar: < [Preset v] > INIT SAVE LOAD
        HStack::new(cx, |cx| {
            ArrowButton::new(cx, |cx| cx.emit(UiEvent::StepPreset(-1)), ArrowDir::Left)
                .class("preset-btn")
                .class("preset-btn-label")
                .width(Pixels(20.0))
                .height(Pixels(22.0));

            Dropdown::new(
                cx,
                move |cx| {
                    HStack::new(cx, move |cx| {
                        build_dropdown_trigger_labels(cx, UiData::current_preset_name);
                    })
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_top(Pixels(0.0))
                    .child_bottom(Pixels(0.0))
                    .overflow(Overflow::Hidden)
                },
                move |cx| {
                    Binding::new(cx, UiData::presets_dir_rev, move |cx, _| {
                        let cfg = crate::presets::JSynConfig::load();
                        let tree =
                            crate::presets::scan_preset_tree(cfg.presets_dir.as_deref());
                        if tree.root_files.is_empty() && tree.folders.is_empty() {
                            bold_label(cx, "Set Presets Dir...", "dropdown-action-item").on_press(
                                |cx| {
                                    cx.emit(PopupEvent::Close);
                                    cx.emit(UiEvent::PickPresetsDir);
                                },
                            );
                        } else {
                            let expanded = UiData::expanded_preset_folder.get(cx);
                            controls::build_directory_tree_items(
                                cx,
                                tree,
                                expanded,
                                |cx, folder_path| {
                                    cx.emit(UiEvent::OpenPresetFolder(folder_path.to_string()));
                                },
                                |cx, folder_path| {
                                    cx.emit(UiEvent::TogglePresetFolder(folder_path.to_string()));
                                },
                                |cx, entry| {
                                    cx.emit(UiEvent::LoadPresetPath(
                                        entry.path.clone(),
                                        entry.name.clone(),
                                    ));
                                    cx.emit(PopupEvent::Close);
                                },
                            );
                        }
                    });
                },
            )
            .width(Pixels(160.0))
            .height(Pixels(22.0));

            ArrowButton::new(cx, |cx| cx.emit(UiEvent::StepPreset(1)), ArrowDir::Right)
                .class("preset-btn")
                .class("preset-btn-label")
                .width(Pixels(20.0))
                .height(Pixels(22.0));

            Button::new(
                cx,
                |cx| cx.emit(UiEvent::InitPatch),
                |cx| bold_label(cx, "INIT", "preset-btn-label").hoverable(false),
            )
            .class("preset-btn")
            .width(Pixels(36.0))
            .height(Pixels(22.0));

            Button::new(
                cx,
                |cx| cx.emit(UiEvent::SavePatch),
                |cx| bold_label(cx, "SAVE", "preset-btn-label").hoverable(false),
            )
            .class("preset-btn")
            .width(Pixels(40.0))
            .height(Pixels(22.0));

            Button::new(
                cx,
                |cx| cx.emit(UiEvent::LoadPatchDialog),
                |cx| bold_label(cx, "LOAD", "preset-btn-label").hoverable(false),
            )
            .class("preset-btn")
            .width(Pixels(40.0))
            .height(Pixels(22.0));
        })
        .col_between(Pixels(3.0))
        .width(Auto)
        .height(Pixels(22.0))
        .top(Stretch(1.0))
        .bottom(Stretch(1.0));

        Element::new(cx).width(Stretch(1.0));

        let mod_ui_hdr = mod_ui.clone();
        HStack::new(cx, move |cx| {
            build_ui_scale_dropdown(cx);

            build_header_knob_cell(
                cx,
                UiData::params,
                |p| &p.master_volume_db,
                "MASTER",
                false,
                AMBER_RGB,
            );

            scope::MasterMeter::new(cx, mod_ui_hdr.telemetry.clone())
                .width(Pixels(20.0))
                .height(Pixels(34.0))
                .top(Stretch(1.0))
                .bottom(Stretch(1.0));
        })
        .col_between(Pixels(8.0))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .width(Auto)
        .height(Stretch(1.0));

        Element::new(cx).class("rack-screw");
    })
    .class("header-bar")
    .col_between(Pixels(10.0))
    .width(Stretch(1.0))
    .height(Pixels(48.0))
    .child_left(Pixels(12.0))
    .child_right(Pixels(12.0))
    .child_top(Stretch(1.0))
    .child_bottom(Stretch(1.0));
}

fn build_osc_column(
    cx: &mut Context,
    bank: Arc<WavetableBank>,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    VStack::new(cx, move |cx| {
        for (idx, accent) in [
            (0, AMBER_RGB),
            (1, GOLD_RGB),
            (2, CORAL_RGB),
            (3, AMBER_RGB),
        ] {
            build_osc_panel(
                cx,
                idx,
                bank.clone(),
                params.clone(),
                gui_context.clone(),
                mod_ui.clone(),
                accent,
            );
        }

        // Per-voice stereo Noise generator strip below OSC 4
        build_noise_panel(cx, params, gui_context, mod_ui);
    })
    .row_between(Pixels(4.0))
    .width(Stretch(2.2))
    .height(Stretch(1.0));
}

fn build_filter_column(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    VStack::new(cx, move |cx| {
        build_filter_panel(
            cx,
            0,
            params.clone(),
            gui_context.clone(),
            mod_ui.clone(),
            AMBER_RGB,
        );
        build_filter_panel(
            cx,
            1,
            params.clone(),
            gui_context.clone(),
            mod_ui.clone(),
            CORAL_RGB,
        );
        build_insert_panel(cx, params, gui_context, mod_ui);
    })
    .row_between(Pixels(5.0))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}

fn build_fx_column(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    VStack::new(cx, move |cx| {
        fx_rack::build_main_fx_column(cx, params, gui_context, mod_ui);
    })
    .row_between(Pixels(4.0))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}

fn build_bottom_tabs(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    VStack::new(cx, move |cx| {
        let mod_ui_bar = mod_ui.clone();
        HStack::new(cx, move |cx| {
            build_tab_button(cx, BottomTab::VoiceEnv, "VOICE", 58.0);
            build_tab_button(cx, BottomTab::ModSeq, "MODSEQ", 64.0);
            build_tab_button(cx, BottomTab::Routing, "ROUTING", 66.0);
            build_tab_button(cx, BottomTab::MacrosGlobal, "MACROS", 66.0);
            build_mod_source_bar(cx, mod_ui_bar);
        })
        .class("tab-bar")
        .col_between(Pixels(4.0))
        .height(Pixels(26.0))
        .width(Stretch(1.0));

        let mod_ui_tabs = mod_ui.clone();
        Binding::new(cx, UiData::active_tab, move |cx, active| {
            match active.get(cx) {
                BottomTab::VoiceEnv => {
                    voice_tab::build_voice_tab(
                        cx,
                        params.clone(),
                        gui_context.clone(),
                        mod_ui_tabs.clone(),
                    );
                }
                BottomTab::ModSeq => {
                    build_modseq_tab(cx, params.clone(), gui_context.clone(), mod_ui_tabs.clone());
                }
                BottomTab::Routing => {
                    build_routing_tab(cx, params.clone(), gui_context.clone(), mod_ui_tabs.clone());
                }
                BottomTab::MacrosGlobal => {
                    build_macros_tab(cx, mod_ui_tabs.clone());
                }
            }
        });
    })
    .row_between(Pixels(3.0))
    .width(Stretch(1.0))
    .height(Pixels(136.0));
}

pub(crate) fn create(
    params: Arc<JSynPluginParams>,
    editor_state: Arc<ViziaState>,
    bank: Arc<WavetableBank>,
    telemetry: Arc<ModUiTelemetry>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Builtin, move |cx, gui_context| {
        cx.add_font_mem(include_bytes!("../../assets/fonts/PlusJakartaSans-Regular.ttf"));
        cx.add_font_mem(include_bytes!("../../assets/fonts/PlusJakartaSans-Bold.ttf"));
        cx.set_default_font(&["Plus Jakarta Sans"]);

        let initial_cfg = crate::presets::JSynConfig::load();
        let initial_scale_pct = initial_cfg.ui_scale.clamp(85, 200);
        let initial_theme = theme::find_theme_by_name(&initial_cfg.theme);
        let dynamic_css = Arc::new(std::sync::RwLock::new(initial_theme.render_css()));
        let _ = cx.add_stylesheet(theme::DynamicCssSource(dynamic_css.clone()));

        let mod_ui = Arc::new(ModUiState::new(params.clone(), telemetry.clone()));
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
            active_tab: BottomTab::VoiceEnv,
            modseq_subtab: ModSeqSubTab::MSeq(0),
            main_fx_bus: 0,
            mod_ui_rev: 0,
            mod_val_rev: 0,
            ui_scale_pct: initial_scale_pct,
            gear_radius: initial_cfg.gear_radius.clamp(0.10, 0.60),
            theme_name: initial_theme.name.clone(),
            themes_rev: 0,
            active_theme: initial_theme,
            dynamic_css,
            macro_names: initial_macro_names,
            current_preset_name: "INIT".to_string(),
            presets_dir_rev: 0,
            wavetables_dir_rev: 0,
            expanded_preset_folder: None,
            expanded_wt_folder: None,
            presets_dir_display: format_dir_display(initial_cfg.presets_dir.as_deref()),
            wavetables_dir_display: format_dir_display(initial_cfg.wavetables_dir.as_deref()),
            tooltip: TooltipState::default(),
            mod_menu: ModContextMenuState::default(),
        }
        .build(cx);

        let bank_for_ui = bank.clone();
        let params_for_ui = params.clone();
        let gui_ctx_for_ui = gui_context.clone();
        let params_menu = params_for_ui.clone();
        let gui_ctx_menu = gui_ctx_for_ui.clone();
        let mod_ui_for_ui = mod_ui.clone();

        VStack::new(cx, move |cx| {
            // Prevent Spacebar (e.g. DAW transport play/stop) from triggering focused buttons.
            cx.add_listener(|_view: &mut VStack, _cx, event| {
                event.map(|window_event, meta| match window_event {
                    WindowEvent::KeyDown(Code::Space, _)
                    | WindowEvent::KeyUp(Code::Space, _)
                    | WindowEvent::Press { mouse: false }
                    | WindowEvent::PressDown { mouse: false } => {
                        meta.consume();
                    }
                    _ => {}
                });
            });

            build_header_bar(cx, mod_ui_for_ui.clone());

            // Main chassis: 3-Column Rack or MATRIX Overview + 4-Tab Bottom Drawer
            VStack::new(cx, move |cx| {
                VStack::new(cx, move |cx| {
                    let params_tabs = params_for_ui.clone();
                    let gui_ctx_tabs = gui_ctx_for_ui.clone();
                    let mod_ui_tabs = mod_ui_for_ui.clone();
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
                                    build_osc_column(
                                        cx,
                                        bank_c,
                                        params_c.clone(),
                                        gui_c.clone(),
                                        mod_ui_c.clone(),
                                    );
                                    build_filter_column(
                                        cx,
                                        params_c.clone(),
                                        gui_c.clone(),
                                        mod_ui_c.clone(),
                                    );
                                    build_fx_column(cx, params_c, gui_c, mod_ui_c);
                                })
                                .col_between(Pixels(8.0))
                                .width(Stretch(1.0))
                                .height(Stretch(1.0));
                            }
                            MainViewMode::Fx => {
                                fx_rack::build_fx_rack_view(
                                    cx,
                                    params_m.clone(),
                                    gui_m.clone(),
                                    mod_ui_m.clone(),
                                );
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

                    build_bottom_tabs(cx, params_tabs, gui_ctx_tabs, mod_ui_tabs);
                })
                .class("section-panel")
                .row_between(Pixels(6.0))
                .width(Stretch(1.0))
                .height(Stretch(1.0));
            })
            .class("main-chassis")
            .width(Stretch(1.0))
            .height(Stretch(1.0));

            // Global floating value tooltip overlay
            HStack::new(cx, |cx| {
                HStack::new(cx, |cx| {
                    bold_label(cx, UiData::tooltip.map(|tt| tt.text.clone()), "knob-tooltip-text")
                        .width(Auto)
                        .height(Stretch(1.0))
                        .child_space(Stretch(1.0))
                        .hoverable(false);
                })
                .class("knob-tooltip")
                .width(Auto)
                .height(Pixels(20.0))
                .hoverable(false);
            })
            .position_type(PositionType::SelfDirected)
            .left(UiData::tooltip.map(|tt| {
                Pixels((tt.center_x - 110.0).clamp(4.0, BASE_WIDTH as f32 - 224.0))
            }))
            .top(UiData::tooltip.map(|tt| {
                let top_y = if tt.anchor_top >= 26.0 {
                    tt.anchor_top - 24.0
                } else {
                    (tt.anchor_bottom + 4.0).min(BASE_HEIGHT as f32 - 24.0)
                };
                Pixels(top_y)
            }))
            .child_left(Stretch(1.0))
            .child_right(Stretch(1.0))
            .width(Pixels(220.0))
            .height(Pixels(20.0))
            .z_index(300)
            .display(UiData::tooltip.map(|tt| tt.visible))
            .hoverable(false);

            // Click-away backdrop for floating Modulation Context Menu
            Element::new(cx)
                .position_type(PositionType::SelfDirected)
                .left(Pixels(0.0))
                .top(Pixels(0.0))
                .width(Stretch(1.0))
                .height(Stretch(1.0))
                .z_index(350)
                .display(UiData::mod_menu.map(|m| m.visible))
                .on_press(|cx| {
                    cx.emit(UiEvent::HideModContextMenu);
                });

            // Floating Modulation Context Menu overlay
            let mod_ui_menu = mod_ui;
            VStack::new(cx, move |cx| {
                let ui_menu = mod_ui_menu.clone();
                let p_menu = params_menu.clone();
                let g_menu = gui_ctx_menu.clone();
                Binding::new(cx, UiData::mod_menu, move |cx, menu_lens| {
                    let menu = menu_lens.get(cx);
                    if !menu.visible {
                        return;
                    }
                    let Some(dest) = ModDestination::from_index(menu.dest_idx) else {
                        return;
                    };
                    let slots = ui_menu.all_slots_for_dest(dest);
                    if slots.is_empty() && !menu.always_show_actions && menu.osc_pitch_idx.is_none()
                    {
                        return;
                    }

                    if let Some(osc_idx) = menu.osc_pitch_idx {
                        build_pitch_mode_menu_section(
                            cx,
                            osc_idx,
                            &p_menu,
                            &g_menu,
                        );
                        Element::new(cx)
                            .class("mod-context-sep")
                            .width(Stretch(1.0))
                            .height(Pixels(1.0));
                    }

                    let header = format!("MOD: {}", dest.name());
                    bold_label(cx, &header, "mod-context-header")
                        .width(Stretch(1.0))
                        .height(Pixels(16.0));

                    if menu.always_show_actions || menu.osc_pitch_idx.is_some() {
                        build_context_menu_actions_row(
                            cx,
                            dest,
                            !slots.is_empty(),
                            menu.show_mod_picker,
                            &ui_menu,
                            &p_menu,
                            &g_menu,
                        );
                        if menu.show_mod_picker {
                            build_context_mod_source_grid(cx, dest, &ui_menu);
                        }
                    }

                    for (slot_idx, slot) in slots {
                        let (r, g, b) = slot.source.color_rgb();
                        let src_name = ui_menu.source_display_name(slot.source);
                        let depth_str = format!("{:+.0}%", slot.depth * 100.0);
                        let ui_byp = ui_menu.clone();
                        let ui_pol = ui_menu.clone();
                        let ui_rem = ui_menu.clone();
                        let keep_open = menu.always_show_actions || menu.osc_pitch_idx.is_some();
                        HStack::new(cx, move |cx| {
                            Element::new(cx)
                                .width(Pixels(4.0))
                                .height(Pixels(12.0))
                                .top(Stretch(1.0))
                                .bottom(Stretch(1.0))
                                .background_color(Color::rgb(r, g, b));
                            bold_label(cx, &src_name, "mod-context-src")
                                .width(Stretch(1.0))
                                .height(Stretch(1.0))
                                .child_top(Stretch(1.0))
                                .child_bottom(Stretch(1.0));
                            bold_label(cx, &depth_str, "mod-context-depth")
                                .width(Pixels(36.0))
                                .height(Stretch(1.0))
                                .child_space(Stretch(1.0));
                            Button::new(
                                cx,
                                move |cx| {
                                    ui_byp.toggle_slot_bypass_at(slot_idx);
                                    cx.emit(UiEvent::ModUiChanged);
                                    cx.emit(UiEvent::ModMenuRefresh);
                                },
                                move |cx| {
                                    bold_label(
                                        cx,
                                        if slot.bypassed { "BYP" } else { "ON" },
                                        "mod-context-btn-text",
                                    )
                                    .child_space(Stretch(1.0))
                                    .hoverable(false)
                                },
                            )
                            .class("mod-context-btn")
                            .checked(!slot.bypassed)
                            .width(Pixels(30.0))
                            .height(Pixels(18.0));
                            Button::new(
                                cx,
                                move |cx| {
                                    ui_pol.toggle_slot_bipolar_at(slot_idx);
                                    cx.emit(UiEvent::ModUiChanged);
                                    cx.emit(UiEvent::ModMenuRefresh);
                                },
                                move |cx| {
                                    bold_label(
                                        cx,
                                        if slot.bipolar { "BIP" } else { "UNI" },
                                        "mod-context-btn-text",
                                    )
                                    .child_space(Stretch(1.0))
                                    .hoverable(false)
                                },
                            )
                            .class("mod-context-btn")
                            .checked(slot.bipolar)
                            .width(Pixels(30.0))
                            .height(Pixels(18.0));
                            CrossButton::new(cx, move |cx| {
                                ui_rem.remove_slot_at(slot_idx);
                                cx.emit(UiEvent::ModUiChanged);
                                if !keep_open && ui_rem.all_slots_for_dest(dest).is_empty() {
                                    cx.emit(UiEvent::HideModContextMenu);
                                } else {
                                    cx.emit(UiEvent::ModMenuRefresh);
                                }
                            })
                            .class("mod-context-btn")
                            .class("mod-context-btn-text")
                            .width(Pixels(20.0))
                            .height(Pixels(18.0));
                        })
                        .class("mod-context-row")
                        .col_between(Pixels(4.0))
                        .width(Stretch(1.0))
                        .height(Pixels(22.0));
                    }
                });
            })
            .class("mod-context-menu")
            .position_type(PositionType::SelfDirected)
            .left(UiData::mod_menu.map(|m| {
                Pixels(m.anchor_x.clamp(6.0, BASE_WIDTH as f32 - 236.0))
            }))
            .top(UiData::mod_menu.map(|m| {
                Pixels(m.anchor_y.clamp(6.0, BASE_HEIGHT as f32 - 236.0))
            }))
            .width(Pixels(228.0))
            .height(Auto)
            .z_index(360)
            .display(UiData::mod_menu.map(|m| m.visible));
        })
        .class("plugin-root")
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })
}

fn build_pitch_mode_menu_section(
    cx: &mut Context,
    osc_idx: usize,
    params: &Arc<JSynPluginParams>,
    gui_context: &Arc<dyn GuiContext>,
) {
    bold_label(cx, "PITCH MODE", "mod-context-header")
        .width(Stretch(1.0))
        .height(Pixels(15.0));

    let cur_mode = params.oscs[osc_idx].pitch_mode.value();
    for (label, target_mode) in [
        ("Pitch (Semitones)", PitchModeParam::Semitones),
        ("Ratio (FM Harmonic)", PitchModeParam::Ratio),
    ] {
        let active = cur_mode == target_mode;
        let p_set = params.clone();
        let g_set = gui_context.clone();
        Button::new(
            cx,
            move |cx| {
                let setter = ParamSetter::new(g_set.as_ref());
                let param = &p_set.oscs[osc_idx].pitch_mode;
                setter.begin_set_parameter(param);
                setter.set_parameter(param, target_mode);
                setter.end_set_parameter(param);
                cx.emit(UiEvent::ModUiChanged);
                cx.emit(UiEvent::HideModContextMenu);
            },
            move |cx| {
                HStack::new(cx, move |cx| {
                    if active {
                        CheckIcon::new(cx)
                            .class("mod-context-check")
                            .width(Pixels(12.0))
                            .height(Pixels(12.0))
                            .top(Stretch(1.0))
                            .bottom(Stretch(1.0));
                    } else {
                        Element::new(cx)
                            .width(Pixels(12.0))
                            .height(Pixels(12.0))
                            .hoverable(false);
                    }
                    bold_label(cx, label, "mod-context-btn-text")
                        .width(Stretch(1.0))
                        .height(Stretch(1.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0))
                        .hoverable(false);
                })
                .col_between(Pixels(4.0))
                .width(Stretch(1.0))
                .height(Stretch(1.0))
                .hoverable(false)
            },
        )
        .class("mod-context-mode-btn")
        .checked(active)
        .width(Stretch(1.0))
        .height(Pixels(20.0));
    }
}

fn build_context_menu_actions_row(
    cx: &mut Context,
    dest: ModDestination,
    has_slots: bool,
    picker_open: bool,
    mod_ui: &Arc<ModUiState>,
    params: &Arc<JSynPluginParams>,
    gui_context: &Arc<dyn GuiContext>,
) {
    let ui_clr = mod_ui.clone();
    let p_rst = params.clone();
    let g_rst = gui_context.clone();
    HStack::new(cx, move |cx| {
        Button::new(
            cx,
            |cx| cx.emit(UiEvent::ToggleModMenuPicker),
            |cx| {
                bold_label(cx, "Modulate", "mod-context-btn-text")
                    .child_space(Stretch(1.0))
                    .hoverable(false)
            },
        )
        .class("mod-context-btn")
        .checked(picker_open)
        .width(Stretch(1.0))
        .height(Stretch(1.0));

        if has_slots {
            Button::new(
                cx,
                move |cx| {
                    ui_clr.remove_all_slots_for_dest(dest);
                    cx.emit(UiEvent::ModUiChanged);
                    cx.emit(UiEvent::ModMenuRefresh);
                },
                |cx| {
                    bold_label(cx, "Clear Mod", "mod-context-btn-text")
                        .child_space(Stretch(1.0))
                        .hoverable(false)
                },
            )
            .class("mod-context-btn")
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        }

        Button::new(
            cx,
            move |cx| {
                reset_dest_param_to_default(&p_rst, &g_rst, dest);
                cx.emit(UiEvent::ModUiChanged);
                cx.emit(UiEvent::HideModContextMenu);
            },
            |cx| {
                bold_label(cx, "Reset", "mod-context-btn-text")
                    .child_space(Stretch(1.0))
                    .hoverable(false)
            },
        )
        .class("mod-context-btn")
        .width(Pixels(48.0))
        .height(Stretch(1.0));
    })
    .col_between(Pixels(4.0))
    .width(Stretch(1.0))
    .height(Pixels(19.0));
}

fn build_context_mod_source_grid(cx: &mut Context, dest: ModDestination, mod_ui: &Arc<ModUiState>) {
    VStack::new(cx, move |cx| {
        for chunk in matrix_tab::ALL_MOD_SOURCES.chunks(7) {
            HStack::new(cx, move |cx| {
                for &src in chunk {
                    let short = mod_source_menu_short_label(src);
                    let ui_assign = mod_ui.clone();
                    Button::new(
                        cx,
                        move |cx| {
                            ui_assign.assign_drop(src, dest);
                            cx.emit(UiEvent::ModUiChanged);
                            cx.emit(UiEvent::ToggleModMenuPicker);
                        },
                        move |cx| {
                            bold_label(cx, short, "mod-context-pick-text")
                                .child_space(Stretch(1.0))
                                .hoverable(false)
                        },
                    )
                    .class("mod-context-pick-btn")
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
                }
            })
            .col_between(Pixels(2.0))
            .width(Stretch(1.0))
            .height(Pixels(17.0));
        }
    })
    .class("mod-context-pick-grid")
    .row_between(Pixels(2.0))
    .width(Stretch(1.0))
    .height(Auto);
}

#[inline]
fn mod_source_menu_short_label(src: ModMatrixSource) -> &'static str {
    match src {
        ModMatrixSource::Env1 => "E1",
        ModMatrixSource::Env2 => "E2",
        ModMatrixSource::Env3 => "E3",
        ModMatrixSource::Env4 => "E4",
        ModMatrixSource::ModSeq1 => "MS1",
        ModMatrixSource::ModSeq2 => "MS2",
        ModMatrixSource::ModSeq3 => "MS3",
        ModMatrixSource::ModSeq4 => "MS4",
        ModMatrixSource::Macro1 => "M1",
        ModMatrixSource::Macro2 => "M2",
        ModMatrixSource::Macro3 => "M3",
        ModMatrixSource::Macro4 => "M4",
        ModMatrixSource::Macro5 => "M5",
        ModMatrixSource::Macro6 => "M6",
        ModMatrixSource::Macro7 => "M7",
        ModMatrixSource::Macro8 => "M8",
        ModMatrixSource::Velocity => "VEL",
        ModMatrixSource::Keytrack => "KEY",
        ModMatrixSource::ModWheel => "MW",
        ModMatrixSource::Aftertouch => "AT",
        ModMatrixSource::Slide => "SLD",
        ModMatrixSource::None => "--",
    }
}

fn reset_dest_param_to_default(
    params: &Arc<JSynPluginParams>,
    gui_context: &Arc<dyn GuiContext>,
    dest: ModDestination,
) {
    let param = match dest {
        ModDestination::Osc1Pitch => Some(&params.oscs[0].pitch),
        ModDestination::Osc2Pitch => Some(&params.oscs[1].pitch),
        ModDestination::Osc3Pitch => Some(&params.oscs[2].pitch),
        ModDestination::Osc4Pitch => Some(&params.oscs[3].pitch),
        ModDestination::Osc1Ratio => Some(&params.oscs[0].ratio),
        ModDestination::Osc2Ratio => Some(&params.oscs[1].ratio),
        ModDestination::Osc3Ratio => Some(&params.oscs[2].ratio),
        ModDestination::Osc4Ratio => Some(&params.oscs[3].ratio),
        ModDestination::Osc1HzOffset => Some(&params.oscs[0].hz_offset),
        ModDestination::Osc2HzOffset => Some(&params.oscs[1].hz_offset),
        ModDestination::Osc3HzOffset => Some(&params.oscs[2].hz_offset),
        ModDestination::Osc4HzOffset => Some(&params.oscs[3].hz_offset),
        _ => None,
    };
    if let Some(p) = param {
        let setter = ParamSetter::new(gui_context.as_ref());
        setter.begin_set_parameter(p);
        setter.set_parameter(p, p.default_plain_value());
        setter.end_set_parameter(p);
    }
}
