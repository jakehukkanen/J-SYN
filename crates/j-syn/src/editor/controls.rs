//! Shared typography, dropdowns, parameter readouts, knob/pill cells, and card helpers.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use j_syn_dsp::WavetableBank;
use nih_plug::prelude::{Enum, GuiContext, Param, ParamSetter};
use nih_plug_vizia::assets;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

use super::widgets::{ParamStepPill, RotaryKnob};
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
        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
        .font_weight(FontWeightKeyword::Bold)
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
        .hoverable(false);
    bold_label(cx, "▾", "compact-pill-text")
        .width(Pixels(10.0))
        .height(Stretch(1.0))
        .child_space(Stretch(1.0))
        .hoverable(false);
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

pub(super) fn build_enum_dropdown<'a, E, P, FMap>(
    cx: &'a mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    params_to_param: FMap,
    items: &'static [(&'static str, E)],
) -> Handle<'a, Dropdown>
where
    E: Enum + Copy + PartialEq + Send + Sync + 'static,
    P: Param<Plain = E> + 'static,
    FMap: Fn(&Arc<JSynPluginParams>) -> &P + Copy + Send + Sync + 'static,
{
    Dropdown::new(
        cx,
        move |cx| {
            HStack::new(
                cx,
                ParamWidgetBase::build_view(
                    UiData::params,
                    params_to_param,
                    move |cx, param_data| {
                        let text_lens = param_data.make_lens(|param| {
                            param.normalized_value_to_string(
                                param.modulated_normalized_value(),
                                true,
                            )
                        });
                        build_dropdown_trigger_labels(cx, text_lens);
                    },
                ),
            )
            .height(Stretch(1.0))
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
                    cx.emit(PopupEvent::Close);
                });
            }
        },
    )
}

pub(super) fn build_bool_dropdown<'a, P, FMap>(
    cx: &'a mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    params_to_param: FMap,
    false_label: &'static str,
    true_label: &'static str,
) -> Handle<'a, Dropdown>
where
    P: Param<Plain = bool> + 'static,
    FMap: Fn(&Arc<JSynPluginParams>) -> &P + Copy + Send + Sync + 'static,
{
    Dropdown::new(
        cx,
        move |cx| {
            HStack::new(
                cx,
                ParamWidgetBase::build_view(
                    UiData::params,
                    params_to_param,
                    move |cx, param_data| {
                        let text_lens = param_data.make_lens(move |param| {
                            if param.modulated_normalized_value() >= 0.5 {
                                true_label.to_string()
                            } else {
                                false_label.to_string()
                            }
                        });
                        build_dropdown_trigger_labels(cx, text_lens);
                    },
                ),
            )
            .height(Stretch(1.0))
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
                    cx.emit(PopupEvent::Close);
                });
            }
        },
    )
}

pub(super) fn build_wavetable_dropdown(
    cx: &mut Context,
    osc_idx: usize,
    bank: Arc<WavetableBank>,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
) -> Handle<'_, Dropdown> {
    const WT_ITEMS: &[(&str, WavetableChoice)] = &[
        ("Basic Shapes", WavetableChoice::BasicShapes),
        ("Digital Formant", WavetableChoice::DigitalFormant),
        ("Harmonic Sync", WavetableChoice::HarmonicSync),
        ("Aggressive PWM", WavetableChoice::AggressivePwm),
        ("Custom .WAV", WavetableChoice::Custom),
    ];
    let dialog_open = Arc::new(AtomicBool::new(false));

    Dropdown::new(
        cx,
        move |cx| {
            HStack::new(
                cx,
                ParamWidgetBase::build_view(
                    UiData::params,
                    move |p| &p.oscs[osc_idx].wt_select,
                    move |cx, param_data| {
                        let text_lens = param_data.make_lens(|param| {
                            param.normalized_value_to_string(
                                param.modulated_normalized_value(),
                                true,
                            )
                        });
                        build_dropdown_trigger_labels(cx, text_lens);
                    },
                ),
            )
            .height(Stretch(1.0))
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

                    std::thread::spawn(move || {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("WAV Audio", &["wav", "WAV"])
                            .pick_file()
                        {
                            if let Ok(wt) = j_syn_dsp::Wavetable::load_from_wav(&path) {
                                bank.swap_custom(osc_idx, Arc::new(wt));
                                if let Ok(mut guard) = params.wavetable_paths.lock() {
                                    guard[osc_idx] = path.to_string_lossy().into_owned();
                                }
                                let setter = ParamSetter::new(gui_context.as_ref());
                                setter.begin_set_parameter(&params.oscs[osc_idx].wt_select);
                                setter.set_parameter(
                                    &params.oscs[osc_idx].wt_select,
                                    WavetableChoice::Custom,
                                );
                                setter.end_set_parameter(&params.oscs[osc_idx].wt_select);
                            }
                        }
                        dialog_flag.store(false, Ordering::Release);
                    });
                }
            });
        },
    )
}

pub(super) fn build_ui_scale_dropdown(cx: &mut Context) {
    VStack::new(cx, |cx| {
        bold_label(cx, "UI SCALE", "mini-label")
            .width(Stretch(1.0))
            .height(Pixels(13.0))
            .child_space(Stretch(1.0));

        Dropdown::new(
            cx,
            |cx| {
                HStack::new(cx, move |cx| {
                    build_dropdown_trigger_labels(
                        cx,
                        UiData::ui_scale_pct.map(|pct| format!("{pct}%")),
                    );
                })
                .height(Stretch(1.0))
            },
            move |cx| {
                for &(label, pct, factor) in &[
                    ("85%", 85_u32, 0.85_f64),
                    ("100%", 100_u32, 1.0_f64),
                    ("115%", 115_u32, 1.15_f64),
                    ("125%", 125_u32, 1.25_f64),
                ] {
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
        .width(Pixels(60.0))
        .height(Pixels(19.0));
    })
    .width(Pixels(60.0))
    .height(Auto)
    .top(Stretch(1.0))
    .bottom(Stretch(1.0))
    .row_between(Pixels(2.0));
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
    HStack::new(cx, move |cx| {
        RotaryKnob::new(cx, params.clone(), params_to_param, bipolar, accent_rgb)
            .width(Pixels(30.0))
            .height(Pixels(30.0))
            .top(Stretch(1.0))
            .bottom(Stretch(1.0));

        VStack::new(cx, move |cx| {
            bold_label(cx, title, "knob-title")
                .width(Stretch(1.0))
                .height(Pixels(11.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0));

            build_param_readout_box(cx, params, params_to_param)
                .width(Pixels(52.0))
                .height(Pixels(15.0));
        })
        .width(Pixels(52.0))
        .height(Auto)
        .top(Stretch(1.0))
        .bottom(Stretch(1.0))
        .row_between(Pixels(1.0));
    })
    .width(Auto)
    .height(Pixels(34.0))
    .col_between(Pixels(5.0));
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
            .height(Pixels(13.0))
            .child_space(Stretch(1.0));

        RotaryKnob::new(cx, params.clone(), params_to_param, bipolar, accent_rgb)
            .width(Pixels(knob_sz))
            .height(Pixels(knob_sz))
            .left(Stretch(1.0))
            .right(Stretch(1.0));

        build_param_readout_box(cx, params, params_to_param)
            .width(Pixels(cell_w - 4.0))
            .height(Pixels(17.0))
            .left(Stretch(1.0))
            .right(Stretch(1.0));
    })
    .class("knob-cell")
    .width(Pixels(cell_w))
    .height(Auto)
    .row_between(Pixels(2.0));
}

pub(super) fn build_flex_knob_cell<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
    accent_rgb: (u8, u8, u8),
    knob_sz: f32,
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

        RotaryKnob::new(cx, params.clone(), params_to_param, bipolar, accent_rgb)
            .width(Pixels(knob_sz))
            .height(Pixels(knob_sz))
            .left(Stretch(1.0))
            .right(Stretch(1.0));

        build_param_readout_box(cx, params, params_to_param)
            .width(Stretch(1.0))
            .height(Pixels(15.0));
    })
    .class("knob-cell")
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(1.0));
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
            .height(Pixels(10.0))
            .child_space(Stretch(1.0));

        ParamStepPill::new(cx, params, params_to_param, num_steps)
            .width(Stretch(1.0))
            .height(Pixels(17.0));
    })
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(1.0));
}

pub(super) fn build_module_header(cx: &mut Context, title: &'static str, height_px: f32) {
    HStack::new(cx, move |cx| {
        bold_label(cx, title, "module-title");
    })
    .class("module-header")
    .height(Pixels(height_px))
    .width(Stretch(1.0));
}

pub(super) fn build_tab_button(cx: &mut Context, tab: BottomTab, title: &'static str) {
    Button::new(
        cx,
        move |cx| cx.emit(UiEvent::SelectTab(tab)),
        move |cx| bold_label(cx, title, "tab-btn-label").hoverable(false),
    )
    .class("tab-btn")
    .checked(UiData::active_tab.map(move |t| *t == tab))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}

pub(super) fn build_placeholder_bay(cx: &mut Context, title: &'static str, subtitle: &'static str) {
    VStack::new(cx, move |cx| {
        bold_label(cx, title, "placeholder-title")
            .width(Stretch(1.0))
            .child_left(Stretch(1.0))
            .child_right(Stretch(1.0));
        bold_label(cx, subtitle, "placeholder-sub")
            .width(Stretch(1.0))
            .child_left(Stretch(1.0))
            .child_right(Stretch(1.0));
    })
    .class("placeholder-bay")
    .width(Stretch(1.0))
    .height(Stretch(1.0))
    .child_top(Stretch(1.0))
    .child_bottom(Stretch(1.0));
}
