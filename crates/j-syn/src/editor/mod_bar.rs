//! Draggable Modulation Source Pills and 60 Hz live telemetry / structural UI redraw ticker.

use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use j_syn_dsp::mod_matrix::{
    ModDestination, ModMatrixSource, ModSlot, ModUiTelemetry, NUM_MOD_DESTS, NUM_MOD_SLOTS,
};
use j_syn_dsp::modseq::{ModSeqPaletteTile, MODSEQ_PALETTE_TILES};
use j_syn_dsp::{FxEngineSwapQueue, FxType, NUM_FX_BUSES, NUM_FX_SLOTS};
use j_syn_ui::ModUiHost;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use super::controls::bold_label;
use super::fx_drag_overlay::FxDragState;
use super::modseq_tab::ModSeqSubTab;
use super::{theme, BottomTab, UiEvent};
use crate::JSynPluginParams;

const UNSET_U8: u8 = 255;
const UNSET_DEST: u16 = u16::MAX;

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
    pub fx_swap_queue: Arc<FxEngineSwapQueue>,
    pub fx_drag: Arc<FxDragState>,
    dragging_source: AtomicU8,
    active_mod_source: AtomicU8,
    hover_dest: AtomicU16,
    active_bottom_tab: AtomicU8,
    active_modseq_subtab: AtomicU8,
    active_view_mode: AtomicU8,
    fx_eq_mask: [AtomicU32; NUM_FX_BUSES],
    fx_collapsed_mask: [AtomicU32; NUM_FX_BUSES],
    mb_dist_tab: [[AtomicU8; NUM_FX_SLOTS]; NUM_FX_BUSES],
    inspected_dest: AtomicU16,
    inspected_slot: AtomicU8,
    active_stamp: AtomicU8,
    active_modseq_row: AtomicU8,
    mseg_snap: AtomicBool,
    last_enabled_mask: AtomicU64,
    external_sync_rev: AtomicU64,
    last_external_sync_rev: AtomicU64,
    pending_preset_name: Mutex<Option<String>>,
    drop_rects: Mutex<Box<[(f32, f32, f32, f32, u8); NUM_MOD_DESTS]>>,
}

impl ModUiState {
    pub fn new(
        params: Arc<JSynPluginParams>,
        telemetry: Arc<ModUiTelemetry>,
        fx_swap_queue: Arc<FxEngineSwapQueue>,
    ) -> Self {
        let initial_mask = Self::compute_enabled_mask(&params);
        Self {
            params,
            telemetry,
            fx_swap_queue,
            fx_drag: Arc::new(FxDragState::new()),
            dragging_source: AtomicU8::new(0),
            active_mod_source: AtomicU8::new(ModMatrixSource::ModSeq1 as u8),
            hover_dest: AtomicU16::new(UNSET_DEST),
            active_bottom_tab: AtomicU8::new(bottom_tab_tag(BottomTab::ModSeq)),
            active_modseq_subtab: AtomicU8::new(0),
            active_view_mode: AtomicU8::new(0),
            fx_eq_mask: std::array::from_fn(|_| AtomicU32::new(0)),
            fx_collapsed_mask: std::array::from_fn(|_| AtomicU32::new(0)),
            mb_dist_tab: std::array::from_fn(|_| std::array::from_fn(|_| AtomicU8::new(0))),
            inspected_dest: AtomicU16::new(UNSET_DEST),
            inspected_slot: AtomicU8::new(UNSET_U8),
            active_stamp: AtomicU8::new(2), // RampDown in MODSEQ_PALETTE_TILES
            active_modseq_row: AtomicU8::new(0),
            mseg_snap: AtomicBool::new(false),
            last_enabled_mask: AtomicU64::new(initial_mask),
            external_sync_rev: AtomicU64::new(0),
            last_external_sync_rev: AtomicU64::new(0),
            pending_preset_name: Mutex::new(None),
            drop_rects: Mutex::new(Box::new([(0.0, 0.0, 0.0, 0.0, UNSET_U8); NUM_MOD_DESTS])),
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
    pub fn active_tile_idx(&self) -> usize {
        (self.active_stamp.load(Ordering::Relaxed) as usize).min(MODSEQ_PALETTE_TILES.len() - 1)
    }

    #[inline]
    pub fn active_tile(&self) -> ModSeqPaletteTile {
        MODSEQ_PALETTE_TILES[self.active_tile_idx()]
    }

    #[inline]
    pub fn active_modseq_row(&self) -> usize {
        (self.active_modseq_row.load(Ordering::Relaxed) as usize).min(1)
    }

    pub fn set_active_modseq_row(&self, row: usize) {
        self.active_modseq_row
            .store((row.min(1)) as u8, Ordering::Relaxed);
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
        let order = params.fx_rack_order.snapshot();
        Self::fnv1a_mix(&mut h, order.explicit as u64);
        for bus in 0..NUM_FX_BUSES {
            Self::fnv1a_mix(&mut h, params.fx_bus_enabled_param(bus).value() as u64);
            Self::fnv1a_mix(&mut h, params.fx_bus_muted_param(bus).value() as u64);
            let active_slots = order.active_slots(bus);
            Self::fnv1a_mix(&mut h, active_slots.len() as u64);
            for &s in active_slots {
                Self::fnv1a_mix(&mut h, s as u64);
            }
            for fx in params.fx_bus_slots(bus) {
                let ty = FxType::from(fx.fx_type.value());
                if ty == FxType::Off {
                    Self::fnv1a_mix(&mut h, 0);
                    continue;
                }
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
                    | ((fx.distortion_mode.value() as u64) << 23)
                    | ((fx.clipper_pcm.value() as u64) << 27);
                Self::fnv1a_mix(&mut h, packed);
                if ty == FxType::MultibandDist {
                    for mb in &fx.mb_bands {
                        let mb_packed = (mb.enabled.value() as u64)
                            | ((mb.solo.value() as u64) << 1)
                            | ((mb.mode.value() as u64) << 2);
                        Self::fnv1a_mix(&mut h, mb_packed);
                    }
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

    pub fn set_active_tile_idx(&self, idx: usize) {
        self.active_stamp.store(
            idx.min(MODSEQ_PALETTE_TILES.len() - 1) as u8,
            Ordering::Relaxed,
        );
    }

    pub fn set_active_tab(&self, tab: BottomTab) {
        self.active_bottom_tab
            .store(bottom_tab_tag(tab), Ordering::Relaxed);
    }

    pub fn set_modseq_subtab(&self, sub: ModSeqSubTab) {
        let (code, src) = match sub {
            ModSeqSubTab::MSeq(i) => {
                let idx = i.min(7);
                (idx as u8, ModMatrixSource::MODSEQS[idx])
            }
            ModSeqSubTab::Env(i) => {
                let idx = i.min(7);
                (8 + idx as u8, ModMatrixSource::ENVS[idx])
            }
        };
        self.active_modseq_subtab.store(code, Ordering::Relaxed);
        self.set_active_mod_source(src);
    }

    #[inline]
    pub fn active_mod_source(&self) -> ModMatrixSource {
        ModMatrixSource::from_u8(self.active_mod_source.load(Ordering::Relaxed))
    }

    pub fn set_active_mod_source(&self, src: ModMatrixSource) {
        if src != ModMatrixSource::None {
            self.active_mod_source.store(src as u8, Ordering::Relaxed);
        }
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
            ModMatrixSource::ModSeq5 => code == 4,
            ModMatrixSource::ModSeq6 => code == 5,
            ModMatrixSource::ModSeq7 => code == 6,
            ModMatrixSource::ModSeq8 => code == 7,
            ModMatrixSource::Env1 => code == 8,
            ModMatrixSource::Env2 => code == 9,
            ModMatrixSource::Env3 => code == 10,
            ModMatrixSource::Env4 => code == 11,
            ModMatrixSource::Env5 => code == 12,
            ModMatrixSource::Env6 => code == 13,
            ModMatrixSource::Env7 => code == 14,
            ModMatrixSource::Env8 => code == 15,
            _ => false,
        }
    }

    #[inline]
    pub fn dragging_source(&self) -> ModMatrixSource {
        ModMatrixSource::from_u8(self.dragging_source.load(Ordering::Relaxed))
    }

    pub fn start_drag(&self, src: ModMatrixSource) {
        self.dragging_source.store(src as u8, Ordering::Relaxed);
        self.set_active_mod_source(src);
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

    pub fn unregister_drop_rect(&self, dest: ModDestination) {
        if let Ok(mut rects) = self.drop_rects.lock() {
            rects[dest.index()] = (0.0, 0.0, 0.0, 0.0, UNSET_U8);
        }
    }

    pub fn set_view_mode(&self, mode: u8) {
        self.active_view_mode.store(mode, Ordering::Relaxed);
    }

    #[inline]
    pub fn fx_eq_subtab(&self, bus: usize, slot: usize) -> bool {
        let b = bus.min(NUM_FX_BUSES - 1);
        let s = slot.min(NUM_FX_SLOTS - 1) as u32;
        ((self.fx_eq_mask[b].load(Ordering::Relaxed) >> s) & 1) != 0
    }

    pub fn set_fx_eq_subtab(&self, bus: usize, slot: usize, is_eq: bool) {
        let b = bus.min(NUM_FX_BUSES - 1);
        let bit = 1_u32 << (slot.min(NUM_FX_SLOTS - 1) as u32);
        if is_eq {
            self.fx_eq_mask[b].fetch_or(bit, Ordering::Relaxed);
        } else {
            self.fx_eq_mask[b].fetch_and(!bit, Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn fx_collapsed(&self, bus: usize, slot: usize) -> bool {
        let b = bus.min(NUM_FX_BUSES - 1);
        let s = slot.min(NUM_FX_SLOTS - 1) as u32;
        ((self.fx_collapsed_mask[b].load(Ordering::Relaxed) >> s) & 1) != 0
    }

    pub fn set_fx_collapsed(&self, bus: usize, slot: usize, collapsed: bool) {
        let b = bus.min(NUM_FX_BUSES - 1);
        let bit = 1_u32 << (slot.min(NUM_FX_SLOTS - 1) as u32);
        if collapsed {
            self.fx_collapsed_mask[b].fetch_or(bit, Ordering::Relaxed);
        } else {
            self.fx_collapsed_mask[b].fetch_and(!bit, Ordering::Relaxed);
        }
    }

    pub fn toggle_fx_collapsed(&self, bus: usize, slot: usize) -> bool {
        let b = bus.min(NUM_FX_BUSES - 1);
        let bit = 1_u32 << (slot.min(NUM_FX_SLOTS - 1) as u32);
        let prev = self.fx_collapsed_mask[b].fetch_xor(bit, Ordering::Relaxed);
        (prev & bit) == 0
    }

    pub fn set_lane_collapsed(&self, bus: usize, collapsed: bool) {
        let b = bus.min(NUM_FX_BUSES - 1);
        let mask = if collapsed {
            (1u32 << (NUM_FX_SLOTS as u32)) - 1
        } else {
            0
        };
        self.fx_collapsed_mask[b].store(mask, Ordering::Relaxed);
    }

    #[inline]
    pub fn fx_mb_tab(&self, bus: usize, slot: usize) -> u8 {
        self.mb_dist_tab[bus.min(NUM_FX_BUSES - 1)][slot.min(NUM_FX_SLOTS - 1)]
            .load(Ordering::Relaxed)
            .min(3)
    }

    pub fn set_fx_mb_tab(&self, bus: usize, slot: usize, tab: u8) {
        self.mb_dist_tab[bus.min(NUM_FX_BUSES - 1)][slot.min(NUM_FX_SLOTS - 1)]
            .store(tab.min(3), Ordering::Relaxed);
    }

    pub fn copy_fx_ui_state(
        &self,
        from_bus: usize,
        from_slot: usize,
        to_bus: usize,
        to_slot: usize,
    ) {
        self.set_fx_eq_subtab(to_bus, to_slot, self.fx_eq_subtab(from_bus, from_slot));
        self.set_fx_collapsed(to_bus, to_slot, self.fx_collapsed(from_bus, from_slot));
        self.set_fx_mb_tab(to_bus, to_slot, self.fx_mb_tab(from_bus, from_slot));
    }

    fn hit_test_cursor(&self, x: f32, y: f32) -> Option<usize> {
        if self.active_view_mode.load(Ordering::Relaxed) != 0 {
            return None;
        }
        let active_tab = self.active_bottom_tab.load(Ordering::Relaxed);
        let rects = self.drop_rects.lock().ok()?;
        for (idx, &(rx, ry, rw, rh, tag)) in rects.iter().enumerate() {
            if rw <= 1.0 || rh <= 1.0 {
                continue;
            }
            if tag != 0 && tag != active_tab {
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
        self.set_active_mod_source(src);
        self.inspected_dest
            .store(dest.index() as u16, Ordering::Relaxed);
        self.inspected_slot.store(slot_idx as u8, Ordering::Relaxed);
        Some(slot_idx)
    }

    /// Collects up to 4 active modulation slots targeting `dest` for ring/bar rendering.
    pub fn slots_for_dest(&self, dest: ModDestination) -> ([ModSlot; 4], [usize; 4], usize) {
        let mat = self.params.mod_matrix.0.load();
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
        let mat = self.params.mod_matrix.0.load();
        mat.slots
            .iter()
            .any(|s| s.enabled && s.source != ModMatrixSource::None && s.destination == dest)
    }

    /// Returns the active modulation slot targeting `dest` for the currently focused source pill.
    pub fn active_slot_for_dest(&self, dest: ModDestination) -> Option<(usize, ModSlot)> {
        let active_src = self.active_mod_source();
        if active_src == ModMatrixSource::None {
            return None;
        }
        let mat = self.params.mod_matrix.0.load();
        mat.slots
            .iter()
            .enumerate()
            .find(|(_, s)| s.enabled && s.source == active_src && s.destination == dest)
            .map(|(idx, s)| (idx, *s))
    }

    pub fn primary_slot_for_dest(&self, dest: ModDestination) -> Option<(usize, ModSlot)> {
        self.active_slot_for_dest(dest)
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
        let mat = self.params.mod_matrix.load();
        mat.slots
            .iter()
            .filter(|s| s.enabled && s.source != ModMatrixSource::None)
            .count()
    }

    pub fn active_entries(&self) -> Vec<(usize, ModSlot)> {
        let mat = self.params.mod_matrix.load();
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
            mat.rebuild_index();
            self.params.mod_matrix.store_value(mat);
        }
    }

    pub fn all_slots_for_dest(&self, dest: ModDestination) -> Vec<(usize, ModSlot)> {
        let mat = self.params.mod_matrix.load();
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
            mat.rebuild_index();
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
            mat.rebuild_index();
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
            mat.rebuild_index();
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

impl ModUiHost for ModUiState {
    #[inline]
    fn dragging_source(&self) -> ModMatrixSource {
        ModUiState::dragging_source(self)
    }

    #[inline]
    fn active_mod_source(&self) -> ModMatrixSource {
        ModUiState::active_mod_source(self)
    }

    #[inline]
    fn is_drop_target(&self, dest: ModDestination) -> bool {
        ModUiState::is_drop_target(self, dest)
    }

    fn register_drop_rect(&self, dest: ModDestination, tab_tag: u8, bounds: BoundingBox) {
        ModUiState::register_drop_rect(self, dest, tab_tag, bounds);
    }

    fn unregister_drop_rect(&self, dest: ModDestination) {
        ModUiState::unregister_drop_rect(self, dest);
    }

    fn slots_for_dest(&self, dest: ModDestination) -> ([ModSlot; 4], [usize; 4], usize) {
        ModUiState::slots_for_dest(self, dest)
    }

    #[inline]
    fn has_slots_for_dest(&self, dest: ModDestination) -> bool {
        ModUiState::has_slots_for_dest(self, dest)
    }

    fn active_slot_for_dest(&self, dest: ModDestination) -> Option<(usize, ModSlot)> {
        ModUiState::active_slot_for_dest(self, dest)
    }

    fn primary_slot_for_dest(&self, dest: ModDestination) -> Option<(usize, ModSlot)> {
        ModUiState::primary_slot_for_dest(self, dest)
    }

    fn focus_slot(&self, dest: ModDestination, slot_idx: usize) {
        ModUiState::focus_slot(self, dest, slot_idx);
    }

    fn adjust_slot_depth(&self, slot_idx: usize, new_depth: f32) {
        ModUiState::adjust_slot_depth(self, slot_idx, new_depth);
    }

    fn remove_primary_slot_for_dest(&self, dest: ModDestination) -> bool {
        ModUiState::remove_primary_slot_for_dest(self, dest)
    }

    #[inline]
    fn dest_offset(&self, dest: ModDestination) -> f32 {
        self.telemetry.dest_offset(dest)
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
            ModMatrixSource::ModSeq5 => Some(ModSeqSubTab::MSeq(4)),
            ModMatrixSource::ModSeq6 => Some(ModSeqSubTab::MSeq(5)),
            ModMatrixSource::ModSeq7 => Some(ModSeqSubTab::MSeq(6)),
            ModMatrixSource::ModSeq8 => Some(ModSeqSubTab::MSeq(7)),
            ModMatrixSource::Env1 => Some(ModSeqSubTab::Env(0)),
            ModMatrixSource::Env2 => Some(ModSeqSubTab::Env(1)),
            ModMatrixSource::Env3 => Some(ModSeqSubTab::Env(2)),
            ModMatrixSource::Env4 => Some(ModSeqSubTab::Env(3)),
            ModMatrixSource::Env5 => Some(ModSeqSubTab::Env(4)),
            ModMatrixSource::Env6 => Some(ModSeqSubTab::Env(5)),
            ModMatrixSource::Env7 => Some(ModSeqSubTab::Env(6)),
            ModMatrixSource::Env8 => Some(ModSeqSubTab::Env(7)),
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
        let pal = theme::current_palette(cx);
        let (r, g, b) = theme::mod_source_rgb(&pal, self.source);
        let is_dragging = self.mod_ui.dragging_source() == self.source;
        let is_arc_active = self.mod_ui.active_mod_source() == self.source;
        let is_active_tab = self.mod_ui.is_source_tab_active(self.source);
        let is_hovered = self.hovered;
        let level = self.mod_ui.telemetry.source_level(self.source).clamp(0.0, 1.0);

        let mut bg = vg::Path::new();
        bg.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 3.0 * dpx);
        let fill = if is_dragging {
            vg::Color::rgba(r, g, b, 105)
        } else if is_arc_active {
            let (mr, mg, mb) = theme::hex_to_rgb(&pal.surface_hover);
            let (tr, tg, tb) = theme::lerp_rgb((mr, mg, mb), (r, g, b), 0.26);
            vg::Color::rgb(tr, tg, tb)
        } else if is_active_tab {
            let (mr, mg, mb) = theme::hex_to_rgb(&pal.surface_mid);
            let (tr, tg, tb) = theme::lerp_rgb((mr, mg, mb), (r, g, b), 0.12);
            vg::Color::rgb(tr, tg, tb)
        } else if is_hovered {
            theme::vg_color(&pal.surface_hover, 255)
        } else {
            theme::vg_color(&pal.surface_mid, 255)
        };
        canvas.fill_path(&bg, &vg::Paint::color(fill));

        // Subtle inner glow frame on the currently active modulation source pill
        if is_arc_active || is_dragging {
            let mut glow = vg::Path::new();
            glow.rounded_rect(
                bounds.x + 0.8 * dpx,
                bounds.y + 0.8 * dpx,
                (bounds.w - 1.6 * dpx).max(1.0),
                (bounds.h - 1.6 * dpx).max(1.0),
                2.5 * dpx,
            );
            let mut glow_paint = vg::Paint::color(vg::Color::rgba(r, g, b, 68));
            glow_paint.set_line_width(2.2 * dpx);
            canvas.stroke_path(&glow, &glow_paint);
        }

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
        let stripe_w = if is_arc_active {
            3.2 * dpx
        } else if is_active_tab {
            2.6 * dpx
        } else {
            2.0 * dpx
        };
        let mut stripe = vg::Path::new();
        stripe.rounded_rect(
            bounds.x + 1.5 * dpx,
            bounds.y + 3.0 * dpx,
            stripe_w,
            bounds.h - 6.0 * dpx,
            1.0 * dpx,
        );
        canvas.fill_path(&stripe, &vg::Paint::color(vg::Color::rgb(r, g, b)));

        let mut border = vg::Paint::color(if is_dragging || is_arc_active {
            vg::Color::rgb(r, g, b)
        } else if is_hovered {
            vg::Color::rgba(r, g, b, 195)
        } else if is_active_tab {
            vg::Color::rgba(r, g, b, 135)
        } else {
            vg::Color::rgba(r, g, b, 85)
        });
        border.set_line_width(
            (if is_arc_active || is_dragging {
                1.65
            } else if is_active_tab {
                1.15
            } else {
                1.0
            }) * dpx,
        );
        canvas.stroke_path(&bg, &border);
    }
}

/// Starts the single 60 Hz UI redraw & structural-state polling timer (`16 ms`)
/// for live mod-ring, scope, playhead animations, FX engine garbage draining, and drag auto-scroll.
pub(crate) fn start_ui_redraw_timer(cx: &mut Context, mod_ui: Arc<ModUiState>) {
    let timer = cx.add_timer(Duration::from_millis(16), None, move |cx, action| {
        if matches!(action, TimerAction::Tick(_)) {
            mod_ui.fx_swap_queue.drain_garbage();
            if let Some((scroll_entity, dy)) =
                mod_ui.fx_drag.edge_scroll_target(cx.scale_factor())
            {
                cx.emit_to(scroll_entity, WindowEvent::MouseScroll(0.0, dy));
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_mod_source_filters_visible_and_editable_arc_cleanly() {
        let params = Arc::new(JSynPluginParams::default());
        let telemetry = Arc::new(ModUiTelemetry::new());
        let swap_queue = Arc::new(FxEngineSwapQueue::new(48_000.0));
        let ui = ModUiState::new(params, telemetry, swap_queue);

        // Assign two different sources (Env2 + ModSeq1) to the same destination
        ui.assign_drop(ModMatrixSource::Env2, ModDestination::Filter1Cutoff)
            .expect("assign Env2");
        ui.assign_drop(ModMatrixSource::ModSeq1, ModDestination::Filter1Cutoff)
            .expect("assign ModSeq1");

        // Last drop (ModSeq1) is active
        assert_eq!(ui.active_mod_source(), ModMatrixSource::ModSeq1);
        let (_, active_ms1) = ui
            .active_slot_for_dest(ModDestination::Filter1Cutoff)
            .expect("ModSeq1 slot active");
        assert_eq!(active_ms1.source, ModMatrixSource::ModSeq1);
        assert_eq!(
            ui.primary_slot_for_dest(ModDestination::Filter1Cutoff)
                .map(|(_, s)| s.source),
            Some(ModMatrixSource::ModSeq1)
        );

        // Switching active pill to Env2 switches arc & collar editing to Env2
        ui.set_active_mod_source(ModMatrixSource::Env2);
        let (_, active_e2) = ui
            .active_slot_for_dest(ModDestination::Filter1Cutoff)
            .expect("Env2 slot active");
        assert_eq!(active_e2.source, ModMatrixSource::Env2);

        // Switching to an unrouted pill (Macro1) keeps has_slots_for_dest true (Option A track+dot)
        // while returning None for active_slot_for_dest / primary_slot_for_dest
        ui.set_active_mod_source(ModMatrixSource::Macro1);
        assert!(ui.has_slots_for_dest(ModDestination::Filter1Cutoff));
        assert!(ui
            .active_slot_for_dest(ModDestination::Filter1Cutoff)
            .is_none());
        assert!(ui
            .primary_slot_for_dest(ModDestination::Filter1Cutoff)
            .is_none());
    }
}

