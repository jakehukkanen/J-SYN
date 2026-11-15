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

use j_syn_dsp::mod_matrix::{ModDestination, ModUiTelemetry};
use j_syn_dsp::WavetableBank;
use nih_plug::prelude::{Editor, GuiContext};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};

use crate::JSynPluginParams;

mod controls;
mod filter_panel;
mod insert_panel;
mod mod_bar;
mod modseq_tab;
mod noise_panel;
mod osc_panel;
mod routing_tab;
mod scope;
mod widgets;

use controls::{
    bold_label, build_bool_dropdown, build_header_knob_cell, build_header_knob_cell_mod,
    build_knob_cell, build_mini_pill_cell, build_module_header, build_placeholder_bay,
    build_tab_button, build_ui_scale_dropdown,
};
use filter_panel::build_filter_panel;
use insert_panel::build_insert_panel;
use mod_bar::{build_matrix_view, build_mod_source_bar, ModUiState};
use modseq_tab::{build_modseq_tab, ModSeqSubTab};
use noise_panel::build_noise_panel;
use osc_panel::build_osc_panel;
use routing_tab::build_routing_tab;
use widgets::LedToggleSwitch;

const BASE_WIDTH: u32 = 1160;
const BASE_HEIGHT: u32 = 760;
const DEFAULT_SCALE: f64 = 1.0;

pub(super) const AMBER_RGB: (u8, u8, u8) = (245, 179, 73);
pub(super) const GOLD_RGB: (u8, u8, u8) = (242, 203, 97);
pub(super) const CORAL_RGB: (u8, u8, u8) = (235, 140, 85);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainViewMode {
    Main,
    Matrix,
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

pub(crate) enum UiEvent {
    SelectViewMode(MainViewMode),
    SelectTab(BottomTab),
    SelectModSeqSubTab(ModSeqSubTab),
    ModUiChanged,
    SetUiScale(u32),
    ShowTooltip(TooltipState),
    HideTooltip,
}

#[derive(Lens)]
pub(crate) struct UiData {
    pub params: Arc<JSynPluginParams>,
    pub mod_ui: Arc<ModUiState>,
    pub view_mode: MainViewMode,
    pub active_tab: BottomTab,
    pub modseq_subtab: ModSeqSubTab,
    pub mod_ui_rev: u64,
    pub ui_scale_pct: u32,
    pub tooltip: TooltipState,
}

impl Model for UiData {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|ui_event, meta| match ui_event {
            UiEvent::SelectViewMode(mode) => {
                self.view_mode = *mode;
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
                self.tooltip.visible = false;
                meta.consume();
            }
            UiEvent::ModUiChanged => {
                self.mod_ui_rev = self.mod_ui_rev.wrapping_add(1);
                meta.consume();
            }
            UiEvent::SetUiScale(pct) => {
                self.ui_scale_pct = *pct;
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
    ViziaState::new_with_default_scale_factor(|| (BASE_WIDTH, BASE_HEIGHT), DEFAULT_SCALE)
}

fn build_header_bar(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    HStack::new(cx, move |cx| {
        Element::new(cx).class("rack-screw");
        HStack::new(cx, move |cx| {
            Element::new(cx).class("brand-pill");
            bold_label(cx, "J-SYN", "header-title");
            bold_label(cx, "16-VOICE WAVETABLE", "header-subtitle");
        })
        .col_between(Pixels(8.0))
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
            .width(Pixels(54.0))
            .height(Pixels(22.0));

            Button::new(
                cx,
                |cx| cx.emit(UiEvent::SelectViewMode(MainViewMode::Matrix)),
                |cx| bold_label(cx, "MATRIX", "view-switch-label").hoverable(false),
            )
            .class("view-switch-btn")
            .checked(UiData::view_mode.map(|m| *m == MainViewMode::Matrix))
            .width(Pixels(62.0))
            .height(Pixels(22.0));
        })
        .class("view-switch-group")
        .col_between(Pixels(3.0))
        .width(Auto)
        .height(Pixels(24.0))
        .top(Stretch(1.0))
        .bottom(Stretch(1.0));

        Element::new(cx).width(Stretch(1.0));

        let params_hdr = params.clone();
        let gui_ctx_hdr = gui_context.clone();
        let mod_ui_hdr = mod_ui.clone();
        HStack::new(cx, move |cx| {
            build_bool_dropdown(
                cx,
                params_hdr.clone(),
                gui_ctx_hdr.clone(),
                |p| &p.hq_mode,
                "Vintage 2007",
                "HQ",
            )
            .width(Pixels(112.0))
            .height(Pixels(24.0))
            .top(Stretch(1.0))
            .bottom(Stretch(1.0));

            build_bool_dropdown(
                cx,
                params_hdr.clone(),
                gui_ctx_hdr.clone(),
                |p| &p.mono_legato,
                "Poly 16",
                "Mono Legato",
            )
            .width(Pixels(106.0))
            .height(Pixels(24.0))
            .top(Stretch(1.0))
            .bottom(Stretch(1.0));

            build_mini_pill_cell(
                cx,
                UiData::params,
                |p| &p.pitch_bend_range,
                "PB RANGE",
                24,
                54.0,
            );

            build_ui_scale_dropdown(cx);

            build_header_knob_cell_mod(
                cx,
                UiData::params,
                |p| &p.glide_ms,
                "GLIDE",
                false,
                GOLD_RGB,
                ModDestination::GlideMs,
                mod_ui_hdr.clone(),
            );

            build_header_knob_cell(
                cx,
                UiData::params,
                |p| &p.master_volume_db,
                "MASTER",
                false,
                AMBER_RGB,
            );
        })
        .col_between(Pixels(10.0))
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

fn build_fx_column(cx: &mut Context) {
    VStack::new(cx, |cx| {
        for (title, subtitle) in [
            ("FX 1: OTT", "MULTIBAND UPWARD/DOWNWARD (M4)"),
            ("FX 2: DISTORTION", "TUBE / DIODE / CLIPPER (M4)"),
            ("FX 3: DELAY / CHORUS", "SYNC DELAY / DIMENSION (M4)"),
            ("FX 4: REVERB", "ALGORITHMIC HALL / SHIMMER (M4)"),
            ("FX 5: CLIPPER (J-SAT)", "ADAA CLIPPER / LIMITER (M4)"),
        ] {
            VStack::new(cx, |cx| {
                build_module_header(cx, title, 18.0);
                build_placeholder_bay(cx, title, subtitle);
            })
            .class("hardware-module")
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        }
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
        .height(Pixels(22.0))
        .width(Stretch(1.0));

        let mod_ui_tabs = mod_ui.clone();
        Binding::new(cx, UiData::active_tab, move |cx, active| {
            match active.get(cx) {
                BottomTab::VoiceEnv => {
                    HStack::new(cx, |cx| {
                        VStack::new(cx, |cx| {
                            build_module_header(cx, "KEYTRACKING", 18.0);

                            HStack::new(cx, move |cx| {
                                build_knob_cell(
                                    cx,
                                    UiData::params,
                                    |p| &p.filters[0].keytrack,
                                    "F1 KEYTRK",
                                    false,
                                    AMBER_RGB,
                                    32.0,
                                    62.0,
                                );
                                build_knob_cell(
                                    cx,
                                    UiData::params,
                                    |p| &p.filters[1].keytrack,
                                    "F2 KEYTRK",
                                    false,
                                    CORAL_RGB,
                                    32.0,
                                    62.0,
                                );

                                HStack::new(cx, |cx| {
                                    LedToggleSwitch::new(
                                        cx,
                                        UiData::params,
                                        |p| &p.oscs[0].keytrack,
                                        "OSC 1 KT",
                                        "OSC 1 KT",
                                        AMBER_RGB,
                                    )
                                    .width(Stretch(1.0))
                                    .height(Pixels(24.0));
                                    LedToggleSwitch::new(
                                        cx,
                                        UiData::params,
                                        |p| &p.oscs[1].keytrack,
                                        "OSC 2 KT",
                                        "OSC 2 KT",
                                        GOLD_RGB,
                                    )
                                    .width(Stretch(1.0))
                                    .height(Pixels(24.0));
                                    LedToggleSwitch::new(
                                        cx,
                                        UiData::params,
                                        |p| &p.oscs[2].keytrack,
                                        "OSC 3 KT",
                                        "OSC 3 KT",
                                        CORAL_RGB,
                                    )
                                    .width(Stretch(1.0))
                                    .height(Pixels(24.0));
                                    LedToggleSwitch::new(
                                        cx,
                                        UiData::params,
                                        |p| &p.oscs[3].keytrack,
                                        "OSC 4 KT",
                                        "OSC 4 KT",
                                        AMBER_RGB,
                                    )
                                    .width(Stretch(1.0))
                                    .height(Pixels(24.0));
                                })
                                .col_between(Pixels(6.0))
                                .width(Stretch(1.0))
                                .height(Pixels(24.0))
                                .top(Stretch(1.0))
                                .bottom(Stretch(1.0));
                            })
                            .col_between(Pixels(12.0))
                            .child_left(Pixels(8.0))
                            .child_right(Pixels(8.0))
                            .child_top(Stretch(1.0))
                            .child_bottom(Stretch(1.0))
                            .width(Stretch(1.0))
                            .height(Stretch(1.0));
                        })
                        .class("hardware-module")
                        .class("drawer-module")
                        .width(Stretch(1.55))
                        .height(Stretch(1.0));

                        VStack::new(cx, |cx| {
                            build_module_header(cx, "VOICE ENGINE", 18.0);
                            build_placeholder_bay(
                                cx,
                                "16-VOICE POLYPHONIC  |  MONO LEGATO",
                                "VINTAGE 2007 (1.5 KHZ TICK)  /  HQ (AUDIO-RATE)  |  ENV 1-4 IN MODSEQ",
                            );
                        })
                        .class("hardware-module")
                        .class("drawer-module")
                        .width(Stretch(0.95))
                        .height(Stretch(1.0));
                    })
                    .col_between(Pixels(6.0))
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
                }
                BottomTab::ModSeq => {
                    build_modseq_tab(cx, params.clone(), gui_context.clone(), mod_ui_tabs.clone());
                }
                BottomTab::Routing => {
                    build_routing_tab(cx, params.clone(), gui_context.clone(), mod_ui_tabs.clone());
                }
                BottomTab::MacrosGlobal => {
                    VStack::new(cx, |cx| {
                        build_module_header(
                            cx,
                            "MACROS / GLOBAL CONFIG (MILESTONE 3)",
                            22.0,
                        );

                        HStack::new(cx, move |cx| {
                            build_placeholder_bay(
                                cx,
                                "MACRO KNOBS 1 - 8",
                                "ASSIGNABLE PERFORMANCE MACROS (M3)",
                            );
                            build_placeholder_bay(
                                cx,
                                "GLOBAL CONFIG & PRESET PATHS",
                                "WAVETABLE LIBRARY / TUNING / QUALITY (M3)",
                            );
                        })
                        .col_between(Pixels(8.0))
                        .width(Stretch(1.0))
                        .height(Stretch(1.0));
                    })
                    .class("hardware-module")
                    .class("drawer-module")
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
                }
            }
        });
    })
    .row_between(Pixels(3.0))
    .width(Stretch(1.0))
    .height(Pixels(118.0));
}

pub(crate) fn create(
    params: Arc<JSynPluginParams>,
    editor_state: Arc<ViziaState>,
    bank: Arc<WavetableBank>,
    telemetry: Arc<ModUiTelemetry>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Builtin, move |cx, gui_context| {
        assets::register_noto_sans_bold(cx);
        cx.set_default_font(&[assets::NOTO_SANS]);

        let _ = cx.add_stylesheet(include_style!("src/theme.css"));

        let mod_ui = Arc::new(ModUiState::new(params.clone(), telemetry.clone()));
        let initial_scale_pct = (cx.user_scale_factor() * 100.0).round() as u32;
        UiData {
            params: params.clone(),
            mod_ui: mod_ui.clone(),
            view_mode: MainViewMode::Main,
            active_tab: BottomTab::VoiceEnv,
            modseq_subtab: ModSeqSubTab::MSeq(0),
            mod_ui_rev: 0,
            ui_scale_pct: initial_scale_pct,
            tooltip: TooltipState::default(),
        }
        .build(cx);

        let bank_for_ui = bank.clone();
        let params_for_ui = params.clone();
        let gui_ctx_for_ui = gui_context.clone();
        let mod_ui_for_ui = mod_ui.clone();

        VStack::new(cx, move |cx| {
            build_header_bar(
                cx,
                params_for_ui.clone(),
                gui_ctx_for_ui.clone(),
                mod_ui_for_ui.clone(),
            );

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
                                        params_c,
                                        gui_c,
                                        mod_ui_c,
                                    );
                                    build_fx_column(cx);
                                })
                                .col_between(Pixels(8.0))
                                .width(Stretch(1.0))
                                .height(Stretch(1.0));
                            }
                            MainViewMode::Matrix => {
                                build_matrix_view(cx, mod_ui_m.clone());
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
                Pixels((tt.center_x - 64.0).clamp(4.0, BASE_WIDTH as f32 - 132.0))
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
            .width(Pixels(128.0))
            .height(Pixels(20.0))
            .z_index(300)
            .display(UiData::tooltip.map(|tt| tt.visible))
            .hoverable(false);
        })
        .class("plugin-root")
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })
}
