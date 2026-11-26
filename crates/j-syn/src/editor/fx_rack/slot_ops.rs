//! Modular FX Unit operations: add, insert, replace, remove, intra-lane reorder
//! (`FxRackOrder` splice), cross-lane move, Alt+Drag duplicate (with `ModMatrix` cloning),
//! reset, and lane bulk actions.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::{ModDestination, ModMatrixSource, ModSlot};
use j_syn_dsp::{FxType, MB_BAND_LEVEL_DEFAULT_NORM, NUM_FX_BUSES, NUM_FX_SLOTS};
use nih_plug::prelude::{GuiContext, ParamSetter};

use super::super::mod_bar::ModUiState;
use crate::{
    DiopserStagesParam, DistortionModeParam, FilterModeParam, FxBusTopologyParam,
    FxFilterSlopeParam, FxSlotPluginParams, FxTypeParam, JSynPluginParams,
};

pub(crate) struct FxPickerCategory {
    pub title: &'static str,
    pub items: &'static [(&'static str, FxTypeParam)],
}

pub(crate) const FX_PICKER_LEFT_COL: &[FxPickerCategory] = &[
    FxPickerCategory {
        title: "DYNAMICS",
        items: &[
            ("OTT", FxTypeParam::Ott),
            ("Limiter", FxTypeParam::Limiter),
        ],
    },
    FxPickerCategory {
        title: "DISTORTION",
        items: &[
            ("Distortion", FxTypeParam::Distortion),
            ("MB Dist", FxTypeParam::MultibandDist),
        ],
    },
    FxPickerCategory {
        title: "UTILITY",
        items: &[
            ("Utility", FxTypeParam::Utility),
            ("Clipper", FxTypeParam::JSatClipper),
        ],
    },
];

pub(crate) const FX_PICKER_RIGHT_COL: &[FxPickerCategory] = &[
    FxPickerCategory {
        title: "MODULATION",
        items: &[
            ("Chorus", FxTypeParam::Chorus),
            ("Flanger", FxTypeParam::Flanger),
            ("Phaser", FxTypeParam::Phaser),
        ],
    },
    FxPickerCategory {
        title: "TIME & SPACE",
        items: &[
            ("Delay", FxTypeParam::Delay),
            ("Reverb", FxTypeParam::Reverb),
        ],
    },
    FxPickerCategory {
        title: "FILTER & TONE",
        items: &[
            ("Filter", FxTypeParam::Filter),
            ("Dispersion", FxTypeParam::Diopser),
            ("Tube Res.", FxTypeParam::TubeResonator),
        ],
    },
];

#[inline]
pub(crate) fn slot_enabled_for_bus(p: &JSynPluginParams, bus_idx: usize, slot_idx: usize) -> bool {
    p.fx_bus_slots(bus_idx)[slot_idx].enabled.value()
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

fn capture_slot(s: &FxSlotPluginParams) -> SlotSnapshot {
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
}

fn write_slot(setter: &ParamSetter, s: &FxSlotPluginParams, snap: SlotSnapshot) {
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
}

fn write_slot_defaults_for_type(
    setter: &ParamSetter,
    slot: &FxSlotPluginParams,
    val: FxTypeParam,
    reset_stack_and_enable: bool,
) {
    let ty: FxType = val.into();
    if reset_stack_and_enable {
        setter.begin_set_parameter(&slot.enabled);
        setter.set_parameter(&slot.enabled, true);
        setter.end_set_parameter(&slot.enabled);

        setter.begin_set_parameter(&slot.stack);
        setter.set_parameter(&slot.stack, 1);
        setter.end_set_parameter(&slot.stack);
    }

    setter.begin_set_parameter(&slot.fx_type);
    setter.set_parameter(&slot.fx_type, val);
    setter.end_set_parameter(&slot.fx_type);

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
    setter.begin_set_parameter(&slot.filter_mode);
    setter.set_parameter(&slot.filter_mode, FilterModeParam::Lp);
    setter.end_set_parameter(&slot.filter_mode);
    setter.begin_set_parameter(&slot.filter_slope);
    setter.set_parameter(&slot.filter_slope, FxFilterSlopeParam::Db12);
    setter.end_set_parameter(&slot.filter_slope);
    setter.begin_set_parameter(&slot.diopser_stages);
    setter.set_parameter(&slot.diopser_stages, DiopserStagesParam::S32);
    setter.end_set_parameter(&slot.diopser_stages);
    setter.begin_set_parameter(&slot.distortion_mode);
    setter.set_parameter(&slot.distortion_mode, DistortionModeParam::Tube);
    setter.end_set_parameter(&slot.distortion_mode);
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
}

fn clear_mod_routes_for_slot(params: &JSynPluginParams, bus: usize, slot: usize) {
    let mut mat = params.mod_matrix.snapshot();
    let mut changed = false;
    for s in &mut mat.slots {
        if let Some((b, sl, _)) = s.destination.fx_slot_param_coords() {
            if b == bus && sl == slot {
                *s = ModSlot::default();
                changed = true;
            }
        }
    }
    if changed {
        mat.rebuild_index();
        params.mod_matrix.store_value(mat);
    }
}

fn remap_mod_routes(
    params: &JSynPluginParams,
    from_bus: usize,
    from_slot: usize,
    to_bus: usize,
    to_slot: usize,
) {
    let mut mat = params.mod_matrix.snapshot();
    let mut changed = false;
    for s in &mut mat.slots {
        if let Some((b, sl, p)) = s.destination.fx_slot_param_coords() {
            if b == from_bus && sl == from_slot {
                s.destination = ModDestination::fx_bus_slot_param(to_bus, to_slot, p);
                changed = true;
            }
        }
    }
    if changed {
        mat.rebuild_index();
        params.mod_matrix.store_value(mat);
    }
}

fn clone_mod_routes(
    params: &JSynPluginParams,
    from_bus: usize,
    from_slot: usize,
    to_bus: usize,
    to_slot: usize,
) {
    let mut mat = params.mod_matrix.snapshot();
    let to_clone: Vec<(ModSlot, usize)> = mat
        .slots
        .iter()
        .filter_map(|s| {
            if !s.enabled || s.source == ModMatrixSource::None {
                return None;
            }
            let (b, sl, p) = s.destination.fx_slot_param_coords()?;
            if b == from_bus && sl == from_slot {
                Some((*s, p))
            } else {
                None
            }
        })
        .collect();
    if to_clone.is_empty() {
        return;
    }
    let mut changed = false;
    for (src_slot, p) in to_clone {
        if let Some(empty) = mat
            .slots
            .iter_mut()
            .find(|s| !s.enabled || s.source == ModMatrixSource::None)
        {
            let mut cloned = src_slot;
            cloned.destination = ModDestination::fx_bus_slot_param(to_bus, to_slot, p);
            *empty = cloned;
            changed = true;
        } else {
            break;
        }
    }
    if changed {
        mat.rebuild_index();
        params.mod_matrix.store_value(mat);
    }
}

/// Adds a new FX unit of type `val` into `bus` at visual insertion index `insert_pos`.
pub(crate) fn add_fx_at(
    params: &JSynPluginParams,
    gui_context: &dyn GuiContext,
    mod_ui: &ModUiState,
    bus: usize,
    insert_pos: usize,
    val: FxTypeParam,
) -> Option<usize> {
    if bus >= NUM_FX_BUSES || val == FxTypeParam::Off {
        return None;
    }
    let mut order = params.reconciled_fx_rack_order();
    let slot_idx = order.first_free_slot(bus)?;
    let setter = ParamSetter::new(gui_context);
    write_slot_defaults_for_type(&setter, &params.fx_bus_slots(bus)[slot_idx], val, true);
    clear_mod_routes_for_slot(params, bus, slot_idx);
    mod_ui.set_fx_eq_subtab(bus, slot_idx, false);
    mod_ui.set_fx_mb_tab(bus, slot_idx, 0);
    mod_ui.set_fx_collapsed(bus, slot_idx, false);
    order.insert_at(bus, insert_pos, slot_idx);
    params.fx_rack_order.store_value(order);
    mod_ui
        .fx_swap_queue
        .provision_slot(bus, slot_idx, FxType::from(val), 1);
    Some(slot_idx)
}

/// Replaces the effect in `(bus, slot)` while keeping its exact visual position in the lane.
pub(crate) fn replace_fx_slot(
    params: &JSynPluginParams,
    gui_context: &dyn GuiContext,
    mod_ui: &ModUiState,
    bus: usize,
    slot: usize,
    val: FxTypeParam,
) {
    if bus >= NUM_FX_BUSES || slot >= NUM_FX_SLOTS || val == FxTypeParam::Off {
        return;
    }
    let setter = ParamSetter::new(gui_context);
    let slot_params = &params.fx_bus_slots(bus)[slot];
    write_slot_defaults_for_type(&setter, slot_params, val, true);
    clear_mod_routes_for_slot(params, bus, slot);
    mod_ui.set_fx_eq_subtab(bus, slot, false);
    mod_ui.set_fx_mb_tab(bus, slot, 0);
    mod_ui.set_fx_collapsed(bus, slot, false);
    mod_ui
        .fx_swap_queue
        .provision_slot(bus, slot, FxType::from(val), 1);
}

/// Resets an existing FX unit's parameters back to defaults for its current `FxType`.
pub(crate) fn reset_fx_slot_defaults(
    params: &JSynPluginParams,
    gui_context: &dyn GuiContext,
    mod_ui: &ModUiState,
    bus: usize,
    slot: usize,
) {
    if bus >= NUM_FX_BUSES || slot >= NUM_FX_SLOTS {
        return;
    }
    let slot_params = &params.fx_bus_slots(bus)[slot];
    let cur = slot_params.fx_type.value();
    if cur == FxTypeParam::Off {
        return;
    }
    let setter = ParamSetter::new(gui_context);
    write_slot_defaults_for_type(&setter, slot_params, cur, false);
    mod_ui.set_fx_eq_subtab(bus, slot, false);
    mod_ui.set_fx_mb_tab(bus, slot, 0);
}

/// Removes an FX unit from `bus`, freeing its backing slot, clearing its mod routes, and retiring its engine.
pub(crate) fn remove_fx_slot(
    params: &JSynPluginParams,
    gui_context: &dyn GuiContext,
    mod_ui: &ModUiState,
    bus: usize,
    slot: usize,
) {
    if bus >= NUM_FX_BUSES || slot >= NUM_FX_SLOTS {
        return;
    }
    let mut order = params.reconciled_fx_rack_order();
    order.remove_slot(bus, slot);
    params.fx_rack_order.store_value(order);

    let setter = ParamSetter::new(gui_context);
    write_slot_defaults_for_type(&setter, &params.fx_bus_slots(bus)[slot], FxTypeParam::Off, true);
    clear_mod_routes_for_slot(params, bus, slot);
    mod_ui.set_fx_eq_subtab(bus, slot, false);
    mod_ui.set_fx_mb_tab(bus, slot, 0);
    mod_ui.set_fx_collapsed(bus, slot, false);
    mod_ui.fx_swap_queue.provision_slot(bus, slot, FxType::Off, 0);
}

/// Reorders a card within the same lane by mutating ONLY `FxRackOrder` (zero param copies,
/// zero ModMatrix remapping, DAW automation preserved!).
pub(crate) fn reorder_fx_within_lane(
    params: &JSynPluginParams,
    bus: usize,
    from_pos: usize,
    to_pos: usize,
) -> bool {
    let mut order = params.reconciled_fx_rack_order();
    if order.move_within_lane(bus, from_pos, to_pos) {
        params.fx_rack_order.store_value(order);
        true
    } else {
        false
    }
}

/// Moves an FX unit from `(from_bus, from_slot)` to `(to_bus, to_pos)`, copying all parameters,
/// `STK`, UI state, and remapping `ModMatrix` routes.
pub(crate) fn move_fx_across_lanes(
    params: &JSynPluginParams,
    gui_context: &dyn GuiContext,
    mod_ui: &ModUiState,
    from_bus: usize,
    from_slot: usize,
    to_bus: usize,
    to_pos: usize,
) -> Option<usize> {
    if from_bus == to_bus
        || from_bus >= NUM_FX_BUSES
        || to_bus >= NUM_FX_BUSES
        || from_slot >= NUM_FX_SLOTS
    {
        return None;
    }
    let mut order = params.reconciled_fx_rack_order();
    let dst_slot = order.first_free_slot(to_bus)?;
    let snap = capture_slot(&params.fx_bus_slots(from_bus)[from_slot]);
    if snap.ty == FxTypeParam::Off {
        return None;
    }
    let ty: FxType = snap.ty.into();
    let stk = snap.stk.clamp(1, 4) as usize;

    let setter = ParamSetter::new(gui_context);
    write_slot(&setter, &params.fx_bus_slots(to_bus)[dst_slot], snap);
    mod_ui.copy_fx_ui_state(from_bus, from_slot, to_bus, dst_slot);
    remap_mod_routes(params, from_bus, from_slot, to_bus, dst_slot);

    order.remove_slot(from_bus, from_slot);
    order.insert_at(to_bus, to_pos, dst_slot);
    params.fx_rack_order.store_value(order);

    write_slot_defaults_for_type(
        &setter,
        &params.fx_bus_slots(from_bus)[from_slot],
        FxTypeParam::Off,
        true,
    );
    mod_ui.set_fx_eq_subtab(from_bus, from_slot, false);
    mod_ui.set_fx_mb_tab(from_bus, from_slot, 0);
    mod_ui.set_fx_collapsed(from_bus, from_slot, false);

    mod_ui.fx_swap_queue.provision_slot(to_bus, dst_slot, ty, stk);
    mod_ui
        .fx_swap_queue
        .provision_slot(from_bus, from_slot, FxType::Off, 0);
    Some(dst_slot)
}

/// Duplicates an FX unit into `(to_bus, to_pos)`, cloning all parameters, `STK`, UI state,
/// AND cloning all active `ModMatrix` routes targeting the source unit!
pub(crate) fn duplicate_fx_unit(
    params: &JSynPluginParams,
    gui_context: &dyn GuiContext,
    mod_ui: &ModUiState,
    from_bus: usize,
    from_slot: usize,
    to_bus: usize,
    to_pos: usize,
) -> Option<usize> {
    if from_bus >= NUM_FX_BUSES || to_bus >= NUM_FX_BUSES || from_slot >= NUM_FX_SLOTS {
        return None;
    }
    let mut order = params.reconciled_fx_rack_order();
    let dst_slot = order.first_free_slot(to_bus)?;
    let snap = capture_slot(&params.fx_bus_slots(from_bus)[from_slot]);
    if snap.ty == FxTypeParam::Off {
        return None;
    }
    let ty: FxType = snap.ty.into();
    let stk = snap.stk.clamp(1, 4) as usize;

    let setter = ParamSetter::new(gui_context);
    write_slot(&setter, &params.fx_bus_slots(to_bus)[dst_slot], snap);
    mod_ui.copy_fx_ui_state(from_bus, from_slot, to_bus, dst_slot);
    clone_mod_routes(params, from_bus, from_slot, to_bus, dst_slot);

    order.insert_at(to_bus, to_pos, dst_slot);
    params.fx_rack_order.store_value(order);

    mod_ui.fx_swap_queue.provision_slot(to_bus, dst_slot, ty, stk);
    Some(dst_slot)
}

pub(crate) fn set_lane_all_bypassed(
    params: &JSynPluginParams,
    gui_context: &dyn GuiContext,
    bus: usize,
    bypassed: bool,
) {
    if bus >= NUM_FX_BUSES {
        return;
    }
    let order = params.reconciled_fx_rack_order();
    let setter = ParamSetter::new(gui_context);
    for &slot_u8 in order.active_slots(bus) {
        let s = &params.fx_bus_slots(bus)[slot_u8 as usize];
        setter.begin_set_parameter(&s.enabled);
        setter.set_parameter(&s.enabled, !bypassed);
        setter.end_set_parameter(&s.enabled);
    }
}

pub(crate) fn clear_lane(
    params: &JSynPluginParams,
    gui_context: &dyn GuiContext,
    mod_ui: &ModUiState,
    bus: usize,
) {
    if bus >= NUM_FX_BUSES {
        return;
    }
    let order = params.reconciled_fx_rack_order();
    let active: Vec<usize> = order.active_slots(bus).iter().map(|&s| s as usize).collect();
    for slot in active {
        remove_fx_slot(params, gui_context, mod_ui, bus, slot);
    }
}

pub(crate) fn set_fx_bus_topology(
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
pub(crate) fn lane_target_label(target: u8) -> &'static str {
    match target {
        1 => "-> B",
        2 => "-> C",
        _ => "-> MST",
    }
}

#[inline]
pub(crate) fn lane_name(bus: usize) -> &'static str {
    match bus {
        0 => "BUS A",
        1 => "BUS B",
        _ => "BUS C",
    }
}
