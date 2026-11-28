//! J-SYN adapter controls re-exporting shared `j_syn_ui::controls::*` plus J-SYN-specific
//! wavetable browser dropdown and center stage tab buttons.

use std::sync::Arc;

use j_syn_dsp::WavetableBank;
use nih_plug::prelude::{Enum, GuiContext, Param, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

pub(crate) use j_syn_ui::controls::{
    ArrowButton, ArrowDir, CheckIcon, CollapseButton, CrossButton, GearIcon, ScrollDropdown,
    bold_label, build_directory_tree_items, build_directory_tree_panes,
    build_dropdown_trigger_labels, build_flex_knob_cell, build_flex_knob_cell_mod,
    build_flex_knob_cell_mod_dim, build_flex_knob_cell_mod_dim_tooltip,
    build_flex_knob_cell_mod_fmt, build_flex_pill_cell, build_header_knob_cell,
    build_inline_pill_cell, build_inline_pill_cell_mod, build_knob_cell,
    build_knob_cell_with_readout, build_module_header, build_param_readout_box,
    build_pitch_tuning_group, build_ratio_tuning_row, build_rotary_knob_with_opt_mod,
    build_ui_scale_dropdown,
};

use super::mod_bar::ModUiState;
use super::{BottomTab, CenterStageTab, UiData, UiEvent};
use crate::{JSynPluginParams, WavetableChoice};

pub(crate) fn build_enum_dropdown<'a, E, P, FMap>(
    cx: &'a mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    params_to_param: FMap,
    items: &'static [(&'static str, E)],
) -> Handle<'a, ScrollDropdown>
where
    E: Enum + Copy + PartialEq + Send + Sync + 'static,
    P: Param<Plain = E> + 'static,
    FMap: Fn(&Arc<JSynPluginParams>) -> &P + Copy + Send + Sync + 'static,
{
    j_syn_ui::controls::build_enum_dropdown(
        cx,
        UiData::params,
        params,
        gui_context,
        params_to_param,
        items,
    )
}

pub(crate) fn build_enum_dropdown_with_override<'a, E, P, FMap, FOverride>(
    cx: &'a mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    params_to_param: FMap,
    items: &'static [(&'static str, E)],
    override_fn: FOverride,
) -> Handle<'a, ScrollDropdown>
where
    E: Enum + Copy + PartialEq + Send + Sync + 'static,
    P: Param<Plain = E> + 'static,
    FMap: Fn(&Arc<JSynPluginParams>) -> &P + Copy + Send + Sync + 'static,
    FOverride: Fn(&Arc<JSynPluginParams>) -> Option<&'static str> + Send + Sync + 'static,
{
    j_syn_ui::controls::build_enum_dropdown_with_override(
        cx,
        UiData::params,
        params,
        gui_context,
        params_to_param,
        items,
        override_fn,
    )
}

pub(crate) fn build_bool_dropdown<'a, P, FMap>(
    cx: &'a mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    params_to_param: FMap,
    false_label: &'static str,
    true_label: &'static str,
) -> Handle<'a, ScrollDropdown>
where
    P: Param<Plain = bool> + 'static,
    FMap: Fn(&Arc<JSynPluginParams>) -> &P + Copy + Send + Sync + 'static,
{
    j_syn_ui::controls::build_bool_dropdown(
        cx,
        UiData::params,
        params,
        gui_context,
        params_to_param,
        false_label,
        true_label,
    )
}

pub(crate) fn build_center_tab_button(
    cx: &mut Context,
    tab: CenterStageTab,
    title: &'static str,
    width_weight: f32,
) {
    Button::new(
        cx,
        move |cx| match tab {
            CenterStageTab::Mod(sub) => {
                cx.emit(UiEvent::SelectTab(BottomTab::ModSeq));
                cx.emit(UiEvent::SelectModSeqSubTab(sub));
            }
            CenterStageTab::Voice => cx.emit(UiEvent::SelectTab(BottomTab::VoiceEnv)),
            CenterStageTab::Routing => cx.emit(UiEvent::SelectTab(BottomTab::Routing)),
        },
        move |cx| bold_label(cx, title, "tab-btn-label").hoverable(false),
    )
    .class("tab-btn")
    .checked(UiData::center_tab.map(move |t| *t == tab))
    .width(Stretch(width_weight))
    .height(Stretch(1.0));
}

pub(crate) const WT_ITEMS: &[(&str, WavetableChoice)] = &[
    ("Basic Shapes", WavetableChoice::BasicShapes),
    ("Digital Formant", WavetableChoice::DigitalFormant),
    ("Harmonic Sync", WavetableChoice::HarmonicSync),
    ("Aggressive PWM", WavetableChoice::AggressivePwm),
];

pub(crate) fn set_wavetable_choice(
    params: &JSynPluginParams,
    gui_context: &Arc<dyn GuiContext>,
    osc_idx: usize,
    choice: WavetableChoice,
) {
    let param = &params.oscs[osc_idx].wt_select;
    let setter = ParamSetter::new(gui_context.as_ref());
    setter.begin_set_parameter(param);
    setter.set_parameter(param, choice);
    setter.end_set_parameter(param);
}

fn scan_wavs_in_folder(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<std::path::PathBuf> = rd
        .filter_map(|res| res.ok().map(|e| e.path()))
        .filter(|path| {
            let hidden = path
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.starts_with('.'));
            !hidden
                && path.is_file()
                && path
                    .extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
        })
        .collect();
    files.sort_by(|a, b| {
        a.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase()
            .cmp(
                &b.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase(),
            )
    });
    files
}

fn spawn_load_wavetable(
    osc_idx: usize,
    path: std::path::PathBuf,
    params: Arc<JSynPluginParams>,
    mod_ui: Arc<ModUiState>,
    bank: Arc<WavetableBank>,
) {
    let load_gen = mod_ui.next_wt_load_gen(osc_idx);
    std::thread::spawn(move || {
        if mod_ui.current_wt_load_gen(osc_idx) != load_gen {
            return;
        }
        let Ok(wt) = j_syn_dsp::Wavetable::load_from_wav(&path) else {
            return;
        };
        if mod_ui.current_wt_load_gen(osc_idx) != load_gen {
            return;
        }
        bank.swap_custom(osc_idx, Arc::new(wt));
        if let Ok(mut guard) = params.wavetable_paths.lock() {
            guard[osc_idx] = path.to_string_lossy().into_owned();
        }
        // Marshal the VST3/CLAP parameter edit back to the UI thread via the 60 Hz timer.
        mod_ui.queue_pending_wt_custom(osc_idx, load_gen);
    });
}

pub(crate) fn step_wavetable(
    params: &Arc<JSynPluginParams>,
    gui_context: &Arc<dyn GuiContext>,
    mod_ui: &Arc<ModUiState>,
    bank: &Arc<WavetableBank>,
    osc_idx: usize,
    delta: isize,
) {
    let cur = params.oscs[osc_idx].wt_select.value();
    if cur == WavetableChoice::Custom {
        let cur_path = params
            .wavetable_paths
            .lock()
            .ok()
            .map(|g| g[osc_idx].clone())
            .unwrap_or_default();
        if !cur_path.trim().is_empty() {
            let cur_pb = std::path::PathBuf::from(&cur_path);
            if let Some(parent) = cur_pb.parent() {
                let files = scan_wavs_in_folder(parent);
                if !files.is_empty() {
                    let pos = files
                        .iter()
                        .position(|p| p == &cur_pb || p.file_name() == cur_pb.file_name())
                        .unwrap_or(0);
                    let next = (pos as isize + delta).rem_euclid(files.len() as isize) as usize;
                    if files.len() > 1 || next != pos {
                        spawn_load_wavetable(
                            osc_idx,
                            files[next].clone(),
                            params.clone(),
                            mod_ui.clone(),
                            bank.clone(),
                        );
                    }
                    return;
                }
            }
        }
    }

    mod_ui.cancel_wt_load(osc_idx);
    let pos = WT_ITEMS.iter().position(|&(_, v)| v == cur).unwrap_or(0);
    let next = (pos as isize + delta).rem_euclid(WT_ITEMS.len() as isize) as usize;
    if next != pos || cur == WavetableChoice::Custom {
        set_wavetable_choice(params, gui_context, osc_idx, WT_ITEMS[next].1);
    }
}

pub(crate) fn build_wavetable_dropdown(
    cx: &mut Context,
    osc_idx: usize,
    bank: Arc<WavetableBank>,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) -> Handle<'_, HStack> {
    HStack::new(cx, move |cx| {
        let p_trigger = params.clone();
        Dropdown::new(
            cx,
            move |cx| {
                let p_label = p_trigger.clone();
                let text_lens = ParamWidgetBase::make_lens(
                    UiData::params,
                    move |p| &p.oscs[osc_idx].wt_select,
                    move |param| {
                        if param.value() == WavetableChoice::Custom {
                            if let Ok(guard) = p_label.wavetable_paths.lock() {
                                let path_str = guard[osc_idx].trim();
                                if !path_str.is_empty() {
                                    if let Some(stem) = std::path::Path::new(path_str)
                                        .file_stem()
                                        .and_then(|s| s.to_str())
                                    {
                                        if !stem.is_empty() {
                                            return stem.to_string();
                                        }
                                    }
                                }
                            }
                        }
                        param.normalized_value_to_string(param.modulated_normalized_value(), true)
                    },
                );
                HStack::new(cx, move |cx| {
                    build_dropdown_trigger_labels(cx, text_lens);
                })
                .width(Stretch(1.0))
                .height(Stretch(1.0))
                .child_top(Pixels(0.0))
                .child_bottom(Pixels(0.0))
                .overflow(Overflow::Hidden)
            },
            move |cx| {
                let bank_dir = bank.clone();
                let p_dir = params.clone();
                let g_dir = gui_context.clone();
                let m_dir = mod_ui.clone();
                Binding::new(cx, UiData::wavetables_dir_rev, move |cx, _| {
                    let cfg = crate::presets::JSynConfig::load();
                    let tree = crate::presets::scan_wavetable_tree(cfg.wavetables_dir.as_deref());
                    let expanded = UiData::expanded_wt_folder.get(cx);
                    let p_hdr = p_dir.clone();
                    let g_hdr = g_dir.clone();
                    let m_hdr = m_dir.clone();
                    let bank_d = bank_dir.clone();
                    let p_d = p_dir.clone();
                    let m_d = m_dir.clone();
                    build_directory_tree_panes(
                        cx,
                        tree,
                        expanded,
                        move |cx| {
                            for &(label, val) in WT_ITEMS {
                                let p_ref = p_hdr.clone();
                                let gui_ctx = g_hdr.clone();
                                let m_ref = m_hdr.clone();
                                bold_label(cx, label, "dropdown-item").on_press(move |cx| {
                                    m_ref.cancel_wt_load(osc_idx);
                                    set_wavetable_choice(&p_ref, &gui_ctx, osc_idx, val);
                                    cx.emit(PopupEvent::Close);
                                });
                            }
                        },
                        |cx, folder_path| {
                            cx.emit(UiEvent::OpenWtFolder(folder_path.to_string()));
                        },
                        |cx, folder_path| {
                            cx.emit(UiEvent::ToggleWtFolder(folder_path.to_string()));
                        },
                        move |cx, entry| {
                            cx.emit(PopupEvent::Close);
                            spawn_load_wavetable(
                                osc_idx,
                                entry.path.clone(),
                                p_d.clone(),
                                m_d.clone(),
                                bank_d.clone(),
                            );
                        },
                    );
                });
            },
        )
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })
}
