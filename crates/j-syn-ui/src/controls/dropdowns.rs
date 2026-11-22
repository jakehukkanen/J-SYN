//! Scroll-wheel enabled dropdowns, generic enum/bool pickers, cascading directory trees, and UI scale menu.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use nih_plug::prelude::{Enum, GuiContext, Param, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

use super::bold_label;
use super::icon_buttons::{ArrowDir, VectorArrow};
use crate::shell::{notify_baseview_resize_macos, UiKitEvent, UiShellState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryFileEntry {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirectoryFolder {
    /// Immediate folder name (e.g. `"Analog"` or `"Bass"`).
    pub name: String,
    /// Relative folder path from the configured root (e.g. `"Bass/Reese"`).
    pub rel_path: String,
    pub files: Vec<DirectoryFileEntry>,
    pub subfolders: Vec<DirectoryFolder>,
}

impl DirectoryFolder {
    pub fn total_file_count(&self) -> usize {
        self.files.len()
            + self
                .subfolders
                .iter()
                .map(Self::total_file_count)
                .sum::<usize>()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirectoryTree {
    pub root_files: Vec<DirectoryFileEntry>,
    pub folders: Vec<DirectoryFolder>,
}

impl DirectoryTree {
    pub fn is_empty(&self) -> bool {
        self.root_files.is_empty() && self.folders.is_empty()
    }
}

pub fn build_dropdown_trigger_labels<T>(cx: &mut Context, text: impl Res<T> + Clone)
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

pub struct ScrollDropdown {
    scroll_accum: f32,
    last_sign: f32,
    last_step: Instant,
    on_step: Box<dyn Fn(&mut EventContext, isize) + Send + Sync>,
    on_activate: Option<Box<dyn Fn(&mut EventContext) + Send + Sync>>,
}

pub trait ScrollDropdownExt {
    fn on_activate<F>(self, cb: F) -> Self
    where
        F: Fn(&mut EventContext) + Send + Sync + 'static;
}

impl ScrollDropdownExt for Handle<'_, ScrollDropdown> {
    fn on_activate<F>(self, cb: F) -> Self
    where
        F: Fn(&mut EventContext) + Send + Sync + 'static,
    {
        self.modify(move |sd| sd.on_activate = Some(Box::new(cb)))
    }
}

impl ScrollDropdown {
    pub fn new<'a, F, B>(cx: &'a mut Context, on_step: F, build_inner: B) -> Handle<'a, Self>
    where
        F: Fn(&mut EventContext, isize) + Send + Sync + 'static,
        B: FnOnce(&mut Context) -> Handle<'_, Dropdown>,
    {
        Self {
            scroll_accum: 0.0,
            last_sign: 0.0,
            last_step: Instant::now() - Duration::from_millis(100),
            on_step: Box::new(on_step),
            on_activate: None,
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
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                if let Some(ref cb) = self.on_activate {
                    cb(cx);
                }
            }
            WindowEvent::MouseScroll(_x, y) => {
                if *y == 0.0
                    || crate::widgets::is_scroll_wheel_guarded(
                        cx.mouse().cursorx,
                        cx.mouse().cursory,
                    )
                {
                    return;
                }
                let popup_open = cx
                    .nth_child(0)
                    .and_then(|dd| cx.with_current(dd, |cx| cx.nth_child(0)))
                    .is_some_and(|trigger| cx.with_current(trigger, |cx| cx.is_checked()));
                let b = cx.bounds();
                let mx = cx.mouse().cursorx;
                let my = cx.mouse().cursory;
                if popup_open || mx < b.x || mx > b.x + b.w || my < b.y || my > b.y + b.h {
                    return;
                }
                if let Some(ref cb) = self.on_activate {
                    cb(cx);
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
            _ => {}
        });
    }
}

/// Wraps dropdown popup list contents so `WindowEvent::MouseScroll` is handled by any inner
/// `ScrollView` first and then consumed before bubbling out to an enclosing FX lane `ScrollView`.
pub struct PopupScrollCapture;

impl PopupScrollCapture {
    pub fn new<'a, F>(cx: &'a mut Context, content: F) -> Handle<'a, Self>
    where
        F: FnOnce(&mut Context),
    {
        Self.build(cx, content)
            .layout_type(LayoutType::Column)
            .width(Stretch(1.0))
            .height(Auto)
    }
}

impl View for PopupScrollCapture {
    fn element(&self) -> Option<&'static str> {
        Some("popup-scroll-capture")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseScroll(_, _) = window_event {
                let b = cx.bounds();
                let mx = cx.mouse().cursorx;
                let my = cx.mouse().cursory;
                if mx >= b.x && mx <= b.x + b.w && my >= b.y && my <= b.y + b.h {
                    meta.consume();
                }
            }
        });
    }
}

type EnumLabelOverride<Params> =
    Arc<dyn Fn(&Arc<Params>) -> Option<&'static str> + Send + Sync>;

#[allow(clippy::too_many_arguments)]
fn build_enum_dropdown_inner<'a, L, Params, E, P, FMap>(
    cx: &'a mut Context,
    params_lens: L,
    params: Arc<Params>,
    gui_context: Arc<dyn GuiContext>,
    params_to_param: FMap,
    items: &'static [(&'static str, E)],
    fixed_override: Option<EnumLabelOverride<Params>>,
    trigger_prefix: Option<&'static str>,
) -> Handle<'a, ScrollDropdown>
where
    L: Lens<Target = Arc<Params>> + Clone,
    Params: Send + Sync + 'static,
    E: Enum + Copy + PartialEq + Send + Sync + 'static,
    P: Param<Plain = E> + 'static,
    FMap: Fn(&Arc<Params>) -> &P + Copy + Send + Sync + 'static,
{
    let p_scroll = params.clone();
    let g_scroll = gui_context.clone();
    let ov_scroll = fixed_override.clone();
    let ov_trigger = fixed_override.clone();
    let ov_dd_disabled = fixed_override.clone();
    let ov_disabled = fixed_override.clone();
    let ov_class = fixed_override;
    let lens_dd = params_lens.clone();
    let lens_dis = params_lens.clone();
    let lens_cls = params_lens.clone();

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
                cx.emit(UiKitEvent::ModUiChanged);
            }
        },
        move |cx| {
            let lens_trig = params_lens.clone();
            let dd = Dropdown::new(
                cx,
                move |cx| {
                    let ov_t = ov_trigger.clone();
                    let text_lens = lens_trig.map(move |params| {
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
                    PopupScrollCapture::new(cx, |cx| {
                        if items.len() > 7 {
                            let p_scroll = params.clone();
                            let g_scroll = gui_context.clone();
                            ScrollView::new(cx, 0.0, 0.0, false, true, move |cx| {
                                VStack::new(cx, move |cx| {
                                    for &(label, val) in items {
                                        let p_ref = p_scroll.clone();
                                        let gui_ctx = g_scroll.clone();
                                        bold_label(cx, label, "dropdown-item").on_press(
                                            move |cx| {
                                                let setter = ParamSetter::new(gui_ctx.as_ref());
                                                let param = params_to_param(&p_ref);
                                                setter.begin_set_parameter(param);
                                                setter.set_parameter(param, val);
                                                setter.end_set_parameter(param);
                                                cx.emit(UiKitEvent::ModUiChanged);
                                                cx.emit(PopupEvent::Close);
                                            },
                                        );
                                    }
                                })
                                .class("dropdown-scroll-content");
                            })
                            .class("dropdown-scroll-compact");
                        } else {
                            for &(label, val) in items {
                                let p_ref = params.clone();
                                let gui_ctx = gui_context.clone();
                                bold_label(cx, label, "dropdown-item").on_press(move |cx| {
                                    let setter = ParamSetter::new(gui_ctx.as_ref());
                                    let param = params_to_param(&p_ref);
                                    setter.begin_set_parameter(param);
                                    setter.set_parameter(param, val);
                                    setter.end_set_parameter(param);
                                    cx.emit(UiKitEvent::ModUiChanged);
                                    cx.emit(PopupEvent::Close);
                                });
                            }
                        }
                    });
                },
            );
            if let Some(ov) = ov_dd_disabled {
                dd.disabled(lens_dd.map(move |params| ov(params).is_some()))
            } else {
                dd
            }
        },
    );

    if let (Some(ov_d), Some(ov_c)) = (ov_disabled, ov_class) {
        handle
            .disabled(lens_dis.map(move |params| ov_d(params).is_some()))
            .toggle_class(
                "dropdown-disabled",
                lens_cls.map(move |params| ov_c(params).is_some()),
            )
    } else {
        handle
    }
}

pub fn build_enum_dropdown<'a, L, Params, E, P, FMap>(
    cx: &'a mut Context,
    params_lens: L,
    params: Arc<Params>,
    gui_context: Arc<dyn GuiContext>,
    params_to_param: FMap,
    items: &'static [(&'static str, E)],
) -> Handle<'a, ScrollDropdown>
where
    L: Lens<Target = Arc<Params>> + Clone,
    Params: Send + Sync + 'static,
    E: Enum + Copy + PartialEq + Send + Sync + 'static,
    P: Param<Plain = E> + 'static,
    FMap: Fn(&Arc<Params>) -> &P + Copy + Send + Sync + 'static,
{
    build_enum_dropdown_inner(
        cx,
        params_lens,
        params,
        gui_context,
        params_to_param,
        items,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_prefixed_enum_dropdown<'a, L, Params, E, P, FMap>(
    cx: &'a mut Context,
    params_lens: L,
    params: Arc<Params>,
    gui_context: Arc<dyn GuiContext>,
    params_to_param: FMap,
    items: &'static [(&'static str, E)],
    trigger_prefix: &'static str,
) -> Handle<'a, ScrollDropdown>
where
    L: Lens<Target = Arc<Params>> + Clone,
    Params: Send + Sync + 'static,
    E: Enum + Copy + PartialEq + Send + Sync + 'static,
    P: Param<Plain = E> + 'static,
    FMap: Fn(&Arc<Params>) -> &P + Copy + Send + Sync + 'static,
{
    build_enum_dropdown_inner(
        cx,
        params_lens,
        params,
        gui_context,
        params_to_param,
        items,
        None,
        Some(trigger_prefix),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_enum_dropdown_with_override<'a, L, Params, E, P, FMap, FOverride>(
    cx: &'a mut Context,
    params_lens: L,
    params: Arc<Params>,
    gui_context: Arc<dyn GuiContext>,
    params_to_param: FMap,
    items: &'static [(&'static str, E)],
    override_fn: FOverride,
) -> Handle<'a, ScrollDropdown>
where
    L: Lens<Target = Arc<Params>> + Clone,
    Params: Send + Sync + 'static,
    E: Enum + Copy + PartialEq + Send + Sync + 'static,
    P: Param<Plain = E> + 'static,
    FMap: Fn(&Arc<Params>) -> &P + Copy + Send + Sync + 'static,
    FOverride: Fn(&Arc<Params>) -> Option<&'static str> + Send + Sync + 'static,
{
    build_enum_dropdown_inner(
        cx,
        params_lens,
        params,
        gui_context,
        params_to_param,
        items,
        Some(Arc::new(override_fn)),
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_bool_dropdown<'a, L, Params, P, FMap>(
    cx: &'a mut Context,
    params_lens: L,
    params: Arc<Params>,
    gui_context: Arc<dyn GuiContext>,
    params_to_param: FMap,
    false_label: &'static str,
    true_label: &'static str,
) -> Handle<'a, ScrollDropdown>
where
    L: Lens<Target = Arc<Params>> + Clone,
    Params: Send + Sync + 'static,
    P: Param<Plain = bool> + 'static,
    FMap: Fn(&Arc<Params>) -> &P + Copy + Send + Sync + 'static,
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
                cx.emit(UiKitEvent::ModUiChanged);
            }
        },
        move |cx| {
            Dropdown::new(
                cx,
                move |cx| {
                    let text_lens = ParamWidgetBase::make_lens(
                        params_lens,
                        move |p| params_to_param(p),
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
                            cx.emit(UiKitEvent::ModUiChanged);
                            cx.emit(PopupEvent::Close);
                        });
                    }
                },
            )
        },
    )
}

#[inline]
fn is_folder_in_active_path(folder_rel: &str, active_path: Option<&str>) -> bool {
    let Some(active) = active_path else {
        return false;
    };
    active == folder_rel
        || active
            .strip_prefix(folder_rel)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn collect_open_folder_panes<'a>(
    mut folders: &'a [DirectoryFolder],
    active_path: Option<&str>,
    out: &mut Vec<&'a DirectoryFolder>,
) {
    let Some(active) = active_path else {
        return;
    };
    while let Some(next_folder) = folders
        .iter()
        .find(|f| is_folder_in_active_path(&f.rel_path, Some(active)))
    {
        out.push(next_folder);
        folders = &next_folder.subfolders;
    }
}

#[allow(clippy::too_many_arguments)]
fn build_folder_row<FOpen, FToggle>(
    cx: &mut Context,
    folder: &DirectoryFolder,
    active_path: Option<&str>,
    on_open_folder: FOpen,
    on_toggle_folder: FToggle,
) where
    FOpen: Fn(&mut EventContext, &str) + Clone + Send + Sync + 'static,
    FToggle: Fn(&mut EventContext, &str) + Clone + Send + Sync + 'static,
{
    let is_open = is_folder_in_active_path(&folder.rel_path, active_path);
    let label = format!("{} ({})", folder.name, folder.total_file_count());
    let rel_hover = folder.rel_path.clone();
    let rel_press = folder.rel_path.clone();
    let row = HStack::new(cx, move |cx| {
        bold_label(cx, &label, "dropdown-folder")
            .width(Stretch(1.0))
            .height(Auto)
            .hoverable(false);
        VectorArrow::caret(cx, ArrowDir::Right, "dropdown-folder")
            .width(Pixels(10.0))
            .height(Stretch(1.0));
    })
    .col_between(Pixels(3.0))
    .class("dropdown-folder")
    .on_hover(move |cx| {
        on_open_folder(cx, &rel_hover);
    })
    .on_press(move |cx| {
        on_toggle_folder(cx, &rel_press);
    });
    if is_open {
        row.class("dropdown-folder-open");
    }
}

/// Render a recursive `DirectoryTree` as cascading side-by-side folder panes.
pub fn build_directory_tree_items<FFile, FOpen, FToggle>(
    cx: &mut Context,
    tree: DirectoryTree,
    expanded_folder: Option<String>,
    on_open_folder: FOpen,
    on_toggle_folder: FToggle,
    on_select_file: FFile,
) where
    FOpen: Fn(&mut EventContext, &str) + Clone + Send + Sync + 'static,
    FToggle: Fn(&mut EventContext, &str) + Clone + Send + Sync + 'static,
    FFile: Fn(&mut EventContext, &DirectoryFileEntry) + Clone + Send + Sync + 'static,
{
    build_directory_tree_panes(
        cx,
        tree,
        expanded_folder,
        |_| {},
        on_open_folder,
        on_toggle_folder,
        on_select_file,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn build_directory_tree_panes<FHeader, FFile, FOpen, FToggle>(
    cx: &mut Context,
    tree: DirectoryTree,
    expanded_folder: Option<String>,
    build_header: FHeader,
    on_open_folder: FOpen,
    on_toggle_folder: FToggle,
    on_select_file: FFile,
) where
    FHeader: FnOnce(&mut Context) + 'static,
    FOpen: Fn(&mut EventContext, &str) + Clone + Send + Sync + 'static,
    FToggle: Fn(&mut EventContext, &str) + Clone + Send + Sync + 'static,
    FFile: Fn(&mut EventContext, &DirectoryFileEntry) + Clone + Send + Sync + 'static,
{
    let mut header_opt = Some(build_header);
    HStack::new(cx, move |cx| {
        let header_fn = header_opt.take();
        let root_files = tree.root_files.clone();
        let root_folders = tree.folders.clone();
        let exp_root = expanded_folder.clone();
        let on_open_root = on_open_folder.clone();
        let on_toggle_root = on_toggle_folder.clone();
        let on_file_root = on_select_file.clone();

        ScrollView::new(cx, 0.0, 0.0, false, true, move |cx| {
            VStack::new(cx, move |cx| {
                if let Some(h) = header_fn {
                    h(cx);
                }
                for entry in &root_files {
                    let entry_c = entry.clone();
                    let on_file = on_file_root.clone();
                    bold_label(cx, &entry.name, "dropdown-item").on_press(move |cx| {
                        on_file(cx, &entry_c);
                    });
                }
                for folder in &root_folders {
                    build_folder_row(
                        cx,
                        folder,
                        exp_root.as_deref(),
                        on_open_root.clone(),
                        on_toggle_root.clone(),
                    );
                }
            })
            .class("dropdown-scroll-content");
        })
        .class("dropdown-cascade-pane");

        let mut open_panes = Vec::new();
        collect_open_folder_panes(&tree.folders, expanded_folder.as_deref(), &mut open_panes);
        for pane_folder in open_panes {
            let pane_files = pane_folder.files.clone();
            let pane_subs = pane_folder.subfolders.clone();
            let exp_sub = expanded_folder.clone();
            let on_open_sub = on_open_folder.clone();
            let on_toggle_sub = on_toggle_folder.clone();
            let on_file_sub = on_select_file.clone();

            ScrollView::new(cx, 0.0, 0.0, false, true, move |cx| {
                VStack::new(cx, move |cx| {
                    for entry in &pane_files {
                        let entry_c = entry.clone();
                        let on_file = on_file_sub.clone();
                        bold_label(cx, &entry.name, "dropdown-item").on_press(move |cx| {
                            on_file(cx, &entry_c);
                        });
                    }
                    for sub in &pane_subs {
                        build_folder_row(
                            cx,
                            sub,
                            exp_sub.as_deref(),
                            on_open_sub.clone(),
                            on_toggle_sub.clone(),
                        );
                    }
                })
                .class("dropdown-scroll-content");
            })
            .class("dropdown-cascade-subpane");
        }
    })
    .class("dropdown-cascade");
}

pub const UI_SCALE_ITEMS: &[(&str, u32, f64)] = &[
    ("85%", 85_u32, 0.85_f64),
    ("100%", 100_u32, 1.0_f64),
    ("115%", 115_u32, 1.15_f64),
    ("125%", 125_u32, 1.25_f64),
    ("133%", 133_u32, 1.33_f64),
    ("150%", 150_u32, 1.50_f64),
    ("175%", 175_u32, 1.75_f64),
    ("200%", 200_u32, 2.00_f64),
];

/// Compact inline UI scale picker (`85%..=200%`) driven by `UiShellState::ui_scale_pct`.
pub fn build_ui_scale_picker(cx: &mut Context) -> Handle<'_, ScrollDropdown> {
    ScrollDropdown::new(
        cx,
        move |cx, delta| {
            let cur_pct = UiShellState::ui_scale_pct.get(cx);
            let pos = UI_SCALE_ITEMS
                .iter()
                .position(|&(_, pct, _)| pct == cur_pct)
                .unwrap_or(1);
            let next =
                (pos as isize + delta).clamp(0, UI_SCALE_ITEMS.len() as isize - 1) as usize;
            if next != pos {
                let (_, pct, factor) = UI_SCALE_ITEMS[next];
                cx.set_user_scale_factor(factor);
                cx.emit(UiKitEvent::SetUiScale(pct));
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
                            UiShellState::ui_scale_pct.map(|pct| format!("{pct}%")),
                        );
                    })
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_top(Pixels(0.0))
                    .child_bottom(Pixels(0.0))
                    .overflow(Overflow::Hidden)
                },
                move |cx| {
                    ScrollView::new(cx, 0.0, 0.0, false, true, move |cx| {
                        VStack::new(cx, |cx| {
                            for &(label, pct, factor) in UI_SCALE_ITEMS {
                                bold_label(cx, label, "dropdown-item").on_press(move |cx| {
                                    cx.set_user_scale_factor(factor);
                                    cx.emit(UiKitEvent::SetUiScale(pct));
                                    cx.emit(PopupEvent::Close);
                                    notify_baseview_resize_macos();
                                });
                            }
                        })
                        .class("dropdown-scroll-content");
                    })
                    .class("dropdown-scroll-compact");
                },
            )
        },
    )
}

pub fn build_ui_scale_dropdown(cx: &mut Context) {
    VStack::new(cx, |cx| {
        bold_label(cx, "UI SCALE", "mini-label")
            .width(Stretch(1.0))
            .height(Pixels(13.0))
            .child_space(Stretch(1.0));

        build_ui_scale_picker(cx)
            .width(Pixels(60.0))
            .height(Pixels(19.0));
    })
    .width(Pixels(60.0))
    .height(Auto)
    .top(Stretch(1.0))
    .bottom(Stretch(1.0))
    .row_between(Pixels(2.0));
}
