//! Dedicated full-panel Settings view (`Gear` tab) for Engine Quality (`Dirty` / `Clean`),
//! Output & Safety DSP toggles (`DC Blocker`, `Safety Limiter`), and File Directories (`Presets`, `Wavetables`).

use std::sync::Arc;

use nih_plug::prelude::GuiContext;
use nih_plug_vizia::vizia::prelude::*;

use crate::JSynPluginParams;

use super::controls::{
    bold_label, build_bool_dropdown, build_dropdown_trigger_labels, build_module_header,
};
use super::widgets::LedToggleSwitch;
use super::{UiData, UiEvent, AMBER_RGB, CORAL_RGB};

pub(super) fn build_settings_view(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
) {
    VStack::new(cx, move |cx| {
        // Card 1: Engine & Safety DSP
        let p_eng = params.clone();
        let g_eng = gui_context.clone();
        VStack::new(cx, move |cx| {
            build_module_header(cx, "ENGINE & SAFETY DSP", 22.0);

            VStack::new(cx, move |cx| {
                HStack::new(cx, |cx| {
                    Dropdown::new(
                        cx,
                        |cx| {
                            HStack::new(cx, |cx| {
                                build_dropdown_trigger_labels(cx, UiData::theme_name);
                            })
                            .width(Stretch(1.0))
                            .height(Stretch(1.0))
                            .child_top(Pixels(0.0))
                            .child_bottom(Pixels(0.0))
                            .overflow(Overflow::Hidden)
                        },
                        |cx| {
                            Binding::new(cx, UiData::themes_rev, |cx, _| {
                                for theme in super::theme::load_all_themes() {
                                    let name = theme.name.clone();
                                    bold_label(cx, &theme.name, "dropdown-item").on_press(
                                        move |cx| {
                                            cx.emit(UiEvent::SelectTheme(name.clone()));
                                            cx.emit(PopupEvent::Close);
                                        },
                                    );
                                }
                            });
                        },
                    )
                    .width(Pixels(142.0))
                    .height(Pixels(26.0))
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));

                    bold_label(
                        cx,
                        "Active UI color theme (built-in presets + custom .json files in config themes folder).",
                        "setting-desc",
                    )
                    .width(Stretch(1.0))
                    .height(Auto)
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));
                })
                .col_between(Pixels(14.0))
                .width(Stretch(1.0))
                .height(Pixels(34.0));

                HStack::new(cx, move |cx| {
                    build_bool_dropdown(
                        cx,
                        p_eng.clone(),
                        g_eng.clone(),
                        |p| &p.hq_mode,
                        "Dirty",
                        "Clean",
                    )
                    .width(Pixels(142.0))
                    .height(Pixels(26.0))
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));

                    bold_label(
                        cx,
                        "Engine interpolation quality: Dirty (classic 2007-style table lookup) vs. Clean (band-limited HQ mip-map interpolation).",
                        "setting-desc",
                    )
                    .width(Stretch(1.0))
                    .height(Auto)
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));
                })
                .col_between(Pixels(14.0))
                .width(Stretch(1.0))
                .height(Pixels(34.0));

                HStack::new(cx, |cx| {
                    LedToggleSwitch::new(
                        cx,
                        UiData::params,
                        |p| &p.dc_blocker,
                        "DC BLOCKER",
                        "DC BLOCKER",
                        (82, 200, 128),
                    )
                    .width(Pixels(142.0))
                    .height(Pixels(26.0))
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));

                    bold_label(
                        cx,
                        "Transparent ~2.5 Hz 1st-order high-pass filter (f64 precision) preventing asymmetric DC offset at the master output.",
                        "setting-desc",
                    )
                    .width(Stretch(1.0))
                    .height(Auto)
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));
                })
                .col_between(Pixels(14.0))
                .width(Stretch(1.0))
                .height(Pixels(34.0));

                HStack::new(cx, |cx| {
                    LedToggleSwitch::new(
                        cx,
                        UiData::params,
                        |p| &p.master_limiter,
                        "SAFETY LIMITER",
                        "SAFETY LIMITER",
                        CORAL_RGB,
                    )
                    .width(Pixels(142.0))
                    .height(Pixels(26.0))
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));

                    bold_label(
                        cx,
                        "Zero-latency brickwall safety limiter catching output peaks above -0.1 dBFS to protect ears and DAW buses.",
                        "setting-desc",
                    )
                    .width(Stretch(1.0))
                    .height(Auto)
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));
                })
                .col_between(Pixels(14.0))
                .width(Stretch(1.0))
                .height(Pixels(34.0));
            })
            .row_between(Pixels(10.0))
            .child_left(Pixels(14.0))
            .child_right(Pixels(14.0))
            .child_top(Pixels(12.0))
            .child_bottom(Pixels(12.0))
            .width(Stretch(1.0))
            .height(Auto);
        })
        .class("hardware-module")
        .width(Stretch(1.0))
        .height(Auto);

        // Card 2: File Directories
        VStack::new(cx, |cx| {
            build_module_header(cx, "FILE DIRECTORIES", 22.0);

            VStack::new(cx, |cx| {
                // Presets Directory
                HStack::new(cx, |cx| {
                    bold_label(cx, "PRESETS DIRECTORY", "setting-label")
                        .width(Pixels(142.0))
                        .height(Pixels(26.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0));

                    bold_label(cx, UiData::presets_dir_display, "param-readout")
                        .width(Stretch(1.0))
                        .height(Pixels(26.0))
                        .child_left(Pixels(8.0))
                        .child_right(Pixels(8.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0));

                    Button::new(
                        cx,
                        |cx| cx.emit(UiEvent::PickPresetsDir),
                        |cx| {
                            bold_label(cx, "Browse...", "preset-btn-label")
                                .child_space(Stretch(1.0))
                                .hoverable(false)
                        },
                    )
                    .class("preset-btn")
                    .width(Pixels(78.0))
                    .height(Pixels(26.0));

                    Button::new(
                        cx,
                        |cx| cx.emit(UiEvent::ClearPresetsDir),
                        |cx| {
                            bold_label(cx, "Clear", "preset-btn-label")
                                .child_space(Stretch(1.0))
                                .hoverable(false)
                        },
                    )
                    .class("preset-btn")
                    .width(Pixels(60.0))
                    .height(Pixels(26.0));
                })
                .col_between(Pixels(10.0))
                .width(Stretch(1.0))
                .height(Pixels(28.0));

                // Wavetables Directory
                HStack::new(cx, |cx| {
                    bold_label(cx, "WAVETABLES DIRECTORY", "setting-label")
                        .width(Pixels(142.0))
                        .height(Pixels(26.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0));

                    bold_label(cx, UiData::wavetables_dir_display, "param-readout")
                        .width(Stretch(1.0))
                        .height(Pixels(26.0))
                        .child_left(Pixels(8.0))
                        .child_right(Pixels(8.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0));

                    Button::new(
                        cx,
                        |cx| cx.emit(UiEvent::PickWavetablesDir),
                        |cx| {
                            bold_label(cx, "Browse...", "preset-btn-label")
                                .child_space(Stretch(1.0))
                                .hoverable(false)
                        },
                    )
                    .class("preset-btn")
                    .width(Pixels(78.0))
                    .height(Pixels(26.0));

                    Button::new(
                        cx,
                        |cx| cx.emit(UiEvent::ClearWavetablesDir),
                        |cx| {
                            bold_label(cx, "Clear", "preset-btn-label")
                                .child_space(Stretch(1.0))
                                .hoverable(false)
                        },
                    )
                    .class("preset-btn")
                    .width(Pixels(60.0))
                    .height(Pixels(26.0));
                })
                .col_between(Pixels(10.0))
                .width(Stretch(1.0))
                .height(Pixels(28.0));

                HStack::new(cx, |cx| {
                    Element::new(cx).width(Stretch(1.0));

                    Button::new(
                        cx,
                        |cx| cx.emit(UiEvent::RescanDirectories),
                        |cx| {
                            bold_label(cx, "Rescan Directories", "preset-btn-label")
                                .child_space(Stretch(1.0))
                                .hoverable(false)
                        },
                    )
                    .class("preset-btn")
                    .width(Pixels(148.0))
                    .height(Pixels(26.0));
                })
                .width(Stretch(1.0))
                .height(Pixels(28.0));
            })
            .row_between(Pixels(12.0))
            .child_left(Pixels(14.0))
            .child_right(Pixels(14.0))
            .child_top(Pixels(12.0))
            .child_bottom(Pixels(12.0))
            .width(Stretch(1.0))
            .height(Auto);
        })
        .class("hardware-module")
        .width(Stretch(1.0))
        .height(Stretch(1.0));

        let _ = AMBER_RGB;
    })
    .row_between(Pixels(10.0))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}
