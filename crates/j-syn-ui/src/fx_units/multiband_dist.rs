//! Shared `MB Dist` (3-band LR4 Multiband Distortion) FX Unit view and parameter contract,
//! consumed identically by both `J-SYN`'s 3-lane FX rack and the standalone `j-mbdist` plugin.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::{DistortionMode, FxType};
use nih_plug::prelude::{BoolParam, Enum, EnumParam, FloatParam, GuiContext, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;

use crate::controls::{bold_label, build_enum_dropdown, ScrollDropdownExt};
use crate::scopes::{
    fx_accent_rgb, FxSlotScopeSnapshot, FxSlotVisualizer, MultibandXoverDragOverlay,
    MultibandXoverHost,
};
use crate::shell::{ModUiHost, UiKitEvent};
use crate::tokens;
use crate::widgets::{KnobDimExt, RotaryKnob, ValueFormatter};

use super::{format_fx_slot_param_value, fx_slot_param_label, mb_band_default_norm};

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistortionModeParam {
    #[name = "Tube"]
    Tube,
    #[name = "Mech"]
    Mech,
    #[name = "Sine"]
    Sine,
    #[name = "Crush"]
    Crush,
    #[name = "Classic A"]
    ClassicA,
    #[name = "Classic B"]
    ClassicB,
    #[name = "Classic C"]
    ClassicC,
    #[name = "Diode A"]
    DiodeA,
    #[name = "Diode B"]
    DiodeB,
    #[name = "Linear Fold"]
    LinearFold,
    #[name = "Sine Fold"]
    SineFold,
    #[name = "Hard Clip"]
    HardClip,
    #[name = "Half Rect"]
    HalfRectify,
    #[name = "Full Rect"]
    FullRectify,
}

impl From<DistortionModeParam> for DistortionMode {
    fn from(v: DistortionModeParam) -> Self {
        match v {
            DistortionModeParam::Tube => Self::Tube,
            DistortionModeParam::Mech => Self::Mech,
            DistortionModeParam::Sine => Self::Sine,
            DistortionModeParam::Crush => Self::Crush,
            DistortionModeParam::ClassicA => Self::ClassicA,
            DistortionModeParam::ClassicB => Self::ClassicB,
            DistortionModeParam::ClassicC => Self::ClassicC,
            DistortionModeParam::DiodeA => Self::DiodeA,
            DistortionModeParam::DiodeB => Self::DiodeB,
            DistortionModeParam::LinearFold => Self::LinearFold,
            DistortionModeParam::SineFold => Self::SineFold,
            DistortionModeParam::HardClip => Self::HardClip,
            DistortionModeParam::HalfRectify => Self::HalfRectify,
            DistortionModeParam::FullRectify => Self::FullRectify,
        }
    }
}

pub const MB_DIST_MODE_ITEMS: &[(&str, DistortionModeParam)] = &[
    ("Tube", DistortionModeParam::Tube),
    ("Mech", DistortionModeParam::Mech),
    ("Sine", DistortionModeParam::Sine),
    ("Crush", DistortionModeParam::Crush),
    ("Classic A", DistortionModeParam::ClassicA),
    ("Classic B", DistortionModeParam::ClassicB),
    ("Classic C", DistortionModeParam::ClassicC),
    ("Diode A", DistortionModeParam::DiodeA),
    ("Diode B", DistortionModeParam::DiodeB),
    ("Lin Fold", DistortionModeParam::LinearFold),
    ("Sin Fold", DistortionModeParam::SineFold),
    ("Hard Clip", DistortionModeParam::HardClip),
    ("Half Rect", DistortionModeParam::HalfRectify),
    ("Full Rect", DistortionModeParam::FullRectify),
];

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

/// Shared `MB Dist` FX Unit body (`ZStack` 3-band LR4 crossover scope with draggable split handles,
/// per-band distortion mode dropdowns + `S` solo pills, `XOV`/`ON` side switches, and 4-knob footer),
/// sized to fit cleanly at `FX_UNIT_W` (`220 px`) and stretch naturally in wider cards.
#[allow(clippy::too_many_arguments)]
pub fn build_multiband_dist_fx_unit_body<L, R, Params, U, FScope>(
    cx: &mut Context,
    params_lens: L,
    ui_rev_lens: R,
    params: Arc<Params>,
    gui_context: Arc<dyn GuiContext>,
    unit_params: U,
    tab_host: Arc<dyn MultibandTabHost>,
    mod_targets: Option<([ModDestination; 4], Arc<dyn ModUiHost>, u8)>,
    scope_provider: FScope,
    knob_sz: f32,
) where
    L: Lens<Target = Arc<Params>> + Clone,
    R: Lens<Target = u64> + Clone,
    Params: Send + Sync + 'static,
    U: MultibandDistUnitParams<Params>,
    FScope: Fn() -> FxSlotScopeSnapshot + Send + Sync + 'static,
{
    let scope_h = tokens::fx_unit_scope_height(FxType::MultibandDist);
    let global_defs = j_syn_dsp::FxSlotParams::default_params_for_type(FxType::MultibandDist);
    let accent = fx_accent_rgb(FxType::MultibandDist);

    let lens_scope = params_lens.clone();
    let rev_scope = ui_rev_lens.clone();
    let p_scope = params.clone();
    let g_scope = gui_context.clone();
    let tab_scope = tab_host.clone();

    VStack::new(cx, move |cx| {
        // Scope ZStack: visualizer + draggable crossover overlay + 3 per-band [Mode | S] clusters
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
                    tab_host: tab_scope.clone(),
                }),
            )
            .position_type(PositionType::SelfDirected)
            .z_index(5)
            .top(Pixels(tokens::FX_UNIT_OVERLAY_PILL_H + tokens::SPACE_SM + 1.0))
            .left(Pixels(0.0))
            .right(Pixels(0.0))
            .bottom(Pixels(0.0));

            HStack::new(cx, move |cx| {
                for band in 0..3 {
                    let lens_band = lens_scope.clone();
                    let rev_band = rev_scope.clone();
                    let p_band = p_scope.clone();
                    let g_band = g_scope.clone();
                    let tab_band = tab_scope.clone();

                    HStack::new(cx, move |cx| {
                        let tab_act = tab_band.clone();
                        let tab_cls = tab_band;
                        build_enum_dropdown(
                            cx,
                            lens_band.clone(),
                            p_band.clone(),
                            g_band.clone(),
                            move |p: &Arc<Params>| unit_params.band_mode(p, band),
                            MB_DIST_MODE_ITEMS,
                        )
                        .on_activate(move |cx| {
                            tab_act.set_mb_tab(band as u8 + 1);
                            cx.emit(UiKitEvent::ModUiChanged);
                        })
                        .class("fx-scope-dd")
                        .toggle_class(
                            "fx-mb-band-active",
                            rev_band.map(move |_| tab_cls.mb_tab() == (band as u8 + 1)),
                        )
                        .width(Stretch(1.0))
                        .height(Pixels(tokens::FX_UNIT_OVERLAY_PILL_H));

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
                        .width(Pixels(14.0))
                        .height(Pixels(tokens::FX_UNIT_OVERLAY_PILL_H));
                    })
                    .col_between(Pixels(tokens::SPACE_XS))
                    .width(Stretch(1.0))
                    .height(Pixels(tokens::FX_UNIT_OVERLAY_PILL_H));
                }
            })
            .col_between(Pixels(tokens::SPACE_SM))
            .position_type(PositionType::SelfDirected)
            .z_index(10)
            .width(Stretch(1.0))
            .height(Pixels(tokens::FX_UNIT_OVERLAY_PILL_H))
            .top(Pixels(tokens::SPACE_SM))
            .left(Pixels(tokens::SPACE_SM))
            .right(Pixels(tokens::SPACE_SM));
        })
        .width(Stretch(1.0))
        .height(Pixels(scope_h));

        // Footer: [XOV / ON] side column + 4 reactive knobs
        let lens_knobs = params_lens;
        let rev_knobs = ui_rev_lens;
        let p_knobs = params;
        let g_knobs = gui_context;
        let tab_knobs = tab_host;

        HStack::new(cx, move |cx| {
            // Left XOV / band ON toggle column
            let lens_side = lens_knobs.clone();
            let rev_side = rev_knobs.clone();
            let p_side = p_knobs.clone();
            let g_side = g_knobs;
            let tab_side_key = tab_knobs.clone();
            let tab_side_btn = tab_knobs.clone();

            Binding::new(
                cx,
                rev_side.clone().map(move |_| tab_side_key.mb_tab()),
                move |cx, tab_lens| {
                    let mb_tab = tab_lens.get(cx);
                    let lens_col = lens_side.clone();
                    let rev_col = rev_side.clone();
                    let p_col = p_side.clone();
                    let g_col = g_side.clone();
                    let tab_xov = tab_side_btn.clone();
                    let tab_xov_cls = tab_side_btn.clone();

                    VStack::new(cx, move |cx| {
                        Button::new(
                            cx,
                            move |cx| {
                                tab_xov.set_mb_tab(0);
                                cx.emit(UiKitEvent::ModUiChanged);
                            },
                            |cx| bold_label(cx, "XOV", "fx-subtab-lbl").hoverable(false),
                        )
                        .class("fx-mb-side-btn")
                        .toggle_class(
                            "fx-subtab-active",
                            rev_col.map(move |_| tab_xov_cls.mb_tab() == 0),
                        )
                        .width(Pixels(22.0))
                        .height(Pixels(15.0));

                        if mb_tab > 0 {
                            let band = (mb_tab - 1) as usize;
                            Button::new(
                                cx,
                                move |cx| {
                                    let setter = ParamSetter::new(g_col.as_ref());
                                    let p = unit_params.band_enabled(&p_col, band);
                                    let next = !p.value();
                                    setter.begin_set_parameter(p);
                                    setter.set_parameter(p, next);
                                    setter.end_set_parameter(p);
                                    cx.emit(UiKitEvent::ModUiChanged);
                                },
                                |cx| bold_label(cx, "ON", "fx-subtab-lbl").hoverable(false),
                            )
                            .class("fx-mb-side-btn")
                            .toggle_class(
                                "fx-subtab-active",
                                lens_col.map(move |p| unit_params.band_enabled(p, band).value()),
                            )
                            .width(Pixels(22.0))
                            .height(Pixels(15.0));
                        } else {
                            Element::new(cx)
                                .width(Pixels(22.0))
                                .height(Pixels(15.0))
                                .hoverable(false);
                        }
                    })
                    .row_between(Pixels(tokens::SPACE_XS))
                    .width(Pixels(22.0))
                    .height(Auto)
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));
                },
            );

            // 4 reactive knobs: XOV (LOW XOV, HIGH XOV, OUT GN, MIX) or Band (DRIVE, BIAS, LEVEL, MIX)
            for p_idx in 0..4 {
                let lens_k = lens_knobs.clone();
                let rev_k = rev_knobs.clone();
                let p_dim = p_knobs.clone();
                let tab_k = tab_knobs.clone();
                let mod_t = mod_targets
                    .as_ref()
                    .map(|(dests, host, tag)| (dests[p_idx], host.clone(), *tag));

                Binding::new(
                    cx,
                    rev_k.map(move |_| tab_k.mb_tab()),
                    move |cx, tab_lens| {
                        let mb_tab = tab_lens.get(cx);
                        let mb_band = (mb_tab > 0).then(|| (mb_tab - 1) as usize);
                        let title =
                            fx_slot_param_label(FxType::MultibandDist, false, false, mb_tab, p_idx);
                        let formatter: ValueFormatter =
                            Arc::new(move |norm: f32, _off: Option<f32>| {
                                format_fx_slot_param_value(
                                    FxType::MultibandDist,
                                    false,
                                    mb_tab,
                                    p_idx,
                                    norm,
                                )
                            });
                        let def_norm = if mb_band.is_some() {
                            mb_band_default_norm(p_idx)
                        } else {
                            global_defs[p_idx]
                        };
                        let lens_cell = lens_k.clone();
                        let p_cell = p_dim.clone();
                        let mod_cell = mod_t.clone();

                        VStack::new(cx, move |cx| {
                            bold_label(cx, title, "knob-title")
                                .width(Stretch(1.0))
                                .height(Pixels(tokens::KNOB_TITLE_COMPACT_H))
                                .child_space(Stretch(1.0));

                            let knob = if let Some(band) = mb_band {
                                RotaryKnob::new_with_formatter(
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
                                    formatter,
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
                                        && mb_band.is_none_or(|b| {
                                            unit_params.band_enabled(p, b).value()
                                        })
                                })
                                .width(Pixels(knob_sz))
                                .height(Pixels(knob_sz))
                                .left(Stretch(1.0))
                                .right(Stretch(1.0));
                        })
                        .class("knob-cell")
                        .width(Stretch(1.0))
                        .height(Auto)
                        .row_between(Pixels(1.0));
                    },
                );
            }
        })
        .col_between(Pixels(3.0))
        .width(Stretch(1.0))
        .height(Auto);
    })
    .row_between(Pixels(tokens::SPACE_SM))
    .width(Stretch(1.0))
    .height(Auto);
}
