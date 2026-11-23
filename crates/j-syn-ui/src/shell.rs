//! Shared plugin UI shell state (`UiShellState`), modulation host trait (`ModUiHost`),
//! cross-plugin events (`UiKitEvent`), font registration, tooltip overlay, and macOS resize hook.

use std::sync::{Arc, RwLock};

use j_syn_dsp::mod_matrix::{ModDestination, ModMatrixSource, ModSlot};
use nih_plug::prelude::{GuiContext, ParamPtr, PluginApi, PluginState};
use nih_plug_vizia::vizia::prelude::*;

use crate::controls::bold_label;
use crate::theme::{self, DynamicCssSource, ThemePalette};
use crate::tokens;

pub const PLUS_JAKARTA_REGULAR_TTF: &[u8] =
    include_bytes!("../assets/fonts/PlusJakartaSans-Regular.ttf");
pub const PLUS_JAKARTA_BOLD_TTF: &[u8] =
    include_bytes!("../assets/fonts/PlusJakartaSans-Bold.ttf");

/// Registers the shared `Plus Jakarta Sans` font family on a Vizia `Context`.
pub fn register_fonts(cx: &mut Context) {
    cx.add_font_mem(PLUS_JAKARTA_REGULAR_TTF);
    cx.add_font_mem(PLUS_JAKARTA_BOLD_TTF);
    cx.set_default_font(&["Plus Jakarta Sans"]);
}

/// Plugin-agnostic modulation UI host trait implemented by full synths like `J-SYN`.
/// Standalone FX plugins without a modulation matrix simply pass `None` for `ModTargetBinding`.
pub trait ModUiHost: Send + Sync + 'static {
    fn dragging_source(&self) -> ModMatrixSource;
    fn active_mod_source(&self) -> ModMatrixSource;
    fn is_drop_target(&self, dest: ModDestination) -> bool;
    fn register_drop_rect(&self, dest: ModDestination, tab_tag: u8, bounds: BoundingBox);
    fn unregister_drop_rect(&self, dest: ModDestination);
    fn slots_for_dest(&self, dest: ModDestination) -> ([ModSlot; 4], [usize; 4], usize);
    fn has_slots_for_dest(&self, dest: ModDestination) -> bool;
    fn active_slot_for_dest(&self, dest: ModDestination) -> Option<(usize, ModSlot)>;
    fn primary_slot_for_dest(&self, dest: ModDestination) -> Option<(usize, ModSlot)>;
    fn focus_slot(&self, dest: ModDestination, slot_idx: usize);
    fn adjust_slot_depth(&self, slot_idx: usize, new_depth: f32);
    fn remove_primary_slot_for_dest(&self, dest: ModDestination) -> bool;
    fn dest_offset(&self, dest: ModDestination) -> f32;
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct TooltipState {
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

/// Events emitted by shared `j-syn-ui` widgets (`RotaryKnob`, `ParamStepPill`, `PitchDragBox`,
/// `RatioDragBox`, `LedToggleSwitch`, `ScrollDropdown`, etc.).
pub enum UiKitEvent {
    ModUiChanged,
    ModValChanged,
    ShowTooltip(TooltipState),
    HideTooltip,
    ShowModContextMenu(ModDestination, f32, f32),
    ShowActionModContextMenu(ModDestination, f32, f32),
    ShowPitchContextMenu(usize, ModDestination, f32, f32),
    SelectTheme(String),
    SetUiScale(u32),
    ReloadThemes,
}

/// Shared root-level Vizia `Model` supplying the active `ThemePalette`, dynamic CSS,
/// floating `TooltipState`, and UI scale state to all widgets and scopes.
#[derive(Lens)]
pub struct UiShellState {
    pub ui_scale_pct: u32,
    pub theme_name: String,
    pub themes_rev: u64,
    #[lens(ignore)]
    pub active_theme: ThemePalette,
    #[lens(ignore)]
    pub dynamic_css: Arc<RwLock<String>>,
    pub tooltip: TooltipState,
    #[lens(ignore)]
    on_theme_change: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    #[lens(ignore)]
    on_scale_change: Option<Arc<dyn Fn(u32) + Send + Sync>>,
}

impl UiShellState {
    pub fn new(initial_theme: ThemePalette, initial_scale_pct: u32) -> Self {
        let dynamic_css = Arc::new(RwLock::new(initial_theme.render_css()));
        Self {
            ui_scale_pct: initial_scale_pct,
            theme_name: initial_theme.name.clone(),
            themes_rev: 0,
            active_theme: initial_theme,
            dynamic_css,
            tooltip: TooltipState::default(),
            on_theme_change: None,
            on_scale_change: None,
        }
    }

    pub fn with_theme_callback<F>(mut self, cb: F) -> Self
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.on_theme_change = Some(Arc::new(cb));
        self
    }

    pub fn with_scale_callback<F>(mut self, cb: F) -> Self
    where
        F: Fn(u32) + Send + Sync + 'static,
    {
        self.on_scale_change = Some(Arc::new(cb));
        self
    }

    /// Registers fonts, dynamic CSS stylesheet, and builds `UiShellState` on `cx`.
    pub fn install(self, cx: &mut Context) {
        register_fonts(cx);
        let _ = cx.add_stylesheet(DynamicCssSource(self.dynamic_css.clone()));
        self.build(cx);
    }
}

impl Model for UiShellState {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|ui_event, meta| match ui_event {
            UiKitEvent::ShowTooltip(tt) => {
                self.tooltip = tt.clone();
                meta.consume();
            }
            UiKitEvent::HideTooltip => {
                self.tooltip.visible = false;
                meta.consume();
            }
            UiKitEvent::SelectTheme(name) => {
                let pal = theme::find_theme_by_name(name);
                if let Ok(mut guard) = self.dynamic_css.write() {
                    *guard = pal.render_css();
                }
                if let Some(ref cb) = self.on_theme_change {
                    cb(&pal.name);
                }
                self.theme_name = pal.name.clone();
                self.active_theme = pal;
                let _ = cx.reload_styles();
                cx.needs_redraw();
                meta.consume();
            }
            UiKitEvent::SetUiScale(pct) => {
                self.ui_scale_pct = *pct;
                if let Some(ref cb) = self.on_scale_change {
                    cb(*pct);
                }
                meta.consume();
            }
            UiKitEvent::ReloadThemes => {
                self.themes_rev = self.themes_rev.wrapping_add(1);
                let pal = theme::find_theme_by_name(&self.theme_name);
                if let Ok(mut guard) = self.dynamic_css.write() {
                    *guard = pal.render_css();
                }
                self.active_theme = pal;
                let _ = cx.reload_styles();
                cx.needs_redraw();
                meta.consume();
            }
            // Modulation / structural events are consumed by plugin-specific root models (e.g. `J-SYN`'s `UiData`).
            UiKitEvent::ModUiChanged
            | UiKitEvent::ModValChanged
            | UiKitEvent::ShowModContextMenu(_, _, _)
            | UiKitEvent::ShowActionModContextMenu(_, _, _)
            | UiKitEvent::ShowPitchContextMenu(_, _, _, _) => {}
        });
    }
}

/// Builds the shared floating value tooltip overlay inside the root `VStack`.
pub fn build_tooltip_overlay(cx: &mut Context, window_w: f32, window_h: f32) {
    let wrap_w = tokens::TOOLTIP_WRAP_W.min((window_w - 8.0).max(80.0));
    HStack::new(cx, |cx| {
        HStack::new(cx, |cx| {
            bold_label(cx, UiShellState::tooltip.map(|tt| tt.text.clone()), "knob-tooltip-text")
                .width(Auto)
                .height(Stretch(1.0))
                .child_space(Stretch(1.0))
                .hoverable(false);
        })
        .class("knob-tooltip")
        .width(Auto)
        .height(Pixels(tokens::TOOLTIP_H))
        .hoverable(false);
    })
    .position_type(PositionType::SelfDirected)
    .left(UiShellState::tooltip.map(move |tt| {
        let est_pill_w = (tt.text.len() as f32 * 5.6 + 18.0).clamp(44.0, wrap_w - 10.0);
        let half_w = est_pill_w * 0.5;
        let clamped_cx = tt
            .center_x
            .clamp(4.0 + half_w, (window_w - 4.0 - half_w).max(4.0 + half_w));
        Pixels(clamped_cx - wrap_w * 0.5)
    }))
    .top(UiShellState::tooltip.map(move |tt| {
        let top_y = if tt.anchor_top >= 26.0 {
            tt.anchor_top - 24.0
        } else {
            (tt.anchor_bottom + 4.0).min((window_h - 24.0).max(2.0))
        };
        Pixels(top_y)
    }))
    .child_left(Stretch(1.0))
    .child_right(Stretch(1.0))
    .width(Pixels(wrap_w))
    .height(Pixels(tokens::TOOLTIP_H))
    .z_index(300)
    .display(UiShellState::tooltip.map(|tt| tt.visible))
    .hoverable(false);
}

/// macOS `baseview` resize workaround: notifies the `baseview_state` NSView via
/// `viewDidChangeBackingProperties:` so DAW host containers resize cleanly when changing UI scale.
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
pub fn notify_baseview_resize_macos() {
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

#[cfg(not(target_os = "macos"))]
pub fn notify_baseview_resize_macos() {}

/// No-op [`GuiContext`] implementation for headless preview rendering and automated tests.
pub struct NoopGuiContext;

impl GuiContext for NoopGuiContext {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Standalone
    }

    fn request_resize(&self) -> bool {
        true
    }

    unsafe fn raw_begin_set_parameter(&self, _param: ParamPtr) {}

    unsafe fn raw_set_parameter_normalized(&self, _param: ParamPtr, _normalized: f32) {}

    unsafe fn raw_end_set_parameter(&self, _param: ParamPtr) {}

    fn get_state(&self) -> PluginState {
        PluginState {
            version: String::new(),
            params: Default::default(),
            fields: Default::default(),
        }
    }

    fn set_state(&self, _state: PluginState) {}
}
