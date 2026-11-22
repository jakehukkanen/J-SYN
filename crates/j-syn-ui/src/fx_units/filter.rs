//! Shared `200 px` variable-height FX Unit view for `Filter` (`80 px` frequency response
//! scope + `20 px` `[Mode ▼] [Slope ▼]` context strip + equidistant `2×2` knob grid with
//! dynamic `DRIVE` / `SCREAM` label).

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::{FilterMode, FxType};
use nih_plug::prelude::{BoolParam, EnumParam, FloatParam, GuiContext};
use nih_plug_vizia::vizia::prelude::*;

use crate::controls::{
    bold_label, build_enum_dropdown, build_enum_dropdown_with_override,
};
use crate::scopes::{fx_accent_rgb, FxSlotScopeSnapshot, FxSlotVisualizer};
use crate::shell::ModUiHost;
use crate::tokens;
use crate::widgets::{KnobDimExt, RotaryKnob, ValueFormatter};

use super::params::{
    FilterModeParam, FxFilterSlopeParam, FX_FILTER_MODE_ITEMS, FX_FILTER_SLOPE_ITEMS,
};
use super::{format_fx_slot_param_value, fx_slot_param_label};

/// Parameter projection trait for the shared `Filter` FX Unit view.
pub trait FilterUnitParams<Params: 'static>: Copy + Send + Sync + 'static {
    fn enabled<'a>(&self, p: &'a Params) -> &'a BoolParam;
    fn mode<'a>(&self, p: &'a Params) -> &'a EnumParam<FilterModeParam>;
    fn slope<'a>(&self, p: &'a Params) -> &'a EnumParam<FxFilterSlopeParam>;
    /// `0` = `CUTOFF`, `1` = `RES`, `2` = `DRIVE`/`SCREAM`, `3` = `MIX`.
    fn param<'a>(&self, p: &'a Params, idx: usize) -> &'a FloatParam;
}

/// Shared `Filter` FX Unit body (`80 px` scope + `20 px` `[Mode ▼] [Slope ▼]` context strip
/// + equidistant `2×2` knob grid).
#[allow(clippy::too_many_arguments)]
pub fn build_filter_fx_unit_body<L, Params, U, FScope>(
    cx: &mut Context,
    params_lens: L,
    params: Arc<Params>,
    gui_context: Arc<dyn GuiContext>,
    unit_params: U,
    mod_targets: Option<([ModDestination; 4], Arc<dyn ModUiHost>, u8)>,
    scope_provider: FScope,
    knob_sz: f32,
) where
    L: Lens<Target = Arc<Params>> + Clone,
    Params: Send + Sync + 'static,
    U: FilterUnitParams<Params>,
    FScope: Fn() -> FxSlotScopeSnapshot + Send + Sync + 'static,
{
    let scope_h = tokens::fx_unit_scope_height(FxType::Filter);
    let defs = j_syn_dsp::FxSlotParams::default_params_for_type(FxType::Filter);
    let accent = fx_accent_rgb(FxType::Filter);

    let lens_mode = params_lens.clone();
    let lens_slope = params_lens.clone();
    let p_mode = params.clone();
    let p_slope = params.clone();
    let g_mode = gui_context.clone();
    let g_slope = gui_context;

    VStack::new(cx, move |cx| {
        // 1. 80 px filter frequency response scope
        FxSlotVisualizer::new(cx, scope_provider)
            .hoverable(false)
            .width(Stretch(1.0))
            .height(Pixels(scope_h));

        // 2. 20 px [Mode ▼ (Stretch)] [Slope ▼ (58 px)] context strip
        HStack::new(cx, move |cx| {
            build_enum_dropdown(
                cx,
                lens_mode,
                p_mode,
                g_mode,
                move |p: &Arc<Params>| unit_params.mode(p),
                FX_FILTER_MODE_ITEMS,
            )
            .width(Stretch(1.0))
            .height(Pixels(tokens::CONTROL_H));

            build_enum_dropdown_with_override(
                cx,
                lens_slope,
                p_slope,
                g_slope,
                move |p: &Arc<Params>| unit_params.slope(p),
                FX_FILTER_SLOPE_ITEMS,
                move |p| {
                    let mode: FilterMode = unit_params.mode(p).value().into();
                    mode.has_fixed_12db_slope().then_some("12 dB")
                },
            )
            .width(Pixels(58.0))
            .height(Pixels(tokens::CONTROL_H));
        })
        .col_between(Pixels(tokens::SPACE_SM))
        .width(Stretch(1.0))
        .height(Pixels(tokens::CONTROL_H));

        // 3. Equidistant 2×2 knob grid with reactive DRIVE / SCREAM title on knob 2
        VStack::new(cx, move |cx| {
            for row in 0..2 {
                let lens_r = params_lens.clone();
                let p_r = params.clone();
                let mod_r = mod_targets.clone();
                HStack::new(cx, move |cx| {
                    for col in 0..2 {
                        let p_idx = row * 2 + col;
                        let formatter: ValueFormatter =
                            Arc::new(move |norm: f32, _off: Option<f32>| {
                                format_fx_slot_param_value(FxType::Filter, false, 0, p_idx, norm)
                            });
                        let lens_k = lens_r.clone();
                        let lens_title = lens_r.clone();
                        let p_dim = p_r.clone();
                        let mod_cell = mod_r
                            .as_ref()
                            .map(|(dests, host, tag)| (dests[p_idx], host.clone(), *tag));

                        VStack::new(cx, move |cx| {
                            if p_idx == 2 {
                                let title_lens = lens_title.map(move |p| {
                                    if unit_params.mode(p).value() == FilterModeParam::Scream {
                                        "SCREAM"
                                    } else {
                                        "DRIVE"
                                    }
                                });
                                bold_label(cx, title_lens, "knob-title")
                                    .width(Stretch(1.0))
                                    .height(Pixels(tokens::KNOB_TITLE_H))
                                    .child_space(Stretch(1.0));
                            } else {
                                let title =
                                    fx_slot_param_label(FxType::Filter, false, false, 0, p_idx);
                                bold_label(cx, title, "knob-title")
                                    .width(Stretch(1.0))
                                    .height(Pixels(tokens::KNOB_TITLE_H))
                                    .child_space(Stretch(1.0));
                            }

                            RotaryKnob::new_with_opt_mod_formatter(
                                cx,
                                lens_k,
                                move |p: &Arc<Params>| unit_params.param(p.as_ref(), p_idx),
                                false,
                                accent,
                                mod_cell,
                                Some(formatter),
                            )
                            .default_norm(Some(defs[p_idx]))
                            .enabled_by(p_dim, move |p| unit_params.enabled(p).value())
                            .width(Pixels(knob_sz))
                            .height(Pixels(knob_sz))
                            .left(Stretch(1.0))
                            .right(Stretch(1.0));
                        })
                        .class("knob-cell")
                        .width(Stretch(1.0))
                        .height(Auto)
                        .row_between(Pixels(tokens::SPACE_XS));
                    }
                })
                .col_between(Pixels(0.0))
                .child_left(Pixels(tokens::FX_UNIT_2X2_INSET_X))
                .child_right(Pixels(tokens::FX_UNIT_2X2_INSET_X))
                .width(Stretch(1.0))
                .height(Auto);
            }
        })
        .class("fx-unit-2x2")
        .row_between(Pixels(tokens::SPACE_MD))
        .top(Pixels(tokens::FX_UNIT_2X2_TOP_PAD))
        .bottom(Pixels(tokens::FX_UNIT_2X2_BOTTOM_PAD))
        .width(Stretch(1.0))
        .height(Auto);
    })
    .row_between(Pixels(tokens::SPACE_SM))
    .width(Stretch(1.0))
    .height(Auto);
}
