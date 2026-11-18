//! Parallel Dual-Bus Master FX Rack UI (`Bus A` & `Bus B`, `Sub Bus`, `MAIN` Bus Switcher,
//! full `4×4` `FX` view, `ZStack` translucent floating overlays, and `MAIN | EQ` sub-tabs).

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::{FxBusTopology, FxType, MB_BAND_LEVEL_DEFAULT_NORM};
use j_syn_ui::fx_units::{
    build_multiband_dist_fx_unit_body, build_utility_fx_unit_body, format_fx_slot_param_value,
    fx_slot_param_label, MultibandDistUnitParams, MultibandTabHost, UtilityUnitParams,
};
use nih_plug::prelude::{BoolParam, EnumParam, FloatParam, GuiContext, Param, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{
    bold_label, build_dropdown_trigger_labels, build_enum_dropdown,
    build_enum_dropdown_with_override, build_flex_knob_cell_mod, build_inline_pill_cell,
    build_prefixed_enum_dropdown, ArrowButton, ArrowDir, ScrollDropdown,
};
use super::fx_scopes::{fx_accent_rgb, slot_scope_snapshot, FxSlotVisualizer};
use super::mod_bar::ModUiState;
use super::tokens;
use super::widgets::{KnobDimExt, LedToggleSwitch, RotaryKnob};
use super::{AMBER_RGB, GOLD_RGB, UiData, UiEvent};
use crate::{
    DiopserStagesParam, DistortionModeParam, FilterModeParam, FxBusTopologyParam,
    FxFilterSlopeParam, FxTypeParam, JSynPluginParams,
};

const FX_TYPE_ITEMS: &[(&str, FxTypeParam)] = &[
    ("Empty", FxTypeParam::Off),
    ("Utility", FxTypeParam::Utility),
    ("OTT", FxTypeParam::Ott),
    ("Distortion", FxTypeParam::Distortion),
    ("MB Dist", FxTypeParam::MultibandDist),
    ("Clipper", FxTypeParam::JSatClipper),
    ("Limiter", FxTypeParam::Limiter),
    ("Chorus", FxTypeParam::Chorus),
    ("Flanger", FxTypeParam::Flanger),
    ("Phaser", FxTypeParam::Phaser),
    ("Delay", FxTypeParam::Delay),
    ("Reverb", FxTypeParam::Reverb),
    ("Filter", FxTypeParam::Filter),
    ("Dispersion", FxTypeParam::Diopser),
    ("Tube Res.", FxTypeParam::TubeResonator),
];

const FX_FILTER_MODE_ITEMS: &[(&str, FilterModeParam)] = &[
    ("LP", FilterModeParam::Lp),
    ("BP", FilterModeParam::Bp),
    ("HP", FilterModeParam::Hp),
    ("NTCH", FilterModeParam::Notch),
    ("BREJ", FilterModeParam::Bandreject),
    ("PEAK", FilterModeParam::Peak),
    ("SCRM", FilterModeParam::Scream),
    ("DAFT", FilterModeParam::Daft),
    ("COMB", FilterModeParam::Comb),
    ("HP+P", FilterModeParam::HpPeak),
    ("HP+N", FilterModeParam::HpNotch),
    ("LP+N", FilterModeParam::LpNotch),
    ("LP+P", FilterModeParam::LpPeak),
    ("PK+P", FilterModeParam::PeakPeak),
    ("NT+N", FilterModeParam::NotchNotch),
];

const FX_FILTER_SLOPE_ITEMS: &[(&str, FxFilterSlopeParam)] = &[
    ("12", FxFilterSlopeParam::Db12),
    ("24", FxFilterSlopeParam::Db24),
    ("48", FxFilterSlopeParam::Db48),
];

const DIOPSER_STAGE_ITEMS: &[(&str, DiopserStagesParam)] = &[
    ("8", DiopserStagesParam::S8),
    ("16", DiopserStagesParam::S16),
    ("32", DiopserStagesParam::S32),
    ("64", DiopserStagesParam::S64),
    ("128", DiopserStagesParam::S128),
];

const DISTORTION_MODE_ITEMS: &[(&str, DistortionModeParam)] = &[
    ("Tube", DistortionModeParam::Tube),
    ("Mech", DistortionModeParam::Mech),
    ("Sine", DistortionModeParam::Sine),
    ("Crush", DistortionModeParam::Crush),
    ("Classic A", DistortionModeParam::ClassicA),
    ("Classic B", DistortionModeParam::ClassicB),
    ("Classic C", DistortionModeParam::ClassicC),
    ("Diode A", DistortionModeParam::DiodeA),
    ("Diode B", DistortionModeParam::DiodeB),
    ("Linear Fold", DistortionModeParam::LinearFold),
    ("Sine Fold", DistortionModeParam::SineFold),
    ("Hard Clip", DistortionModeParam::HardClip),
    ("Half Rect", DistortionModeParam::HalfRectify),
    ("Full Rect", DistortionModeParam::FullRectify),
];

#[inline]
fn slot_enabled_for_bus(p: &JSynPluginParams, bus_idx: usize, slot_idx: usize) -> bool {
    p.fx_bus_slots(bus_idx)[slot_idx].enabled.value()
}

fn swap_mod_destinations_for_slots(
    params: &JSynPluginParams,
    bus_idx: usize,
    slot_a: usize,
    slot_b: usize,
) {
    let mut mat = params.mod_matrix.snapshot();
    let mut changed = false;
    for slot in &mut mat.slots {
        if let Some((b, s, p)) = slot.destination.fx_slot_param_coords() {
            if b == bus_idx && s == slot_a {
                slot.destination = ModDestination::fx_bus_slot_param(bus_idx, slot_b, p);
                changed = true;
            } else if b == bus_idx && s == slot_b {
                slot.destination = ModDestination::fx_bus_slot_param(bus_idx, slot_a, p);
                changed = true;
            }
        }
    }
    if changed {
        params.mod_matrix.store_value(mat);
    }
}

struct SlotSnapshot {
    en: bool,
    ty: FxTypeParam,
    stk: i32,
    p_main: [f32; 4],
    p_eq: [f32; 4],
    flt_m: FilterModeParam,
    flt_s: FxFilterSlopeParam,
    diop_s: DiopserStagesParam,
    tr_flt: bool,
    util_inv_l: bool,
    util_inv_r: bool,
    lim_tp: bool,
    dist_m: DistortionModeParam,
    mb_bands: [(DistortionModeParam, bool, bool, f32, f32, f32, f32); 3],
}

/// Swaps all settings, EQ sub-tabs, and modulation assignments between two slots of `bus_idx`.
fn swap_fx_slots(
    params: &Arc<JSynPluginParams>,
    gui_context: &dyn GuiContext,
    mod_ui: &ModUiState,
    bus_idx: usize,
    slot_a: usize,
    slot_b: usize,
) {
    if slot_a == slot_b || slot_a >= 8 || slot_b >= 8 {
        return;
    }

    let slots = params.fx_bus_slots(bus_idx);
    let capture = |idx: usize| -> SlotSnapshot {
        let s = &slots[idx];
        SlotSnapshot {
            en: s.enabled.value(),
            ty: s.fx_type.value(),
            stk: s.stack.value(),
            p_main: [s.p1.value(), s.p2.value(), s.p3.value(), s.p4.value()],
            p_eq: [s.p5.value(), s.p6.value(), s.p7.value(), s.p8.value()],
            flt_m: s.filter_mode.value(),
            flt_s: s.filter_slope.value(),
            diop_s: s.diopser_stages.value(),
            tr_flt: s.tube_res_filter_enabled.value(),
            util_inv_l: s.utility_inv_l.value(),
            util_inv_r: s.utility_inv_r.value(),
            lim_tp: s.limiter_true_peak.value(),
            dist_m: s.distortion_mode.value(),
            mb_bands: std::array::from_fn(|b| {
                let mb = &s.mb_bands[b];
                (
                    mb.mode.value(),
                    mb.enabled.value(),
                    mb.solo.value(),
                    mb.drive.value(),
                    mb.bias.value(),
                    mb.level.value(),
                    mb.mix.value(),
                )
            }),
        }
    };
    let a = capture(slot_a);
    let b = capture(slot_b);

    let setter = ParamSetter::new(gui_context);
    let write = |idx: usize, snap: SlotSnapshot| {
        let s = &slots[idx];
        setter.begin_set_parameter(&s.enabled);
        setter.set_parameter(&s.enabled, snap.en);
        setter.end_set_parameter(&s.enabled);

        setter.begin_set_parameter(&s.fx_type);
        setter.set_parameter(&s.fx_type, snap.ty);
        setter.end_set_parameter(&s.fx_type);

        setter.begin_set_parameter(&s.stack);
        setter.set_parameter(&s.stack, snap.stk);
        setter.end_set_parameter(&s.stack);

        for (k, param) in s.params_array().into_iter().enumerate() {
            setter.begin_set_parameter(param);
            setter.set_parameter(param, snap.p_main[k]);
            setter.end_set_parameter(param);
        }
        for (k, param) in s.eq_params_array().into_iter().enumerate() {
            setter.begin_set_parameter(param);
            setter.set_parameter(param, snap.p_eq[k]);
            setter.end_set_parameter(param);
        }

        setter.begin_set_parameter(&s.filter_mode);
        setter.set_parameter(&s.filter_mode, snap.flt_m);
        setter.end_set_parameter(&s.filter_mode);

        setter.begin_set_parameter(&s.filter_slope);
        setter.set_parameter(&s.filter_slope, snap.flt_s);
        setter.end_set_parameter(&s.filter_slope);

        setter.begin_set_parameter(&s.diopser_stages);
        setter.set_parameter(&s.diopser_stages, snap.diop_s);
        setter.end_set_parameter(&s.diopser_stages);

        setter.begin_set_parameter(&s.tube_res_filter_enabled);
        setter.set_parameter(&s.tube_res_filter_enabled, snap.tr_flt);
        setter.end_set_parameter(&s.tube_res_filter_enabled);

        setter.begin_set_parameter(&s.utility_inv_l);
        setter.set_parameter(&s.utility_inv_l, snap.util_inv_l);
        setter.end_set_parameter(&s.utility_inv_l);

        setter.begin_set_parameter(&s.utility_inv_r);
        setter.set_parameter(&s.utility_inv_r, snap.util_inv_r);
        setter.end_set_parameter(&s.utility_inv_r);

        setter.begin_set_parameter(&s.limiter_true_peak);
        setter.set_parameter(&s.limiter_true_peak, snap.lim_tp);
        setter.end_set_parameter(&s.limiter_true_peak);

        setter.begin_set_parameter(&s.distortion_mode);
        setter.set_parameter(&s.distortion_mode, snap.dist_m);
        setter.end_set_parameter(&s.distortion_mode);

        for (b, (m, en, solo, drv, bias, lvl, mix)) in snap.mb_bands.into_iter().enumerate() {
            let mb = &s.mb_bands[b];
            setter.begin_set_parameter(&mb.mode);
            setter.set_parameter(&mb.mode, m);
            setter.end_set_parameter(&mb.mode);
            setter.begin_set_parameter(&mb.enabled);
            setter.set_parameter(&mb.enabled, en);
            setter.end_set_parameter(&mb.enabled);
            setter.begin_set_parameter(&mb.solo);
            setter.set_parameter(&mb.solo, solo);
            setter.end_set_parameter(&mb.solo);
            for (fp, val) in [
                (&mb.drive, drv),
                (&mb.bias, bias),
                (&mb.level, lvl),
                (&mb.mix, mix),
            ] {
                setter.begin_set_parameter(fp);
                setter.set_parameter(fp, val);
                setter.end_set_parameter(fp);
            }
        }
    };

    write(slot_a, b);
    write(slot_b, a);
    mod_ui.swap_fx_eq_subtab(bus_idx, slot_a, slot_b);
    mod_ui.swap_fx_mb_tab(bus_idx, slot_a, slot_b);
    swap_mod_destinations_for_slots(params, bus_idx, slot_a, slot_b);
}

fn set_fx_slot_type_with_defaults(
    params: &JSynPluginParams,
    gui_context: &Arc<dyn GuiContext>,
    mod_ui: &ModUiState,
    bus_idx: usize,
    slot_idx: usize,
    val: FxTypeParam,
) {
    let setter = ParamSetter::new(gui_context.as_ref());
    let slot = &params.fx_bus_slots(bus_idx)[slot_idx];
    let prev = slot.fx_type.value();
    setter.begin_set_parameter(&slot.fx_type);
    setter.set_parameter(&slot.fx_type, val);
    setter.end_set_parameter(&slot.fx_type);
    if prev != val {
        let ty: FxType = val.into();
        let defs = j_syn_dsp::FxSlotParams::default_params_for_type(ty);
        for (p, def_v) in slot.params_array().into_iter().zip(defs.into_iter()) {
            setter.begin_set_parameter(p);
            setter.set_parameter(p, def_v);
            setter.end_set_parameter(p);
        }
        let eq_defs = j_syn_dsp::FxSlotParams::default_eq_params_for_type(ty);
        for (p, def_v) in slot.eq_params_array().into_iter().zip(eq_defs.into_iter()) {
            setter.begin_set_parameter(p);
            setter.set_parameter(p, def_v);
            setter.end_set_parameter(p);
        }
        setter.begin_set_parameter(&slot.tube_res_filter_enabled);
        setter.set_parameter(&slot.tube_res_filter_enabled, true);
        setter.end_set_parameter(&slot.tube_res_filter_enabled);
        setter.begin_set_parameter(&slot.utility_inv_l);
        setter.set_parameter(&slot.utility_inv_l, false);
        setter.end_set_parameter(&slot.utility_inv_l);
        setter.begin_set_parameter(&slot.utility_inv_r);
        setter.set_parameter(&slot.utility_inv_r, false);
        setter.end_set_parameter(&slot.utility_inv_r);
        setter.begin_set_parameter(&slot.limiter_true_peak);
        setter.set_parameter(&slot.limiter_true_peak, false);
        setter.end_set_parameter(&slot.limiter_true_peak);
        for mb in &slot.mb_bands {
            setter.begin_set_parameter(&mb.mode);
            setter.set_parameter(&mb.mode, DistortionModeParam::Tube);
            setter.end_set_parameter(&mb.mode);
            setter.begin_set_parameter(&mb.enabled);
            setter.set_parameter(&mb.enabled, true);
            setter.end_set_parameter(&mb.enabled);
            setter.begin_set_parameter(&mb.solo);
            setter.set_parameter(&mb.solo, false);
            setter.end_set_parameter(&mb.solo);
            for (fp, val) in [
                (&mb.drive, 0.30),
                (&mb.bias, 0.50),
                (&mb.level, MB_BAND_LEVEL_DEFAULT_NORM),
                (&mb.mix, 1.00),
            ] {
                setter.begin_set_parameter(fp);
                setter.set_parameter(fp, val);
                setter.end_set_parameter(fp);
            }
        }
        mod_ui.set_fx_eq_subtab(bus_idx, slot_idx, false);
        mod_ui.set_fx_mb_tab(bus_idx, slot_idx, 0);
    }
}

fn build_fx_type_dropdown(
    cx: &mut Context,
    bus_idx: usize,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

    let p_scroll = params.clone();
    let g_scroll = gui_context.clone();
    let m_scroll = mod_ui.clone();
    ScrollDropdown::new(
        cx,
        move |_cx, delta| {
            let cur = p_scroll.fx_bus_slots(bus_idx)[slot_idx].fx_type.value();
            let pos = FX_TYPE_ITEMS
                .iter()
                .position(|&(_, v)| v == cur)
                .unwrap_or(0);
            let next = (pos as isize + delta).clamp(0, FX_TYPE_ITEMS.len() as isize - 1) as usize;
            if next != pos {
                set_fx_slot_type_with_defaults(
                    &p_scroll,
                    &g_scroll,
                    &m_scroll,
                    bus_idx,
                    slot_idx,
                    FX_TYPE_ITEMS[next].1,
                );
            }
        },
        move |cx| {
            Dropdown::new(
                cx,
                move |cx| {
                    let text_lens = ParamWidgetBase::make_lens(
                        UiData::params,
                        move |p| &p.fx_bus_slots(bus_idx)[slot_idx].fx_type,
                        |param| {
                            param.normalized_value_to_string(
                                param.modulated_normalized_value(),
                                true,
                            )
                        },
                    );
                    HStack::new(cx, move |cx| {
                        build_dropdown_trigger_labels(cx, text_lens);
                    })
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_top(Pixels(0.0))
                    .child_bottom(Pixels(0.0))
                    .overflow(Overflow::Hidden)
                },
                move |cx| {
                    for &(label, val) in FX_TYPE_ITEMS {
                        let p_ref = params.clone();
                        let gui_ctx = gui_context.clone();
                        let m_ref = mod_ui.clone();
                        bold_label(cx, label, "dropdown-item").on_press(move |cx| {
                            set_fx_slot_type_with_defaults(
                                &p_ref,
                                &gui_ctx,
                                &m_ref,
                                bus_idx,
                                slot_idx,
                                val,
                            );
                            cx.emit(PopupEvent::Close);
                        });
                    }
                },
            )
        },
    )
    .width(Stretch(1.0))
    .height(Pixels(tokens::FX_UNIT_HEADER_H));
}

fn build_fx_slot_header(
    cx: &mut Context,
    bus_idx: usize,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    HStack::new(cx, move |cx| {
        LedToggleSwitch::new(
            cx,
            UiData::params,
            move |p| &p.fx_bus_slots(bus_idx)[slot_idx].enabled,
            "OFF",
            "ON",
            AMBER_RGB,
        )
        .width(Pixels(tokens::FX_UNIT_LED_W))
        .height(Pixels(tokens::FX_UNIT_ARROW_H))
        .top(Stretch(1.0))
        .bottom(Stretch(1.0));

        build_fx_type_dropdown(
            cx,
            bus_idx,
            slot_idx,
            params.clone(),
            gui_context.clone(),
            mod_ui.clone(),
        );

        HStack::new(cx, move |cx| {
            build_inline_pill_cell(
                cx,
                UiData::params,
                move |p| &p.fx_bus_slots(bus_idx)[slot_idx].stack,
                "STK",
                4,
                1.0,
            )
            .class("stk-pill");
        })
        .width(Pixels(tokens::FX_UNIT_STACK_W))
        .height(Pixels(tokens::FX_UNIT_HEADER_H));

        let p_up = params.clone();
        let g_up = gui_context.clone();
        let m_up = mod_ui.clone();
        ArrowButton::new(
            cx,
            move |cx| {
                if slot_idx > 0 {
                    swap_fx_slots(&p_up, g_up.as_ref(), &m_up, bus_idx, slot_idx, slot_idx - 1);
                    cx.emit(UiEvent::ModUiChanged);
                }
            },
            ArrowDir::Up,
        )
        .class("fx-move-btn")
        .class("fx-move-label")
        .toggle_class("fx-move-disabled", UiData::mod_ui_rev.map(move |_| slot_idx == 0))
        .width(Pixels(tokens::FX_UNIT_ARROW_W))
        .height(Pixels(tokens::FX_UNIT_ARROW_H));

        ArrowButton::new(
            cx,
            move |cx| {
                if slot_idx < 7 {
                    swap_fx_slots(
                        &params,
                        gui_context.as_ref(),
                        &mod_ui,
                        bus_idx,
                        slot_idx,
                        slot_idx + 1,
                    );
                    cx.emit(UiEvent::ModUiChanged);
                }
            },
            ArrowDir::Down,
        )
        .class("fx-move-btn")
        .class("fx-move-label")
        .toggle_class("fx-move-disabled", UiData::mod_ui_rev.map(move |_| slot_idx == 7))
        .width(Pixels(tokens::FX_UNIT_ARROW_W))
        .height(Pixels(tokens::FX_UNIT_ARROW_H));
    })
    .col_between(Pixels(tokens::SPACE_XS))
    .height(Pixels(tokens::FX_UNIT_HEADER_H))
    .width(Stretch(1.0));
}

fn build_fx_scope_zstack(
    cx: &mut Context,
    bus_idx: usize,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
    scope_h: f32,
) {
    ZStack::new(cx, move |cx| {
        let p_vis = params.clone();
        let m_vis = mod_ui.clone();
        FxSlotVisualizer::new(cx, move || slot_scope_snapshot(bus_idx, slot_idx, &p_vis, &m_vis))
            .hoverable(false)
            .width(Stretch(1.0))
            .height(Stretch(1.0));

        // Ghost-translucent floating top-corner scope controls (declared after visualizer)
        let p_top = params.clone();
        let g_top = gui_context.clone();
        let m_top = mod_ui.clone();
        Binding::new(
            cx,
            UiData::params.map(move |p| p.fx_bus_slots(bus_idx)[slot_idx].fx_type.value() as usize),
            move |cx, fx_idx_lens| {
                let fx_ty = FxTypeParam::from(FxType::from_index(fx_idx_lens.get(cx)));
                if fx_ty == FxTypeParam::Off {
                    return;
                }
                let p_ov = p_top.clone();
                let g_ov = g_top.clone();
                let m_ov = m_top.clone();

                HStack::new(cx, move |cx| {
                    let add_util_inv_pill =
                        |cx: &mut Context,
                         label: &'static str,
                         pick: fn(&crate::FxSlotPluginParams) -> &nih_plug::prelude::BoolParam| {
                            let p_u = p_ov.clone();
                            let g_u = g_ov.clone();
                            Button::new(
                                cx,
                                move |cx| {
                                    let setter = ParamSetter::new(g_u.as_ref());
                                    let p = pick(&p_u.fx_bus_slots(bus_idx)[slot_idx]);
                                    let next = !p.value();
                                    setter.begin_set_parameter(p);
                                    setter.set_parameter(p, next);
                                    setter.end_set_parameter(p);
                                    cx.emit(UiEvent::ModUiChanged);
                                },
                                move |cx| bold_label(cx, label, "fx-subtab-lbl").hoverable(false),
                            )
                            .class("fx-scope-pill")
                            .class("fx-scope-flt-btn")
                            .toggle_class(
                                "fx-subtab-active",
                                UiData::params
                                    .map(move |p| pick(&p.fx_bus_slots(bus_idx)[slot_idx]).value()),
                            )
                            .width(Pixels(30.0))
                            .height(Pixels(17.0));
                        };

                    match fx_ty {
                        FxTypeParam::Distortion => {
                            build_enum_dropdown(
                                cx,
                                p_ov.clone(),
                                g_ov.clone(),
                                move |p| &p.fx_bus_slots(bus_idx)[slot_idx].distortion_mode,
                                DISTORTION_MODE_ITEMS,
                            )
                            .class("fx-scope-dd")
                            .width(Pixels(72.0))
                            .height(Pixels(17.0));
                        }
                        FxTypeParam::Filter => {
                            build_enum_dropdown(
                                cx,
                                p_ov.clone(),
                                g_ov.clone(),
                                move |p| &p.fx_bus_slots(bus_idx)[slot_idx].filter_mode,
                                FX_FILTER_MODE_ITEMS,
                            )
                            .class("fx-scope-dd")
                            .width(Pixels(52.0))
                            .height(Pixels(17.0));
                        }
                        FxTypeParam::Diopser => {
                            build_prefixed_enum_dropdown(
                                cx,
                                p_ov.clone(),
                                g_ov.clone(),
                                move |p| &p.fx_bus_slots(bus_idx)[slot_idx].diopser_stages,
                                DIOPSER_STAGE_ITEMS,
                                "STG ",
                            )
                            .class("fx-scope-dd")
                            .width(Pixels(56.0))
                            .height(Pixels(17.0));
                        }
                        FxTypeParam::TubeResonator => {
                            let p_tr = p_ov.clone();
                            let g_tr = g_ov.clone();
                            Button::new(
                                cx,
                                move |cx| {
                                    let setter = ParamSetter::new(g_tr.as_ref());
                                    let p = &p_tr.fx_bus_slots(bus_idx)[slot_idx]
                                        .tube_res_filter_enabled;
                                    let next = !p.value();
                                    setter.begin_set_parameter(p);
                                    setter.set_parameter(p, next);
                                    setter.end_set_parameter(p);
                                    cx.emit(UiEvent::ModUiChanged);
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
                                UiData::params.map(move |p| {
                                    p.fx_bus_slots(bus_idx)[slot_idx]
                                        .tube_res_filter_enabled
                                        .value()
                                }),
                            )
                            .width(Pixels(38.0))
                            .height(Pixels(17.0));
                        }
                        FxTypeParam::Utility => {
                            add_util_inv_pill(cx, "Ø L", |s| &s.utility_inv_l);
                        }
                        FxTypeParam::Limiter => {
                            add_util_inv_pill(cx, "TP", |s| &s.limiter_true_peak);
                        }
                        _ => {}
                    }

                    Element::new(cx).width(Stretch(1.0)).hoverable(false);

                    match fx_ty {
                        FxTypeParam::Filter => {
                            build_enum_dropdown_with_override(
                                cx,
                                p_ov.clone(),
                                g_ov.clone(),
                                move |p| &p.fx_bus_slots(bus_idx)[slot_idx].filter_slope,
                                FX_FILTER_SLOPE_ITEMS,
                                move |p| {
                                    let mode: j_syn_dsp::FilterMode =
                                        p.fx_bus_slots(bus_idx)[slot_idx].filter_mode.value().into();
                                    mode.has_fixed_12db_slope().then_some("12")
                                },
                            )
                            .class("fx-scope-dd")
                            .width(Pixels(36.0))
                            .height(Pixels(17.0));
                        }
                        FxTypeParam::Utility => {
                            add_util_inv_pill(cx, "Ø R", |s| &s.utility_inv_r);
                        }
                        _ => {}
                    }

                    if let Some(sub_lbl) = FxType::from(fx_ty).subtab_label() {
                        let sub_w = if matches!(sub_lbl, "LFO" | "ADV" | "MOD") {
                            26.0
                        } else {
                            22.0
                        };
                        HStack::new(cx, move |cx| {
                            let m_main = m_ov.clone();
                            let m_main_cls = m_ov.clone();
                            Button::new(
                                cx,
                                move |cx| {
                                    m_main.set_fx_eq_subtab(bus_idx, slot_idx, false);
                                    cx.emit(UiEvent::ModUiChanged);
                                },
                                |cx| bold_label(cx, "MAIN", "fx-subtab-lbl").hoverable(false),
                            )
                            .class("fx-subtab-btn")
                            .toggle_class(
                                "fx-subtab-active",
                                UiData::mod_ui_rev.map(move |_| !m_main_cls.fx_eq_subtab(bus_idx, slot_idx)),
                            )
                            .width(Pixels(32.0))
                            .height(Stretch(1.0));

                            let m_eq = m_ov.clone();
                            let m_eq_cls = m_ov.clone();
                            Button::new(
                                cx,
                                move |cx| {
                                    m_eq.set_fx_eq_subtab(bus_idx, slot_idx, true);
                                    cx.emit(UiEvent::ModUiChanged);
                                },
                                move |cx| bold_label(cx, sub_lbl, "fx-subtab-lbl").hoverable(false),
                            )
                            .class("fx-subtab-btn")
                            .toggle_class(
                                "fx-subtab-active",
                                UiData::mod_ui_rev.map(move |_| m_eq_cls.fx_eq_subtab(bus_idx, slot_idx)),
                            )
                            .width(Pixels(sub_w))
                            .height(Stretch(1.0));
                        })
                        .class("fx-scope-pill")
                        .class("fx-scope-subtabs")
                        .col_between(Pixels(2.0))
                        .width(Auto)
                        .height(Pixels(17.0));
                    }
                })
                .col_between(Pixels(4.0))
                .position_type(PositionType::SelfDirected)
                .z_index(10)
                .width(Stretch(1.0))
                .height(Pixels(17.0))
                .top(Pixels(4.0))
                .left(Pixels(4.0))
                .right(Pixels(4.0));
            },
        );
    })
    .width(Stretch(1.0))
    .height(Pixels(scope_h));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FxKnobViewKey {
    fx_type: FxTypeParam,
    is_scream: bool,
    is_eq: bool,
}

impl Data for FxKnobViewKey {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

fn build_fx_knob_row(
    cx: &mut Context,
    bus_idx: usize,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    mod_ui: Arc<ModUiState>,
    knob_sz: f32,
    tab_tag: u8,
) {
    HStack::new(cx, move |cx| {
        for p_idx in 0..4 {
            let p_dim = params.clone();
            let p_key = params.clone();
            let mod_ui = mod_ui.clone();
            let mod_key = mod_ui.clone();
            Binding::new(
                cx,
                UiData::mod_ui_rev.map(move |_| {
                    let s = &p_key.fx_bus_slots(bus_idx)[slot_idx];
                    let fx_type = s.fx_type.value();
                    FxKnobViewKey {
                        fx_type,
                        is_scream: s.filter_mode.value() == FilterModeParam::Scream,
                        is_eq: FxType::from(fx_type).subtab_label().is_some()
                            && mod_key.fx_eq_subtab(bus_idx, slot_idx),
                    }
                }),
                move |cx, key_lens| {
                    let key = key_lens.get(cx);
                    let param_slot = if key.is_eq { 4 + p_idx } else { p_idx };
                    let snap_steps = if key.fx_type == FxTypeParam::TubeResonator
                        && !key.is_eq
                        && p_idx == 0
                    {
                        j_syn_dsp::fx::tube_resonator::TUBE_RES_MIDI_SPAN as u32
                    } else {
                        0
                    };
                    let def_norm = if key.is_eq {
                        j_syn_dsp::FxSlotParams::default_eq_params_for_type(key.fx_type.into())[p_idx]
                    } else {
                        j_syn_dsp::FxSlotParams::default_params_for_type(key.fx_type.into())[p_idx]
                    };
                    let title = fx_slot_param_label(
                        key.fx_type.into(),
                        key.is_eq,
                        key.is_scream,
                        0,
                        p_idx,
                    );
                    let accent = fx_accent_rgb(key.fx_type.into());
                    let p_cell = p_dim.clone();
                    let mod_cell = mod_ui.clone();
                    let formatter = Arc::new(move |norm: f32, _off: Option<f32>| {
                        format_fx_slot_param_value(
                            key.fx_type.into(),
                            key.is_eq,
                            0,
                            p_idx,
                            norm,
                        )
                    });
                    VStack::new(cx, move |cx| {
                        bold_label(cx, title, "knob-title")
                            .width(Stretch(1.0))
                            .height(Pixels(12.0))
                            .child_space(Stretch(1.0));

                        RotaryKnob::new_with_mod_formatter(
                            cx,
                            UiData::params,
                            move |p| p.fx_bus_slots(bus_idx)[slot_idx].param_at(param_slot),
                            false,
                            accent,
                            ModDestination::fx_bus_slot_param(bus_idx, slot_idx, param_slot),
                            mod_cell,
                            tab_tag,
                            formatter,
                        )
                        .default_norm(Some(def_norm))
                        .snap_steps(snap_steps)
                        .enabled_by(p_cell, move |p| slot_enabled_for_bus(p, bus_idx, slot_idx))
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
}

fn set_fx_bus_topology(
    params: &JSynPluginParams,
    gui_context: &Arc<dyn GuiContext>,
    topo: FxBusTopologyParam,
) {
    let setter = ParamSetter::new(gui_context.as_ref());
    setter.begin_set_parameter(&params.fx_bus_topology);
    setter.set_parameter(&params.fx_bus_topology, topo);
    setter.end_set_parameter(&params.fx_bus_topology);
}

#[inline]
fn lane_target_label(target: u8) -> &'static str {
    match target {
        1 => "-> B",
        2 => "-> C",
        _ => "-> MST",
    }
}

fn build_lane_route_dropdown(
    cx: &mut Context,
    bus_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
) {
    const BUS_A_TARGETS: &[(&str, u8)] = &[("-> MST", 0), ("-> B", 1), ("-> C", 2)];
    const BUS_B_TARGETS: &[(&str, u8)] = &[("-> MST", 0), ("-> C", 2)];

    let items: &'static [(&'static str, u8)] = if bus_idx == 0 {
        BUS_A_TARGETS
    } else {
        BUS_B_TARGETS
    };
    let cur_target = move |topo: FxBusTopology| -> u8 {
        if bus_idx == 0 {
            topo.bus_a_target()
        } else {
            topo.bus_b_target()
        }
    };
    let next_topo_for = move |topo: FxBusTopology, target: u8| -> FxBusTopologyParam {
        let next = if bus_idx == 0 {
            FxBusTopology::from_lane_targets(target, topo.bus_b_target())
        } else {
            FxBusTopology::from_lane_targets(topo.bus_a_target(), target)
        };
        FxBusTopologyParam::from(next)
    };

    let p_scroll = params.clone();
    let g_scroll = gui_context.clone();
    let p_trig = params.clone();
    let p_pop = params.clone();
    let g_pop = gui_context;

    ScrollDropdown::new(
        cx,
        move |cx, delta| {
            let cur: FxBusTopology = p_scroll.fx_bus_topology.value().into();
            let target = cur_target(cur);
            let pos = items.iter().position(|&(_, t)| t == target).unwrap_or(0);
            let next = (pos as isize + delta).clamp(0, items.len() as isize - 1) as usize;
            if next != pos {
                set_fx_bus_topology(&p_scroll, &g_scroll, next_topo_for(cur, items[next].1));
                cx.emit(UiEvent::ModUiChanged);
            }
        },
        move |cx| {
            let p_trig = p_trig.clone();
            let p_pop = p_pop.clone();
            let g_pop = g_pop.clone();
            Dropdown::new(
                cx,
                move |cx| {
                    let p_lbl = p_trig.clone();
                    HStack::new(cx, move |cx| {
                        build_dropdown_trigger_labels(
                            cx,
                            UiData::params.map(move |p| {
                                let topo: FxBusTopology = p.fx_bus_topology.value().into();
                                let _ = &p_lbl;
                                lane_target_label(cur_target(topo)).to_string()
                            }),
                        );
                    })
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_top(Pixels(0.0))
                    .child_bottom(Pixels(0.0))
                    .overflow(Overflow::Hidden)
                },
                move |cx| {
                    for &(label, target) in items {
                        let p_set = p_pop.clone();
                        let g_set = g_pop.clone();
                        bold_label(cx, label, "dropdown-item").on_press(move |cx| {
                            let cur: FxBusTopology = p_set.fx_bus_topology.value().into();
                            set_fx_bus_topology(&p_set, &g_set, next_topo_for(cur, target));
                            cx.emit(UiEvent::ModUiChanged);
                            cx.emit(PopupEvent::Close);
                        });
                    }
                },
            )
        },
    )
    .toggle_class(
        "routing-pill-active",
        UiData::params.map(move |p| {
            let topo: FxBusTopology = p.fx_bus_topology.value().into();
            cur_target(topo) != 0
        }),
    )
    .width(Stretch(1.0))
    .height(Pixels(tokens::PILL_H));
}

fn build_lane_header(
    cx: &mut Context,
    bus_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    let title = match bus_idx {
        0 => "BUS A",
        1 => "BUS B",
        _ => "BUS C",
    };
    let (lvl_dest, pan_dest) = match bus_idx {
        0 => (ModDestination::BusALevel, ModDestination::BusAPan),
        1 => (ModDestination::BusBLevel, ModDestination::BusBPan),
        _ => (ModDestination::BusCLevel, ModDestination::BusCPan),
    };

    HStack::new(cx, move |cx| {
        let p_hdr = params.clone();
        let g_hdr = gui_context;
        VStack::new(cx, move |cx| {
            bold_label(cx, title, "module-title")
                .width(Stretch(1.0))
                .height(Pixels(tokens::MACRO_HDR_H))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0));

            if bus_idx < 2 {
                build_lane_route_dropdown(cx, bus_idx, p_hdr, g_hdr);
            } else {
                bold_label(cx, "-> MST", "mini-label")
                    .width(Stretch(1.0))
                    .height(Pixels(tokens::PILL_H))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0));
            }
        })
        .row_between(Pixels(tokens::SPACE_SM))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .width(Pixels(tokens::FX_LANE_ROUTE_COL_W))
        .height(Stretch(1.0));

        Element::new(cx).width(Stretch(1.0));

        let m_lvl = mod_ui.clone();
        HStack::new(cx, move |cx| {
            build_flex_knob_cell_mod(
                cx,
                UiData::params,
                move |p| p.fx_bus_level_param(bus_idx),
                "LEVEL",
                false,
                AMBER_RGB,
                tokens::KNOB_OSC,
                lvl_dest,
                m_lvl,
                0,
            );

            build_flex_knob_cell_mod(
                cx,
                UiData::params,
                move |p| p.fx_bus_pan_param(bus_idx),
                "PAN",
                true,
                GOLD_RGB,
                tokens::KNOB_OSC,
                pan_dest,
                mod_ui,
                0,
            );
        })
        .col_between(Pixels(tokens::SPACE_SM))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .width(Pixels(tokens::FX_LANE_KNOBS_W))
        .height(Stretch(1.0));
    })
    .class("fx-bus-header")
    .col_between(Pixels(tokens::SPACE_SM))
    .height(Pixels(tokens::FX_LANE_HEADER_H))
    .width(Stretch(1.0));
}

fn build_fx_unit_card(
    cx: &mut Context,
    bus_idx: usize,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    let p_dim = params.clone();
    VStack::new(cx, move |cx| {
        build_fx_slot_header(
            cx,
            bus_idx,
            slot_idx,
            params.clone(),
            gui_context.clone(),
            mod_ui.clone(),
        );

        let p_body = params.clone();
        let g_body = gui_context.clone();
        let m_body = mod_ui.clone();
        Binding::new(
            cx,
            UiData::params
                .map(move |p| p.fx_bus_slots(bus_idx)[slot_idx].fx_type.value() as usize),
            move |cx, fx_idx_lens| {
                let fx_type = FxType::from_index(fx_idx_lens.get(cx));
                if fx_type == FxType::Off {
                    return;
                }
                let p_unit = p_body.clone();
                let g_unit = g_body.clone();
                let m_unit = m_body.clone();
                if fx_type == FxType::Utility {
                    #[derive(Clone, Copy)]
                    struct JSynUtilitySlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl UtilityUnitParams<JSynPluginParams> for JSynUtilitySlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn width<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p1
                        }
                        fn pan<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p2
                        }
                        fn gain<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p3
                        }
                        fn mix<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p4
                        }
                        fn inv_l<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].utility_inv_l
                        }
                        fn inv_r<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].utility_inv_r
                        }
                    }
                    let mod_dests = [
                        ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 0),
                        ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 1),
                        ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 2),
                        ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 3),
                    ];
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    build_utility_fx_unit_body(
                        cx,
                        UiData::params,
                        p_unit,
                        g_unit,
                        JSynUtilitySlotParams { bus_idx, slot_idx },
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT,
                    );
                    return;
                }
                if fx_type == FxType::MultibandDist {
                    #[derive(Clone, Copy)]
                    struct JSynMbDistSlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl MultibandDistUnitParams<JSynPluginParams> for JSynMbDistSlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn low_xover<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p1
                        }
                        fn high_xover<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p2
                        }
                        fn out_gain<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p3
                        }
                        fn mix<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p4
                        }
                        fn band_enabled<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].enabled
                        }
                        fn band_solo<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].solo
                        }
                        fn band_mode<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a EnumParam<DistortionModeParam> {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].mode
                        }
                        fn band_drive<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].drive
                        }
                        fn band_bias<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].bias
                        }
                        fn band_level<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].level
                        }
                        fn band_mix<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].mix
                        }
                    }
                    struct JSynMbTabHost {
                        bus_idx: usize,
                        slot_idx: usize,
                        mod_ui: Arc<ModUiState>,
                    }
                    impl MultibandTabHost for JSynMbTabHost {
                        fn mb_tab(&self) -> u8 {
                            self.mod_ui.fx_mb_tab(self.bus_idx, self.slot_idx)
                        }
                        fn set_mb_tab(&self, tab: u8) {
                            self.mod_ui.set_fx_mb_tab(self.bus_idx, self.slot_idx, tab);
                        }
                    }
                    let mod_dests = [
                        ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 0),
                        ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 1),
                        ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 2),
                        ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 3),
                    ];
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    let tab_host = Arc::new(JSynMbTabHost {
                        bus_idx,
                        slot_idx,
                        mod_ui: m_unit.clone(),
                    });
                    build_multiband_dist_fx_unit_body(
                        cx,
                        UiData::params,
                        UiData::mod_ui_rev,
                        p_unit,
                        g_unit,
                        JSynMbDistSlotParams { bus_idx, slot_idx },
                        tab_host,
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT,
                    );
                    return;
                }
                let scope_h = tokens::fx_unit_scope_height(fx_type);
                VStack::new(cx, move |cx| {
                    build_fx_scope_zstack(
                        cx,
                        bus_idx,
                        slot_idx,
                        p_unit.clone(),
                        g_unit,
                        m_unit.clone(),
                        scope_h,
                    );

                    build_fx_knob_row(
                        cx,
                        bus_idx,
                        slot_idx,
                        p_unit,
                        m_unit,
                        tokens::KNOB_FX_UNIT,
                        0,
                    );
                })
                .row_between(Pixels(tokens::SPACE_SM))
                .width(Stretch(1.0))
                .height(Auto);
            },
        );
    })
    .class("hardware-module")
    .class("fx-full-card")
    .toggle_class(
        "module-disabled",
        UiData::mod_ui_rev.map(move |_| !slot_enabled_for_bus(&p_dim, bus_idx, slot_idx)),
    )
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(tokens::SPACE_SM));
}

/// Right Wing: 3-lane scrollable FX Rack (`Bus A`, `Bus B`, `Bus C`, 8 slots each).
pub(crate) fn build_three_lane_fx_rack(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    HStack::new(cx, move |cx| {
        for bus_idx in 0..3 {
            let p_lane = params.clone();
            let g_lane = gui_context.clone();
            let m_lane = mod_ui.clone();
            VStack::new(cx, move |cx| {
                build_lane_header(
                    cx,
                    bus_idx,
                    p_lane.clone(),
                    g_lane.clone(),
                    m_lane.clone(),
                );

                ScrollView::new(cx, 0.0, 0.0, false, true, move |cx| {
                    VStack::new(cx, move |cx| {
                        for slot_idx in 0..8 {
                            build_fx_unit_card(
                                cx,
                                bus_idx,
                                slot_idx,
                                p_lane.clone(),
                                g_lane.clone(),
                                m_lane.clone(),
                            );
                        }
                    })
                    .row_between(Pixels(tokens::SPACE_SM))
                    .child_right(Pixels(tokens::FX_LANE_SCROLL_GUTTER))
                    .width(Stretch(1.0))
                    .height(Auto);
                })
                .width(Stretch(1.0))
                .height(Stretch(1.0));
            })
            .row_between(Pixels(tokens::SPACE_SM))
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        }
    })
    .col_between(Pixels(tokens::SPACE_MD))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}
