//! Shared `MB Dist` (3-band LR4 Multiband Distortion) FX Unit view and parameter contract,
//! consumed identically by both `J-SYN`'s 3-lane FX rack and the standalone `j-mbdist` plugin.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::{mb_ordered_xover_hz, FxType};
use nih_plug::prelude::{BoolParam, EnumParam, FloatParam, GuiContext, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;

use crate::controls::{bold_label, build_enum_dropdown};
use crate::scopes::{
    fx_accent_rgb, FxSlotScopeSnapshot, FxSlotVisualizer, MultibandXoverDragOverlay,
    MultibandXoverHost,
};
use crate::shell::{ModUiHost, UiKitEvent};
use crate::tokens;
use crate::widgets::{KnobDimExt, LedToggleSwitch, RotaryKnob, ValueFormatter};

use super::params::{DistortionModeParam, MB_DIST_MODE_ITEMS};
use super::{format_fx_slot_param_value, format_hz, fx_slot_param_label, mb_band_default_norm};

/// Parameter projection trait for the shared `MB Dist` FX Unit view.
/// Implemented as a tiny `Copy` marker/coordinate struct by both standalone `j-mbdist`
/// and `J-SYN`'s `(bus_idx, slot_idx)` rack slots.
pub trait MultibandDistUnitParams<Params: 'static>: Copy + Send + Sync + 'static {
    fn enabled<'a>(&self, p: &'a Params) -> &'a BoolParam;
    fn low_xover<'a>(&self, p: &'a Params) -> &'a FloatParam;
    fn high_xover<'a>(&self, p: &'a Params) -> &'a FloatParam;
    fn out_gain<'a>(&self, p: &'a Params) -> &'a FloatParam;
    fn mix<'a>(&self, p: &'a Params) -> &'a FloatParam;
    fn band_enabled<'a>(&self, p: &'a Params, band: usize) -> &'a BoolParam;
    fn band_solo<'a>(&self, p: &'a Params, band: usize) -> &'a BoolParam;
    fn band_mode<'a>(&self, p: &'a Params, band: usize) -> &'a EnumParam<DistortionModeParam>;
    fn band_drive<'a>(&self, p: &'a Params, band: usize) -> &'a FloatParam;
    fn band_bias<'a>(&self, p: &'a Params, band: usize) -> &'a FloatParam;
    fn band_level<'a>(&self, p: &'a Params, band: usize) -> &'a FloatParam;
    fn band_mix<'a>(&self, p: &'a Params, band: usize) -> &'a FloatParam;
}

/// Host abstraction for the active `MB Dist` editing tab (`0 = XOV`, `1 = LOW`, `2 = MID`, `3 = HIGH`).
pub trait MultibandTabHost: Send + Sync + 'static {
    fn mb_tab(&self) -> u8;
    fn set_mb_tab(&self, tab: u8);
}

struct UnitMbXoverHost<Params, U> {
    params: Arc<Params>,
    gui_context: Arc<dyn GuiContext>,
    unit_params: U,
    tab_host: Arc<dyn MultibandTabHost>,
}

impl<Params, U> MultibandXoverHost for UnitMbXoverHost<Params, U>
where
    Params: Send + Sync + 'static,
    U: MultibandDistUnitParams<Params>,
{
    fn xover_norms(&self) -> (f32, f32) {
        (
            self.unit_params.low_xover(&self.params).value(),
            self.unit_params.high_xover(&self.params).value(),
        )
    }

    fn begin_xover(&self, handle: usize) {
        let param = if handle == 0 {
            self.unit_params.low_xover(&self.params)
        } else {
            self.unit_params.high_xover(&self.params)
        };
        ParamSetter::new(self.gui_context.as_ref()).begin_set_parameter(param);
    }

    fn set_xover(&self, handle: usize, norm: f32) {
        let param = if handle == 0 {
            self.unit_params.low_xover(&self.params)
        } else {
            self.unit_params.high_xover(&self.params)
        };
        ParamSetter::new(self.gui_context.as_ref()).set_parameter(param, norm);
    }

    fn end_xover(&self, handle: usize) {
        let param = if handle == 0 {
            self.unit_params.low_xover(&self.params)
        } else {
            self.unit_params.high_xover(&self.params)
        };
        ParamSetter::new(self.gui_context.as_ref()).end_set_parameter(param);
    }

    fn select_band_tab(&self, tab: u8) {
        self.tab_host.set_mb_tab(tab);
    }
}

#[allow(clippy::too_many_arguments)]
fn build_mb_knob_cell<L, R, Params, U>(
    cx: &mut Context,
    p_idx: usize,
    params_lens: L,
    ui_rev_lens: R,
    params: Arc<Params>,
    unit_params: U,
    tab_host: Arc<dyn MultibandTabHost>,
    mod_targets: Option<([ModDestination; 16], Arc<dyn ModUiHost>, u8)>,
    global_defs: [f32; 4],
    accent: (u8, u8, u8),
    knob_sz: f32,
) where
    L: Lens<Target = Arc<Params>> + Clone,
    R: Lens<Target = u64> + Clone,
    Params: Send + Sync + 'static,
    U: MultibandDistUnitParams<Params>,
{
    Binding::new(
        cx,
        ui_rev_lens.map(move |_| tab_host.mb_tab()),
        move |cx, tab_lens| {
            let mb_tab = tab_lens.get(cx);
            let mb_band = (mb_tab > 0).then(|| (mb_tab - 1) as usize);
            let title = fx_slot_param_label(FxType::MultibandDist, false, false, mb_tab, p_idx);
            let formatter: ValueFormatter = Arc::new(move |norm: f32, _off: Option<f32>| {
                format_fx_slot_param_value(FxType::MultibandDist, false, mb_tab, p_idx, norm)
            });
            let def_norm = if mb_band.is_some() {
                mb_band_default_norm(p_idx)
            } else {
                global_defs[p_idx]
            };
            let lens_cell = params_lens.clone();
            let p_cell = params.clone();
            let mod_cell = mod_targets.as_ref().map(|(dests, host, tag)| {
                let dest_idx = (mb_tab as usize).min(3) * 4 + p_idx;
                (dests[dest_idx], host.clone(), *tag)
            });

            VStack::new(cx, move |cx| {
                bold_label(cx, title, "knob-title")
                    .width(Stretch(1.0))
                    .height(Pixels(tokens::KNOB_TITLE_H))
                    .child_space(Stretch(1.0));

                let knob = if let Some(band) = mb_band {
                    RotaryKnob::new_with_opt_mod_formatter(
                        cx,
                        lens_cell,
                        move |p: &Arc<Params>| match p_idx {
                            0 => unit_params.band_drive(p, band),
                            1 => unit_params.band_bias(p, band),
                            2 => unit_params.band_level(p, band),
                            _ => unit_params.band_mix(p, band),
                        },
                        p_idx == 1,
                        accent,
                        mod_cell,
                        Some(formatter),
                    )
                } else {
                    RotaryKnob::new_with_opt_mod_formatter(
                        cx,
                        lens_cell,
                        move |p: &Arc<Params>| match p_idx {
                            0 => unit_params.low_xover(p),
                            1 => unit_params.high_xover(p),
                            2 => unit_params.out_gain(p),
                            _ => unit_params.mix(p),
                        },
                        p_idx == 2,
                        accent,
                        mod_cell,
                        Some(formatter),
                    )
                };

                knob.default_norm(Some(def_norm))
                    .enabled_by(p_cell, move |p| {
                        unit_params.enabled(p).value()
                            && mb_band.is_none_or(|b| unit_params.band_enabled(p, b).value())
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
        },
    );
}

/// Shared `MB Dist` FX Unit body designed to fit cleanly down to `210 px` width and stretch
/// naturally in wider cards:
/// - Clean 3-band LR4 crossover scope with compact `[L][S]`, `[M][S]`, `[H][S]` top pills
/// - `[XOV] [LOW] [MID] [HIGH]` band selector bar
/// - Constant-height context strip (live crossover Hz readout on `XOV`, or `[● ON] [Mode ▼]` on bands)
/// - Spacious `2×2` knob grid
#[allow(clippy::too_many_arguments)]
pub fn build_multiband_dist_fx_unit_body<L, R, Params, U, FScope>(
    cx: &mut Context,
    params_lens: L,
    ui_rev_lens: R,
    params: Arc<Params>,
    gui_context: Arc<dyn GuiContext>,
    unit_params: U,
    tab_host: Arc<dyn MultibandTabHost>,
    mod_targets: Option<([ModDestination; 16], Arc<dyn ModUiHost>, u8)>,
    scope_provider: FScope,
    knob_sz: f32,
) where
    L: Lens<Target = Arc<Params>> + Clone,
    R: Lens<Target = u64> + Clone,
    Params: Send + Sync + 'static,
    U: MultibandDistUnitParams<Params>,
    FScope: Fn() -> FxSlotScopeSnapshot + Send + Sync + 'static,
{
    const BAND_SHORT_LABELS: [&str; 3] = ["L", "M", "H"];
    const TAB_LABELS: [&str; 4] = ["XOV", "LOW", "MID", "HIGH"];

    let scope_h = tokens::fx_unit_scope_height(FxType::MultibandDist);
    let global_defs = j_syn_dsp::FxSlotParams::default_params_for_type(FxType::MultibandDist);
    let accent = fx_accent_rgb(FxType::MultibandDist);

    let lens_scope = params_lens.clone();
    let p_scope = params.clone();
    let g_scope = gui_context.clone();
    let tab_scope = tab_host.clone();

    let rev_tabs = ui_rev_lens.clone();
    let tab_bar_host = tab_host.clone();

    let lens_ctx = params_lens.clone();
    let rev_ctx = ui_rev_lens.clone();
    let p_ctx = params.clone();
    let g_ctx = gui_context;
    let tab_ctx = tab_host.clone();

    VStack::new(cx, move |cx| {
        // 1. Clean 3-band LR4 scope with compact [L][S] | [M][S] | [H][S] top pills
        ZStack::new(cx, move |cx| {
            FxSlotVisualizer::new(cx, scope_provider)
                .hoverable(false)
                .width(Stretch(1.0))
                .height(Stretch(1.0));

            MultibandXoverDragOverlay::new(
                cx,
                Arc::new(UnitMbXoverHost {
                    params: p_scope.clone(),
                    gui_context: g_scope.clone(),
                    unit_params,
                    tab_host: tab_scope,
                }),
            )
            .position_type(PositionType::SelfDirected)
            .top(Pixels(tokens::FX_UNIT_OVERLAY_PILL_H + tokens::SPACE_SM + 1.0))
            .left(Pixels(0.0))
            .right(Pixels(0.0))
            .bottom(Pixels(0.0));

            HStack::new(cx, move |cx| {
                for (band, short_lbl) in BAND_SHORT_LABELS.iter().copied().enumerate() {
                    if band > 0 {
                        Element::new(cx).width(Stretch(1.0)).hoverable(false);
                    }
                    let lens_band = lens_scope.clone();
                    let p_band = p_scope.clone();
                    let g_band = g_scope.clone();

                    HStack::new(cx, move |cx| {
                        // [L] / [M] / [H]: band enable/bypass toggle
                        let p_en = p_band.clone();
                        let g_en = g_band.clone();
                        Button::new(
                            cx,
                            move |cx| {
                                let setter = ParamSetter::new(g_en.as_ref());
                                let p = unit_params.band_enabled(&p_en, band);
                                let next = !p.value();
                                setter.begin_set_parameter(p);
                                setter.set_parameter(p, next);
                                setter.end_set_parameter(p);
                                cx.emit(UiKitEvent::ModUiChanged);
                            },
                            move |cx| bold_label(cx, short_lbl, "fx-subtab-lbl").hoverable(false),
                        )
                        .class("fx-scope-pill")
                        .class("fx-scope-flt-btn")
                        .toggle_class(
                            "fx-subtab-active",
                            lens_band
                                .clone()
                                .map(move |p| unit_params.band_enabled(p, band).value()),
                        )
                        .width(Pixels(16.0))
                        .height(Pixels(tokens::FX_UNIT_OVERLAY_PILL_H));

                        // [S]: band solo toggle (Shift = multi-solo, click = exclusive solo)
                        let p_s = p_band;
                        let g_s = g_band;
                        Button::new(
                            cx,
                            move |cx| {
                                let setter = ParamSetter::new(g_s.as_ref());
                                let multi_solo = cx.modifiers().contains(Modifiers::SHIFT);
                                if multi_solo {
                                    let p = unit_params.band_solo(&p_s, band);
                                    let next = !p.value();
                                    setter.begin_set_parameter(p);
                                    setter.set_parameter(p, next);
                                    setter.end_set_parameter(p);
                                } else {
                                    let sole_active = unit_params.band_solo(&p_s, band).value()
                                        && (0..3).all(|i| {
                                            i == band || !unit_params.band_solo(&p_s, i).value()
                                        });
                                    for i in 0..3 {
                                        let p = unit_params.band_solo(&p_s, i);
                                        let target = !sole_active && i == band;
                                        if p.value() != target {
                                            setter.begin_set_parameter(p);
                                            setter.set_parameter(p, target);
                                            setter.end_set_parameter(p);
                                        }
                                    }
                                }
                                cx.emit(UiKitEvent::ModUiChanged);
                            },
                            |cx| bold_label(cx, "S", "fx-subtab-lbl").hoverable(false),
                        )
                        .class("fx-scope-pill")
                        .class("fx-scope-flt-btn")
                        .toggle_class(
                            "fx-subtab-active",
                            lens_band.map(move |p| unit_params.band_solo(p, band).value()),
                        )
                        .width(Pixels(16.0))
                        .height(Pixels(tokens::FX_UNIT_OVERLAY_PILL_H));
                    })
                    .col_between(Pixels(tokens::SPACE_XS))
                    .width(Auto)
                    .height(Pixels(tokens::FX_UNIT_OVERLAY_PILL_H));
                }
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

        // 2. Active mode/band selector bar: [XOV] [LOW] [MID] [HIGH]
        HStack::new(cx, move |cx| {
            for (tab_idx, label) in TAB_LABELS.iter().copied().enumerate() {
                let tab_u8 = tab_idx as u8;
                let host_click = tab_bar_host.clone();
                let host_cls = tab_bar_host.clone();
                Button::new(
                    cx,
                    move |cx| {
                        host_click.set_mb_tab(tab_u8);
                        cx.emit(UiKitEvent::ModUiChanged);
                    },
                    move |cx| bold_label(cx, label, "fx-subtab-lbl").hoverable(false),
                )
                .class("fx-mb-side-btn")
                .toggle_class(
                    "fx-subtab-active",
                    rev_tabs
                        .clone()
                        .map(move |_| host_cls.mb_tab() == tab_u8),
                )
                .width(Stretch(1.0))
                .height(Pixels(tokens::PILL_H));
            }
        })
        .col_between(Pixels(tokens::SPACE_XS))
        .width(Stretch(1.0))
        .height(Pixels(tokens::PILL_H));

        // 3. Constant-height context strip: live crossover readout on XOV, or [● ON] [Mode ▼] on bands
        Binding::new(
            cx,
            rev_ctx.map(move |_| tab_ctx.mb_tab()),
            move |cx, tab_lens| {
                let mb_tab = tab_lens.get(cx);
                let lens_row = lens_ctx.clone();
                let p_row = p_ctx.clone();
                let g_row = g_ctx.clone();

                if mb_tab == 0 {
                    let xov_text = lens_row.map(move |p| {
                        let (lo_hz, hi_hz) = mb_ordered_xover_hz(
                            unit_params.low_xover(p).value(),
                            unit_params.high_xover(p).value(),
                        );
                        format!("LOW {}  •  HIGH {}", format_hz(lo_hz), format_hz(hi_hz))
                    });
                    HStack::new(cx, move |cx| {
                        bold_label(cx, xov_text, "knob-readout-text")
                            .width(Stretch(1.0))
                            .height(Stretch(1.0))
                            .child_space(Stretch(1.0));
                    })
                    .class("knob-readout-box")
                    .width(Stretch(1.0))
                    .height(Pixels(tokens::CONTROL_H));
                } else {
                    let band = (mb_tab - 1) as usize;
                    HStack::new(cx, move |cx| {
                        LedToggleSwitch::new(
                            cx,
                            lens_row.clone(),
                            move |p: &Arc<Params>| unit_params.band_enabled(p, band),
                            "OFF",
                            "ON",
                            accent,
                        )
                        .width(Pixels(tokens::FX_UNIT_LED_W))
                        .height(Pixels(tokens::FX_UNIT_ARROW_H))
                        .top(Stretch(1.0))
                        .bottom(Stretch(1.0));

                        build_enum_dropdown(
                            cx,
                            lens_row,
                            p_row,
                            g_row,
                            move |p: &Arc<Params>| unit_params.band_mode(p, band),
                            MB_DIST_MODE_ITEMS,
                        )
                        .width(Stretch(1.0))
                        .height(Pixels(tokens::CONTROL_H));
                    })
                    .col_between(Pixels(tokens::SPACE_SM))
                    .width(Stretch(1.0))
                    .height(Pixels(tokens::CONTROL_H));
                }
            },
        );

        // 4. 2×2 knob grid: (LOW XOV / HIGH XOV, OUT GN / MIX) or (DRIVE / BIAS, LEVEL / MIX)
        VStack::new(cx, move |cx| {
            for row in 0..2 {
                let lens_r = params_lens.clone();
                let rev_r = ui_rev_lens.clone();
                let p_r = params.clone();
                let tab_r = tab_host.clone();
                let mod_r = mod_targets.clone();
                HStack::new(cx, move |cx| {
                    for col in 0..2 {
                        let p_idx = row * 2 + col;
                        build_mb_knob_cell(
                            cx,
                            p_idx,
                            lens_r.clone(),
                            rev_r.clone(),
                            p_r.clone(),
                            unit_params,
                            tab_r.clone(),
                            mod_r.clone(),
                            global_defs,
                            accent,
                            knob_sz,
                        );
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
