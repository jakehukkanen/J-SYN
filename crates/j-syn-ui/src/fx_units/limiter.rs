//! Shared `200 px` variable-height FX Unit view for `Limiter` (`ZStack` `80 px` scope +
//! top-left `TP` pill + `18 px` `MAIN | ADV` tab bar + equidistant `2×2` knob grid).

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::FxType;
use nih_plug::prelude::{BoolParam, FloatParam, GuiContext};
use nih_plug_vizia::vizia::prelude::*;

use crate::controls::bold_label;
use crate::scopes::{FxSlotScopeSnapshot, FxSlotVisualizer};
use crate::shell::{ModUiHost, UiKitEvent};
use crate::tokens;

use super::simple_units::{build_2x2_knob_grid, FxSubTabHost};
use super::build_overlay_bool_pill;

/// Parameter projection trait for the shared `Limiter` FX Unit view (with `TP` True Peak toggle).
pub trait LimiterUnitParams<Params: 'static>: Copy + Send + Sync + 'static {
    fn enabled<'a>(&self, p: &'a Params) -> &'a BoolParam;
    fn true_peak<'a>(&self, p: &'a Params) -> &'a BoolParam;
    /// `idx` in `0..4` = `MAIN` tab (`GAIN`, `CEIL`, `LOOKAHD`, `MIX`),
    /// `4..8` = `ADV` tab (`RELEASE`, `KNEE`, `LINK`, `PUNCH`).
    fn param<'a>(&self, p: &'a Params, idx: usize) -> &'a FloatParam;
}

/// Shared `Limiter` FX Unit body (`80 px` `ZStack` scope + top-left `TP` pill + `18 px`
/// `MAIN | ADV` tab bar + constant-height `2×2` knob grid).
#[allow(clippy::too_many_arguments)]
pub fn build_limiter_fx_unit_body<L, R, Params, U, FScope>(
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
    U: LimiterUnitParams<Params>,
    FScope: Fn() -> FxSlotScopeSnapshot + Send + Sync + 'static,
{
    let scope_h = tokens::fx_unit_scope_height(FxType::Limiter);
    let main_defs = j_syn_dsp::FxSlotParams::default_params_for_type(FxType::Limiter);
    let sub_defs = j_syn_dsp::FxSlotParams::default_eq_params_for_type(FxType::Limiter);

    let lens_scope = params_lens.clone();
    let p_scope = params.clone();
    let rev_tabs = ui_rev_lens.clone();
    let rev_grid = ui_rev_lens;
    let host_tabs = tab_host.clone();
    let host_grid = tab_host;

    VStack::new(cx, move |cx| {
        // 1. 80 px scope ZStack with top-left TP (True Peak) pill
        ZStack::new(cx, move |cx| {
            FxSlotVisualizer::new(cx, scope_provider)
                .hoverable(false)
                .width(Stretch(1.0))
                .height(Stretch(1.0));

            HStack::new(cx, move |cx| {
                build_overlay_bool_pill(
                    cx,
                    lens_scope,
                    p_scope,
                    gui_context,
                    "TP",
                    move |p: &Arc<Params>| unit_params.true_peak(p),
                );
                Element::new(cx).width(Stretch(1.0)).hoverable(false);
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

        // 2. 18 px MAIN | ADV tab bar
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
                |cx| bold_label(cx, "ADV", "fx-subtab-lbl").hoverable(false),
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

        // 3. Constant-height 2×2 knob grid switching between MAIN (0..4) and ADV (4..8)
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
                build_2x2_knob_grid(
                    cx,
                    FxType::Limiter,
                    is_sub,
                    [false; 4],
                    defs,
                    knob_sz,
                    params_lens.clone(),
                    params.clone(),
                    move |p| unit_params.enabled(p),
                    move |p, idx| unit_params.param(p, base + idx),
                    mod_slice,
                );
            },
        );
    })
    .row_between(Pixels(tokens::SPACE_SM))
    .width(Stretch(1.0))
    .height(Auto);
}
