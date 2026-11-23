//! Slot swapping, default initialization, and bus-topology helpers for the FX Rack.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::{FxType, MB_BAND_LEVEL_DEFAULT_NORM};
use nih_plug::prelude::{GuiContext, ParamSetter};

use super::super::mod_bar::ModUiState;
use crate::{
    DiopserStagesParam, DistortionModeParam, FilterModeParam, FxBusTopologyParam,
    FxFilterSlopeParam, FxTypeParam, JSynPluginParams,
};

pub(super) const FX_TYPE_ITEMS: &[(&str, FxTypeParam)] = &[
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

#[inline]
pub(super) fn slot_enabled_for_bus(p: &JSynPluginParams, bus_idx: usize, slot_idx: usize) -> bool {
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
    tr_dirty: bool,
    util_inv_l: bool,
    util_inv_r: bool,
    lim_tp: bool,
    clip_pcm: bool,
    dist_m: DistortionModeParam,
    mb_bands: [(DistortionModeParam, bool, bool, f32, f32, f32, f32); 3],
}

/// Swaps all settings, EQ sub-tabs, and modulation assignments between two slots of `bus_idx`.
pub(super) fn swap_fx_slots(
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
            tr_dirty: s.tube_res_dirty_lfo.value(),
            util_inv_l: s.utility_inv_l.value(),
            util_inv_r: s.utility_inv_r.value(),
            lim_tp: s.limiter_true_peak.value(),
            clip_pcm: s.clipper_pcm.value(),
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

        setter.begin_set_parameter(&s.tube_res_dirty_lfo);
        setter.set_parameter(&s.tube_res_dirty_lfo, snap.tr_dirty);
        setter.end_set_parameter(&s.tube_res_dirty_lfo);

        setter.begin_set_parameter(&s.utility_inv_l);
        setter.set_parameter(&s.utility_inv_l, snap.util_inv_l);
        setter.end_set_parameter(&s.utility_inv_l);

        setter.begin_set_parameter(&s.utility_inv_r);
        setter.set_parameter(&s.utility_inv_r, snap.util_inv_r);
        setter.end_set_parameter(&s.utility_inv_r);

        setter.begin_set_parameter(&s.limiter_true_peak);
        setter.set_parameter(&s.limiter_true_peak, snap.lim_tp);
        setter.end_set_parameter(&s.limiter_true_peak);

        setter.begin_set_parameter(&s.clipper_pcm);
        setter.set_parameter(&s.clipper_pcm, snap.clip_pcm);
        setter.end_set_parameter(&s.clipper_pcm);

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
    mod_ui.swap_fx_collapsed(bus_idx, slot_a, slot_b);
    swap_mod_destinations_for_slots(params, bus_idx, slot_a, slot_b);
}

pub(super) fn set_fx_slot_type_with_defaults(
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
        for (p, def_v) in slot.params_array().into_iter().zip(defs) {
            setter.begin_set_parameter(p);
            setter.set_parameter(p, def_v);
            setter.end_set_parameter(p);
        }
        let eq_defs = j_syn_dsp::FxSlotParams::default_eq_params_for_type(ty);
        for (p, def_v) in slot.eq_params_array().into_iter().zip(eq_defs) {
            setter.begin_set_parameter(p);
            setter.set_parameter(p, def_v);
            setter.end_set_parameter(p);
        }
        setter.begin_set_parameter(&slot.tube_res_filter_enabled);
        setter.set_parameter(&slot.tube_res_filter_enabled, true);
        setter.end_set_parameter(&slot.tube_res_filter_enabled);
        setter.begin_set_parameter(&slot.tube_res_dirty_lfo);
        setter.set_parameter(&slot.tube_res_dirty_lfo, true);
        setter.end_set_parameter(&slot.tube_res_dirty_lfo);
        setter.begin_set_parameter(&slot.utility_inv_l);
        setter.set_parameter(&slot.utility_inv_l, false);
        setter.end_set_parameter(&slot.utility_inv_l);
        setter.begin_set_parameter(&slot.utility_inv_r);
        setter.set_parameter(&slot.utility_inv_r, false);
        setter.end_set_parameter(&slot.utility_inv_r);
        setter.begin_set_parameter(&slot.limiter_true_peak);
        setter.set_parameter(&slot.limiter_true_peak, false);
        setter.end_set_parameter(&slot.limiter_true_peak);
        setter.begin_set_parameter(&slot.clipper_pcm);
        setter.set_parameter(&slot.clipper_pcm, true);
        setter.end_set_parameter(&slot.clipper_pcm);
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
        mod_ui.set_fx_collapsed(bus_idx, slot_idx, false);
    }
}

pub(super) fn set_fx_bus_topology(
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
pub(super) fn lane_target_label(target: u8) -> &'static str {
    match target {
        1 => "-> B",
        2 => "-> C",
        _ => "-> MST",
    }
}
