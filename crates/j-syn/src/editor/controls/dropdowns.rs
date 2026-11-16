//! Scroll-wheel enabled dropdowns, enum/bool pickers, directory trees, wavetable & UI scale menus.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use j_syn_dsp::WavetableBank;
use nih_plug::prelude::{Enum, GuiContext, Param, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

use super::bold_label;
use super::icon_buttons::{ArrowDir, VectorArrow};
use super::super::{notify_baseview_resize_macos, UiData, UiEvent};
use crate::{JSynPluginParams, WavetableChoice};

pub(crate) fn build_dropdown_trigger_labels<T>(cx: &mut Context, text: impl Res<T> + Clone)
where
    T: ToStringLocalized,
{
    bold_label(cx, text, "compact-pill-text")
        .width(Stretch(1.0))
        .height(Stretch(1.0))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .overflow(Overflow::Hidden)
        .hoverable(false);
    VectorArrow::caret(cx, ArrowDir::Down, "compact-pill-text")
        .width(Pixels(10.0))
        .height(Stretch(1.0));
}

pub(crate) struct ScrollDropdown {
    scroll_accum: f32,
    last_sign: f32,
    last_step: Instant,
    on_step: Box<dyn Fn(&mut EventContext, isize) + Send + Sync>,
}

impl ScrollDropdown {
    pub(crate) fn new<'a, F, B>(cx: &'a mut Context, on_step: F, build_inner: B) -> Handle<'a, Self>
    where
        F: Fn(&mut EventContext, isize) + Send + Sync + 'static,
        B: FnOnce(&mut Context) -> Handle<'_, Dropdown>,
    {
        Self {
            scroll_accum: 0.0,
            last_sign: 0.0,
            last_step: Instant::now() - Duration::from_millis(100),
            on_step: Box::new(on_step),
        }
        .build(cx, |cx| {
            build_inner(cx).width(Stretch(1.0)).height(Stretch(1.0));
        })
    }
}

impl View for ScrollDropdown {
    fn element(&self) -> Option<&'static str> {
        Some("scroll-dropdown")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseScroll(_x, y) = window_event {
                if *y == 0.0 {
                    return;
                }
                let sign = y.signum();
                if self.last_sign != 0.0 && sign != self.last_sign {
                    self.scroll_accum = 0.0;
                }
                self.last_sign = sign;
                self.scroll_accum += *y;

                let now = Instant::now();
                if self.scroll_accum.abs() >= 0.8
                    && now.duration_since(self.last_step) >= Duration::from_millis(40)
                {
                    let delta = if self.scroll_accum < 0.0 { 1 } else { -1 };
                    (self.on_step)(cx, delta);
                    self.scroll_accum = 0.0;
                    self.last_step = now;
                }
                meta.consume();
            }
        });
    }
}

type EnumLabelOverride = Arc<dyn Fn(&Arc<JSynPluginParams>) -> Option<&'static str> + Send + Sync>;

fn build_enum_dropdown_inner<'a, E, P, FMap>(
    cx: &'a mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    params_to_param: FMap,
    items: &'static [(&'static str, E)],
    fixed_override: Option<EnumLabelOverride>,
    trigger_prefix: Option<&'static str>,
) -> Handle<'a, ScrollDropdown>
where
    E: Enum + Copy + PartialEq + Send + Sync + 'static,
    P: Param<Plain = E> + 'static,
    FMap: Fn(&Arc<JSynPluginParams>) -> &P + Copy + Send + Sync + 'static,
{
    let p_scroll = params.clone();
    let g_scroll = gui_context.clone();
    let ov_scroll = fixed_override.clone();
    let ov_trigger = fixed_override.clone();
    let ov_dd_disabled = fixed_override.clone();
    let ov_disabled = fixed_override.clone();
    let ov_class = fixed_override;

    let handle = ScrollDropdown::new(
        cx,
        move |cx, delta| {
            if items.is_empty() {
                return;
            }
            if let Some(ref ov) = ov_scroll {
                if ov(&p_scroll).is_some() {
                    return;
                }
            }
            let param = params_to_param(&p_scroll);
            let cur = param.unmodulated_plain_value();
            let pos = items.iter().position(|&(_, v)| v == cur).unwrap_or(0);
            let next = (pos as isize + delta).clamp(0, items.len() as isize - 1) as usize;
            if next != pos {
                let setter = ParamSetter::new(g_scroll.as_ref());
                setter.begin_set_parameter(param);
                setter.set_parameter(param, items[next].1);
                setter.end_set_parameter(param);
                cx.emit(super::UiEvent::ModUiChanged);
            }
        },
        move |cx| {
            let dd = Dropdown::new(
                cx,
                move |cx| {
                    let ov_t = ov_trigger.clone();
                    let text_lens = UiData::params.map(move |params| {
                        if let Some(ref ov) = ov_t {
                            if let Some(fixed) = ov(params) {
                                return fixed.to_string();
                            }
                        }
                        let param = params_to_param(params);
                        let cur = param.unmodulated_plain_value();
                        let base = items
                            .iter()
                            .find(|&&(_, v)| v == cur)
                            .map(|&(lbl, _)| lbl.to_string())
                            .unwrap_or_else(|| {
                                param.normalized_value_to_string(
                                    param.modulated_normalized_value(),
                                    true,
                                )
                            });
                        if let Some(prefix) = trigger_prefix {
                            format!("{prefix}{base}")
                        } else {
                            base
                        }
                    });
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
                    for &(label, val) in items {
                        let p_ref = params.clone();
                        let gui_ctx = gui_context.clone();
                        bold_label(cx, label, "dropdown-item").on_press(move |cx| {
                            let setter = ParamSetter::new(gui_ctx.as_ref());
                            let param = params_to_param(&p_ref);
                            setter.begin_set_parameter(param);
                            setter.set_parameter(param, val);
                            setter.end_set_parameter(param);
                            cx.emit(super::UiEvent::ModUiChanged);
                            cx.emit(PopupEvent::Close);
                        });
                    }
                },
            );
            if let Some(ov) = ov_dd_disabled {
                dd.disabled(UiData::params.map(move |params| ov(params).is_some()))
            } else {
                dd
            }
        },
    );

    if let (Some(ov_d), Some(ov_c)) = (ov_disabled, ov_class) {
        handle
            .disabled(UiData::params.map(move |params| ov_d(params).is_some()))
            .toggle_class(
                "dropdown-disabled",
                UiData::params.map(move |params| ov_c(params).is_some()),
            )
    } else {
        handle
    }
}

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
    build_enum_dropdown_inner(cx, params, gui_context, params_to_param, items, None, None)
}

pub(crate) fn build_prefixed_enum_dropdown<'a, E, P, FMap>(
    cx: &'a mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    params_to_param: FMap,
    items: &'static [(&'static str, E)],
    trigger_prefix: &'static str,
) -> Handle<'a, ScrollDropdown>
where
    E: Enum + Copy + PartialEq + Send + Sync + 'static,
    P: Param<Plain = E> + 'static,
    FMap: Fn(&Arc<JSynPluginParams>) -> &P + Copy + Send + Sync + 'static,
{
    build_enum_dropdown_inner(
        cx,
        params,
        gui_context,
        params_to_param,
        items,
        None,
        Some(trigger_prefix),
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
    build_enum_dropdown_inner(
        cx,
        params,
        gui_context,
        params_to_param,
        items,
        Some(Arc::new(override_fn)),
        None,
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
    let p_scroll = params.clone();
    let g_scroll = gui_context.clone();
    ScrollDropdown::new(
        cx,
        move |cx, delta| {
            let param = params_to_param(&p_scroll);
            let cur = if param.unmodulated_plain_value() {
                1isize
            } else {
                0isize
            };
            let next = (cur + delta).clamp(0, 1);
            if next != cur {
                let setter = ParamSetter::new(g_scroll.as_ref());
                setter.begin_set_parameter(param);
                setter.set_parameter(param, next == 1);
                setter.end_set_parameter(param);
                cx.emit(super::UiEvent::ModUiChanged);
            }
        },
        move |cx| {
            Dropdown::new(
                cx,
                move |cx| {
                    let text_lens = ParamWidgetBase::make_lens(
                        UiData::params,
                        params_to_param,
                        move |param| {
                            if param.modulated_normalized_value() >= 0.5 {
                                true_label.to_string()
                            } else {
                                false_label.to_string()
                            }
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
                    for &(label, val) in &[(false_label, false), (true_label, true)] {
                        let p_ref = params.clone();
                        let gui_ctx = gui_context.clone();
                        bold_label(cx, label, "dropdown-item").on_press(move |cx| {
                            let setter = ParamSetter::new(gui_ctx.as_ref());
                            let param = params_to_param(&p_ref);
                            setter.begin_set_parameter(param);
                            setter.set_parameter(param, val);
                            setter.end_set_parameter(param);
                            cx.emit(super::UiEvent::ModUiChanged);
                            cx.emit(PopupEvent::Close);
                        });
                    }
                },
            )
        },
    )
}

pub(crate) const WT_ITEMS: &[(&str, WavetableChoice)] = &[
    ("Basic Shapes", WavetableChoice::BasicShapes),
    ("Digital Formant", WavetableChoice::DigitalFormant),
    ("Harmonic Sync", WavetableChoice::HarmonicSync),
    ("Aggressive PWM", WavetableChoice::AggressivePwm),
    ("Custom .WAV", WavetableChoice::Custom),
];

fn set_wavetable_choice(
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

pub(crate) fn step_wavetable(
    params: &Arc<JSynPluginParams>,
    gui_context: &Arc<dyn GuiContext>,
    osc_idx: usize,
    delta: isize,
) {
    let cur = params.oscs[osc_idx].wt_select.value();
    let pos = WT_ITEMS.iter().position(|&(_, v)| v == cur).unwrap_or(0);
    let next = (pos as isize + delta).rem_euclid(WT_ITEMS.len() as isize) as usize;
    if next != pos {
        set_wavetable_choice(params, gui_context, osc_idx, WT_ITEMS[next].1);
    }
}

/// Render a recursive `DirectoryTree` as a scrollable list of root items and
/// collapsible folder categories (`▸ Folder (N)` / `▾ Folder (N)`).
/// Clicking a folder toggles its expanded state without closing the dropdown;
/// clicking a file invokes `on_select_file`.
pub(crate) fn build_directory_tree_items<FFile, FFolder>(
    cx: &mut Context,
    tree: crate::presets::DirectoryTree,
    expanded_folder: Option<String>,
    on_toggle_folder: FFolder,
    on_select_file: FFile,
) where
    FFolder: Fn(&mut EventContext, &str) + Clone + Send + Sync + 'static,
    FFile: Fn(&mut EventContext, &crate::presets::DirectoryFileEntry)
        + Clone
        + Send
        + Sync
        + 'static,
{
    if tree.root_files.is_empty() && tree.folders.is_empty() {
        return;
    }
    ScrollView::new(cx, 0.0, 0.0, false, true, move |cx| {
        VStack::new(cx, move |cx| {
            for entry in &tree.root_files {
                let entry_c = entry.clone();
                let on_file = on_select_file.clone();
                bold_label(cx, &entry.name, "dropdown-item").on_press(move |cx| {
                    on_file(cx, &entry_c);
                });
            }
            for folder in &tree.folders {
                let is_open = expanded_folder.as_deref() == Some(folder.name.as_str());
                let label = format!("{} ({})", folder.name, folder.files.len());
                let folder_name = folder.name.clone();
                let on_toggle = on_toggle_folder.clone();
                let row = HStack::new(cx, move |cx| {
                    VectorArrow::caret(
                        cx,
                        if is_open { ArrowDir::Down } else { ArrowDir::Right },
                        "dropdown-folder",
                    )
                    .width(Pixels(10.0))
                    .height(Stretch(1.0));
                    bold_label(cx, &label, "dropdown-folder")
                        .width(Stretch(1.0))
                        .height(Auto)
                        .hoverable(false);
                })
                .col_between(Pixels(3.0))
                .class("dropdown-folder")
                .on_press(move |cx| {
                    on_toggle(cx, &folder_name);
                });
                if is_open {
                    row.class("dropdown-folder-open");
                    for entry in &folder.files {
                        let entry_c = entry.clone();
                        let on_file = on_select_file.clone();
                        bold_label(cx, &entry.name, "dropdown-subitem").on_press(move |cx| {
                            on_file(cx, &entry_c);
                        });
                    }
                }
            }
        })
        .class("dropdown-scroll-content");
    })
    .class("dropdown-scroll");
}

pub(crate) fn build_wavetable_dropdown(
    cx: &mut Context,
    osc_idx: usize,
    bank: Arc<WavetableBank>,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
) -> Handle<'_, HStack> {
    let dialog_open = Arc::new(AtomicBool::new(false));

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
                        param.normalized_value_to_string(
                            param.modulated_normalized_value(),
                            true,
                        )
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
                for &(label, val) in WT_ITEMS {
                    let p_ref = params.clone();
                    let gui_ctx = gui_context.clone();
                    bold_label(cx, label, "dropdown-item").on_press(move |cx| {
                        let setter = ParamSetter::new(gui_ctx.as_ref());
                        let param = &p_ref.oscs[osc_idx].wt_select;
                        setter.begin_set_parameter(param);
                        setter.set_parameter(param, val);
                        setter.end_set_parameter(param);
                        cx.emit(PopupEvent::Close);
                    });
                }

                let bank_ref = bank.clone();
                let p_ref = params.clone();
                let gui_ctx = gui_context.clone();
                let dialog_flag_ref = dialog_open.clone();
                bold_label(cx, "Load .WAV...", "dropdown-action-item").on_press(move |cx| {
                    cx.emit(PopupEvent::Close);
                    if !dialog_flag_ref.swap(true, Ordering::AcqRel) {
                        let bank = bank_ref.clone();
                        let params = p_ref.clone();
                        let gui_context = gui_ctx.clone();
                        let dialog_flag = dialog_flag_ref.clone();
                        let start_dir = crate::presets::JSynConfig::load().wavetables_dir;

                        std::thread::spawn(move || {
                            let mut dlg =
                                rfd::FileDialog::new().add_filter("WAV Audio", &["wav", "WAV"]);
                            if let Some(dir) = start_dir.filter(|s| !s.trim().is_empty()) {
                                dlg = dlg.set_directory(dir);
                            }
                            if let Some(path) = dlg.pick_file() {
                                let _ = crate::presets::load_wavetable_file_for_osc(
                                    osc_idx,
                                    &path,
                                    &params,
                                    &gui_context,
                                    &bank,
                                );
                            }
                            dialog_flag.store(false, Ordering::Release);
                        });
                    }
                });

                let bank_dir = bank.clone();
                let p_dir = params.clone();
                let g_dir = gui_context.clone();
                Binding::new(cx, UiData::wavetables_dir_rev, move |cx, _| {
                    let cfg = crate::presets::JSynConfig::load();
                    let tree =
                        crate::presets::scan_wavetable_tree(cfg.wavetables_dir.as_deref());
                    let expanded = UiData::expanded_wt_folder.get(cx);
                    let bank_d = bank_dir.clone();
                    let p_d = p_dir.clone();
                    let g_d = g_dir.clone();
                    build_directory_tree_items(
                        cx,
                        tree,
                        expanded,
                        |cx, folder_name| {
                            cx.emit(UiEvent::ToggleWtFolder(folder_name.to_string()));
                        },
                        move |cx, entry| {
                            cx.emit(PopupEvent::Close);
                            let bank = bank_d.clone();
                            let params = p_d.clone();
                            let gui_context = g_d.clone();
                            let path = entry.path.clone();
                            std::thread::spawn(move || {
                                let _ = crate::presets::load_wavetable_file_for_osc(
                                    osc_idx,
                                    &path,
                                    &params,
                                    &gui_context,
                                    &bank,
                                );
                            });
                        },
                    );
                });
            },
        )
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })
}

pub(crate) fn build_ui_scale_dropdown(cx: &mut Context) {
    const SCALE_ITEMS: &[(&str, u32, f64)] = &[
        ("85%", 85_u32, 0.85_f64),
        ("100%", 100_u32, 1.0_f64),
        ("115%", 115_u32, 1.15_f64),
        ("125%", 125_u32, 1.25_f64),
        ("133%", 133_u32, 1.33_f64),
        ("150%", 150_u32, 1.50_f64),
        ("175%", 175_u32, 1.75_f64),
        ("200%", 200_u32, 2.00_f64),
    ];

    VStack::new(cx, |cx| {
        bold_label(cx, "UI SCALE", "mini-label")
            .width(Stretch(1.0))
            .height(Pixels(13.0))
            .child_space(Stretch(1.0));

        ScrollDropdown::new(
            cx,
            move |cx, delta| {
                let cur_pct = UiData::ui_scale_pct.get(cx);
                let pos = SCALE_ITEMS
                    .iter()
                    .position(|&(_, pct, _)| pct == cur_pct)
                    .unwrap_or(1);
                let next =
                    (pos as isize + delta).clamp(0, SCALE_ITEMS.len() as isize - 1) as usize;
                if next != pos {
                    let (_, pct, factor) = SCALE_ITEMS[next];
                    cx.set_user_scale_factor(factor);
                    cx.emit(UiEvent::SetUiScale(pct));
                    #[cfg(target_os = "macos")]
                    notify_baseview_resize_macos();
                }
            },
            |cx| {
                Dropdown::new(
                    cx,
                    |cx| {
                        HStack::new(cx, move |cx| {
                            build_dropdown_trigger_labels(
                                cx,
                                UiData::ui_scale_pct.map(|pct| format!("{pct}%")),
                            );
                        })
                        .width(Stretch(1.0))
                        .height(Stretch(1.0))
                        .child_top(Pixels(0.0))
                        .child_bottom(Pixels(0.0))
                        .overflow(Overflow::Hidden)
                    },
                    move |cx| {
                        for &(label, pct, factor) in SCALE_ITEMS {
                            bold_label(cx, label, "dropdown-item").on_press(move |cx| {
                                cx.set_user_scale_factor(factor);
                                cx.emit(UiEvent::SetUiScale(pct));
                                cx.emit(PopupEvent::Close);
                                #[cfg(target_os = "macos")]
                                notify_baseview_resize_macos();
                            });
                        }
                    },
                )
            },
        )
        .width(Pixels(60.0))
        .height(Pixels(19.0));
    })
    .width(Pixels(60.0))
    .height(Auto)
    .top(Stretch(1.0))
    .bottom(Stretch(1.0))
    .row_between(Pixels(2.0));
}
