//! Shared `200 px` variable-height FX Unit view for `Tube Resonator` (`80 px` `ZStack`
//! scope + top-left `[● FLT]` & top-right `DIRTY` pills + `18 px` `MAIN | LFO` tab bar +
//! equidistant `2×2` knob grid with semitone-snapped `TUNE` and `FLT`-linked dimming).

use std::sync::Arc;

use j_syn_dsp::fx::tube_resonator::TUBE_RES_MIDI_SPAN;
use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::FxType;
use nih_plug::prelude::{BoolParam, FloatParam, GuiContext, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;

use crate::controls::bold_label;
use crate::scopes::{FxSlotScopeSnapshot, FxSlotVisualizer};
use crate::shell::{ModUiHost, UiKitEvent};
use crate::tokens;
use crate::widgets::{KnobDimExt, RotaryKnob, ValueFormatter};

use super::simple_units::FxSubTabHost;
use super::{format_fx_slot_param_value, fx_slot_param_label};

/// Parameter projection trait for the shared `Tube Resonator` FX Unit view.
pub trait TubeResonatorUnitParams<Params: 'static>: Copy + Send + Sync + 'static {
    fn enabled<'a>(&self, p: &'a Params) -> &'a BoolParam;
    fn filter_enabled<'a>(&self, p: &'a Params) -> &'a BoolParam;
    fn dirty_lfo<'a>(&self, p: &'a Params) -> &'a BoolParam;
    /// `idx` in `0..4` = `MAIN` tab (`TUNE`, `FLT FREQ`, `FLT WDTH`, `MIX`),
    /// `4..8` = `LFO` tab (`LFO AMT`, `LFO SPD`, `FEEDBK`, `DAMP`).
    fn param<'a>(&self, p: &'a Params, idx: usize) -> &'a FloatParam;
}

/// Shared `Tube Resonator` FX Unit body (`80 px` `ZStack` scope + `[● FLT]` / `DIRTY` pills
/// + `18 px` `MAIN | LFO` tab bar + constant-height `2×2` knob grid).
#[allow(clippy::too_many_arguments)]
pub fn build_tube_res_fx_unit_body<L, R, Params, U, FScope>(
    cx: &mut Context,
    params_lens: L,
    ui_rev_lens: R,
    params: Arc<Params>,
    gui_context: Arc<dyn GuiContext>,
    unit_params: U,
    tab_host: Arc<dyn FxSubTabHost>,
    mod_targets: Option<([ModDestination; 8], Arc<dyn ModUiHost>, u8)>,
    scope_provider: FScope,
    knob_sz: f32,
) where
    L: Lens<Target = Arc<Params>> + Clone,
    R: Lens<Target = u64> + Clone,
    Params: Send + Sync + 'static,
    U: TubeResonatorUnitParams<Params>,
    FScope: Fn() -> FxSlotScopeSnapshot + Send + Sync + 'static,
{
    let scope_h = tokens::fx_unit_scope_height(FxType::TubeResonator);
    let main_defs = j_syn_dsp::FxSlotParams::default_params_for_type(FxType::TubeResonator);
    let sub_defs = j_syn_dsp::FxSlotParams::default_eq_params_for_type(FxType::TubeResonator);

    let lens_flt = params_lens.clone();
    let lens_dirty = params_lens.clone();
    let p_flt = params.clone();
    let p_dirty = params.clone();
    let g_flt = gui_context.clone();
    let g_dirty = gui_context;
    let rev_tabs = ui_rev_lens.clone();
    let rev_grid = ui_rev_lens;
    let host_tabs = tab_host.clone();
    let host_grid = tab_host;

    VStack::new(cx, move |cx| {
        // 1. 80 px scope ZStack with top-left [● FLT] and top-right DIRTY pills
        ZStack::new(cx, move |cx| {
            FxSlotVisualizer::new(cx, scope_provider)
                .hoverable(false)
                .width(Stretch(1.0))
                .height(Stretch(1.0));

            HStack::new(cx, move |cx| {
                Button::new(
                    cx,
                    move |cx| {
                        let setter = ParamSetter::new(g_flt.as_ref());
                        let p = unit_params.filter_enabled(&p_flt);
                        let next = !p.value();
                        setter.begin_set_parameter(p);
                        setter.set_parameter(p, next);
                        setter.end_set_parameter(p);
                        cx.emit(UiKitEvent::ModUiChanged);
                    },
                    |cx| {
                        HStack::new(cx, |cx| {
                            Element::new(cx)
                                .class("fx-flt-dot")
                                .hoverable(false)
                                .width(Pixels(5.0))
                                .height(Pixels(5.0))
                                .top(Stretch(1.0))
                                .bottom(Stretch(1.0));
                            bold_label(cx, "FLT", "fx-subtab-lbl").hoverable(false);
                        })
                        .col_between(Pixels(3.0))
                        .child_space(Stretch(1.0))
                        .hoverable(false)
                    },
                )
                .class("fx-scope-pill")
                .class("fx-scope-flt-btn")
                .toggle_class(
                    "fx-subtab-active",
                    lens_flt.map(move |p| unit_params.filter_enabled(p).value()),
                )
                .width(Pixels(38.0))
                .height(Pixels(tokens::FX_UNIT_OVERLAY_PILL_H));

                Element::new(cx).width(Stretch(1.0)).hoverable(false);

                Button::new(
                    cx,
                    move |cx| {
                        let setter = ParamSetter::new(g_dirty.as_ref());
                        let p = unit_params.dirty_lfo(&p_dirty);
                        let next = !p.value();
                        setter.begin_set_parameter(p);
                        setter.set_parameter(p, next);
                        setter.end_set_parameter(p);
                        cx.emit(UiKitEvent::ModUiChanged);
                    },
                    |cx| bold_label(cx, "DIRTY", "fx-subtab-lbl").hoverable(false),
                )
                .class("fx-scope-pill")
                .class("fx-scope-flt-btn")
                .toggle_class(
                    "fx-subtab-active",
                    lens_dirty.map(move |p| unit_params.dirty_lfo(p).value()),
                )
                .width(Pixels(38.0))
                .height(Pixels(tokens::FX_UNIT_OVERLAY_PILL_H));
            })
            .position_type(PositionType::SelfDirected)
            .width(Stretch(1.0))
            .height(Pixels(tokens::FX_UNIT_OVERLAY_PILL_H))
            .top(Pixels(tokens::SPACE_SM))
            .left(Pixels(tokens::SPACE_SM))
            .right(Pixels(tokens::SPACE_SM));
        })
        .width(Stretch(1.0))
        .height(Pixels(scope_h));

        // 2. 18 px MAIN | LFO tab bar
        HStack::new(cx, move |cx| {
            let h_main_click = host_tabs.clone();
            let h_main_cls = host_tabs.clone();
            Button::new(
                cx,
                move |cx| {
                    h_main_click.set_subtab(false);
                    cx.emit(UiKitEvent::ModUiChanged);
                },
                |cx| bold_label(cx, "MAIN", "fx-subtab-lbl").hoverable(false),
            )
            .class("fx-mb-side-btn")
            .toggle_class(
                "fx-subtab-active",
                rev_tabs.clone().map(move |_| !h_main_cls.is_subtab()),
            )
            .width(Stretch(1.0))
            .height(Pixels(tokens::PILL_H));

            let h_sub_click = host_tabs.clone();
            let h_sub_cls = host_tabs;
            Button::new(
                cx,
                move |cx| {
                    h_sub_click.set_subtab(true);
                    cx.emit(UiKitEvent::ModUiChanged);
                },
                |cx| bold_label(cx, "LFO", "fx-subtab-lbl").hoverable(false),
            )
            .class("fx-mb-side-btn")
            .toggle_class(
                "fx-subtab-active",
                rev_tabs.map(move |_| h_sub_cls.is_subtab()),
            )
            .width(Stretch(1.0))
            .height(Pixels(tokens::PILL_H));
        })
        .col_between(Pixels(tokens::SPACE_XS))
        .width(Stretch(1.0))
        .height(Pixels(tokens::PILL_H));

        // 3. Constant-height 2×2 knob grid switching between MAIN (0..4) and LFO (4..8)
        Binding::new(
            cx,
            rev_grid.map(move |_| host_grid.is_subtab()),
            move |cx, sub_lens| {
                let is_sub = sub_lens.get(cx);
                let base = if is_sub { 4 } else { 0 };
                let defs = if is_sub { sub_defs } else { main_defs };
                let mod_slice = mod_targets.as_ref().map(|(dests, host, tag)| {
                    (
                        [
                            dests[base],
                            dests[base + 1],
                            dests[base + 2],
                            dests[base + 3],
                        ],
                        host.clone(),
                        *tag,
                    )
                });
                let lens_grid = params_lens.clone();
                let p_grid = params.clone();

                VStack::new(cx, move |cx| {
                    for row in 0..2 {
                        let lens_r = lens_grid.clone();
                        let p_r = p_grid.clone();
                        let mod_r = mod_slice.clone();
                        HStack::new(cx, move |cx| {
                            for col in 0..2 {
                                let p_idx = row * 2 + col;
                                let title = fx_slot_param_label(
                                    FxType::TubeResonator,
                                    is_sub,
                                    false,
                                    0,
                                    p_idx,
                                );
                                let formatter: ValueFormatter =
                                    Arc::new(move |norm: f32, _off: Option<f32>| {
                                        format_fx_slot_param_value(
                                            FxType::TubeResonator,
                                            is_sub,
                                            0,
                                            p_idx,
                                            norm,
                                        )
                                    });
                                let lens_k = lens_r.clone();
                                let p_dim = p_r.clone();
                                let mod_cell = mod_r
                                    .as_ref()
                                    .map(|(dests, host, tag)| (dests[p_idx], host.clone(), *tag));
                                let snap_steps = if !is_sub && p_idx == 0 {
                                    TUBE_RES_MIDI_SPAN as u32
                                } else {
                                    0
                                };
                                let is_flt_knob = !is_sub && (p_idx == 1 || p_idx == 2);

                                VStack::new(cx, move |cx| {
                                    bold_label(cx, title, "knob-title")
                                        .width(Stretch(1.0))
                                        .height(Pixels(tokens::KNOB_TITLE_H))
                                        .child_space(Stretch(1.0));

                                    RotaryKnob::new_with_opt_mod_formatter(
                                        cx,
                                        lens_k,
                                        move |p: &Arc<Params>| {
                                            unit_params.param(p.as_ref(), base + p_idx)
                                        },
                                        false,
                                        mod_cell,
                                        Some(formatter),
                                    )
                                    .default_norm(Some(defs[p_idx]))
                                    .snap_steps(snap_steps)
                                    .enabled_by(p_dim, move |p| {
                                        unit_params.enabled(p).value()
                                            && (!is_flt_knob
                                                || unit_params.filter_enabled(p).value())
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
            },
        );
    })
    .row_between(Pixels(tokens::SPACE_SM))
    .width(Stretch(1.0))
    .height(Auto);
}
