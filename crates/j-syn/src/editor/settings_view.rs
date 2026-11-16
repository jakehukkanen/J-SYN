//! Dedicated full-panel Settings view (`Gear` tab) for Engine Quality (`Dirty` / `Clean`),
//! Output & Safety DSP toggles (`DC Blocker`, `Safety Limiter`), and File Directories (`Presets`, `Wavetables`).

use std::sync::Arc;

use nih_plug::prelude::GuiContext;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::util::ModifiersExt;

use crate::JSynPluginParams;

use super::controls::{
    bold_label, build_bool_dropdown, build_dropdown_trigger_labels, build_module_header,
};
use super::widgets::LedToggleSwitch;
use super::{UiData, UiEvent, AMBER_RGB, CORAL_RGB, GOLD_RGB};

const GEAR_MIN: f32 = 0.10;
const GEAR_MAX: f32 = 0.60;
const GEAR_DEFAULT: f32 = 0.32;

struct GearSizeKnob {
    dragging: bool,
    last_y: f32,
}

impl GearSizeKnob {
    fn new<'a>(cx: &'a mut Context) -> Handle<'a, Self> {
        Self {
            dragging: false,
            last_y: 0.0,
        }
        .build(cx, |_| {})
        .cursor(CursorIcon::NsResize)
        .bind(UiData::gear_radius, |mut handle, _| handle.needs_redraw())
    }
}

impl View for GearSizeKnob {
    fn element(&self) -> Option<&'static str> {
        Some("gear-size-knob")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                if cx.modifiers().command() || cx.modifiers().contains(Modifiers::CTRL) {
                    cx.emit(UiEvent::SetGearRadius(GEAR_DEFAULT));
                    cx.emit(UiEvent::CommitGearRadius);
                } else {
                    self.dragging = true;
                    self.last_y = cx.mouse().cursory;
                    cx.capture();
                }
                meta.consume();
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                cx.emit(UiEvent::SetGearRadius(GEAR_DEFAULT));
                cx.emit(UiEvent::CommitGearRadius);
                meta.consume();
            }
            WindowEvent::MouseMove(_x, y) if self.dragging => {
                let dy = self.last_y - *y;
                self.last_y = *y;
                let sens = if cx.modifiers().shift() {
                    0.0007
                } else {
                    0.0030
                };
                let cur = UiData::gear_radius.get(cx);
                let next = (cur + dy * sens).clamp(GEAR_MIN, GEAR_MAX);
                cx.emit(UiEvent::SetGearRadius(next));
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) if self.dragging => {
                self.dragging = false;
                cx.release();
                cx.emit(UiEvent::CommitGearRadius);
                meta.consume();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 2.0 || bounds.h <= 2.0 {
            return;
        }
        let dpx = cx.scale_factor();
        let pal = super::theme::current_palette(cx);
        let (ar, ag, ab) = AMBER_RGB;
        let cx_p = bounds.x + bounds.w * 0.5;
        let cy_p = bounds.y + bounds.h * 0.5;
        let radius = bounds.w.min(bounds.h) * 0.44;
        let norm = ((UiData::gear_radius.get(cx) - GEAR_MIN) / (GEAR_MAX - GEAR_MIN)).clamp(0.0, 1.0);

        let start = std::f32::consts::PI * 0.75;
        let span = std::f32::consts::PI * 1.50;
        let end = start + span;
        let val_angle = start + norm * span;

        let mut track = vg::Path::new();
        track.arc(cx_p, cy_p, radius, start, end, vg::Solidity::Solid);
        let mut track_paint = vg::Paint::color(super::theme::vg_color(&pal.border_mid, 220));
        track_paint.set_line_width(2.2 * dpx);
        track_paint.set_line_cap(vg::LineCap::Round);
        canvas.stroke_path(&track, &track_paint);

        if norm > 0.005 {
            let mut active = vg::Path::new();
            active.arc(cx_p, cy_p, radius, start, val_angle, vg::Solidity::Solid);
            let mut act_paint = vg::Paint::color(vg::Color::rgb(ar, ag, ab));
            act_paint.set_line_width(2.4 * dpx);
            act_paint.set_line_cap(vg::LineCap::Round);
            canvas.stroke_path(&active, &act_paint);
        }

        let mut body = vg::Path::new();
        body.circle(cx_p, cy_p, radius * 0.72);
        let body_paint = vg::Paint::linear_gradient(
            cx_p,
            cy_p - radius * 0.72,
            cx_p,
            cy_p + radius * 0.72,
            super::theme::vg_color(&pal.surface_hover, 255),
            super::theme::vg_color(&pal.surface_dark, 255),
        );
        canvas.fill_path(&body, &body_paint);

        let mut ptr = vg::Path::new();
        ptr.move_to(
            cx_p + val_angle.cos() * (radius * 0.22),
            cy_p + val_angle.sin() * (radius * 0.22),
        );
        ptr.line_to(
            cx_p + val_angle.cos() * (radius * 0.64),
            cy_p + val_angle.sin() * (radius * 0.64),
        );
        let mut ptr_paint = vg::Paint::color(super::theme::vg_color(&pal.text_primary, 255));
        ptr_paint.set_line_width(1.6 * dpx);
        ptr_paint.set_line_cap(vg::LineCap::Round);
        canvas.stroke_path(&ptr, &ptr_paint);
    }
}

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

                HStack::new(cx, |cx| {
                    LedToggleSwitch::new(
                        cx,
                        UiData::params,
                        |p| &p.modseq_swing,
                        "MODSEQ SWING",
                        "MODSEQ SWING",
                        GOLD_RGB,
                    )
                    .width(Pixels(142.0))
                    .height(Pixels(26.0))
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));

                    bold_label(
                        cx,
                        "Experimental: Applies alternating step-phase tension warping (groove swing) to MODSEQ sequences and keeps step tension on stamp. Off = straight step modulation.",
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
                    HStack::new(cx, |cx| {
                        GearSizeKnob::new(cx)
                            .width(Pixels(28.0))
                            .height(Pixels(28.0))
                            .top(Stretch(1.0))
                            .bottom(Stretch(1.0));
                        VStack::new(cx, |cx| {
                            bold_label(cx, "GEAR SIZE", "mini-label")
                                .width(Stretch(1.0))
                                .height(Pixels(12.0));
                            bold_label(
                                cx,
                                UiData::gear_radius.map(|r| format!("{r:.2}")),
                                "param-readout",
                            )
                            .width(Pixels(44.0))
                            .height(Pixels(17.0))
                            .child_space(Stretch(1.0));
                        })
                        .row_between(Pixels(2.0))
                        .width(Auto)
                        .height(Auto)
                        .top(Stretch(1.0))
                        .bottom(Stretch(1.0));
                    })
                    .col_between(Pixels(8.0))
                    .width(Pixels(142.0))
                    .height(Auto)
                    .child_left(Stretch(1.0))
                    .child_right(Stretch(1.0))
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));

                    bold_label(
                        cx,
                        "Adjusts the header settings cogwheel outer radius ratio (0.10 to 0.60, saved to config.json; default 0.32).",
                        "setting-desc",
                    )
                    .width(Stretch(1.0))
                    .height(Auto)
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));
                })
                .col_between(Pixels(14.0))
                .width(Stretch(1.0))
                .height(Pixels(38.0));
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
