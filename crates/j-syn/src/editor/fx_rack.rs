//! Parallel Dual-Bus Master FX Rack UI (`Bus A` & `Bus B`, `Sub Bus`, `MAIN` Bus Switcher,
//! full `4×4` `FX` view, `ZStack` translucent floating overlays, and `MAIN | EQ` sub-tabs).

use std::sync::Arc;

use j_syn_dsp::filter::cutoff_norm_to_hz;
use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::{
    diopser_freq_hz, flanger_delay_ms, flanger_feedback, flanger_phase_deg, flanger_rate_hz,
    format_tube_res_tune, format_utility_pan, fx_high_cut_hz, fx_low_cut_hz, limiter_ceil_db,
    limiter_gain_db, limiter_lookahead_ms, limiter_release_ms, mb_band_level_db, mb_out_gain_db,
    mb_xover_high_hz, mb_xover_low_hz, tube_res_decay_ms, tube_res_flt_freq_hz,
    tube_res_flt_width_oct, tube_res_lfo_rate_hz, utility_gain_db_from_norm,
    utility_width_from_norm, FxBusTopology, FxRackState, FxType, MB_BAND_LEVEL_DEFAULT_NORM,
};
use nih_plug::prelude::{FloatParam, GuiContext, Param, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{
    bold_label, build_dropdown_trigger_labels, build_enum_dropdown,
    build_enum_dropdown_with_override, build_inline_pill_cell, build_prefixed_enum_dropdown,
    ArrowButton, ArrowDir, ScrollDropdown, ScrollDropdownExt,
};
use super::fx_scopes::{fx_accent_rgb, FxSlotVisualizer, MultibandXoverDragOverlay};
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

const MB_DIST_MODE_ITEMS: &[(&str, DistortionModeParam)] = &[
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

#[inline]
fn slot_enabled_for_bus(p: &JSynPluginParams, bus_idx: usize, slot_idx: usize) -> bool {
    p.fx_bus_slots(bus_idx)[slot_idx].enabled.value()
}

#[inline]
fn fx_slot_param_label(
    fx_ty: FxTypeParam,
    is_eq: bool,
    is_scream: bool,
    mb_tab: u8,
    param_idx: usize,
) -> &'static str {
    if fx_ty == FxTypeParam::MultibandDist {
        return if mb_tab == 0 {
            match param_idx {
                0 => "LOW XOV",
                1 => "HIGH XOV",
                2 => "OUT GN",
                _ => "MIX",
            }
        } else {
            match param_idx {
                0 => "DRIVE",
                1 => "BIAS",
                2 => "LEVEL",
                _ => "MIX",
            }
        };
    }
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
            FxTypeParam::Flanger => match param_idx {
                0 => "FEEDBK",
                1 => "PHASE",
                2 => "LOW CUT",
                _ => "TONE",
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
        FxTypeParam::Flanger => match param_idx {
            0 => "RATE",
            1 => "DEPTH",
            2 => "DELAY",
            _ => "MIX",
        },
        FxTypeParam::MultibandDist => "--",
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
    mb_tab: u8,
    param_idx: usize,
    norm: f32,
) -> String {
    let n = norm.clamp(0.0, 1.0);
    if fx_ty == FxTypeParam::MultibandDist {
        return if mb_tab == 0 {
            match param_idx {
                0 => format_hz(mb_xover_low_hz(n)),
                1 => format_hz(mb_xover_high_hz(n)),
                2 => format!("{:+.1} dB", mb_out_gain_db(n)),
                _ => format!("{:.0}%", n * 100.0),
            }
        } else {
            match param_idx {
                0 | 3 => format!("{:.0}%", n * 100.0),
                1 => format!("{:+.0}%", (n - 0.5) * 200.0),
                _ => format!("{:+.1} dB", mb_band_level_db(n)),
            }
        };
    }
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
        if fx_ty == FxTypeParam::Flanger {
            return match param_idx {
                0 => format!("{:+.0}%", flanger_feedback(n) * 100.0),
                1 => format!("{:.0}°", flanger_phase_deg(n)),
                2 => format_hz(fx_low_cut_hz(n)),
                _ => format_hz(fx_high_cut_hz(n)),
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
        FxTypeParam::Flanger => match param_idx {
            0 => format!("{:.2} Hz", flanger_rate_hz(n)),
            2 => format!("{:.2} ms", flanger_delay_ms(n)),
            _ => format!("{:.0}%", n * 100.0),
        },
        FxTypeParam::MultibandDist => String::new(),
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
    .height(Pixels(18.0));
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
        .width(Pixels(30.0))
        .height(Pixels(16.0))
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
        .width(Pixels(36.0))
        .height(Pixels(tokens::SNAPIN_HEADER_H));

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
        .width(Pixels(13.0))
        .height(Pixels(16.0));

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
        .width(Pixels(13.0))
        .height(Pixels(16.0));
    })
    .col_between(Pixels(tokens::SPACE_XS))
    .height(Pixels(tokens::SNAPIN_HEADER_H))
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

                if fx_ty == FxTypeParam::MultibandDist {
                    MultibandXoverDragOverlay::new(
                        cx,
                        bus_idx,
                        slot_idx,
                        p_ov.clone(),
                        g_ov.clone(),
                        m_ov.clone(),
                    )
                    .position_type(PositionType::SelfDirected)
                    .z_index(5)
                    .top(Pixels(22.0))
                    .left(Pixels(0.0))
                    .right(Pixels(0.0))
                    .bottom(Pixels(0.0));

                    HStack::new(cx, move |cx| {
                        for band in 0..3 {
                            if band > 0 {
                                Element::new(cx).width(Stretch(1.0)).hoverable(false);
                            }
                            let p_band = p_ov.clone();
                            let g_band = g_ov.clone();
                            let m_band = m_ov.clone();
                            HStack::new(cx, move |cx| {
                                let m_act = m_band.clone();
                                let m_cls = m_band.clone();
                                build_enum_dropdown(
                                    cx,
                                    p_band.clone(),
                                    g_band.clone(),
                                    move |p| &p.fx_bus_slots(bus_idx)[slot_idx].mb_bands[band].mode,
                                    MB_DIST_MODE_ITEMS,
                                )
                                .on_activate(move |cx| {
                                    m_act.set_fx_mb_tab(bus_idx, slot_idx, band as u8 + 1);
                                    cx.emit(UiEvent::ModUiChanged);
                                })
                                .class("fx-scope-dd")
                                .toggle_class(
                                    "fx-mb-band-active",
                                    UiData::mod_ui_rev.map(move |_| {
                                        m_cls.fx_mb_tab(bus_idx, slot_idx) == (band as u8 + 1)
                                    }),
                                )
                                .width(Pixels(60.0))
                                .height(Pixels(17.0));

                                let p_s = p_band.clone();
                                let g_s = g_band.clone();
                                Button::new(
                                    cx,
                                    move |cx| {
                                        let slot = &p_s.fx_bus_slots(bus_idx)[slot_idx];
                                        let setter = ParamSetter::new(g_s.as_ref());
                                        let multi_solo = cx.modifiers().contains(Modifiers::SHIFT);
                                        if multi_solo {
                                            let p = &slot.mb_bands[band].solo;
                                            let next = !p.value();
                                            setter.begin_set_parameter(p);
                                            setter.set_parameter(p, next);
                                            setter.end_set_parameter(p);
                                        } else {
                                            let sole_active = slot.mb_bands[band].solo.value()
                                                && (0..3).all(|i| {
                                                    i == band || !slot.mb_bands[i].solo.value()
                                                });
                                            for (i, mb) in slot.mb_bands.iter().enumerate() {
                                                let target = !sole_active && i == band;
                                                if mb.solo.value() != target {
                                                    setter.begin_set_parameter(&mb.solo);
                                                    setter.set_parameter(&mb.solo, target);
                                                    setter.end_set_parameter(&mb.solo);
                                                }
                                            }
                                        }
                                        cx.emit(UiEvent::ModUiChanged);
                                    },
                                    |cx| bold_label(cx, "S", "fx-subtab-lbl").hoverable(false),
                                )
                                .class("fx-scope-pill")
                                .class("fx-scope-flt-btn")
                                .toggle_class(
                                    "fx-subtab-active",
                                    UiData::params.map(move |p| {
                                        p.fx_bus_slots(bus_idx)[slot_idx].mb_bands[band]
                                            .solo
                                            .value()
                                    }),
                                )
                                .width(Pixels(14.0))
                                .height(Pixels(17.0));
                            })
                            .col_between(Pixels(2.0))
                            .width(Auto)
                            .height(Pixels(17.0));
                        }
                    })
                    .position_type(PositionType::SelfDirected)
                    .z_index(10)
                    .width(Stretch(1.0))
                    .height(Pixels(17.0))
                    .top(Pixels(4.0))
                    .left(Pixels(4.0))
                    .right(Pixels(4.0));
                    return;
                }
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
    .height(Stretch(1.0));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FxKnobViewKey {
    fx_type: FxTypeParam,
    is_scream: bool,
    is_eq: bool,
    mb_tab: u8,
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
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
    knob_sz: f32,
    tab_tag: u8,
) {
    let p_side_key = params.clone();
    let m_side_key = mod_ui.clone();
    let p_side_btn = params.clone();
    let g_side_btn = gui_context;
    let m_side_btn = mod_ui.clone();

    HStack::new(cx, move |cx| {
        Binding::new(
            cx,
            UiData::mod_ui_rev.map(move |_| {
                let s = &p_side_key.fx_bus_slots(bus_idx)[slot_idx];
                if s.fx_type.value() == FxTypeParam::MultibandDist {
                    Some(m_side_key.fx_mb_tab(bus_idx, slot_idx))
                } else {
                    None
                }
            }),
            move |cx, side_lens| {
                let Some(mb_tab) = side_lens.get(cx) else {
                    return;
                };
                let p_col = p_side_btn.clone();
                let g_col = g_side_btn.clone();
                let m_col = m_side_btn.clone();
                VStack::new(cx, move |cx| {
                    let m_xov = m_col.clone();
                    let m_xov_cls = m_col.clone();
                    Button::new(
                        cx,
                        move |cx| {
                            m_xov.set_fx_mb_tab(bus_idx, slot_idx, 0);
                            cx.emit(UiEvent::ModUiChanged);
                        },
                        |cx| bold_label(cx, "XOV", "fx-subtab-lbl").hoverable(false),
                    )
                    .class("fx-mb-side-btn")
                    .toggle_class(
                        "fx-subtab-active",
                        UiData::mod_ui_rev.map(move |_| m_xov_cls.fx_mb_tab(bus_idx, slot_idx) == 0),
                    )
                    .width(Pixels(22.0))
                    .height(Pixels(15.0));

                    if mb_tab > 0 {
                        let band = (mb_tab - 1) as usize;
                        let p_on = p_col.clone();
                        let g_on = g_col.clone();
                        Button::new(
                            cx,
                            move |cx| {
                                let setter = ParamSetter::new(g_on.as_ref());
                                let p = &p_on.fx_bus_slots(bus_idx)[slot_idx].mb_bands[band].enabled;
                                let next = !p.value();
                                setter.begin_set_parameter(p);
                                setter.set_parameter(p, next);
                                setter.end_set_parameter(p);
                                cx.emit(UiEvent::ModUiChanged);
                            },
                            |cx| bold_label(cx, "ON", "fx-subtab-lbl").hoverable(false),
                        )
                        .class("fx-mb-side-btn")
                        .toggle_class(
                            "fx-subtab-active",
                            UiData::params.map(move |p| {
                                p.fx_bus_slots(bus_idx)[slot_idx].mb_bands[band].enabled.value()
                            }),
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
                .row_between(Pixels(2.0))
                .width(Pixels(22.0))
                .height(Auto)
                .top(Stretch(1.0))
                .bottom(Stretch(1.0));
            },
        );

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
                        mb_tab: if fx_type == FxTypeParam::MultibandDist {
                            mod_key.fx_mb_tab(bus_idx, slot_idx)
                        } else {
                            0
                        },
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
                    let mb_band = (key.mb_tab > 0 && key.fx_type == FxTypeParam::MultibandDist)
                        .then(|| (key.mb_tab - 1) as usize);
                    let def_norm = if mb_band.is_some() {
                        [0.30, 0.50, MB_BAND_LEVEL_DEFAULT_NORM, 1.00][p_idx]
                    } else if key.is_eq {
                        j_syn_dsp::FxSlotParams::default_eq_params_for_type(key.fx_type.into())[p_idx]
                    } else {
                        j_syn_dsp::FxSlotParams::default_params_for_type(key.fx_type.into())[p_idx]
                    };
                    let title = fx_slot_param_label(
                        key.fx_type,
                        key.is_eq,
                        key.is_scream,
                        key.mb_tab,
                        p_idx,
                    );
                    let accent = fx_accent_rgb(key.fx_type);
                    let p_cell = p_dim.clone();
                    let mod_cell = mod_ui.clone();
                    let formatter = Arc::new(move |norm: f32, _off: Option<f32>| {
                        format_fx_slot_param_value(key.fx_type, key.is_eq, key.mb_tab, p_idx, norm)
                    });
                    VStack::new(cx, move |cx| {
                        bold_label(cx, title, "knob-title")
                            .width(Stretch(1.0))
                            .height(Pixels(12.0))
                            .child_space(Stretch(1.0));

                        let knob = if let Some(band) = mb_band {
                            RotaryKnob::new_with_formatter(
                                cx,
                                UiData::params,
                                move |p| p.fx_bus_slots(bus_idx)[slot_idx].mb_bands[band].param_at(p_idx),
                                p_idx == 1,
                                accent,
                                formatter,
                            )
                        } else {
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
                        };
                        knob.default_norm(Some(def_norm))
                            .snap_steps(snap_steps)
                            .enabled_by(p_cell, move |p| {
                                slot_enabled_for_bus(p, bus_idx, slot_idx)
                                    && mb_band.is_none_or(|b| {
                                        p.fx_bus_slots(bus_idx)[slot_idx].mb_bands[b]
                                            .enabled
                                            .value()
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
            super::mod_bar::FX_VIEW_TAG,
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

#[inline]
fn lane_target_label(target: u8) -> &'static str {
    match target {
        1 => "-> B",
        2 => "-> C",
        _ => "-> MST",
    }
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
    let no_dim: Option<fn(&JSynPluginParams) -> bool> = None;

    HStack::new(cx, move |cx| {
        bold_label(cx, title, "module-title")
            .width(Auto)
            .height(Stretch(1.0))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0));

        // Phase Plant-style inline lane output target switcher (cycles targets cleanly)
        if bus_idx < 2 {
            let p_cyc = params.clone();
            let g_cyc = gui_context.clone();
            Button::new(
                cx,
                move |cx| {
                    let cur: FxBusTopology = p_cyc.fx_bus_topology.value().into();
                    let next_topo = if bus_idx == 0 {
                        let next_a = match cur.bus_a_target() {
                            0 => 1,
                            1 => 2,
                            _ => 0,
                        };
                        FxBusTopology::from_lane_targets(next_a, cur.bus_b_target())
                    } else {
                        let next_b = if cur.bus_b_target() == 0 { 2 } else { 0 };
                        FxBusTopology::from_lane_targets(cur.bus_a_target(), next_b)
                    };
                    set_fx_bus_topology(&p_cyc, &g_cyc, FxBusTopologyParam::from(next_topo));
                    cx.emit(UiEvent::ModUiChanged);
                },
                move |cx| {
                    bold_label(
                        cx,
                        UiData::params.map(move |p| {
                            let topo: FxBusTopology = p.fx_bus_topology.value().into();
                            let t = if bus_idx == 0 {
                                topo.bus_a_target()
                            } else {
                                topo.bus_b_target()
                            };
                            lane_target_label(t).to_string()
                        }),
                        "fx-subtab-lbl",
                    )
                    .hoverable(false)
                },
            )
            .class("fx-scope-pill")
            .toggle_class(
                "routing-pill-active",
                UiData::params.map(move |p| {
                    let topo: FxBusTopology = p.fx_bus_topology.value().into();
                    if bus_idx == 0 {
                        topo.bus_a_target() != 0
                    } else {
                        topo.bus_b_target() != 0
                    }
                }),
            )
            .width(Pixels(40.0))
            .height(Pixels(16.0))
            .top(Stretch(1.0))
            .bottom(Stretch(1.0));
        } else {
            bold_label(cx, "-> MST", "mini-label")
                .width(Pixels(40.0))
                .height(Stretch(1.0))
                .child_space(Stretch(1.0));
        }

        Element::new(cx).width(Stretch(1.0));

        build_compact_bar_knob(
            cx,
            params.clone(),
            move |p| p.fx_bus_level_param(bus_idx),
            "L",
            8.0,
            tokens::KNOB_MINI,
            false,
            AMBER_RGB,
            lvl_dest,
            mod_ui.clone(),
            no_dim,
        );

        build_compact_bar_knob(
            cx,
            params,
            move |p| p.fx_bus_pan_param(bus_idx),
            "P",
            8.0,
            tokens::KNOB_MINI,
            true,
            GOLD_RGB,
            pan_dest,
            mod_ui,
            no_dim,
        );
    })
    .class("fx-bus-header")
    .col_between(Pixels(tokens::SPACE_SM))
    .height(Pixels(24.0))
    .width(Stretch(1.0));
}

fn build_snapin_card(
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

        let p_scope = params.clone();
        let g_scope = gui_context.clone();
        let m_scope = mod_ui.clone();
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

        build_fx_knob_row(
            cx,
            bus_idx,
            slot_idx,
            params,
            gui_context,
            mod_ui,
            tokens::KNOB_SNAPIN,
            super::mod_bar::FX_VIEW_TAG,
        );
    })
    .class("hardware-module")
    .class("fx-full-card")
    .toggle_class(
        "module-disabled",
        UiData::mod_ui_rev.map(move |_| !slot_enabled_for_bus(&p_dim, bus_idx, slot_idx)),
    )
    .width(Stretch(1.0))
    .height(Pixels(tokens::SNAPIN_CARD_H))
    .row_between(Pixels(tokens::SPACE_XS));
}

/// Right Wing: Phase Plant-inspired 3-lane Snapin FX Rack (`Bus A`, `Bus B`, `Bus C`, 8 slots each).
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
                            build_snapin_card(
                                cx,
                                bus_idx,
                                slot_idx,
                                p_lane.clone(),
                                g_lane.clone(),
                                m_lane.clone(),
                            );
                        }
                    })
                    .row_between(Pixels(tokens::SPACE_XS))
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
