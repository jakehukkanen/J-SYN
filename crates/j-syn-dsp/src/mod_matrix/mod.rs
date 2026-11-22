//! Lock-free 32-slot Modulation Matrix and real-time UI telemetry primitives.

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

/// Pre-allocated fixed-size 32-slot modulation matrix (`Copy`, lock-free swap & persistence).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ModMatrixState {
    pub slots: [ModSlot; NUM_MOD_SLOTS],
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ModMatrixStateWire {
    #[serde(deserialize_with = "crate::serde_lenient::fixed_array")]
    slots: [ModSlot; NUM_MOD_SLOTS],
}

impl<'de> Deserialize<'de> for ModMatrixState {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Some(w) = Option::<ModMatrixStateWire>::deserialize(deserializer)? else {
            return Ok(Self::default());
        };
        Ok(Self { slots: w.slots })
    }
}

impl Default for ModMatrixState {
    fn default() -> Self {
        Self {
            slots: [ModSlot::default(); NUM_MOD_SLOTS],
        }
    }
}

impl ModMatrixState {
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
            return Some(free_idx);
        }
        None
    }

    pub fn clear_slot(&mut self, slot_idx: usize) {
        if slot_idx < NUM_MOD_SLOTS {
            self.slots[slot_idx] = ModSlot::default();
        }
    }

    pub fn swap_slots(&mut self, a: usize, b: usize) {
        if a < NUM_MOD_SLOTS && b < NUM_MOD_SLOTS && a != b {
            self.slots.swap(a, b);
        }
    }

    /// Evaluates all 32 slots for a voice's current `[0..1]` source values and accumulates
    /// per-destination normalized offsets into `out_deltas`. Zero heap allocations.
    #[inline]
    pub fn evaluate_destinations(
        &self,
        source_values: &[f32; NUM_ACTIVE_SOURCES],
        out_deltas: &mut [f32; NUM_MOD_DESTS],
    ) {
        out_deltas.fill(0.0);
        for slot in &self.slots {
            if !slot.enabled || slot.bypassed {
                continue;
            }
            if let Some(src_idx) = slot.source.active_index() {
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
                let mut sig = if slot.bipolar {
                    raw * 2.0 - 1.0
                } else {
                    raw
                };
                if let Some(aux_idx) = slot.aux_source.active_index() {
                    sig *= source_values[aux_idx].clamp(0.0, 1.0);
                }
                let dest_idx = slot.destination.index();
                if dest_idx < NUM_MOD_DESTS {
                    out_deltas[dest_idx] += sig * slot.depth.clamp(-1.0, 1.0);
                }
            }
        }
    }
}

