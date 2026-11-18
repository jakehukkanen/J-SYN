//! Draggable Modulation Source Pills and 60 Hz live telemetry / structural UI redraw ticker.

use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use j_syn_dsp::mod_matrix::{
    ModDestination, ModMatrixSource, ModSlot, ModUiTelemetry, NUM_MOD_DESTS, NUM_MOD_SLOTS,
};
use j_syn_dsp::modseq::ModSeqStepShape;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use super::controls::bold_label;
use super::modseq_tab::ModSeqSubTab;
use super::{BottomTab, UiEvent};
use crate::JSynPluginParams;

const UNSET_U8: u8 = 255;
const UNSET_DEST: u16 = u16::MAX;
pub(crate) const FX_VIEW_TAG: u8 = 5;

/// Converts `BottomTab` to the visibility tag used for drop-target hit testing (`1..=3`, `0` = always visible).
#[inline]
pub(crate) fn bottom_tab_tag(tab: BottomTab) -> u8 {
    match tab {
        BottomTab::VoiceEnv => 1,
        BottomTab::ModSeq => 2,
        BottomTab::Routing => 3,
    }
}

/// Shared lock-free UI coordination state for drag-and-drop assignment, drop rects, and inspector focus.
pub(crate) struct ModUiState {
    pub params: Arc<JSynPluginParams>,
    pub telemetry: Arc<ModUiTelemetry>,
    dragging_source: AtomicU8,
    hover_dest: AtomicU16,
    active_bottom_tab: AtomicU8,
    active_modseq_subtab: AtomicU8,
    active_view_mode: AtomicU8,
    fx_eq_mask: AtomicU32,
    mb_dist_tab: [[AtomicU8; 8]; 3],
    inspected_dest: AtomicU16,
    inspected_slot: AtomicU8,
    active_stamp: AtomicU8,
    mseg_snap: AtomicBool,
    last_enabled_mask: AtomicU64,
    external_sync_rev: AtomicU64,
    last_external_sync_rev: AtomicU64,
    pending_preset_name: Mutex<Option<String>>,
    drop_rects: Mutex<[(f32, f32, f32, f32, u8); NUM_MOD_DESTS]>,
}

impl ModUiState {
    pub fn new(params: Arc<JSynPluginParams>, telemetry: Arc<ModUiTelemetry>) -> Self {
        let initial_mask = Self::compute_enabled_mask(&params);
        Self {
            params,
            telemetry,
            dragging_source: AtomicU8::new(0),
            hover_dest: AtomicU16::new(UNSET_DEST),
            active_bottom_tab: AtomicU8::new(bottom_tab_tag(BottomTab::ModSeq)),
            active_modseq_subtab: AtomicU8::new(0),
            active_view_mode: AtomicU8::new(0),
            fx_eq_mask: AtomicU32::new(0),
            mb_dist_tab: std::array::from_fn(|_| std::array::from_fn(|_| AtomicU8::new(0))),
            inspected_dest: AtomicU16::new(UNSET_DEST),
            inspected_slot: AtomicU8::new(UNSET_U8),
            active_stamp: AtomicU8::new(ModSeqStepShape::RampDown as u8),
            mseg_snap: AtomicBool::new(false),
            last_enabled_mask: AtomicU64::new(initial_mask),
            external_sync_rev: AtomicU64::new(0),
            last_external_sync_rev: AtomicU64::new(0),
            pending_preset_name: Mutex::new(None),
            drop_rects: Mutex::new([(0.0, 0.0, 0.0, 0.0, UNSET_U8); NUM_MOD_DESTS]),
        }
    }

    pub fn notify_external_sync(&self, preset_name: Option<String>) {
        if let Some(name) = preset_name {
            if let Ok(mut guard) = self.pending_preset_name.lock() {
                *guard = Some(name);
            }
        }
        self.external_sync_rev.fetch_add(1, Ordering::Release);
    }

    pub fn poll_external_sync_changed(&self) -> bool {
        let cur = self.external_sync_rev.load(Ordering::Acquire);
        let prev = self.last_external_sync_rev.swap(cur, Ordering::Relaxed);
        cur != prev
    }

    pub fn take_pending_preset_name(&self) -> Option<String> {
        self.pending_preset_name
            .lock()
            .ok()
            .and_then(|mut g| g.take())
    }

    #[inline]
    pub fn active_stamp(&self) -> ModSeqStepShape {
        ModSeqStepShape::from_u8(self.active_stamp.load(Ordering::Relaxed))
    }

    #[inline]
    fn fnv1a_mix(hash: &mut u64, value: u64) {
        *hash ^= value;
        *hash = hash.wrapping_mul(0x100_0000_01b3);
    }

    /// Hashes structural module states (enables, unison>1, inserts, sub bus, both FX buses'
    /// slot type/stack/filter mode/slope/diopser stages) so host automation or swaps refresh the UI.
    pub fn compute_enabled_mask(params: &JSynPluginParams) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for osc in &params.oscs {
            Self::fnv1a_mix(&mut h, osc.enabled.value() as u64);
            Self::fnv1a_mix(&mut h, (osc.unison.value() > 1) as u64);
            Self::fnv1a_mix(&mut h, osc.pitch_mode.value() as u64);
        }
        Self::fnv1a_mix(&mut h, params.noise.enabled.value() as u64);
        Self::fnv1a_mix(&mut h, params.filters[0].enabled.value() as u64);
        Self::fnv1a_mix(&mut h, params.filters[1].enabled.value() as u64);
        Self::fnv1a_mix(&mut h, params.routing.inserts[0].enabled.value() as u64);
        Self::fnv1a_mix(&mut h, params.routing.inserts[1].enabled.value() as u64);
        Self::fnv1a_mix(&mut h, params.routing.inserts[0].mode.value() as u64);
        Self::fnv1a_mix(&mut h, params.routing.inserts[1].mode.value() as u64);
        Self::fnv1a_mix(&mut h, params.sub_bus.enabled.value() as u64);
        Self::fnv1a_mix(&mut h, params.fx_bus_topology.value() as u64);
        for bus in 0..3 {
            for fx in params.fx_bus_slots(bus) {
                let packed = (fx.enabled.value() as u64)
                    | ((fx.fx_type.value() as u64) << 1)
                    | ((fx.stack.value().clamp(1, 4) as u64) << 6)
                    | ((fx.filter_mode.value() as u64) << 9)
                    | ((fx.filter_slope.value() as u64) << 14)
                    | ((fx.diopser_stages.value() as u64) << 16)
                    | ((fx.tube_res_filter_enabled.value() as u64) << 19)
                    | ((fx.utility_inv_l.value() as u64) << 20)
                    | ((fx.utility_inv_r.value() as u64) << 21)
                    | ((fx.limiter_true_peak.value() as u64) << 22)
                    | ((fx.distortion_mode.value() as u64) << 23);
                Self::fnv1a_mix(&mut h, packed);
                for mb in &fx.mb_bands {
                    let mb_packed = (mb.enabled.value() as u64)
                        | ((mb.solo.value() as u64) << 1)
                        | ((mb.mode.value() as u64) << 2);
                    Self::fnv1a_mix(&mut h, mb_packed);
                }
            }
        }
        h
    }

    pub fn poll_enabled_mask_changed(&self) -> bool {
        let next = Self::compute_enabled_mask(&self.params);
        let prev = self.last_enabled_mask.swap(next, Ordering::Relaxed);
        prev != next
    }

    #[inline]
    pub fn mseg_snap(&self) -> bool {
        self.mseg_snap.load(Ordering::Relaxed)
    }

    pub fn toggle_mseg_snap(&self) -> bool {
        let next = !self.mseg_snap.load(Ordering::Relaxed);
        self.mseg_snap.store(next, Ordering::Relaxed);
        next
    }

    pub fn set_active_stamp(&self, shape: ModSeqStepShape) {
        self.active_stamp.store(shape as u8, Ordering::Relaxed);
    }

    pub fn set_active_tab(&self, tab: BottomTab) {
        self.active_bottom_tab
            .store(bottom_tab_tag(tab), Ordering::Relaxed);
    }

    pub fn set_modseq_subtab(&self, sub: ModSeqSubTab) {
        let code = match sub {
            ModSeqSubTab::MSeq(i) => (i as u8).min(3),
            ModSeqSubTab::Env(i) => 4 + (i as u8).min(3),
        };
        self.active_modseq_subtab.store(code, Ordering::Relaxed);
    }

    #[inline]
    pub fn is_source_tab_active(&self, src: ModMatrixSource) -> bool {
        if self.active_bottom_tab.load(Ordering::Relaxed) != bottom_tab_tag(BottomTab::ModSeq) {
            return false;
        }
        let code = self.active_modseq_subtab.load(Ordering::Relaxed);
        match src {
            ModMatrixSource::ModSeq1 => code == 0,
            ModMatrixSource::ModSeq2 => code == 1,
            ModMatrixSource::ModSeq3 => code == 2,
            ModMatrixSource::ModSeq4 => code == 3,
            ModMatrixSource::Env1 => code == 4,
            ModMatrixSource::Env2 => code == 5,
            ModMatrixSource::Env3 => code == 6,
            ModMatrixSource::Env4 => code == 7,
            _ => false,
        }
    }

    #[inline]
    pub fn dragging_source(&self) -> ModMatrixSource {
        ModMatrixSource::from_u8(self.dragging_source.load(Ordering::Relaxed))
    }

    pub fn start_drag(&self, src: ModMatrixSource) {
        self.dragging_source.store(src as u8, Ordering::Relaxed);
        self.hover_dest.store(UNSET_DEST, Ordering::Relaxed);
    }

    pub fn clear_drag(&self) {
        self.dragging_source.store(0, Ordering::Relaxed);
        self.hover_dest.store(UNSET_DEST, Ordering::Relaxed);
    }

    #[inline]
    pub fn is_drop_target(&self, dest: ModDestination) -> bool {
        self.dragging_source() != ModMatrixSource::None
            && self.hover_dest.load(Ordering::Relaxed) == (dest.index() as u16)
    }

    pub fn register_drop_rect(&self, dest: ModDestination, tab_tag: u8, bounds: BoundingBox) {
        if let Ok(mut rects) = self.drop_rects.lock() {
            rects[dest.index()] = (bounds.x, bounds.y, bounds.w, bounds.h, tab_tag);
        }
    }

    pub fn set_view_mode(&self, mode: u8) {
        self.active_view_mode.store(mode, Ordering::Relaxed);
    }

    #[inline]
    pub fn fx_eq_subtab(&self, bus: usize, slot: usize) -> bool {
        let bit = (bus.min(2) * 8 + (slot & 7)) as u32;
        ((self.fx_eq_mask.load(Ordering::Relaxed) >> bit) & 1) != 0
    }

    pub fn set_fx_eq_subtab(&self, bus: usize, slot: usize, is_eq: bool) {
        let bit = 1_u32 << ((bus.min(2) * 8 + (slot & 7)) as u32);
        if is_eq {
            self.fx_eq_mask.fetch_or(bit, Ordering::Relaxed);
        } else {
            self.fx_eq_mask.fetch_and(!bit, Ordering::Relaxed);
        }
    }

    pub fn swap_fx_eq_subtab(&self, bus: usize, slot_a: usize, slot_b: usize) {
        let a = self.fx_eq_subtab(bus, slot_a);
        let b = self.fx_eq_subtab(bus, slot_b);
        self.set_fx_eq_subtab(bus, slot_a, b);
        self.set_fx_eq_subtab(bus, slot_b, a);
    }

    #[inline]
    pub fn fx_mb_tab(&self, bus: usize, slot: usize) -> u8 {
        self.mb_dist_tab[bus.min(2)][slot & 7]
            .load(Ordering::Relaxed)
            .min(3)
    }

    pub fn set_fx_mb_tab(&self, bus: usize, slot: usize, tab: u8) {
        self.mb_dist_tab[bus.min(2)][slot & 7].store(tab.min(3), Ordering::Relaxed);
    }

    pub fn swap_fx_mb_tab(&self, bus: usize, slot_a: usize, slot_b: usize) {
        let a = self.fx_mb_tab(bus, slot_a);
        let b = self.fx_mb_tab(bus, slot_b);
        self.set_fx_mb_tab(bus, slot_a, b);
        self.set_fx_mb_tab(bus, slot_b, a);
    }

    fn hit_test_cursor(&self, x: f32, y: f32) -> Option<usize> {
        let view_mode = self.active_view_mode.load(Ordering::Relaxed);
        if view_mode != 0 {
            return None;
        }
        let active_tab = self.active_bottom_tab.load(Ordering::Relaxed);
        let rects = self.drop_rects.lock().ok()?;
        for (idx, &(rx, ry, rw, rh, tag)) in rects.iter().enumerate() {
            if rw <= 1.0 || rh <= 1.0 {
                continue;
            }
            let visible = tag == 0 || tag == FX_VIEW_TAG || tag == active_tab;
            if !visible {
                continue;
            }
            if x >= rx && x <= rx + rw && y >= ry && y <= ry + rh {
                return Some(idx);
            }
        }
        None
    }

    /// Updates hovered destination during drag; returns `true` when hover changed.
    pub fn update_hover_from_cursor(&self, x: f32, y: f32) -> bool {
        let next = self
            .hit_test_cursor(x, y)
            .map(|idx| idx as u16)
            .unwrap_or(UNSET_DEST);
        let prev = self.hover_dest.swap(next, Ordering::Relaxed);
        prev != next
    }

    /// Completes a drag-drop onto any hovered destination; returns `true` if matrix state changed.
    pub fn finish_drop_at_cursor(&self, x: f32, y: f32) -> bool {
        let src = self.dragging_source();
        let target_idx = self.hit_test_cursor(x, y);
        self.clear_drag();
        if let (Some(dest), false) = (
            target_idx.and_then(ModDestination::from_index),
            src == ModMatrixSource::None,
        ) {
            return self.assign_drop(src, dest).is_some();
        }
        false
    }

    pub fn assign_drop(&self, src: ModMatrixSource, dest: ModDestination) -> Option<usize> {
        let mut mat = self.params.mod_matrix.snapshot();
        let slot_idx = mat.assign_or_update(src, dest, 0.25)?;
        self.params.mod_matrix.store_value(mat);
        self.inspected_dest
            .store(dest.index() as u16, Ordering::Relaxed);
        self.inspected_slot.store(slot_idx as u8, Ordering::Relaxed);
        Some(slot_idx)
    }

    /// Collects up to 4 active modulation slots targeting `dest` for ring/bar rendering.
    pub fn slots_for_dest(&self, dest: ModDestination) -> ([ModSlot; 4], [usize; 4], usize) {
        let mat = self.params.mod_matrix.snapshot();
        let mut out_slots = [ModSlot::default(); 4];
        let mut out_indices = [0usize; 4];
        let mut count = 0usize;
        for (idx, slot) in mat.slots.iter().enumerate() {
            if slot.enabled && slot.source != ModMatrixSource::None && slot.destination == dest {
                if count < 4 {
                    out_slots[count] = *slot;
                    out_indices[count] = idx;
                    count += 1;
                } else {
                    break;
                }
            }
        }
        (out_slots, out_indices, count)
    }

    #[inline]
    pub fn has_slots_for_dest(&self, dest: ModDestination) -> bool {
        self.slots_for_dest(dest).2 > 0
    }

    pub fn primary_slot_for_dest(&self, dest: ModDestination) -> Option<(usize, ModSlot)> {
        let mat = self.params.mod_matrix.snapshot();
        let inspected = self.inspected_slot.load(Ordering::Relaxed) as usize;
        if inspected < NUM_MOD_SLOTS {
            let s = mat.slots[inspected];
            if s.enabled && s.source != ModMatrixSource::None && s.destination == dest {
                return Some((inspected, s));
            }
        }
        mat.slots
            .iter()
            .enumerate()
            .find(|(_, s)| s.enabled && s.source != ModMatrixSource::None && s.destination == dest)
            .map(|(idx, s)| (idx, *s))
    }

    pub fn focus_slot(&self, dest: ModDestination, slot_idx: usize) {
        self.inspected_dest
            .store(dest.index() as u16, Ordering::Relaxed);
        self.inspected_slot.store(slot_idx as u8, Ordering::Relaxed);
    }

    pub fn adjust_slot_depth(&self, slot_idx: usize, new_depth: f32) {
        if slot_idx >= NUM_MOD_SLOTS {
            return;
        }
        let mut mat = self.params.mod_matrix.snapshot();
        if mat.slots[slot_idx].enabled {
            mat.slots[slot_idx].depth = new_depth.clamp(-1.0, 1.0);
            self.params.mod_matrix.store_value(mat);
            self.focus_slot(mat.slots[slot_idx].destination, slot_idx);
        }
    }

    pub fn remove_primary_slot_for_dest(&self, dest: ModDestination) -> bool {
        let Some((idx, _)) = self.primary_slot_for_dest(dest) else {
            return false;
        };
        let mut mat = self.params.mod_matrix.snapshot();
        mat.clear_slot(idx);
        self.params.mod_matrix.store_value(mat);

        if let Some((next_idx, _)) = mat
            .slots
            .iter()
            .enumerate()
            .find(|(_, s)| s.enabled && s.source != ModMatrixSource::None && s.destination == dest)
        {
            self.focus_slot(dest, next_idx);
        } else {
            self.inspected_slot.store(UNSET_U8, Ordering::Relaxed);
            self.inspected_dest.store(UNSET_DEST, Ordering::Relaxed);
        }
        true
    }

    pub fn remove_all_slots_for_dest(&self, dest: ModDestination) -> bool {
        let mut mat = self.params.mod_matrix.snapshot();
        let mut cleared = false;
        for idx in 0..NUM_MOD_SLOTS {
            let s = mat.slots[idx];
            if s.enabled && s.source != ModMatrixSource::None && s.destination == dest {
                mat.clear_slot(idx);
                cleared = true;
            }
        }
        if cleared {
            self.params.mod_matrix.store_value(mat);
            if self.inspected_dest.load(Ordering::Relaxed) as usize == dest.index() {
                self.inspected_slot.store(UNSET_U8, Ordering::Relaxed);
                self.inspected_dest.store(UNSET_DEST, Ordering::Relaxed);
            }
        }
        cleared
    }

    pub fn active_slot_count(&self) -> usize {
        let mat = self.params.mod_matrix.snapshot();
        mat.slots
            .iter()
            .filter(|s| s.enabled && s.source != ModMatrixSource::None)
            .count()
    }

    pub fn active_entries(&self) -> Vec<(usize, ModSlot)> {
        let mat = self.params.mod_matrix.snapshot();
        mat.slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.enabled && s.source != ModMatrixSource::None)
            .map(|(idx, s)| (idx, *s))
            .collect()
    }

    pub fn toggle_slot_bipolar_at(&self, slot_idx: usize) {
        if slot_idx >= NUM_MOD_SLOTS {
            return;
        }
        let mut mat = self.params.mod_matrix.snapshot();
        if mat.slots[slot_idx].enabled {
            mat.slots[slot_idx].bipolar = !mat.slots[slot_idx].bipolar;
            self.params.mod_matrix.store_value(mat);
            self.focus_slot(mat.slots[slot_idx].destination, slot_idx);
        }
    }

    pub fn remove_slot_at(&self, slot_idx: usize) {
        if slot_idx >= NUM_MOD_SLOTS {
            return;
        }
        let mut mat = self.params.mod_matrix.snapshot();
        let old_dest = mat.slots[slot_idx].destination;
        mat.clear_slot(slot_idx);
        self.params.mod_matrix.store_value(mat);
        if self.inspected_slot.load(Ordering::Relaxed) as usize == slot_idx {
            if let Some((next_idx, _)) = mat.slots.iter().enumerate().find(|(_, s)| {
                s.enabled && s.source != ModMatrixSource::None && s.destination == old_dest
            }) {
                self.focus_slot(old_dest, next_idx);
            } else {
                self.inspected_slot.store(UNSET_U8, Ordering::Relaxed);
                self.inspected_dest.store(UNSET_DEST, Ordering::Relaxed);
            }
        }
    }

    pub fn toggle_slot_bypass_at(&self, slot_idx: usize) {
        if slot_idx >= NUM_MOD_SLOTS {
            return;
        }
        let mut mat = self.params.mod_matrix.snapshot();
        if mat.slots[slot_idx].enabled {
            mat.slots[slot_idx].bypassed = !mat.slots[slot_idx].bypassed;
            self.params.mod_matrix.store_value(mat);
        }
    }

    pub fn all_slots_for_dest(&self, dest: ModDestination) -> Vec<(usize, ModSlot)> {
        let mat = self.params.mod_matrix.snapshot();
        mat.slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.enabled && s.source != ModMatrixSource::None && s.destination == dest)
            .map(|(i, s)| (i, *s))
            .collect()
    }

    pub fn clear_all_slots(&self) {
        let mut mat = self.params.mod_matrix.snapshot();
        for idx in 0..NUM_MOD_SLOTS {
            mat.clear_slot(idx);
        }
        self.params.mod_matrix.store_value(mat);
        self.inspected_slot.store(UNSET_U8, Ordering::Relaxed);
        self.inspected_dest.store(UNSET_DEST, Ordering::Relaxed);
    }

    pub fn source_display_name(&self, src: ModMatrixSource) -> String {
        let macro_idx = match src {
            ModMatrixSource::Macro1 => Some(0),
            ModMatrixSource::Macro2 => Some(1),
            ModMatrixSource::Macro3 => Some(2),
            ModMatrixSource::Macro4 => Some(3),
            ModMatrixSource::Macro5 => Some(4),
            ModMatrixSource::Macro6 => Some(5),
            ModMatrixSource::Macro7 => Some(6),
            ModMatrixSource::Macro8 => Some(7),
            _ => None,
        };
        if let Some(i) = macro_idx {
            if let Ok(guard) = self.params.macro_names.lock() {
                let trimmed = guard[i].trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
        src.full_label().to_string()
    }

    pub fn aux_short_label(&self, src: ModMatrixSource) -> String {
        if src == ModMatrixSource::None {
            return "NONE".to_string();
        }
        let macro_idx = match src {
            ModMatrixSource::Macro1 => Some(0),
            ModMatrixSource::Macro2 => Some(1),
            ModMatrixSource::Macro3 => Some(2),
            ModMatrixSource::Macro4 => Some(3),
            ModMatrixSource::Macro5 => Some(4),
            ModMatrixSource::Macro6 => Some(5),
            ModMatrixSource::Macro7 => Some(6),
            ModMatrixSource::Macro8 => Some(7),
            _ => None,
        };
        if let Some(i) = macro_idx {
            if let Ok(guard) = self.params.macro_names.lock() {
                let trimmed = guard[i].trim();
                if !trimmed.is_empty() {
                    return trimmed.chars().take(7).collect();
                }
            }
        }
        src.short_label().to_string()
    }

    pub fn set_slot_depth_at(&self, slot_idx: usize, depth: f32) {
        if slot_idx >= NUM_MOD_SLOTS {
            return;
        }
        let mut mat = self.params.mod_matrix.snapshot();
        if mat.slots[slot_idx].enabled {
            mat.slots[slot_idx].depth = depth.clamp(-1.0, 1.0);
            self.params.mod_matrix.store_value(mat);
        }
    }

    pub fn set_slot_curve_at(&self, slot_idx: usize, curve: f32) {
        if slot_idx >= NUM_MOD_SLOTS {
            return;
        }
        let mut mat = self.params.mod_matrix.snapshot();
        if mat.slots[slot_idx].enabled {
            mat.slots[slot_idx].curve = curve;
            self.params.mod_matrix.store_value(mat);
        }
    }

    pub fn set_slot_source_at(&self, slot_idx: usize, source: ModMatrixSource) {
        if slot_idx >= NUM_MOD_SLOTS {
            return;
        }
        let mut mat = self.params.mod_matrix.snapshot();
        if mat.slots[slot_idx].enabled {
            mat.slots[slot_idx].source = source;
            self.params.mod_matrix.store_value(mat);
        }
    }

    pub fn set_slot_destination_at(&self, slot_idx: usize, destination: ModDestination) {
        if slot_idx >= NUM_MOD_SLOTS {
            return;
        }
        let mut mat = self.params.mod_matrix.snapshot();
        if mat.slots[slot_idx].enabled {
            mat.slots[slot_idx].destination = destination;
            self.params.mod_matrix.store_value(mat);
        }
    }

    pub fn set_slot_aux_at(&self, slot_idx: usize, aux: ModMatrixSource) {
        if slot_idx >= NUM_MOD_SLOTS {
            return;
        }
        let mut mat = self.params.mod_matrix.snapshot();
        if mat.slots[slot_idx].enabled {
            mat.slots[slot_idx].aux_source = aux;
            self.params.mod_matrix.store_value(mat);
        }
    }

    pub fn move_active_slot(&self, slot_idx: usize, dir: isize) {
        if slot_idx >= NUM_MOD_SLOTS {
            return;
        }
        let mut mat = self.params.mod_matrix.snapshot();
        let active: Vec<usize> = mat
            .slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| (s.enabled && s.source != ModMatrixSource::None).then_some(i))
            .collect();
        let Some(pos) = active.iter().position(|&i| i == slot_idx) else {
            return;
        };
        let target_pos = pos as isize + dir;
        if target_pos < 0 || target_pos >= active.len() as isize {
            return;
        }
        let other_idx = active[target_pos as usize];
        mat.swap_slots(slot_idx, other_idx);
        self.params.mod_matrix.store_value(mat);
    }
}

/// Draggable modulation source pill (`E1..E4`, `MS1..MS4`, `VEL`, `KEY`, `MW`, `AT`, `M1..M8`).
pub(crate) struct ModSourcePill {
    source: ModMatrixSource,
    mod_ui: Arc<ModUiState>,
    drag_active: bool,
    drag_moved: bool,
    hovered: bool,
    start_x: f32,
    start_y: f32,
}

impl ModSourcePill {
    pub(crate) fn new<'a>(
        cx: &'a mut Context,
        source: ModMatrixSource,
        mod_ui: Arc<ModUiState>,
    ) -> Handle<'a, Self> {
        Self {
            source,
            mod_ui,
            drag_active: false,
            drag_moved: false,
            hovered: false,
            start_x: 0.0,
            start_y: 0.0,
        }
        .build(cx, |cx| {
            bold_label(cx, source.short_label(), "mod-src-label")
                .width(Stretch(1.0))
                .height(Auto)
                .top(Stretch(1.0))
                .bottom(Stretch(1.0))
                .child_left(Stretch(1.0))
                .child_right(Stretch(1.0))
                .hoverable(false);
        })
    }

    fn subtab_for_source(src: ModMatrixSource) -> Option<ModSeqSubTab> {
        match src {
            ModMatrixSource::ModSeq1 => Some(ModSeqSubTab::MSeq(0)),
            ModMatrixSource::ModSeq2 => Some(ModSeqSubTab::MSeq(1)),
            ModMatrixSource::ModSeq3 => Some(ModSeqSubTab::MSeq(2)),
            ModMatrixSource::ModSeq4 => Some(ModSeqSubTab::MSeq(3)),
            ModMatrixSource::Env1 => Some(ModSeqSubTab::Env(0)),
            ModMatrixSource::Env2 => Some(ModSeqSubTab::Env(1)),
            ModMatrixSource::Env3 => Some(ModSeqSubTab::Env(2)),
            ModMatrixSource::Env4 => Some(ModSeqSubTab::Env(3)),
            _ => None,
        }
    }
}

impl View for ModSourcePill {
    fn element(&self) -> Option<&'static str> {
        Some("mod-source-pill")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseOver => {
                self.hovered = true;
                cx.needs_redraw();
            }
            WindowEvent::MouseOut => {
                self.hovered = false;
                cx.needs_redraw();
            }
            WindowEvent::MouseDown(MouseButton::Left)
            | WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                self.drag_active = true;
                self.drag_moved = false;
                self.start_x = cx.mouse().cursorx;
                self.start_y = cx.mouse().cursory;
                self.mod_ui.start_drag(self.source);
                cx.capture();
                cx.emit(UiEvent::ModUiChanged);
                cx.needs_redraw();
                meta.consume();
            }
            WindowEvent::MouseMove(x, y) => {
                if self.drag_active {
                    let dist = (*x - self.start_x).hypot(*y - self.start_y);
                    if !self.drag_moved && dist >= 4.0 * cx.scale_factor() {
                        self.drag_moved = true;
                    }
                    if self.drag_moved && self.mod_ui.update_hover_from_cursor(*x, *y) {
                        cx.emit(UiEvent::ModUiChanged);
                    }
                    cx.needs_redraw();
                    meta.consume();
                }
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.drag_active {
                    self.drag_active = false;
                    let moved = self.drag_moved;
                    self.drag_moved = false;
                    cx.release();
                    if moved {
                        self.mod_ui.finish_drop_at_cursor(cx.mouse().cursorx, cx.mouse().cursory);
                    } else {
                        self.mod_ui.clear_drag();
                        if let Some(sub) = Self::subtab_for_source(self.source) {
                            cx.emit(UiEvent::SelectTab(BottomTab::ModSeq));
                            cx.emit(UiEvent::SelectModSeqSubTab(sub));
                        }
                    }
                    cx.emit(UiEvent::ModUiChanged);
                    cx.needs_redraw();
                    meta.consume();
                }
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 1.0 || bounds.h <= 1.0 {
            return;
        }
        let dpx = cx.scale_factor();
        let pal = super::theme::current_palette(cx);
        let (r, g, b) = super::theme::mod_source_rgb(&pal, self.source);
        let is_dragging = self.mod_ui.dragging_source() == self.source;
        let is_active_tab = self.mod_ui.is_source_tab_active(self.source);
        let is_hovered = self.hovered;
        let level = self.mod_ui.telemetry.source_level(self.source).clamp(0.0, 1.0);

        let mut bg = vg::Path::new();
        bg.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 3.0 * dpx);
        let fill = if is_dragging {
            vg::Color::rgba(r, g, b, 90)
        } else if is_active_tab {
            let (mr, mg, mb) = super::theme::hex_to_rgb(&pal.surface_hover);
            let (tr, tg, tb) = super::theme::lerp_rgb((mr, mg, mb), (r, g, b), 0.16);
            vg::Color::rgb(tr, tg, tb)
        } else if is_hovered {
            super::theme::vg_color(&pal.surface_hover, 255)
        } else {
            super::theme::vg_color(&pal.surface_mid, 255)
        };
        canvas.fill_path(&bg, &vg::Paint::color(fill));

        // Live telemetry bottom activity bar
        if level > 0.01 {
            let bar_w = ((bounds.w - 4.0 * dpx) * level).max(2.0 * dpx);
            let mut bar = vg::Path::new();
            bar.rounded_rect(
                bounds.x + 2.0 * dpx,
                bounds.y + bounds.h - 3.0 * dpx,
                bar_w,
                2.0 * dpx,
                1.0 * dpx,
            );
            canvas.fill_path(&bar, &vg::Paint::color(vg::Color::rgba(r, g, b, 235)));
        }

        // Left colored badge accent + subtle outline
        let stripe_w = if is_active_tab { 3.0 * dpx } else { 2.2 * dpx };
        let mut stripe = vg::Path::new();
        stripe.rounded_rect(
            bounds.x + 1.5 * dpx,
            bounds.y + 3.0 * dpx,
            stripe_w,
            bounds.h - 6.0 * dpx,
            1.0 * dpx,
        );
        canvas.fill_path(&stripe, &vg::Paint::color(vg::Color::rgb(r, g, b)));

        let mut border = vg::Paint::color(if is_dragging || is_active_tab {
            vg::Color::rgb(r, g, b)
        } else if is_hovered {
            vg::Color::rgba(r, g, b, 195)
        } else {
            vg::Color::rgba(r, g, b, 90)
        });
        border.set_line_width((if is_active_tab { 1.35 } else { 1.0 }) * dpx);
        canvas.stroke_path(&bg, &border);
    }
}

/// Starts the single 60 Hz UI redraw & structural-state polling timer (`16 ms`)
/// for live mod-ring, scope, and playhead animations.
pub(crate) fn start_ui_redraw_timer(cx: &mut Context, mod_ui: Arc<ModUiState>) {
    let timer = cx.add_timer(Duration::from_millis(16), None, move |cx, action| {
        if matches!(action, TimerAction::Tick(_)) {
            if mod_ui.poll_enabled_mask_changed() {
                cx.emit(UiEvent::ModUiChanged);
            }
            if mod_ui.poll_external_sync_changed() {
                cx.emit(UiEvent::SyncExternalState);
            }
            cx.needs_redraw();
        }
    });
    cx.start_timer(timer);
}

