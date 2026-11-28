//! Shared typography, vector icon buttons, dropdowns, parameter readouts, knob/pill cells, and card chrome.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

use crate::shell::ModUiHost;
use crate::tokens;
use crate::widgets::{
    KnobDimExt, ParamStepPill, PitchDragBox, PitchGroupModOverlay, PitchPart, RatioDragBox,
    RotaryKnob, ValueFormatter,
};

mod dropdowns;
mod icon_buttons;

pub use dropdowns::*;
pub use icon_buttons::*;

pub fn bold_label<'a, T>(
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

pub fn build_rotary_knob_with_opt_mod<'a, L, Params, P, FMap>(
    cx: &'a mut Context,
    params: L,
    params_to_param: FMap,
    bipolar: bool,
    mod_target: Option<(ModDestination, Arc<dyn ModUiHost>, u8)>,
) -> Handle<'a, RotaryKnob>
where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    match mod_target {
        Some((dest, mod_ui, tab_tag)) => {
            RotaryKnob::new_with_mod(cx, params, params_to_param, bipolar, dest, mod_ui, tab_tag)
        }
        None => RotaryKnob::new(cx, params, params_to_param, bipolar),
    }
}

pub fn build_header_knob_cell<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    HStack::new(cx, move |cx| {
        RotaryKnob::new(cx, params, params_to_param, bipolar)
            .width(Pixels(tokens::KNOB_HEADER))
            .height(Pixels(tokens::KNOB_HEADER))
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

pub fn build_param_readout_box<'a, L, Params, P, FMap>(
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

pub fn build_knob_cell_with_readout<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
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
            .height(Pixels(tokens::KNOB_TITLE_COMPACT_H))
            .child_space(Stretch(1.0));

        build_rotary_knob_with_opt_mod(
            cx,
            params.clone(),
            params_to_param,
            bipolar,
            None,
        )
        .width(Pixels(knob_sz))
        .height(Pixels(knob_sz))
        .left(Stretch(1.0))
        .right(Stretch(1.0));

        build_param_readout_box(cx, params, params_to_param)
            .width(Pixels(cell_w - tokens::CARD_PAD))
            .height(Pixels(tokens::KNOB_READOUT_H))
            .left(Stretch(1.0))
            .right(Stretch(1.0));
    })
    .class("knob-cell")
    .width(Pixels(cell_w))
    .height(Auto)
    .row_between(Pixels(1.0));
}

fn knob_cell_frame(
    cx: &mut Context,
    title: &'static str,
    width: Units,
    knob_sz: f32,
    build_knob: impl FnOnce(&mut Context) -> Handle<'_, RotaryKnob>,
) {
    VStack::new(cx, move |cx| {
        bold_label(cx, title, "knob-title")
            .width(Stretch(1.0))
            .height(Pixels(tokens::KNOB_TITLE_H))
            .child_space(Stretch(1.0));

        build_knob(cx)
            .width(Pixels(knob_sz))
            .height(Pixels(knob_sz))
            .left(Stretch(1.0))
            .right(Stretch(1.0));
    })
    .class("knob-cell")
    .width(width)
    .height(Auto)
    .row_between(Pixels(tokens::SPACE_XS));
}

pub fn build_knob_cell<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
    knob_sz: f32,
    cell_w: f32,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    knob_cell_frame(cx, title, Pixels(cell_w), knob_sz, move |cx| {
        build_rotary_knob_with_opt_mod(cx, params, params_to_param, bipolar, None)
    });
}

#[allow(clippy::too_many_arguments)]
pub fn build_knob_cell_mod_fmt<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
    knob_sz: f32,
    cell_w: f32,
    dest: ModDestination,
    mod_ui: Arc<dyn ModUiHost>,
    tab_tag: u8,
    value_formatter: ValueFormatter,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    knob_cell_frame(cx, title, Pixels(cell_w), knob_sz, move |cx| {
        RotaryKnob::new_with_mod_formatter(
            cx,
            params,
            params_to_param,
            bipolar,
            dest,
            mod_ui,
            tab_tag,
            value_formatter,
        )
    });
}

pub fn build_flex_knob_cell<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
    knob_sz: f32,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    knob_cell_frame(cx, title, Stretch(1.0), knob_sz, move |cx| {
        build_rotary_knob_with_opt_mod(cx, params, params_to_param, bipolar, None)
    });
}

#[allow(clippy::too_many_arguments)]
pub fn build_flex_knob_cell_mod_fmt<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
    knob_sz: f32,
    dest: ModDestination,
    mod_ui: Arc<dyn ModUiHost>,
    tab_tag: u8,
    value_formatter: ValueFormatter,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    knob_cell_frame(cx, title, Stretch(1.0), knob_sz, move |cx| {
        RotaryKnob::new_with_mod_formatter(
            cx,
            params,
            params_to_param,
            bipolar,
            dest,
            mod_ui,
            tab_tag,
            value_formatter,
        )
    });
}

#[allow(clippy::too_many_arguments)]
pub fn build_flex_knob_cell_mod<L, Params, P, FMap>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
    knob_sz: f32,
    dest: ModDestination,
    mod_ui: Arc<dyn ModUiHost>,
    tab_tag: u8,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    knob_cell_frame(cx, title, Stretch(1.0), knob_sz, move |cx| {
        RotaryKnob::new_with_mod(
            cx,
            params,
            params_to_param,
            bipolar,
            dest,
            mod_ui,
            tab_tag,
        )
    });
}

#[allow(clippy::too_many_arguments)]
pub fn build_flex_knob_cell_mod_dim<L, Params, DimParams, P, FMap, FDim>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
    knob_sz: f32,
    dest: ModDestination,
    mod_ui: Arc<dyn ModUiHost>,
    tab_tag: u8,
    dim_params: Arc<DimParams>,
    dim_check: FDim,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    DimParams: Send + Sync + 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
    FDim: Fn(&DimParams) -> bool + Send + Sync + 'static,
{
    knob_cell_frame(cx, title, Stretch(1.0), knob_sz, move |cx| {
        RotaryKnob::new_with_mod(
            cx,
            params,
            params_to_param,
            bipolar,
            dest,
            mod_ui,
            tab_tag,
        )
        .enabled_by(dim_params, dim_check)
    });
}

#[allow(clippy::too_many_arguments)]
pub fn build_flex_knob_cell_mod_dim_tooltip<L, Params, DimParams, P, FMap, FDim, FTt>(
    cx: &mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    bipolar: bool,
    knob_sz: f32,
    dest: ModDestination,
    mod_ui: Arc<dyn ModUiHost>,
    tab_tag: u8,
    dim_params: Arc<DimParams>,
    dim_check: FDim,
    tooltip_fmt: FTt,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    DimParams: Send + Sync + 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
    FDim: Fn(&DimParams) -> bool + Send + Sync + 'static,
    FTt: Fn(&DimParams) -> String + Send + Sync + 'static,
{
    knob_cell_frame(cx, title, Stretch(1.0), knob_sz, move |cx| {
        RotaryKnob::new_with_mod(
            cx,
            params,
            params_to_param,
            bipolar,
            dest,
            mod_ui,
            tab_tag,
        )
        .enabled_by(dim_params.clone(), dim_check)
        .disabled_tooltip(dim_params, tooltip_fmt)
    });
}

pub fn build_inline_pill_cell<'a, L, Params, P, FMap>(
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
        .height(Pixels(tokens::CONTROL_H))
}

#[allow(clippy::too_many_arguments)]
pub fn build_inline_pill_cell_mod<'a, L, Params, P, FMap>(
    cx: &'a mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    num_steps: usize,
    weight: f32,
    dest: ModDestination,
    mod_ui: Arc<dyn ModUiHost>,
    tab_tag: u8,
) -> Handle<'a, ParamStepPill>
where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    ParamStepPill::new_inline_with_mod(
        cx,
        params,
        params_to_param,
        num_steps,
        title,
        dest,
        mod_ui,
        tab_tag,
    )
    .width(Stretch(weight))
    .height(Pixels(tokens::CONTROL_H))
}

pub fn build_mini_pill_cell<L, Params, P, FMap>(
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
    .row_between(Pixels(tokens::SPACE_XS));
}

pub fn build_flex_pill_cell<L, Params, P, FMap>(
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
            .height(Pixels(tokens::KNOB_TITLE_COMPACT_H))
            .child_space(Stretch(1.0));

        ParamStepPill::new(cx, params, params_to_param, num_steps)
            .width(Stretch(1.0))
            .height(Pixels(tokens::READOUT_H));
    })
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(1.0));
}

#[allow(clippy::too_many_arguments)]
pub fn build_pitch_tuning_group<'a, L, Params, P, FMap>(
    cx: &'a mut Context,
    osc_idx: usize,
    params: L,
    params_to_param: FMap,
    dest: ModDestination,
    mod_ui: Arc<dyn ModUiHost>,
    tab_tag: u8,
) -> Handle<'a, ZStack>
where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    ZStack::new(cx, move |cx| {
        let p_row = params.clone();
        let m_row = mod_ui.clone();
        HStack::new(cx, move |cx| {
            for part in [PitchPart::Octave, PitchPart::Semitone, PitchPart::Fine] {
                PitchDragBox::new_osc_pitch_part(
                    cx,
                    osc_idx,
                    part,
                    p_row.clone(),
                    params_to_param,
                    dest,
                    m_row.clone(),
                    tab_tag,
                )
                .width(Stretch(1.0))
                .height(Stretch(1.0));
            }
        })
        .col_between(Pixels(tokens::SPACE_XS))
        .width(Stretch(1.0))
        .height(Stretch(1.0));

        PitchGroupModOverlay::new(cx, params, params_to_param, dest, mod_ui, tab_tag)
            .position_type(PositionType::SelfDirected)
            .left(Pixels(0.0))
            .top(Pixels(0.0))
            .width(Stretch(1.0))
            .height(Stretch(1.0));
    })
    .width(Stretch(1.0))
    .height(Stretch(1.0))
}

#[allow(clippy::too_many_arguments)]
pub fn build_ratio_tuning_row<'a, L, Params, PR, PH, FMapR, FMapH>(
    cx: &'a mut Context,
    osc_idx: usize,
    params: L,
    ratio_map: FMapR,
    hz_map: FMapH,
    ratio_dest: ModDestination,
    hz_dest: ModDestination,
    mod_ui: Arc<dyn ModUiHost>,
    tab_tag: u8,
) -> Handle<'a, HStack>
where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    PR: Param + 'static,
    PH: Param + 'static,
    FMapR: Fn(&Params) -> &PR + Copy + 'static,
    FMapH: Fn(&Params) -> &PH + Copy + 'static,
{
    HStack::new(cx, move |cx| {
        RatioDragBox::new_osc_ratio(
            cx,
            osc_idx,
            params.clone(),
            ratio_map,
            ratio_dest,
            mod_ui.clone(),
            tab_tag,
        )
        .width(Stretch(2.008))
        .height(Stretch(1.0));
        ParamStepPill::new_inline_with_mod_menu(
            cx,
            params,
            hz_map,
            200,
            "HZ",
            hz_dest,
            mod_ui,
            tab_tag,
        )
        .class("stk-pill")
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })
    .col_between(Pixels(tokens::SPACE_XS))
    .width(Stretch(1.0))
    .height(Stretch(1.0))
}

pub fn build_module_header(cx: &mut Context, title: &'static str, height_px: f32) {
    HStack::new(cx, move |cx| {
        bold_label(cx, title, "module-title");
    })
    .class("module-header")
    .height(Pixels(height_px))
    .width(Stretch(1.0));
}
