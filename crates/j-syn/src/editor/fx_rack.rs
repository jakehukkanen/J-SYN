//! Parallel Dual-Bus Master FX Rack UI (`Bus A` & `Bus B`, `Sub Bus`, `MAIN` Bus Switcher,
//! full `4×4` `FX` view, `ZStack` translucent floating overlays, and `MAIN | EQ` sub-tabs).

use std::sync::Arc;

use j_syn_dsp::filter::cutoff_norm_to_hz;
use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::{
    diopser_freq_hz, format_tube_res_tune, format_utility_pan, fx_high_cut_hz, fx_low_cut_hz,
    limiter_ceil_db, limiter_gain_db, limiter_lookahead_ms, limiter_release_ms, tube_res_decay_ms,
    tube_res_flt_freq_hz, tube_res_flt_width_oct, tube_res_lfo_rate_hz,
    utility_gain_db_from_norm, utility_width_from_norm, FxRackState, FxType,
};
use nih_plug::prelude::{FloatParam, GuiContext, Param, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{
    bold_label, build_dropdown_trigger_labels, build_enum_dropdown,
    build_enum_dropdown_with_override, build_inline_pill_cell, build_prefixed_enum_dropdown,
    ArrowButton, ArrowDir, ScrollDropdown,
};
use super::fx_scopes::{fx_accent_rgb, FxSlotVisualizer, SubClipperScope};
use super::mod_bar::{ModUiState, FX_VIEW_TAG, MAIN_FX_A_TAG, MAIN_FX_B_TAG};
use super::widgets::{KnobDimExt, LedToggleSwitch, RotaryKnob};
use super::{AMBER_RGB, CORAL_RGB, GOLD_RGB, UiData, UiEvent};
use crate::{
    DiopserStagesParam, DistortionModeParam, FilterModeParam, FxBusTopologyParam,
    FxFilterSlopeParam, FxTypeParam, JSynPluginParams,
};

const FX_TYPE_ITEMS: &[(&str, FxTypeParam)] = &[
    ("Empty", FxTypeParam::Off),
    ("OTT", FxTypeParam::Ott),
    ("Distortion", FxTypeParam::Distortion),
    ("Chorus", FxTypeParam::Chorus),
    ("Phaser", FxTypeParam::Phaser),
    ("Delay", FxTypeParam::Delay),
    ("Reverb", FxTypeParam::Reverb),
    ("Clipper", FxTypeParam::JSatClipper),
    ("Filter", FxTypeParam::Filter),
    ("Dispersion", FxTypeParam::Diopser),
    ("Tube Res.", FxTypeParam::TubeResonator),
    ("Utility", FxTypeParam::Utility),
    ("Limiter", FxTypeParam::Limiter),
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
fn sub_bus_enabled(p: &JSynPluginParams) -> bool {
    p.sub_bus.enabled.value()
}

#[inline]
fn slot_enabled_for_bus(p: &JSynPluginParams, bus_idx: usize, slot_idx: usize) -> bool {
    p.fx_bus_slots(bus_idx)[slot_idx].enabled.value()
}

#[inline]
fn main_bus_tag(bus_idx: usize) -> u8 {
    if bus_idx == 0 {
        MAIN_FX_A_TAG
    } else {
        MAIN_FX_B_TAG
    }
}

#[inline]
fn fx_slot_param_label(
    fx_ty: FxTypeParam,
    is_eq: bool,
    is_scream: bool,
    param_idx: usize,
) -> &'static str {
    if is_eq {
        return match fx_ty {
            FxTypeParam::Chorus => match param_idx {
                0 => "LOW CUT",
                1 => "HIGH CUT",
                2 => "SPREAD",
                _ => "FEEDBK",
            },
            FxTypeParam::Delay => match param_idx {
                0 => "LOW CUT",
                1 => "HIGH CUT",
                2 => "PING-PNG",
                _ => "WIDTH",
            },
            FxTypeParam::Reverb => match param_idx {
                0 => "LOW CUT",
                1 => "HIGH CUT",
                2 => "PRE-DLY",
                _ => "WIDTH",
            },
            FxTypeParam::TubeResonator => match param_idx {
                0 => "LFO AMT",
                1 => "LFO SPD",
                2 => "FEEDBK",
                _ => "DAMP",
            },
            FxTypeParam::Limiter => match param_idx {
                0 => "RELEASE",
                1 => "KNEE",
                2 => "LINK",
                _ => "PUNCH",
            },
            _ => "--",
        };
    }

    match fx_ty {
        FxTypeParam::Off => "--",
        FxTypeParam::Ott => match param_idx {
            0 => "DEPTH",
            1 => "TIME",
            2 => "IN GAIN",
            _ => "OUT GN",
        },
        FxTypeParam::Distortion => match param_idx {
            0 => "BIAS",
            1 => "DRIVE",
            2 => "TONE",
            _ => "MIX",
        },
        FxTypeParam::Chorus => match param_idx {
            0 => "RATE",
            1 => "DEPTH",
            2 => "DIM",
            _ => "MIX",
        },
        FxTypeParam::Delay => match param_idx {
            0 => "TIME",
            1 => "FEEDBK",
            2 => "MOD",
            _ => "MIX",
        },
        FxTypeParam::Reverb => match param_idx {
            0 => "SIZE",
            1 => "DECAY",
            2 => "DAMP",
            _ => "MIX",
        },
        FxTypeParam::JSatClipper => match param_idx {
            0 => "DRIVE",
            1 => "CEIL",
            2 => "KNEE",
            _ => "MIX",
        },
        FxTypeParam::Filter => match param_idx {
            0 => "CUTOFF",
            1 => "RES",
            2 => {
                if is_scream {
                    "SCREAM"
                } else {
                    "DRIVE"
                }
            }
            _ => "MIX",
        },
        FxTypeParam::Phaser => match param_idx {
            0 => "RATE",
            1 => "DEPTH",
            2 => "FEEDBK",
            _ => "MIX",
        },
        FxTypeParam::Diopser => match param_idx {
            0 => "FREQ",
            1 => "RES",
            2 => "SPREAD",
            _ => "MIX",
        },
        FxTypeParam::TubeResonator => match param_idx {
            0 => "TUNE",
            1 => "FLT FREQ",
            2 => "FLT WDTH",
            _ => "MIX",
        },
        FxTypeParam::Utility => match param_idx {
            0 => "WIDTH",
            1 => "PAN",
            2 => "GAIN",
            _ => "MIX",
        },
        FxTypeParam::Limiter => match param_idx {
            0 => "GAIN",
            1 => "CEIL",
            2 => "LOOKAHD",
            _ => "MIX",
        },
    }
}

#[inline]
fn format_hz(hz: f32) -> String {
    if hz >= 10_000.0 {
        format!("{:.1} kHz", hz / 1000.0)
    } else if hz >= 1000.0 {
        format!("{:.2} kHz", hz / 1000.0)
    } else {
        format!("{:.0} Hz", hz)
    }
}

#[inline]
fn format_fx_slot_param_value(
    fx_ty: FxTypeParam,
    is_eq: bool,
    param_idx: usize,
    norm: f32,
) -> String {
    let n = norm.clamp(0.0, 1.0);
    if is_eq {
        if fx_ty == FxTypeParam::TubeResonator {
            return match param_idx {
                0 => format!("{:.0}%", n * 100.0),
                1 => format!("{:.2} Hz", tube_res_lfo_rate_hz(n)),
                2 => format!("{:.1} ms", tube_res_decay_ms(n)),
                _ => format!("{:.0}%", n * 100.0),
            };
        }
        if fx_ty == FxTypeParam::Limiter {
            return match param_idx {
                0 => format!("{:.0} ms", limiter_release_ms(n)),
                _ => format!("{:.0}%", n * 100.0),
            };
        }
        return match param_idx {
            0 => format_hz(fx_low_cut_hz(n)),
            1 => format_hz(fx_high_cut_hz(n)),
            2 => match fx_ty {
                FxTypeParam::Reverb => format!("{:.0} ms", n * 250.0),
                FxTypeParam::Chorus => format!("{:.0}%", n * 200.0),
                _ => format!("{:.0}%", n * 100.0),
            },
            _ => match fx_ty {
                FxTypeParam::Chorus => format!("{:.0}%", n * 75.0),
                _ => format!("{:.0}%", n * 200.0),
            },
        };
    }

    match fx_ty {
        FxTypeParam::Off => "--".to_string(),
        FxTypeParam::Ott => match param_idx {
            0 => format!("{:.0}%", n * 100.0),
            1 => format!("{:.2}x", 0.25 * 16.0_f32.powf(n)),
            2 | 3 => format!("{:+.1} dB", -18.0 + n * 36.0),
            _ => String::new(),
        },
        FxTypeParam::Distortion => match param_idx {
            0 | 2 => format!("{:+.0}%", (n - 0.5) * 200.0),
            1 | 3 => format!("{:.0}%", n * 100.0),
            _ => String::new(),
        },
        FxTypeParam::Chorus => match param_idx {
            0 => format!("{:.2} Hz", 0.05 * 100.0_f32.powf(n)),
            _ => format!("{:.0}%", n * 100.0),
        },
        FxTypeParam::Delay => match param_idx {
            0 => FxRackState::format_delay_sync(n).to_string(),
            _ => format!("{:.0}%", n * 100.0),
        },
        FxTypeParam::Reverb => format!("{:.0}%", n * 100.0),
        FxTypeParam::JSatClipper => match param_idx {
            0 => format!("+{:.1} dB", n * 24.0),
            1 => format!("{:.1} dB", -18.0 + n * 18.0),
            _ => format!("{:.0}%", n * 100.0),
        },
        FxTypeParam::Filter => match param_idx {
            0 => format_hz(cutoff_norm_to_hz(n)),
            _ => format!("{:.0}%", n * 100.0),
        },
        FxTypeParam::Phaser => match param_idx {
            0 => format!("{:.2} Hz", 0.05 * 120.0_f32.powf(n)),
            _ => format!("{:.0}%", n * 100.0),
        },
        FxTypeParam::Diopser => match param_idx {
            0 => format_hz(diopser_freq_hz(n)),
            _ => format!("{:.0}%", n * 100.0),
        },
        FxTypeParam::TubeResonator => match param_idx {
            0 => format_tube_res_tune(n),
            1 => format_hz(tube_res_flt_freq_hz(n)),
            2 => format!("{:.2} oct", tube_res_flt_width_oct(n)),
            _ => format!("{:.0}%", n * 100.0),
        },
        FxTypeParam::Utility => match param_idx {
            0 => format!("{:.0}%", utility_width_from_norm(n) * 100.0),
            1 => format_utility_pan(n),
            2 => {
                let db = utility_gain_db_from_norm(n);
                let clean = if db.abs() < 0.05 { 0.0 } else { db };
                if clean == 0.0 {
                    "0.0 dB".to_string()
                } else {
                    format!("{clean:+.1} dB")
                }
            }
            _ => format!("{:.0}%", n * 100.0),
        },
        FxTypeParam::Limiter => match param_idx {
            0 => format!("+{:.1} dB", limiter_gain_db(n)),
            1 => format!("{:.1} dB", limiter_ceil_db(n)),
            2 => {
                let ms = limiter_lookahead_ms(n);
                if ms < 0.05 {
                    "0.0 (Clip)".to_string()
                } else {
                    format!("{ms:.1} ms")
                }
            }
            _ => format!("{:.0}%", n * 100.0),
        },
    }
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
    };

    write(slot_a, b);
    write(slot_b, a);
    mod_ui.swap_fx_eq_subtab(bus_idx, slot_a, slot_b);
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
        mod_ui.set_fx_eq_subtab(bus_idx, slot_idx, false);
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
    .height(Pixels(18.0));
}

fn build_fx_slot_header(
    cx: &mut Context,
    bus_idx: usize,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
    show_reorder: bool,
) {
    let slot_title: &'static str = match slot_idx {
        0 => "FX 1",
        1 => "FX 2",
        2 => "FX 3",
        3 => "FX 4",
        4 => "FX 5",
        5 => "FX 6",
        6 => "FX 7",
        _ => "FX 8",
    };

    HStack::new(cx, move |cx| {
        LedToggleSwitch::new(
            cx,
            UiData::params,
            move |p| &p.fx_bus_slots(bus_idx)[slot_idx].enabled,
            "OFF",
            "ON",
            AMBER_RGB,
        )
        .width(Pixels(if show_reorder { 40.0 } else { 46.0 }))
        .height(Pixels(17.0))
        .top(Stretch(1.0))
        .bottom(Stretch(1.0));

        bold_label(cx, slot_title, "module-title")
            .width(Pixels(25.0))
            .height(Stretch(1.0))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0));

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
        .width(Pixels(if show_reorder { 50.0 } else { 58.0 }))
        .height(Pixels(18.0));

        if show_reorder {
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
            .width(Pixels(16.0))
            .height(Pixels(17.0));

            let p_dn = params.clone();
            let g_dn = gui_context.clone();
            let m_dn = mod_ui.clone();
            ArrowButton::new(
                cx,
                move |cx| {
                    if slot_idx < 7 {
                        swap_fx_slots(&p_dn, g_dn.as_ref(), &m_dn, bus_idx, slot_idx, slot_idx + 1);
                        cx.emit(UiEvent::ModUiChanged);
                    }
                },
                ArrowDir::Down,
            )
            .class("fx-move-btn")
            .class("fx-move-label")
            .toggle_class("fx-move-disabled", UiData::mod_ui_rev.map(move |_| slot_idx == 7))
            .width(Pixels(16.0))
            .height(Pixels(17.0));
        }
    })
    .col_between(Pixels(3.0))
    .height(Pixels(19.0))
    .width(Stretch(1.0));
}

fn build_fx_scope_zstack(
    cx: &mut Context,
    bus_idx: usize,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    ZStack::new(cx, move |cx| {
        FxSlotVisualizer::new(cx, bus_idx, slot_idx, params.clone(), mod_ui.clone())
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
                        let sub_w = if sub_lbl == "LFO" || sub_lbl == "ADV" {
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
    .height(Stretch(1.0));
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
                    let title = fx_slot_param_label(key.fx_type, key.is_eq, key.is_scream, p_idx);
                    let accent = fx_accent_rgb(key.fx_type);
                    let p_cell = p_dim.clone();
                    let mod_cell = mod_ui.clone();
                    let formatter = Arc::new(move |norm: f32, _off: Option<f32>| {
                        format_fx_slot_param_value(key.fx_type, key.is_eq, p_idx, norm)
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

#[allow(clippy::too_many_arguments)]
fn build_compact_bar_knob<P, FMap>(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    params_to_param: FMap,
    title: &'static str,
    title_w: f32,
    knob_sz: f32,
    bipolar: bool,
    accent: (u8, u8, u8),
    dest: ModDestination,
    mod_ui: Arc<ModUiState>,
    dim_check: Option<fn(&JSynPluginParams) -> bool>,
) where
    P: FloatParamTag + 'static,
    FMap: Fn(&Arc<JSynPluginParams>) -> &P + Copy + Send + Sync + 'static,
{
    HStack::new(cx, move |cx| {
        let mut knob = RotaryKnob::new_with_mod(
            cx,
            UiData::params,
            params_to_param,
            bipolar,
            accent,
            dest,
            mod_ui,
            FX_VIEW_TAG,
        );
        if let Some(check) = dim_check {
            knob = knob.enabled_by(params, check);
        }
        knob.width(Pixels(knob_sz))
            .height(Pixels(knob_sz))
            .top(Stretch(1.0))
            .bottom(Stretch(1.0));

        bold_label(cx, title, "knob-title")
            .width(Pixels(title_w))
            .height(Stretch(1.0))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0));
    })
    .col_between(Pixels(4.0))
    .width(Auto)
    .height(Stretch(1.0));
}

trait FloatParamTag: nih_plug::prelude::Param {}
impl FloatParamTag for FloatParam {}

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

fn build_fx_top_banner(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    let no_dim: Option<fn(&JSynPluginParams) -> bool> = None;
    let p_sub = params.clone();
    let p_topo = params;

    HStack::new(cx, move |cx| {
        // Left Half: SUB / DIRECT BUS
        HStack::new(cx, move |cx| {
            LedToggleSwitch::new(
                cx,
                UiData::params,
                |p| &p.sub_bus.enabled,
                "SUB OFF",
                "SUB FX",
                CORAL_RGB,
            )
            .width(Pixels(72.0))
            .height(Pixels(21.0))
            .top(Stretch(1.0))
            .bottom(Stretch(1.0));

            build_compact_bar_knob(
                cx,
                p_sub.clone(),
                |p| &p.sub_bus.cutoff,
                "LP CUTOFF",
                56.0,
                26.0,
                false,
                GOLD_RGB,
                ModDestination::SubCutoff,
                mod_ui.clone(),
                Some(sub_bus_enabled),
            );

            build_compact_bar_knob(
                cx,
                p_sub.clone(),
                |p| &p.sub_bus.drive,
                "CLIP DRIVE",
                60.0,
                26.0,
                false,
                CORAL_RGB,
                ModDestination::SubDrive,
                mod_ui.clone(),
                Some(sub_bus_enabled),
            );

            build_compact_bar_knob(
                cx,
                p_sub.clone(),
                |p| &p.sub_bus.level,
                "LEVEL",
                36.0,
                26.0,
                false,
                AMBER_RGB,
                ModDestination::SubLevel,
                mod_ui.clone(),
                no_dim,
            );

            SubClipperScope::new(cx, p_sub, mod_ui.telemetry.clone())
                .width(Stretch(1.0))
                .min_width(Pixels(56.0))
                .height(Pixels(28.0))
                .top(Stretch(1.0))
                .bottom(Stretch(1.0));
        })
        .class("fx-rack-banner")
        .col_between(Pixels(12.0))
        .height(Stretch(1.0))
        .width(Stretch(1.0));

        // Right Half: BUS ROUTING (Parallel A | B vs Serial A -> B)
        HStack::new(cx, move |cx| {
            bold_label(cx, "BUS ROUTING", "module-title")
                .width(Auto)
                .height(Stretch(1.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0));

            bold_label(
                cx,
                "SUB BYPASSES FX -> DIRECT OUT",
                "fx-rack-sub",
            )
            .width(Auto)
            .height(Stretch(1.0))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0));

            Element::new(cx).width(Stretch(1.0));

            HStack::new(cx, {
                let p_par = p_topo.clone();
                let g_par = gui_context.clone();
                let p_ser = p_topo.clone();
                let g_ser = gui_context;
                move |cx| {
                    Button::new(
                        cx,
                        move |cx| {
                            set_fx_bus_topology(&p_par, &g_par, FxBusTopologyParam::Parallel);
                            cx.emit(UiEvent::ModUiChanged);
                        },
                        |cx| bold_label(cx, "PARALLEL (A | B)", "fx-subtab-lbl").hoverable(false),
                    )
                    .class("fx-subtab-btn")
                    .toggle_class(
                        "fx-subtab-active",
                        UiData::params
                            .map(|p| p.fx_bus_topology.value() == FxBusTopologyParam::Parallel),
                    )
                    .width(Pixels(108.0))
                    .height(Stretch(1.0));

                    Button::new(
                        cx,
                        move |cx| {
                            set_fx_bus_topology(&p_ser, &g_ser, FxBusTopologyParam::Serial);
                            cx.emit(UiEvent::ModUiChanged);
                        },
                        |cx| bold_label(cx, "SERIAL (A -> B)", "fx-subtab-lbl").hoverable(false),
                    )
                    .class("fx-subtab-btn")
                    .toggle_class(
                        "fx-subtab-active",
                        UiData::params
                            .map(|p| p.fx_bus_topology.value() == FxBusTopologyParam::Serial),
                    )
                    .width(Pixels(102.0))
                    .height(Stretch(1.0));
                }
            })
            .class("fx-bus-switch")
            .col_between(Pixels(2.0))
            .width(Auto)
            .height(Pixels(22.0))
            .top(Stretch(1.0))
            .bottom(Stretch(1.0));
        })
        .class("fx-rack-banner")
        .col_between(Pixels(10.0))
        .height(Stretch(1.0))
        .width(Stretch(1.0));
    })
    .col_between(Pixels(10.0))
    .height(Pixels(38.0))
    .width(Stretch(1.0));
}

fn build_bus_column_fx_grid(
    cx: &mut Context,
    bus_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    let title = if bus_idx == 0 { "BUS A (FX 1-8)" } else { "BUS B (FX 1-8)" };
    let (lvl_dest, pan_dest) = if bus_idx == 0 {
        (ModDestination::BusALevel, ModDestination::BusAPan)
    } else {
        (ModDestination::BusBLevel, ModDestination::BusBPan)
    };
    let no_dim: Option<fn(&JSynPluginParams) -> bool> = None;

    VStack::new(cx, move |cx| {
        HStack::new(cx, {
            let p_hdr = params.clone();
            let m_hdr = mod_ui.clone();
            move |cx| {
                bold_label(cx, title, "module-title")
                    .width(Auto)
                    .height(Stretch(1.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0));

                Element::new(cx).width(Stretch(1.0));

                build_compact_bar_knob(
                    cx,
                    p_hdr.clone(),
                    move |p| p.fx_bus_level_param(bus_idx),
                    "LEVEL",
                    36.0,
                    22.0,
                    false,
                    AMBER_RGB,
                    lvl_dest,
                    m_hdr.clone(),
                    no_dim,
                );

                build_compact_bar_knob(
                    cx,
                    p_hdr,
                    move |p| p.fx_bus_pan_param(bus_idx),
                    "PAN",
                    24.0,
                    22.0,
                    true,
                    GOLD_RGB,
                    pan_dest,
                    m_hdr,
                    no_dim,
                );
            }
        })
        .class("fx-bus-header")
        .col_between(Pixels(10.0))
        .height(Pixels(28.0))
        .width(Stretch(1.0));

        // Column-major 2x4 grid: FX 1..4 down the left column, FX 5..8 down the right column.
        HStack::new(cx, move |cx| {
            for col in 0..2 {
                let p_col = params.clone();
                let g_col = gui_context.clone();
                let m_col = mod_ui.clone();
                VStack::new(cx, move |cx| {
                    for row in 0..4 {
                        let slot_idx = col * 4 + row;
                        let p_card = p_col.clone();
                        let g_card = g_col.clone();
                        let m_card = m_col.clone();
                        let p_dim = p_col.clone();
                        VStack::new(cx, move |cx| {
                            build_fx_slot_header(
                                cx,
                                bus_idx,
                                slot_idx,
                                p_card.clone(),
                                g_card.clone(),
                                m_card.clone(),
                                true,
                            );

                            build_fx_scope_zstack(
                                cx,
                                bus_idx,
                                slot_idx,
                                p_card.clone(),
                                g_card,
                                m_card.clone(),
                            );

                            build_fx_knob_row(
                                cx,
                                bus_idx,
                                slot_idx,
                                p_card,
                                m_card,
                                27.0,
                                FX_VIEW_TAG,
                            );
                        })
                        .class("hardware-module")
                        .class("fx-full-card")
                        .toggle_class(
                            "module-disabled",
                            UiData::mod_ui_rev
                                .map(move |_| !slot_enabled_for_bus(&p_dim, bus_idx, slot_idx)),
                        )
                        .width(Stretch(1.0))
                        .height(Stretch(1.0))
                        .row_between(Pixels(3.0));
                    }
                })
                .row_between(Pixels(4.0))
                .width(Stretch(1.0))
                .height(Stretch(1.0));
            }
        })
        .col_between(Pixels(4.0))
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })
    .row_between(Pixels(4.0))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}

/// Builds the compact `FX 1`–`FX 4` column on the right side of `MAIN` with a `BUS A | BUS B` switcher.
pub(crate) fn build_main_fx_column(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    VStack::new(cx, move |cx| {
        HStack::new(cx, |cx| {
            bold_label(cx, "MASTER FX", "module-title")
                .width(Auto)
                .height(Stretch(1.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0));

            Element::new(cx).width(Stretch(1.0));

            HStack::new(cx, |cx| {
                Button::new(
                    cx,
                    |cx| cx.emit(UiEvent::SelectMainFxBus(0)),
                    |cx| bold_label(cx, "BUS A", "fx-subtab-lbl").hoverable(false),
                )
                .class("fx-subtab-btn")
                .toggle_class("fx-subtab-active", UiData::main_fx_bus.map(|&b| b == 0))
                .width(Pixels(42.0))
                .height(Stretch(1.0));

                Button::new(
                    cx,
                    |cx| cx.emit(UiEvent::SelectMainFxBus(1)),
                    |cx| bold_label(cx, "BUS B", "fx-subtab-lbl").hoverable(false),
                )
                .class("fx-subtab-btn")
                .toggle_class("fx-subtab-active", UiData::main_fx_bus.map(|&b| b == 1))
                .width(Pixels(42.0))
                .height(Stretch(1.0));
            })
            .class("fx-bus-switch")
            .col_between(Pixels(2.0))
            .width(Auto)
            .height(Pixels(18.0));
        })
        .class("fx-bus-header")
        .height(Pixels(22.0))
        .width(Stretch(1.0));

        Binding::new(cx, UiData::main_fx_bus, move |cx, bus_lens| {
            let bus_idx = bus_lens.get(cx).min(1);
            let tag = main_bus_tag(bus_idx);
            let p_bus = params.clone();
            let g_bus = gui_context.clone();
            let m_bus = mod_ui.clone();
            VStack::new(cx, move |cx| {
                for slot_idx in 0..4 {
                    let p_card = p_bus.clone();
                    let g_card = g_bus.clone();
                    let m_card = m_bus.clone();
                    let p_dim = p_bus.clone();
                    VStack::new(cx, move |cx| {
                        build_fx_slot_header(
                            cx,
                            bus_idx,
                            slot_idx,
                            p_card.clone(),
                            g_card.clone(),
                            m_card.clone(),
                            false,
                        );

                        let p_scope = p_card.clone();
                        let g_scope = g_card.clone();
                        let m_scope = m_card.clone();
                        Binding::new(
                            cx,
                            UiData::params
                                .map(move |p| p.fx_bus_slots(bus_idx)[slot_idx].fx_type.value() as usize),
                            move |cx, fx_idx_lens| {
                                if FxType::from_index(fx_idx_lens.get(cx)) != FxType::Off {
                                    build_fx_scope_zstack(
                                        cx,
                                        bus_idx,
                                        slot_idx,
                                        p_scope.clone(),
                                        g_scope.clone(),
                                        m_scope.clone(),
                                    );
                                }
                            },
                        );

                        build_fx_knob_row(cx, bus_idx, slot_idx, p_card, m_card, 32.0, tag);
                    })
                    .class("hardware-module")
                    .class("fx-compact-card")
                    .toggle_class(
                        "module-disabled",
                        UiData::mod_ui_rev.map(move |_| !slot_enabled_for_bus(&p_dim, bus_idx, slot_idx)),
                    )
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .row_between(Pixels(3.0));
                }
            })
            .row_between(Pixels(4.0))
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        });
    })
    .row_between(Pixels(4.0))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}

/// Builds the full-screen `4×4` Parallel Dual-Bus `FX` view + top `SUB BUS` banner (`MainViewMode::Fx`).
pub(crate) fn build_fx_rack_view(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    VStack::new(cx, move |cx| {
        build_fx_top_banner(cx, params.clone(), gui_context.clone(), mod_ui.clone());

        HStack::new(cx, move |cx| {
            build_bus_column_fx_grid(
                cx,
                0,
                params.clone(),
                gui_context.clone(),
                mod_ui.clone(),
            );
            build_bus_column_fx_grid(cx, 1, params, gui_context, mod_ui);
        })
        .col_between(Pixels(10.0))
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })
    .row_between(Pixels(4.0))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}
