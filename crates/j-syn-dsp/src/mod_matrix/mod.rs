//! Lock-free 256-slot Modulation Matrix and real-time UI telemetry primitives.

use serde::{Deserialize, Serialize};

mod destinations;
mod telemetry;

pub use destinations::*;
pub use telemetry::*;

/// Single modulation routing slot inside the fixed 32-slot matrix.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct ModSlot {
    pub source: ModMatrixSource,
    pub destination: ModDestination,
    pub depth: f32,
    pub bipolar: bool,
    pub enabled: bool,
    pub bypassed: bool,
    pub aux_source: ModMatrixSource,
    pub curve: f32,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ModSlotWire {
    #[serde(deserialize_with = "crate::serde_lenient::known_variant")]
    source: Option<ModMatrixSource>,
    #[serde(deserialize_with = "crate::serde_lenient::known_variant")]
    destination: Option<ModDestination>,
    depth: f32,
    bipolar: bool,
    enabled: bool,
    bypassed: bool,
    #[serde(deserialize_with = "crate::serde_lenient::known_variant")]
    aux_source: Option<ModMatrixSource>,
    curve: f32,
}

impl<'de> Deserialize<'de> for ModSlot {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Some(w) = Option::<ModSlotWire>::deserialize(deserializer)? else {
            return Ok(Self::default());
        };
        let (source, destination, mut enabled) = match (w.source, w.destination) {
            (Some(src), Some(dst)) => (src, dst, w.enabled),
            (Some(src), None) => (src, ModDestination::default(), false),
            (None, dst) => (ModMatrixSource::None, dst.unwrap_or_default(), false),
        };
        if source == ModMatrixSource::None {
            enabled = false;
        }
        Ok(Self {
            source,
            destination,
            depth: crate::serde_lenient::finite_clamped(w.depth, 0.0, -1.0, 1.0),
            bipolar: w.bipolar,
            enabled,
            bypassed: w.bypassed,
            aux_source: w.aux_source.unwrap_or(ModMatrixSource::None),
            curve: crate::serde_lenient::finite_clamped(w.curve, 0.0, -1.0, 3.0),
        })
    }
}

/// Pre-allocated fixed-size 256-slot modulation matrix (`Copy`, lock-free swap & compacted evaluation).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModMatrixState {
    pub slots: [ModSlot; NUM_MOD_SLOTS],
    active_indices: [u16; NUM_MOD_SLOTS],
    active_len: u16,
    active_source_mask: u32,
}

impl Serialize for ModMatrixState {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let trimmed_len = self
            .slots
            .iter()
            .rposition(|s| s.enabled && s.source != ModMatrixSource::None)
            .map(|i| i + 1)
            .unwrap_or(0);
        let mut st = serializer.serialize_struct("ModMatrixState", 1)?;
        st.serialize_field("slots", &self.slots[..trimmed_len])?;
        st.end()
    }
}

#[derive(Deserialize)]
#[serde(default)]
struct ModMatrixStateWire {
    #[serde(deserialize_with = "crate::serde_lenient::fixed_array")]
    slots: [ModSlot; NUM_MOD_SLOTS],
}

impl Default for ModMatrixStateWire {
    fn default() -> Self {
        Self {
            slots: [ModSlot::default(); NUM_MOD_SLOTS],
        }
    }
}

impl<'de> Deserialize<'de> for ModMatrixState {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Some(w) = Option::<ModMatrixStateWire>::deserialize(deserializer)? else {
            return Ok(Self::default());
        };
        let mut out = Self {
            slots: w.slots,
            active_indices: [0; NUM_MOD_SLOTS],
            active_len: 0,
            active_source_mask: 0,
        };
        out.rebuild_index();
        Ok(out)
    }
}

impl Default for ModMatrixState {
    fn default() -> Self {
        Self {
            slots: [ModSlot::default(); NUM_MOD_SLOTS],
            active_indices: [0; NUM_MOD_SLOTS],
            active_len: 0,
            active_source_mask: 0,
        }
    }
}

impl ModMatrixState {
    /// Rebuilds the compacted active-slot index and `active_source_mask`.
    /// Must be called after mutating `self.slots` directly.
    pub fn rebuild_index(&mut self) {
        let mut len = 0usize;
        let mut mask = 0u32;
        for (i, slot) in self.slots.iter().enumerate() {
            if slot.enabled && !slot.bypassed && slot.source != ModMatrixSource::None {
                self.active_indices[len] = i as u16;
                len += 1;
                mask |= 1u32 << (slot.source as u8);
                if slot.aux_source != ModMatrixSource::None {
                    mask |= 1u32 << (slot.aux_source as u8);
                }
            }
        }
        self.active_len = len as u16;
        self.active_source_mask = mask;
    }

    #[inline]
    pub fn active_source_mask(&self) -> u32 {
        self.active_source_mask
    }

    #[inline]
    pub fn is_source_routed(&self, src: ModMatrixSource) -> bool {
        (self.active_source_mask & (1u32 << (src as u8))) != 0
    }

    /// Finds an active slot matching `(source, destination)` or allocates the first free slot.
    pub fn assign_or_update(
        &mut self,
        source: ModMatrixSource,
        destination: ModDestination,
        default_depth: f32,
    ) -> Option<usize> {
        if source == ModMatrixSource::None {
            return None;
        }
        if let Some(idx) = self
            .slots
            .iter()
            .position(|s| s.enabled && s.source == source && s.destination == destination)
        {
            return Some(idx);
        }
        if let Some(free_idx) = self
            .slots
            .iter()
            .position(|s| !s.enabled || s.source == ModMatrixSource::None)
        {
            self.slots[free_idx] = ModSlot {
                source,
                destination,
                depth: default_depth.clamp(-1.0, 1.0),
                bipolar: false,
                enabled: true,
                bypassed: false,
                aux_source: ModMatrixSource::None,
                curve: 0.0,
            };
            self.rebuild_index();
            return Some(free_idx);
        }
        None
    }

    pub fn clear_slot(&mut self, slot_idx: usize) {
        if slot_idx < NUM_MOD_SLOTS {
            self.slots[slot_idx] = ModSlot::default();
            self.rebuild_index();
        }
    }

    pub fn swap_slots(&mut self, a: usize, b: usize) {
        if a < NUM_MOD_SLOTS && b < NUM_MOD_SLOTS && a != b {
            self.slots.swap(a, b);
            self.rebuild_index();
        }
    }

    /// Evaluates active modulation slots into voice/base destination offsets (`0..NUM_VOICE_MOD_DESTS`).
    /// Loops only over compacted active slots so unused capacity costs zero CPU.
    #[inline]
    pub fn evaluate_voice_destinations(
        &self,
        source_values: &[f32; NUM_ACTIVE_SOURCES],
        out_deltas: &mut [f32; NUM_VOICE_MOD_DESTS],
    ) {
        out_deltas.fill(0.0);
        for &slot_idx in &self.active_indices[..self.active_len as usize] {
            let slot = &self.slots[slot_idx as usize];
            let dest_idx = slot.destination.index();
            if dest_idx >= NUM_VOICE_MOD_DESTS {
                continue;
            }
            if let Some(sig) = Self::eval_slot_signal(slot, source_values) {
                out_deltas[dest_idx] += sig * slot.depth.clamp(-1.0, 1.0);
            }
        }
    }

    /// Evaluates all active slots for a voice's current `[0..1]` source values and accumulates
    /// per-destination normalized offsets into `out_deltas`. Zero heap allocations.
    #[inline]
    pub fn evaluate_destinations(
        &self,
        source_values: &[f32; NUM_ACTIVE_SOURCES],
        out_deltas: &mut [f32; NUM_MOD_DESTS],
    ) {
        out_deltas.fill(0.0);
        for &slot_idx in &self.active_indices[..self.active_len as usize] {
            let slot = &self.slots[slot_idx as usize];
            let dest_idx = slot.destination.index();
            if dest_idx >= NUM_MOD_DESTS {
                continue;
            }
            if let Some(sig) = Self::eval_slot_signal(slot, source_values) {
                out_deltas[dest_idx] += sig * slot.depth.clamp(-1.0, 1.0);
            }
        }
    }

    #[inline]
    fn eval_slot_signal(
        slot: &ModSlot,
        source_values: &[f32; NUM_ACTIVE_SOURCES],
    ) -> Option<f32> {
        let src_idx = slot.source.active_index()?;
        let mut raw = source_values[src_idx].clamp(0.0, 1.0);
        if (slot.curve - 2.0).abs() < 0.1 {
            raw = raw * raw * (3.0 - 2.0 * raw);
        } else if (slot.curve - 3.0).abs() < 0.1 {
            raw = (raw * 4.0).floor() * 0.25;
        } else {
            let curve = slot.curve.clamp(-1.0, 1.0);
            if curve.abs() > 1e-4 {
                let exp = (curve * 2.5).exp();
                raw = raw.powf(exp);
            }
        }
        let mut sig = if slot.bipolar { raw * 2.0 - 1.0 } else { raw };
        if let Some(aux_idx) = slot.aux_source.active_index() {
            sig *= source_values[aux_idx].clamp(0.0, 1.0);
        }
        Some(sig)
    }
}

