//! Warm dark-mode skeuomorphic Vizia GUI for `J-SYN` (Milestone 1).
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

use std::sync::Arc;

use j_syn_dsp::WavetableBank;
use nih_plug::prelude::{Editor, GuiContext};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};

use crate::JSynPluginParams;

mod controls;
mod osc_panel;
mod scope;
mod widgets;

use controls::{
    bold_label, build_bool_dropdown, build_header_knob_cell, build_knob_cell,
    build_mini_pill_cell, build_module_header, build_placeholder_bay, build_tab_button,
    build_ui_scale_dropdown,
};
use osc_panel::build_osc_panel;

const BASE_WIDTH: u32 = 1160;
const BASE_HEIGHT: u32 = 760;
const DEFAULT_SCALE: f64 = 1.0;

pub(super) const AMBER_RGB: (u8, u8, u8) = (245, 179, 73);
pub(super) const GOLD_RGB: (u8, u8, u8) = (242, 203, 97);
pub(super) const CORAL_RGB: (u8, u8, u8) = (235, 140, 85);

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

pub(crate) enum UiEvent {
    SelectTab(BottomTab),
    SetUiScale(u32),
}

#[derive(Lens)]
pub(crate) struct UiData {
    pub params: Arc<JSynPluginParams>,
    pub active_tab: BottomTab,
    pub ui_scale_pct: u32,
}

impl Model for UiData {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|ui_event, meta| match ui_event {
            UiEvent::SelectTab(tab) => {
                self.active_tab = *tab;
                meta.consume();
            }
            UiEvent::SetUiScale(pct) => {
                self.ui_scale_pct = *pct;
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

        Element::new(cx).width(Stretch(1.0));

        let params_hdr = params.clone();
        let gui_ctx_hdr = gui_context.clone();
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

            build_header_knob_cell(
                cx,
                UiData::params,
                |p| &p.glide_ms,
                "GLIDE",
                false,
                GOLD_RGB,
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
                accent,
            );
        }

        // Reserved compact Noise slot below OSC 4
        HStack::new(cx, move |cx| {
            bold_label(cx, "NOISE (M2)", "module-title").width(Pixels(92.0));
            build_placeholder_bay(
                cx,
                "NOISE GENERATOR",
                "COLOR / AMP / FILTER ROUTING (MILESTONE 2)",
            );
        })
        .class("hardware-module")
        .col_between(Pixels(8.0))
        .width(Stretch(1.0))
        .height(Pixels(38.0));
    })
    .row_between(Pixels(4.0))
    .width(Stretch(2.2))
    .height(Stretch(1.0));
}

fn build_filter_column(cx: &mut Context) {
    VStack::new(cx, |cx| {
        for (header, title, subtitle) in [
            (
                "FILTER 1 (M2)",
                "MULTIMODE FILTER 1",
                "LP / BP / HP / DAFT / SCREAM",
            ),
            ("FILTER 2 (M2)", "MULTIMODE FILTER 2", "DUAL / COMB / FORMANT"),
        ] {
            VStack::new(cx, |cx| {
                build_module_header(cx, header, 22.0);
                build_placeholder_bay(cx, title, subtitle);
            })
            .class("hardware-module")
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        }
    })
    .row_between(Pixels(6.0))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}

fn build_fx_column(cx: &mut Context) {
    VStack::new(cx, |cx| {
        for (title, subtitle) in [
            ("FX 1: OTT", "MULTIBAND UPWARD/DOWNWARD (M4)"),
            ("FX 2: DISTORTION", "TUBE / DIODE / CLIPPER (M4)"),
            ("FX 3: DELAY / CHORUS", "SYNC DELAY / DIMENSION (M4)"),
            ("FX 4: REVERB / CLIP", "HALL REVERB / LIMITER (M4)"),
        ] {
            VStack::new(cx, |cx| {
                build_module_header(cx, title, 20.0);
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

fn build_bottom_tabs(cx: &mut Context) {
    VStack::new(cx, |cx| {
        HStack::new(cx, move |cx| {
            build_tab_button(cx, BottomTab::VoiceEnv, "VOICE & ENV");
            build_tab_button(cx, BottomTab::ModSeq, "MODSEQ");
            build_tab_button(cx, BottomTab::Routing, "ROUTING");
            build_tab_button(cx, BottomTab::MacrosGlobal, "MACROS / GLOBAL");
        })
        .class("tab-bar")
        .col_between(Pixels(6.0))
        .height(Pixels(24.0))
        .width(Stretch(1.0));

        Binding::new(cx, UiData::active_tab, move |cx, active| {
            match active.get(cx) {
                BottomTab::VoiceEnv => {
                    VStack::new(cx, |cx| {
                        build_module_header(cx, "AMP ENVELOPE (ADSR)", 20.0);

                        HStack::new(cx, move |cx| {
                            build_knob_cell(
                                cx,
                                UiData::params,
                                |p| &p.amp_attack,
                                "ATTACK",
                                false,
                                AMBER_RGB,
                                40.0,
                                78.0,
                            );
                            build_knob_cell(
                                cx,
                                UiData::params,
                                |p| &p.amp_decay,
                                "DECAY",
                                false,
                                AMBER_RGB,
                                40.0,
                                78.0,
                            );
                            build_knob_cell(
                                cx,
                                UiData::params,
                                |p| &p.amp_sustain,
                                "SUSTAIN",
                                false,
                                GOLD_RGB,
                                40.0,
                                78.0,
                            );
                            build_knob_cell(
                                cx,
                                UiData::params,
                                |p| &p.amp_release,
                                "RELEASE",
                                false,
                                CORAL_RGB,
                                40.0,
                                78.0,
                            );
                        })
                        .col_between(Pixels(28.0))
                        .child_left(Stretch(1.0))
                        .child_right(Stretch(1.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0))
                        .width(Stretch(1.0))
                        .height(Stretch(1.0));
                    })
                    .class("hardware-module")
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
                }
                BottomTab::ModSeq => {
                    VStack::new(cx, |cx| {
                        build_module_header(
                            cx,
                            "MODSEQ / PERFORMER & LFO MODULATION (MILESTONE 3)",
                            22.0,
                        );

                        HStack::new(cx, move |cx| {
                            build_placeholder_bay(
                                cx,
                                "16-STEP PERFORMER & MSEG CURVES",
                                "BIPOLAR STEP SEQUENCER / GLIDE / CURVE SHAPING (M3)",
                            );
                            build_placeholder_bay(
                                cx,
                                "MODULATION MATRIX & RADIAL RINGS",
                                "ENV 1-4 / LFO 1-2 / MACRO ASSIGNMENTS (M3)",
                            );
                        })
                        .col_between(Pixels(8.0))
                        .width(Stretch(1.0))
                        .height(Stretch(1.0));
                    })
                    .class("hardware-module")
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
                }
                BottomTab::Routing => {
                    VStack::new(cx, |cx| {
                        build_module_header(cx, "ROUTING (M2)", 22.0);

                        build_placeholder_bay(
                            cx,
                            "SERIAL / PARALLEL FILTER & INSERT ROUTING",
                            "OSC 1-4 + NOISE -> F1 / F2 -> FEEDBACK -> MASTER (M2)",
                        );
                    })
                    .class("hardware-module")
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
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
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
                }
            }
        });
    })
    .row_between(Pixels(4.0))
    .width(Stretch(1.0))
    .height(Pixels(146.0));
}

pub(crate) fn create(
    params: Arc<JSynPluginParams>,
    editor_state: Arc<ViziaState>,
    bank: Arc<WavetableBank>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Builtin, move |cx, gui_context| {
        assets::register_noto_sans_bold(cx);
        cx.set_default_font(&[assets::NOTO_SANS]);

        let _ = cx.add_stylesheet(include_style!("src/theme.css"));

        let initial_scale_pct = (cx.user_scale_factor() * 100.0).round() as u32;
        UiData {
            params: params.clone(),
            active_tab: BottomTab::VoiceEnv,
            ui_scale_pct: initial_scale_pct,
        }
        .build(cx);

        let bank_for_ui = bank.clone();
        let params_for_ui = params.clone();
        let gui_ctx_for_ui = gui_context.clone();

        VStack::new(cx, move |cx| {
            build_header_bar(cx, params_for_ui.clone(), gui_ctx_for_ui.clone());

            // Main chassis: 3-Column Layout (Oscillators | Filters | Master FX) + 4-Tab Bottom Bar
            VStack::new(cx, move |cx| {
                VStack::new(cx, move |cx| {
                    HStack::new(cx, move |cx| {
                        build_osc_column(
                            cx,
                            bank_for_ui.clone(),
                            params_for_ui.clone(),
                            gui_ctx_for_ui.clone(),
                        );
                        build_filter_column(cx);
                        build_fx_column(cx);
                    })
                    .col_between(Pixels(8.0))
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));

                    build_bottom_tabs(cx);
                })
                .class("section-panel")
                .row_between(Pixels(6.0))
                .width(Stretch(1.0))
                .height(Stretch(1.0));
            })
            .class("main-chassis")
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        })
        .class("plugin-root")
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })
}
