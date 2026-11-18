//! Shared typography, vector icon buttons, dropdowns, parameter readouts, knob/pill cells, and card chrome.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

use super::mod_bar::ModUiState;
use super::widgets::{
    KnobDimExt, ParamStepPill, PitchDragBox, RatioDragBox, RotaryKnob, ValueFormatter,
};
use super::{BottomTab, CenterStageTab, UiData, UiEvent};
use crate::JSynPluginParams;

mod dropdowns;
mod icon_buttons;

pub(crate) use dropdowns::*;
pub(crate) use icon_buttons::*;

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
    disabled_tooltip: Option<Arc<dyn Fn(&JSynPluginParams) -> String + Send + Sync>>,
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
        if let Some(fmt) = disabled_tooltip {
            knob = knob.disabled_tooltip(move |p| fmt(p));
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
        None,
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_flex_knob_cell_mod_dim_tooltip<L, Params, P, FMap, FDim, FTt>(
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
    dim_check: FDim,
    tooltip_fmt: FTt,
) where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
    FDim: Fn(&JSynPluginParams) -> bool + Send + Sync + 'static,
    FTt: Fn(&JSynPluginParams) -> String + Send + Sync + 'static,
{
    VStack::new(cx, move |cx| {
        bold_label(cx, title, "knob-title")
            .width(Stretch(1.0))
            .height(Pixels(14.0))
            .child_space(Stretch(1.0));

        build_rotary_knob_with_opt_mod(
            cx,
            params,
            params_to_param,
            bipolar,
            accent_rgb,
            Some((dest, mod_ui, tab_tag)),
        )
        .enabled_by(dim_params, dim_check)
        .disabled_tooltip(tooltip_fmt)
        .width(Pixels(knob_sz))
        .height(Pixels(knob_sz))
        .left(Stretch(1.0))
        .right(Stretch(1.0));
    })
    .class("knob-cell")
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(2.0));
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

#[allow(clippy::too_many_arguments)]
pub(super) fn build_inline_pill_cell_mod<'a, L, Params, P, FMap>(
    cx: &'a mut Context,
    params: L,
    params_to_param: FMap,
    title: &'static str,
    num_steps: usize,
    weight: f32,
    dest: ModDestination,
    mod_ui: Arc<ModUiState>,
    tab_tag: u8,
) -> Handle<'a, ParamStepPill>
where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    P: Param + 'static,
    FMap: Fn(&Params) -> &P + Copy + 'static,
{
    ParamStepPill::new_inline_with_mod(cx, params, params_to_param, num_steps, title, dest, mod_ui, tab_tag)
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

#[allow(clippy::too_many_arguments)]
pub(super) fn build_pitch_pill_cell<'a, L, Params, P, FMap>(
    cx: &'a mut Context,
    osc_idx: usize,
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
        PitchDragBox::new_osc_pitch(cx, osc_idx, params, params_to_param, dest, mod_ui, tab_tag)
            .width(Stretch(1.0))
            .height(Pixels(19.0));
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_ratio_pill_cell<'a, L, Params, PR, PH, FMapR, FMapH>(
    cx: &'a mut Context,
    osc_idx: usize,
    params: L,
    ratio_map: FMapR,
    hz_map: FMapH,
    ratio_dest: ModDestination,
    hz_dest: ModDestination,
    mod_ui: Arc<ModUiState>,
    tab_tag: u8,
) -> Handle<'a, VStack>
where
    L: Lens<Target = Params> + Clone,
    Params: 'static,
    PR: Param + 'static,
    PH: Param + 'static,
    FMapR: Fn(&Params) -> &PR + Copy + 'static,
    FMapH: Fn(&Params) -> &PH + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        bold_label(cx, "RATIO", "mini-label")
            .class("pitch-mode-ratio-lbl")
            .width(Stretch(1.0))
            .height(Pixels(12.0))
            .child_space(Stretch(1.0));
        RatioDragBox::new_osc_ratio(
            cx,
            osc_idx,
            params.clone(),
            ratio_map,
            ratio_dest,
            mod_ui.clone(),
            tab_tag,
        )
        .width(Stretch(1.0))
        .height(Pixels(19.0));
        ParamStepPill::new_with_mod_menu(cx, params, hz_map, 200, hz_dest, mod_ui, tab_tag)
            .class("hz-offset-pill")
            .width(Stretch(1.0))
            .height(Pixels(17.0));
    })
    .class("stacked-pill-cell")
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(2.0))
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

pub(super) fn build_center_tab_button(
    cx: &mut Context,
    tab: CenterStageTab,
    title: &'static str,
    width_weight: f32,
) {
    Button::new(
        cx,
        move |cx| match tab {
            CenterStageTab::Mod(sub) => {
                cx.emit(UiEvent::SelectTab(BottomTab::ModSeq));
                cx.emit(UiEvent::SelectModSeqSubTab(sub));
            }
            CenterStageTab::Voice => cx.emit(UiEvent::SelectTab(BottomTab::VoiceEnv)),
            CenterStageTab::Routing => cx.emit(UiEvent::SelectTab(BottomTab::Routing)),
        },
        move |cx| bold_label(cx, title, "tab-btn-label").hoverable(false),
    )
    .class("tab-btn")
    .checked(UiData::center_tab.map(move |t| *t == tab))
    .width(Stretch(width_weight))
    .height(Stretch(1.0));
}
