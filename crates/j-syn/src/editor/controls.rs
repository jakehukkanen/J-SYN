//! Shared typography, dropdowns, parameter readouts, knob/pill cells, and card helpers.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::WavetableBank;
use nih_plug::prelude::{Enum, GuiContext, Param, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

use super::mod_bar::ModUiState;
use super::widgets::{KnobDimExt, ParamStepPill, PitchDragBox, RotaryKnob, ValueFormatter};
use super::{BottomTab, UiData, UiEvent};
use crate::{JSynPluginParams, WavetableChoice};

pub(super) fn bold_label<'a, T>(
    cx: &'a mut Context,
    text: impl Res<T> + Clone,
    class_name: &str,
) -> Handle<'a, Label>
where
    T: ToStringLocalized,
{
    Label::new(cx, text)
        .class(class_name)
        .font_family(vec![FamilyOwned::Name(String::from("Plus Jakarta Sans"))])
        .font_weight(FontWeightKeyword::Bold)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ArrowDir {
    Left,
    Right,
    Up,
    Down,
}

#[inline]
fn resolve_glyph_color(cx: &DrawContext) -> nih_plug_vizia::vizia::vg::Color {
    use nih_plug_vizia::vizia::vg;
    let fc = cx.font_color();
    if fc.a() == 0 {
        let pal = super::theme::current_palette(cx);
        super::theme::vg_color(&pal.text_primary, 255)
    } else {
        vg::Color::rgba(fc.r(), fc.g(), fc.b(), fc.a())
    }
}

fn paint_button_box(cx: &DrawContext, canvas: &mut Canvas) {
    use nih_plug_vizia::vizia::vg;

    let b = cx.bounds();
    let dpx = cx.scale_factor();
    let rad = cx.border_top_left_radius().max(0.0);
    let lw = cx.border_width().max(1.0 * dpx);

    let bg = cx.background_color();
    if bg.a() > 0 {
        let mut path = vg::Path::new();
        path.rounded_rect(b.x, b.y, b.w, b.h, rad);
        canvas.fill_path(
            &path,
            &vg::Paint::color(vg::Color::rgba(bg.r(), bg.g(), bg.b(), bg.a())),
        );
    }

    let bc = cx.border_color();
    if bc.a() > 0 && lw > 0.0 {
        let inset = lw * 0.5;
        let mut border = vg::Path::new();
        border.rounded_rect(
            b.x + inset,
            b.y + inset,
            (b.w - lw).max(0.0),
            (b.h - lw).max(0.0),
            (rad - inset).max(0.0),
        );
        let mut stroke = vg::Paint::color(vg::Color::rgba(bc.r(), bc.g(), bc.b(), bc.a()));
        stroke.set_line_width(lw);
        canvas.stroke_path(&border, &stroke);
    }
}

fn draw_triangle_glyph(
    cx: &DrawContext,
    canvas: &mut Canvas,
    dir: ArrowDir,
    fixed_size_px: Option<f32>,
) {
    use nih_plug_vizia::vizia::vg;

    let b = cx.bounds();
    if b.w <= 1.0 || b.h <= 1.0 {
        return;
    }

    let cx_pt = (b.x + b.w * 0.5).round();
    let cy_pt = (b.y + b.h * 0.5).round();
    let size = match fixed_size_px {
        Some(px) => px * cx.scale_factor(),
        None => b.w.min(b.h) * 0.34,
    };
    let half_span = size * 0.52;
    let half_base = size * 0.56;
    let nudge = size * 0.04;

    let mut tri = vg::Path::new();
    match dir {
        ArrowDir::Left => {
            tri.move_to(cx_pt - half_span - nudge, cy_pt);
            tri.line_to(cx_pt + half_span - nudge, cy_pt - half_base);
            tri.line_to(cx_pt + half_span - nudge, cy_pt + half_base);
        }
        ArrowDir::Right => {
            tri.move_to(cx_pt + half_span + nudge, cy_pt);
            tri.line_to(cx_pt - half_span + nudge, cy_pt - half_base);
            tri.line_to(cx_pt - half_span + nudge, cy_pt + half_base);
        }
        ArrowDir::Up => {
            tri.move_to(cx_pt, cy_pt - half_span - nudge);
            tri.line_to(cx_pt - half_base, cy_pt + half_span - nudge);
            tri.line_to(cx_pt + half_base, cy_pt + half_span - nudge);
        }
        ArrowDir::Down => {
            tri.move_to(cx_pt, cy_pt + half_span + nudge);
            tri.line_to(cx_pt - half_base, cy_pt - half_span + nudge);
            tri.line_to(cx_pt + half_base, cy_pt - half_span + nudge);
        }
    }
    tri.close();
    canvas.fill_path(&tri, &vg::Paint::color(resolve_glyph_color(cx)));
}

fn handle_icon_button_event(
    action: &Option<Box<dyn Fn(&mut EventContext)>>,
    cx: &mut EventContext,
    event: &mut Event,
) {
    event.map(|window_event, meta| match window_event {
        WindowEvent::PressDown { mouse } => {
            if *mouse {
                cx.capture();
            }
            cx.focus();
        }
        WindowEvent::Press { .. } => {
            if meta.target == cx.current() {
                if let Some(cb) = action {
                    (cb)(cx);
                }
            }
        }
        WindowEvent::MouseUp(button) if *button == MouseButton::Left => {
            cx.release();
        }
        WindowEvent::ActionRequest(req) => {
            if let Action::Default = req.action {
                if let Some(cb) = action {
                    (cb)(cx);
                }
            }
        }
        _ => {}
    });
}

/// Self-contained arrow button drawing both its rounded border/background and centered triangle
/// from the exact same `cx.bounds()` rect in one pass.
pub(super) struct ArrowButton {
    action: Option<Box<dyn Fn(&mut EventContext)>>,
    dir_fn: Box<dyn Fn(&mut DrawContext) -> ArrowDir>,
}

impl ArrowButton {
    pub(super) fn new<'a, A>(
        cx: &'a mut Context,
        action: A,
        dir: ArrowDir,
    ) -> Handle<'a, Self>
    where
        A: 'static + Fn(&mut EventContext),
    {
        Self {
            action: Some(Box::new(action)),
            dir_fn: Box::new(move |_| dir),
        }
        .build(cx, |_| {})
        .role(Role::Button)
        .default_action_verb(DefaultActionVerb::Click)
        .cursor(CursorIcon::Hand)
        .navigable(true)
    }

    pub(super) fn reactive<'a, A, L, F>(
        cx: &'a mut Context,
        action: A,
        lens: L,
        dir_fn: F,
    ) -> Handle<'a, Self>
    where
        A: 'static + Fn(&mut EventContext),
        L: Lens,
        <L as Lens>::Target: Data,
        F: 'static + Fn(&mut DrawContext) -> ArrowDir,
    {
        Self {
            action: Some(Box::new(action)),
            dir_fn: Box::new(dir_fn),
        }
        .build(cx, |_| {})
        .role(Role::Button)
        .default_action_verb(DefaultActionVerb::Click)
        .cursor(CursorIcon::Hand)
        .navigable(true)
        .bind(lens, |mut handle, _| handle.needs_redraw())
    }
}

impl View for ArrowButton {
    fn element(&self) -> Option<&'static str> {
        Some("arrow-button")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        handle_icon_button_event(&self.action, cx, event);
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        if b.w <= 1.0 || b.h <= 1.0 {
            return;
        }
        paint_button_box(cx, canvas);
        let dir = (self.dir_fn)(cx);
        draw_triangle_glyph(cx, canvas, dir, None);
    }
}

/// Self-contained close (`×`) button drawing both its rounded box and centered cross in one pass.
pub(super) struct CrossButton {
    action: Option<Box<dyn Fn(&mut EventContext)>>,
}

impl CrossButton {
    pub(super) fn new<'a, A>(cx: &'a mut Context, action: A) -> Handle<'a, Self>
    where
        A: 'static + Fn(&mut EventContext),
    {
        Self {
            action: Some(Box::new(action)),
        }
        .build(cx, |_| {})
        .role(Role::Button)
        .default_action_verb(DefaultActionVerb::Click)
        .cursor(CursorIcon::Hand)
        .navigable(true)
    }
}

impl View for CrossButton {
    fn element(&self) -> Option<&'static str> {
        Some("cross-button")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        handle_icon_button_event(&self.action, cx, event);
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        use nih_plug_vizia::vizia::vg;

        let b = cx.bounds();
        if b.w <= 1.0 || b.h <= 1.0 {
            return;
        }
        paint_button_box(cx, canvas);

        let cx_pt = (b.x + b.w * 0.5).round();
        let cy_pt = (b.y + b.h * 0.5).round();
        let r = b.w.min(b.h) * 0.17;
        let mut cross = vg::Path::new();
        cross.move_to(cx_pt - r, cy_pt - r);
        cross.line_to(cx_pt + r, cy_pt + r);
        cross.move_to(cx_pt + r, cy_pt - r);
        cross.line_to(cx_pt - r, cy_pt + r);
        let mut paint = vg::Paint::color(resolve_glyph_color(cx));
        paint.set_line_width((1.5 * cx.scale_factor()).max(1.3));
        paint.set_line_cap(vg::LineCap::Round);
        canvas.stroke_path(&cross, &paint);
    }
}

/// Compact triangle caret view for dropdown triggers and folder disclosure rows.
pub(super) struct VectorArrow {
    dir: ArrowDir,
}

impl VectorArrow {
    pub(super) fn caret<'a>(
        cx: &'a mut Context,
        dir: ArrowDir,
        class_name: &str,
    ) -> Handle<'a, Self> {
        Self { dir }
            .build(cx, |_| {})
            .class(class_name)
            .hoverable(false)
    }
}

impl View for VectorArrow {
    fn element(&self) -> Option<&'static str> {
        Some("vector-arrow")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        draw_triangle_glyph(cx, canvas, self.dir, Some(4.5));
    }
}

pub(super) fn build_dropdown_trigger_labels<T>(cx: &mut Context, text: impl Res<T> + Clone)
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

pub(super) struct ScrollDropdown {
    scroll_accum: f32,
    last_sign: f32,
    last_step: Instant,
    on_step: Box<dyn Fn(&mut EventContext, isize) + Send + Sync>,
}

impl ScrollDropdown {
    pub(super) fn new<'a, F, B>(cx: &'a mut Context, on_step: F, build_inner: B) -> Handle<'a, Self>
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

pub(super) fn build_enum_dropdown<'a, E, P, FMap>(
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

pub(super) fn build_prefixed_enum_dropdown<'a, E, P, FMap>(
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

pub(super) fn build_enum_dropdown_with_override<'a, E, P, FMap, FOverride>(
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

pub(super) fn build_bool_dropdown<'a, P, FMap>(
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

pub(super) const WT_ITEMS: &[(&str, WavetableChoice)] = &[
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

pub(super) fn step_wavetable(
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
pub(super) fn build_directory_tree_items<FFile, FFolder>(
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

pub(super) fn build_wavetable_dropdown(
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

pub(super) fn build_ui_scale_dropdown(cx: &mut Context) {
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
                    super::notify_baseview_resize_macos();
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
                                super::notify_baseview_resize_macos();
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

pub(super) fn build_rotary_knob_with_opt_mod<'a, L, Params, P, FMap>(
    cx: &'a mut Context,
    params: L,
    params_to_param: FMap,
    bipolar: bool,
    accent_rgb: (u8, u8, u8),
    mod_target: Option<(ModDestination, Arc<ModUiState>, u8)>,
) -> Handle<'a, RotaryKnob>
where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    match mod_target {
        Some((dest, mod_ui, tab_tag)) => {
            RotaryKnob::new_with_mod(cx, params, params_to_param, bipolar, accent_rgb, dest, mod_ui, tab_tag)
        }
        None => RotaryKnob::new(cx, params, params_to_param, bipolar, accent_rgb),
    }
}

fn build_header_knob_cell_inner<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
    accent_rgb: (u8, u8, u8),
    mod_target: Option<(ModDestination, Arc<ModUiState>, u8)>,
    enabled_state: Option<(Arc<JSynPluginParams>, fn(&JSynPluginParams) -> bool)>,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    HStack::new(cx, move |cx| {
        let mut knob = build_rotary_knob_with_opt_mod(
            cx,
            params,
            params_to_param,
            bipolar,
            accent_rgb,
            mod_target,
        );
        if let Some((dim_params, dim_check)) = enabled_state {
            knob = knob.enabled_by(dim_params, dim_check);
        }
        knob.width(Pixels(36.0))
            .height(Pixels(36.0))
            .top(Stretch(1.0))
            .bottom(Stretch(1.0));

        bold_label(cx, title, "knob-title")
            .width(Pixels(46.0))
            .height(Stretch(1.0))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0));
    })
    .width(Auto)
    .height(Pixels(38.0))
    .col_between(Pixels(5.0));
}

pub(super) fn build_header_knob_cell<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
    accent_rgb: (u8, u8, u8),
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    build_header_knob_cell_inner(
        cx,
        params,
        params_to_param,
        title,
        bipolar,
        accent_rgb,
        None,
        None,
    );
}

pub(super) fn build_header_knob_cell_mod_dim<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
    accent_rgb: (u8, u8, u8),
    dest: ModDestination,
    mod_ui: Arc<ModUiState>,
    dim_params: Arc<JSynPluginParams>,
    dim_check: fn(&JSynPluginParams) -> bool,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    build_header_knob_cell_inner(
        cx,
        params,
        params_to_param,
        title,
        bipolar,
        accent_rgb,
        Some((dest, mod_ui, 0)),
        Some((dim_params, dim_check)),
    );
}

pub(super) fn build_param_readout_box<'a, L, Params, P, FMap>(
    cx: &'a mut Context,
    params: L,
    params_to_param: FMap,
) -> Handle<'a, HStack>
where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    HStack::new(
        cx,
        ParamWidgetBase::build_view(params, params_to_param, move |cx, param_data| {
            let readout_lens = param_data.make_lens(|param| {
                param.normalized_value_to_string(param.modulated_normalized_value(), true)
            });
            bold_label(cx, readout_lens, "knob-readout-text")
                .width(Stretch(1.0))
                .height(Stretch(1.0))
                .child_space(Stretch(1.0));
        }),
    )
    .class("knob-readout-box")
}

pub(super) fn build_knob_cell_with_readout<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
    accent_rgb: (u8, u8, u8),
    knob_sz: f32,
    cell_w: f32,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        bold_label(cx, title, "knob-title")
            .width(Stretch(1.0))
            .height(Pixels(12.0))
            .child_space(Stretch(1.0));

        build_rotary_knob_with_opt_mod(
            cx,
            params.clone(),
            params_to_param,
            bipolar,
            accent_rgb,
            None,
        )
        .width(Pixels(knob_sz))
        .height(Pixels(knob_sz))
        .left(Stretch(1.0))
        .right(Stretch(1.0));

        build_param_readout_box(cx, params, params_to_param)
            .width(Pixels(cell_w - 6.0))
            .height(Pixels(14.0))
            .left(Stretch(1.0))
            .right(Stretch(1.0));
    })
    .class("knob-cell")
    .width(Pixels(cell_w))
    .height(Auto)
    .row_between(Pixels(1.0));
}

pub(super) fn build_knob_cell<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
    accent_rgb: (u8, u8, u8),
    knob_sz: f32,
    cell_w: f32,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        bold_label(cx, title, "knob-title")
            .width(Stretch(1.0))
            .height(Pixels(14.0))
            .child_space(Stretch(1.0));

        build_rotary_knob_with_opt_mod(cx, params, params_to_param, bipolar, accent_rgb, None)
            .width(Pixels(knob_sz))
            .height(Pixels(knob_sz))
            .left(Stretch(1.0))
            .right(Stretch(1.0));
    })
    .class("knob-cell")
    .width(Pixels(cell_w))
    .height(Auto)
    .row_between(Pixels(2.0));
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_knob_cell_mod_fmt<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
    accent_rgb: (u8, u8, u8),
    knob_sz: f32,
    cell_w: f32,
    dest: ModDestination,
    mod_ui: Arc<ModUiState>,
    tab_tag: u8,
    value_formatter: ValueFormatter,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        bold_label(cx, title, "knob-title")
            .width(Stretch(1.0))
            .height(Pixels(14.0))
            .child_space(Stretch(1.0));

        RotaryKnob::new_with_mod_formatter(
            cx,
            params,
            params_to_param,
            bipolar,
            accent_rgb,
            dest,
            mod_ui,
            tab_tag,
            value_formatter,
        )
        .width(Pixels(knob_sz))
        .height(Pixels(knob_sz))
        .left(Stretch(1.0))
        .right(Stretch(1.0));
    })
    .class("knob-cell")
    .width(Pixels(cell_w))
    .height(Auto)
    .row_between(Pixels(2.0));
}

fn build_flex_knob_cell_mod_inner<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
    accent_rgb: (u8, u8, u8),
    knob_sz: f32,
    dest: ModDestination,
    mod_ui: Arc<ModUiState>,
    tab_tag: u8,
    enabled_state: Option<(Arc<JSynPluginParams>, fn(&JSynPluginParams) -> bool)>,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        bold_label(cx, title, "knob-title")
            .width(Stretch(1.0))
            .height(Pixels(14.0))
            .child_space(Stretch(1.0));

        let mut knob = build_rotary_knob_with_opt_mod(
            cx,
            params,
            params_to_param,
            bipolar,
            accent_rgb,
            Some((dest, mod_ui, tab_tag)),
        );
        if let Some((dim_params, dim_check)) = enabled_state {
            knob = knob.enabled_by(dim_params, dim_check);
        }
        knob.width(Pixels(knob_sz))
            .height(Pixels(knob_sz))
            .left(Stretch(1.0))
            .right(Stretch(1.0));
    })
    .class("knob-cell")
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(2.0));
}

pub(super) fn build_flex_knob_cell_mod<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
    accent_rgb: (u8, u8, u8),
    knob_sz: f32,
    dest: ModDestination,
    mod_ui: Arc<ModUiState>,
    tab_tag: u8,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    build_flex_knob_cell_mod_inner(
        cx,
        params,
        params_to_param,
        title,
        bipolar,
        accent_rgb,
        knob_sz,
        dest,
        mod_ui,
        tab_tag,
        None,
    );
}

pub(super) fn build_flex_knob_cell_mod_dim<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
    accent_rgb: (u8, u8, u8),
    knob_sz: f32,
    dest: ModDestination,
    mod_ui: Arc<ModUiState>,
    tab_tag: u8,
    dim_params: Arc<JSynPluginParams>,
    dim_check: fn(&JSynPluginParams) -> bool,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    build_flex_knob_cell_mod_inner(
        cx,
        params,
        params_to_param,
        title,
        bipolar,
        accent_rgb,
        knob_sz,
        dest,
        mod_ui,
        tab_tag,
        Some((dim_params, dim_check)),
    );
}

pub(super) fn build_inline_pill_cell<'a, L, Params, P, FMap>(
    cx: &'a mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    num_steps: usize,
    weight: f32,
) -> Handle<'a, ParamStepPill>
where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    ParamStepPill::new_inline(cx, params, params_to_param, num_steps, title)
        .width(Stretch(weight))
        .height(Pixels(20.0))
}

pub(super) fn build_mini_pill_cell<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    num_steps: usize,
    pill_w: f32,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        bold_label(cx, title, "mini-label")
            .width(Stretch(1.0))
            .height(Pixels(13.0))
            .child_space(Stretch(1.0));

        ParamStepPill::new(cx, params, params_to_param, num_steps)
            .width(Pixels(pill_w))
            .height(Pixels(19.0));
    })
    .width(Pixels(pill_w))
    .height(Auto)
    .top(Stretch(1.0))
    .bottom(Stretch(1.0))
    .row_between(Pixels(2.0));
}

pub(super) fn build_flex_pill_cell<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    num_steps: usize,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        bold_label(cx, title, "mini-label")
            .width(Stretch(1.0))
            .height(Pixels(12.0))
            .child_space(Stretch(1.0));

        ParamStepPill::new(cx, params, params_to_param, num_steps)
            .width(Stretch(1.0))
            .height(Pixels(17.0));
    })
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(1.0));
}

fn build_stacked_pill_shell<'a>(
    cx: &'a mut Context,
    title: &'static str,
    build_widget: impl FnOnce(&mut Context),
) -> Handle<'a, VStack> {
    VStack::new(cx, move |cx| {
        bold_label(cx, title, "mini-label")
            .width(Stretch(1.0))
            .height(Pixels(12.0))
            .child_space(Stretch(1.0));
        build_widget(cx);
    })
    .class("stacked-pill-cell")
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(2.0))
}

pub(super) fn build_pitch_pill_cell<'a, L, Params, P, FMap>(
    cx: &'a mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    dest: ModDestination,
    mod_ui: Arc<ModUiState>,
    tab_tag: u8,
) -> Handle<'a, VStack>
where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    build_stacked_pill_shell(cx, title, move |cx| {
        PitchDragBox::new_with_mod(cx, params, params_to_param, dest, mod_ui, tab_tag)
            .width(Stretch(1.0))
            .height(Pixels(19.0));
    })
}

pub(super) fn build_stacked_pill_cell<'a, L, Params, P, FMap>(
    cx: &'a mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    num_steps: usize,
) -> Handle<'a, VStack>
where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    build_stacked_pill_shell(cx, title, move |cx| {
        ParamStepPill::new(cx, params, params_to_param, num_steps)
            .width(Stretch(1.0))
            .height(Pixels(19.0));
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_stacked_pill_cell_mod<'a, L, Params, P, FMap>(
    cx: &'a mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    num_steps: usize,
    dest: ModDestination,
    mod_ui: Arc<ModUiState>,
    tab_tag: u8,
) -> Handle<'a, VStack>
where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    build_stacked_pill_shell(cx, title, move |cx| {
        ParamStepPill::new_with_mod(cx, params, params_to_param, num_steps, dest, mod_ui, tab_tag)
            .width(Stretch(1.0))
            .height(Pixels(19.0));
    })
}

pub(super) fn build_module_header(cx: &mut Context, title: &'static str, height_px: f32) {
    HStack::new(cx, move |cx| {
        bold_label(cx, title, "module-title");
    })
    .class("module-header")
    .height(Pixels(height_px))
    .width(Stretch(1.0));
}

pub(super) fn build_tab_button(
    cx: &mut Context,
    tab: BottomTab,
    title: &'static str,
    width_px: f32,
) {
    Button::new(
        cx,
        move |cx| cx.emit(UiEvent::SelectTab(tab)),
        move |cx| bold_label(cx, title, "tab-btn-label").hoverable(false),
    )
    .class("tab-btn")
    .checked(UiData::active_tab.map(move |t| *t == tab))
    .width(Pixels(width_px))
    .height(Stretch(1.0));
}
