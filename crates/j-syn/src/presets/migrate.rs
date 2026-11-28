//! Legacy `v1` and distortion-slot parameter migration helpers for `JSynPatch`.

use std::collections::BTreeMap;

use nih_plug::params::internals::ParamPtr;

use super::JSynPatch;

#[inline]
fn clamp_finite_norm(norm: f32, ptr: ParamPtr) -> f32 {
    if norm.is_finite() {
        norm.clamp(0.0, 1.0)
    } else {
        // SAFETY: Caller passes valid `ParamPtr` from a live `JSynPluginParams`.
        unsafe { ptr.default_normalized_value() }
    }
}

#[inline]
fn enum_name_alias<'a>(id: &str, name: &'a str) -> &'a str {
    if name == "Sub" && (id.ends_with("_direct") || id == "f1_out_bus" || id == "f2_out_bus") {
        "Bus C"
    } else if id == "fx_bus_topo" {
        match name {
            "Parallel (A | B)" => "Parallel (A | B | C)",
            "Serial (A -> B)" => "Serial (A -> B | C)",
            _ => name,
        }
    } else if id.starts_with("ins_mode_") {
        match name {
            "Tube -> Mech" => "T -> M",
            "Mech -> Tube" => "M -> T",
            "Sine Shaper" => "Sine",
            "Bitcrush" => "Crush",
            "Linear Fold" => "Lin Fold",
            "Sine Fold" => "Sin Fold",
            _ => name,
        }
    } else if id.starts_with("ins_pos_") {
        match name {
            "Pre-F1" => "| F1",
            "Between F1->F2" => "F1 | F2",
            "Post-F2" | "F2 |" | "   | F2" | "        | F2" => "| F2",
            _ => name,
        }
    } else {
        name
    }
}

/// Maps `version < 2` normalized enum floats whose variant count expanded in newer builds.
fn migrate_legacy_v1_enum_norm(
    id: &str,
    raw_norm: f32,
    params: &BTreeMap<String, f32>,
    ptr: ParamPtr,
) -> Option<f32> {
    let old_steps = if id.starts_with("fx_type_") || id.starts_with("bus_b_fx_type_") {
        if params.contains_key("fx_tr_flt_1") {
            Some(10usize)
        } else if params.contains_key("fx_diop_s_1") {
            // `v0.1` / `v0.1.1`: 10 FxType variants (Empty..=Dispersion).
            Some(9)
        } else if params.contains_key("flt_slope_1") {
            Some(8)
        } else {
            Some(7)
        }
    } else if (id == "flt_mode_1" || id == "flt_mode_2") && !params.contains_key("flt_slope_1") {
        // Earliest builds before `flt_slope` / Bandreject / Peak+Peak / Notch+Notch had 12 filter modes.
        Some(11)
    } else if id.starts_with("ms_sync_") {
        // `v1` presets stored 4 variants (Free, Straight, Triplet, Dotted) -> 3 steps.
        Some(3)
    } else {
        None
    }?;

    let old_idx = (raw_norm * old_steps as f32)
        .round()
        .clamp(0.0, old_steps as f32);
    // SAFETY: Caller passes valid `ParamPtr` from a live `JSynPluginParams`.
    Some(clamp_finite_norm(
        unsafe { ptr.preview_normalized(old_idx) },
        ptr,
    ))
}

fn legacy_distortion_slot_keys(id: &str) -> Option<(String, String, String)> {
    if let Some(suffix) = id
        .strip_prefix("bus_b_fx_dist_m_")
        .or_else(|| id.strip_prefix("bus_b_fx_p1_"))
    {
        Some((
            format!("bus_b_fx_type_{suffix}"),
            format!("bus_b_fx_dist_m_{suffix}"),
            format!("bus_b_fx_p1_{suffix}"),
        ))
    } else {
        id.strip_prefix("fx_dist_m_")
            .or_else(|| id.strip_prefix("fx_p1_"))
            .map(|suffix| {
                (
                    format!("fx_type_{suffix}"),
                    format!("fx_dist_m_{suffix}"),
                    format!("fx_p1_{suffix}"),
                )
            })
    }
}

fn is_legacy_distortion_slot(patch: &JSynPatch, type_id: &str, dist_m_id: &str) -> bool {
    if patch.enum_names.contains_key(dist_m_id)
        || patch.plain_params.contains_key(dist_m_id)
        || patch.params.contains_key(dist_m_id)
    {
        return false;
    }
    if patch
        .enum_names
        .get(type_id)
        .is_some_and(|n| n.eq_ignore_ascii_case("Distortion"))
    {
        return true;
    }
    if patch
        .plain_params
        .get(type_id)
        .is_some_and(|v| v.is_finite() && v.round() as i32 == 2)
    {
        return true;
    }
    if let Some(&raw_norm) = patch.params.get(type_id) {
        if raw_norm.is_finite() {
            let n = raw_norm.clamp(0.0, 1.0);
            let old_steps = if patch.params.contains_key("fx_tr_flt_1") {
                10.0
            } else if patch.params.contains_key("fx_diop_s_1") {
                9.0
            } else if patch.params.contains_key("flt_slope_1") {
                8.0
            } else if patch.version < 2 {
                7.0
            } else {
                12.0
            };
            return (n * old_steps).round() as i32 == 2;
        }
    }
    false
}

pub(super) fn resolve_patch_param_norm(patch: &JSynPatch, id: &str, ptr: ParamPtr) -> f32 {
    // SAFETY: Caller passes valid `ParamPtr` from a live `JSynPluginParams`.
    unsafe {
        match ptr {
            ParamPtr::EnumParam(_) => {
                let steps = ptr.step_count().unwrap_or(0);
                if let Some(raw_name) = patch.enum_names.get(id) {
                    let aliased = enum_name_alias(id, raw_name);
                    if let Some(n) = ptr.string_to_normalized_value(aliased) {
                        return clamp_finite_norm(n, ptr);
                    }
                    if let Some(&plain) = patch.plain_params.get(id) {
                        if plain.is_finite() {
                            let idx = plain.round();
                            if idx >= 0.0 && idx <= steps as f32 {
                                return clamp_finite_norm(ptr.preview_normalized(idx), ptr);
                            }
                        }
                    }
                    return ptr.default_normalized_value();
                }

                if let Some(&plain) = patch.plain_params.get(id) {
                    if plain.is_finite() && plain >= 0.0 {
                        let idx = plain.round();
                        if idx <= steps as f32 {
                            return clamp_finite_norm(ptr.preview_normalized(idx), ptr);
                        }
                        return ptr.default_normalized_value();
                    }
                }

                if let Some(&raw_norm) = patch.params.get(id) {
                    if raw_norm.is_finite() {
                        let n = raw_norm.clamp(0.0, 1.0);
                        if patch.version < 2 {
                            if let Some(migrated) =
                                migrate_legacy_v1_enum_norm(id, n, &patch.params, ptr)
                            {
                                return migrated;
                            }
                        }
                        return n;
                    }
                }

                if id.starts_with("fx_dist_m_") || id.starts_with("bus_b_fx_dist_m_") {
                    if let Some((type_id, dist_m_id, p1_id)) = legacy_distortion_slot_keys(id) {
                        if is_legacy_distortion_slot(patch, &type_id, &dist_m_id) {
                            let old_p1 = patch
                                .plain_params
                                .get(&p1_id)
                                .or_else(|| patch.params.get(&p1_id))
                                .copied()
                                .filter(|v| v.is_finite())
                                .unwrap_or(0.0)
                                .clamp(0.0, 1.0);
                            let idx =
                                j_syn_dsp::waveshaper::legacy_distortion_submode_index(old_p1);
                            return clamp_finite_norm(ptr.preview_normalized(idx as f32), ptr);
                        }
                    }
                }

                ptr.default_normalized_value()
            }
            ParamPtr::IntParam(_) => {
                if let Some(&plain) = patch.plain_params.get(id) {
                    if plain.is_finite() {
                        return clamp_finite_norm(ptr.preview_normalized(plain.round()), ptr);
                    }
                }
                if let Some(&norm) = patch.params.get(id) {
                    if norm.is_finite() {
                        let clamped = norm.clamp(0.0, 1.0);
                        if patch.version < 2 && id.starts_with("ms_steps_") {
                            let legacy_steps = (2.0 + clamped * 14.0).round();
                            return clamp_finite_norm(ptr.preview_normalized(legacy_steps), ptr);
                        }
                        return clamped;
                    }
                }
                ptr.default_normalized_value()
            }
            ParamPtr::FloatParam(_) => {
                if id.starts_with("fx_p1_") || id.starts_with("bus_b_fx_p1_") {
                    if let Some((type_id, dist_m_id, _)) = legacy_distortion_slot_keys(id) {
                        if is_legacy_distortion_slot(patch, &type_id, &dist_m_id) {
                            return 0.50;
                        }
                    }
                }
                if let Some(&plain) = patch.plain_params.get(id) {
                    if plain.is_finite() {
                        return clamp_finite_norm(ptr.preview_normalized(plain), ptr);
                    }
                }
                if let Some(&norm) = patch.params.get(id) {
                    if norm.is_finite() {
                        return norm.clamp(0.0, 1.0);
                    }
                }
                ptr.default_normalized_value()
            }
            ParamPtr::BoolParam(_) => {
                if let Some(&plain) = patch.plain_params.get(id) {
                    if plain.is_finite() {
                        return if plain >= 0.5 { 1.0 } else { 0.0 };
                    }
                }
                if let Some(&norm) = patch.params.get(id) {
                    if norm.is_finite() {
                        return if norm >= 0.5 { 1.0 } else { 0.0 };
                    }
                }
                ptr.default_normalized_value()
            }
        }
    }
}
