//! Shared `200 px` variable-height FX Unit view for `Filter` (`80 px` frequency response
//! scope + `20 px` `[Mode ▼] [Slope ▼]` context strip + 3-over-2 knob grid:
//! Row 1 = `CUTOFF`, `RES`, `FREQ 2`; Row 2 = dynamic `DRIVE` / `SCREAM`, `MIX`).

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
    /// Secondary peak/notch cutoff for multi-filter modes (`HP + Peak`, `Notch + Notch`, etc.).
    fn freq2<'a>(&self, p: &'a Params) -> &'a FloatParam;
}

/// Shared `Filter` FX Unit body (`80 px` scope + `20 px` `[Mode ▼] [Slope ▼]` context strip
/// + 3-over-2 knob grid).
#[allow(clippy::too_many_arguments)]
pub fn build_filter_fx_unit_body<L, Params, U, FScope>(
    cx: &mut Context,
    params_lens: L,
    params: Arc<Params>,
    gui_context: Arc<dyn GuiContext>,
    unit_params: U,
    mod_targets: Option<([ModDestination; 5], Arc<dyn ModUiHost>, u8)>,
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
    let eq_defs = j_syn_dsp::FxSlotParams::default_eq_params_for_type(FxType::Filter);
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

        // 3. 3-over-2 knob grid: Row 1 = CUTOFF, RES, FREQ 2; Row 2 = DRIVE/SCREAM, MIX
        let lens_r1 = params_lens.clone();
        let p_r1 = params.clone();
        let mod_r1 = mod_targets.clone();
        VStack::new(cx, move |cx| {
            // Row 1: 3 evenly spaced knobs (CUTOFF, RES, FREQ 2)
            HStack::new(cx, move |cx| {
                for (slot_idx, is_freq2) in [(0usize, false), (1, false), (4, true)] {
                    let title = if is_freq2 {
                        fx_slot_param_label(FxType::Filter, true, false, 0, 0)
                    } else {
                        fx_slot_param_label(FxType::Filter, false, false, 0, slot_idx)
                    };
                    let formatter: ValueFormatter = Arc::new(move |norm: f32, _off: Option<f32>| {
                        if is_freq2 {
                            format_fx_slot_param_value(FxType::Filter, true, 0, 0, norm)
                        } else {
                            format_fx_slot_param_value(FxType::Filter, false, 0, slot_idx, norm)
                        }
                    });
                    let def_norm = if is_freq2 { eq_defs[0] } else { defs[slot_idx] };
                    let lens_k = lens_r1.clone();
                    let p_dim = p_r1.clone();
                    let mod_cell = mod_r1
                        .as_ref()
                        .map(|(dests, host, tag)| (dests[slot_idx], host.clone(), *tag));

                    VStack::new(cx, move |cx| {
                        bold_label(cx, title, "knob-title")
                            .width(Stretch(1.0))
                            .height(Pixels(tokens::KNOB_TITLE_H))
                            .child_space(Stretch(1.0));

                        RotaryKnob::new_with_opt_mod_formatter(
                            cx,
                            lens_k,
                            move |p: &Arc<Params>| {
                                if is_freq2 {
                                    unit_params.freq2(p.as_ref())
                                } else {
                                    unit_params.param(p.as_ref(), slot_idx)
                                }
                            },
                            false,
                            accent,
                            mod_cell,
                            Some(formatter),
                        )
                        .default_norm(Some(def_norm))
                        .enabled_by(p_dim, move |p| {
                            unit_params.enabled(p).value()
                                && (!is_freq2
                                    || FilterMode::from(unit_params.mode(p).value()).is_multi())
                        })
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
            .child_left(Pixels(tokens::SPACE_SM))
            .child_right(Pixels(tokens::SPACE_SM))
            .width(Stretch(1.0))
            .height(Auto);

            // Row 2: 2 evenly spaced knobs (DRIVE/SCREAM, MIX)
            HStack::new(cx, move |cx| {
                for p_idx in [2usize, 3] {
                    let formatter: ValueFormatter =
                        Arc::new(move |norm: f32, _off: Option<f32>| {
                            format_fx_slot_param_value(FxType::Filter, false, 0, p_idx, norm)
                        });
                    let lens_k = params_lens.clone();
                    let lens_title = params_lens.clone();
                    let p_dim = params.clone();
                    let mod_cell = mod_targets
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
