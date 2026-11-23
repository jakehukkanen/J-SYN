//! Lazy per-slot FX engine enum (`ActiveFxEngine`), explicit drag-and-drop lane ordering
//! (`FxRackOrder`), and wait-free audio-thread engine swap / garbage-return queues
//! (`FxEngineSwapQueue`) enabling 90 FX units (`30 × 3`) with minimal RAM and zero RT alloc/free.

use std::cell::UnsafeCell;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::diopser::DiopserStereo;
use super::dynamics::{DistortionStereo, JSatClipperStereo, OttStereo};
use super::limiter::LimiterStereo;
use super::modulation::{ChorusStereo, DelayStereo, FlangerStereo, PhaserStereo, ReverbStereo};
use super::multiband_dist::MultiBandDistStereo;
use super::tube_resonator::TubeResonatorStereo;
use super::utility::UtilityStereo;
use super::{
    FxBusParams, FxFilterStereo, FxSlotParams, FxType, MAX_FX_STACK, MbDistBandParams,
    NUM_FX_BUSES, NUM_FX_SLOTS,
};

/// Single active FX processing stage. Empty (`Off`) slots hold zero heap buffers.
#[allow(clippy::large_enum_variant)]
pub enum ActiveFxEngine {
    Off,
    Ott(Box<OttStereo>),
    Distortion(Box<DistortionStereo>),
    Chorus(Box<ChorusStereo>),
    Delay(Box<DelayStereo>),
    Reverb(Box<ReverbStereo>),
    JSatClipper(Box<JSatClipperStereo>),
    Filter(Box<FxFilterStereo>),
    Phaser(Box<PhaserStereo>),
    Diopser(Box<DiopserStereo>),
    TubeResonator(Box<TubeResonatorStereo>),
    Utility(Box<UtilityStereo>),
    Limiter(Box<LimiterStereo>),
    MultibandDist(Box<MultiBandDistStereo>),
    Flanger(Box<FlangerStereo>),
}

impl ActiveFxEngine {
    /// Constructs the exact DSP processor for `fx_type` (call off the audio thread).
    pub fn new_for_type(fx_type: FxType, sample_rate: f32) -> Self {
        let sr = sample_rate.max(1_000.0);
        match fx_type {
            FxType::Off => Self::Off,
            FxType::Ott => Self::Ott(Box::new(OttStereo::new(sr))),
            FxType::Distortion => Self::Distortion(Box::new(DistortionStereo::new(sr))),
            FxType::Chorus => Self::Chorus(Box::new(ChorusStereo::new(sr))),
            FxType::Delay => Self::Delay(Box::new(DelayStereo::new(sr))),
            FxType::Reverb => Self::Reverb(Box::new(ReverbStereo::new(sr))),
            FxType::JSatClipper => Self::JSatClipper(Box::default()),
            FxType::Filter => Self::Filter(Box::new(FxFilterStereo::new(sr))),
            FxType::Phaser => Self::Phaser(Box::new(PhaserStereo::new(sr))),
            FxType::Diopser => Self::Diopser(Box::new(DiopserStereo::new(sr))),
            FxType::TubeResonator => Self::TubeResonator(Box::new(TubeResonatorStereo::new(sr))),
            FxType::Utility => Self::Utility(Box::new(UtilityStereo::new(sr))),
            FxType::Limiter => Self::Limiter(Box::new(LimiterStereo::new(sr))),
            FxType::MultibandDist => Self::MultibandDist(Box::new(MultiBandDistStereo::new(sr))),
            FxType::Flanger => Self::Flanger(Box::new(FlangerStereo::new(sr))),
        }
    }

    #[inline]
    pub fn fx_type(&self) -> FxType {
        match self {
            Self::Off => FxType::Off,
            Self::Ott(_) => FxType::Ott,
            Self::Distortion(_) => FxType::Distortion,
            Self::Chorus(_) => FxType::Chorus,
            Self::Delay(_) => FxType::Delay,
            Self::Reverb(_) => FxType::Reverb,
            Self::JSatClipper(_) => FxType::JSatClipper,
            Self::Filter(_) => FxType::Filter,
            Self::Phaser(_) => FxType::Phaser,
            Self::Diopser(_) => FxType::Diopser,
            Self::TubeResonator(_) => FxType::TubeResonator,
            Self::Utility(_) => FxType::Utility,
            Self::Limiter(_) => FxType::Limiter,
            Self::MultibandDist(_) => FxType::MultibandDist,
            Self::Flanger(_) => FxType::Flanger,
        }
    }

    #[inline]
    pub fn is_off(&self) -> bool {
        matches!(self, Self::Off)
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(1_000.0);
        match self {
            Self::Off | Self::JSatClipper(_) => {}
            Self::Ott(e) => e.set_sample_rate(sr),
            Self::Distortion(e) => e.set_sample_rate(sr),
            Self::Chorus(e) => e.set_sample_rate(sr),
            Self::Delay(e) => e.set_sample_rate(sr),
            Self::Reverb(e) => e.set_sample_rate(sr),
            Self::Filter(e) => e.set_sample_rate(sr),
            Self::Phaser(e) => e.set_sample_rate(sr),
            Self::Diopser(e) => e.set_sample_rate(sr),
            Self::TubeResonator(e) => e.set_sample_rate(sr),
            Self::Utility(e) => e.set_sample_rate(sr),
            Self::Limiter(e) => e.set_sample_rate(sr),
            Self::MultibandDist(e) => e.set_sample_rate(sr),
            Self::Flanger(e) => e.set_sample_rate(sr),
        }
    }

    pub fn reset(&mut self) {
        match self {
            Self::Off => {}
            Self::Ott(e) => e.reset(),
            Self::Distortion(e) => e.reset(),
            Self::Chorus(e) => e.reset(),
            Self::Delay(e) => e.reset(),
            Self::Reverb(e) => e.reset(),
            Self::JSatClipper(e) => e.reset(),
            Self::Filter(e) => e.reset(),
            Self::Phaser(e) => e.reset(),
            Self::Diopser(e) => e.reset(),
            Self::TubeResonator(e) => e.reset(),
            Self::Utility(e) => e.reset(),
            Self::Limiter(e) => e.reset(),
            Self::MultibandDist(e) => e.reset(),
            Self::Flanger(e) => e.reset(),
        }
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub fn process_sample(
        &mut self,
        slot: &FxSlotParams,
        in_l: f32,
        in_r: f32,
        p: [f32; 4],
        eq: [f32; 4],
        eff_bands: &[MbDistBandParams; 3],
        tempo_bpm: f32,
    ) -> (f32, f32, f32) {
        match self {
            Self::Off => (in_l, in_r, 0.0),
            Self::Ott(e) => e.process(in_l, in_r, p),
            Self::Distortion(e) => e.process(in_l, in_r, slot.distortion_mode, p),
            Self::Chorus(e) => e.process(in_l, in_r, p, eq),
            Self::Delay(e) => e.process(in_l, in_r, p, eq, tempo_bpm),
            Self::Reverb(e) => e.process(in_l, in_r, p, eq),
            Self::JSatClipper(e) => e.process(in_l, in_r, p, slot.clipper_pcm),
            Self::Filter(e) => e.process(in_l, in_r, slot.filter_mode, slot.filter_slope, p, eq[0]),
            Self::Phaser(e) => e.process(in_l, in_r, p),
            Self::Diopser(e) => e.process(in_l, in_r, p, slot.diopser_stages as usize),
            Self::TubeResonator(e) => e.process(
                in_l,
                in_r,
                p,
                eq,
                slot.tube_res_filter_enabled,
                slot.tube_res_dirty_lfo,
            ),
            Self::Utility(e) => e.process(in_l, in_r, p, slot.utility_inv_l, slot.utility_inv_r),
            Self::Limiter(e) => e.process(in_l, in_r, p, eq, slot.limiter_true_peak),
            Self::MultibandDist(e) => e.process(in_l, in_r, eff_bands, p),
            Self::Flanger(e) => e.process(in_l, in_r, p, eq),
        }
    }
}

/// Persisted visual & DSP execution order for each FX lane (`Bus A`, `Bus B`, `Bus C`).
///
/// Reordering cards inside a lane mutates only `FxRackOrder`, keeping backing `nih-plug`
/// parameter IDs, DAW automation lanes, and `ModMatrix` destinations completely intact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FxRackOrder {
    pub lanes: [[u8; NUM_FX_SLOTS]; NUM_FX_BUSES],
    pub counts: [u8; NUM_FX_BUSES],
    /// When `false` (default for legacy patches and pure-DSP tests), `FxBusState` scans active
    /// (`fx_type != Off`) slots `0..NUM_FX_SLOTS` in ascending order.
    pub explicit: bool,
}

impl Default for FxRackOrder {
    fn default() -> Self {
        Self {
            lanes: [[0; NUM_FX_SLOTS]; NUM_FX_BUSES],
            counts: [0; NUM_FX_BUSES],
            explicit: false,
        }
    }
}

impl FxRackOrder {
    pub const fn empty_explicit() -> Self {
        Self {
            lanes: [[0; NUM_FX_SLOTS]; NUM_FX_BUSES],
            counts: [0; NUM_FX_BUSES],
            explicit: true,
        }
    }

    /// Builds an explicit lane order by scanning non-`Off` slots in ascending index order.
    pub fn from_active_buses(buses: &[FxBusParams; NUM_FX_BUSES]) -> Self {
        let mut order = Self::empty_explicit();
        for (bus_idx, bus) in buses.iter().enumerate() {
            let mut count = 0usize;
            for (slot_idx, slot) in bus.slots.iter().enumerate() {
                if slot.fx_type != FxType::Off && count < NUM_FX_SLOTS {
                    order.lanes[bus_idx][count] = slot_idx as u8;
                    count += 1;
                }
            }
            order.counts[bus_idx] = count as u8;
        }
        order
    }

    #[inline]
    pub fn active_slots(&self, bus: usize) -> &[u8] {
        let b = bus.min(NUM_FX_BUSES - 1);
        let len = (self.counts[b] as usize).min(NUM_FX_SLOTS);
        &self.lanes[b][..len]
    }

    #[inline]
    pub fn len(&self, bus: usize) -> usize {
        self.active_slots(bus).len()
    }

    #[inline]
    pub fn contains_slot(&self, bus: usize, slot: usize) -> bool {
        self.active_slots(bus).contains(&(slot as u8))
    }

    /// Returns the first unused backing slot index (`0..NUM_FX_SLOTS`) in `bus`.
    pub fn first_free_slot(&self, bus: usize) -> Option<usize> {
        let active = self.active_slots(bus);
        if active.len() >= NUM_FX_SLOTS {
            return None;
        }
        (0..NUM_FX_SLOTS).find(|&s| !active.contains(&(s as u8)))
    }

    /// Inserts backing `slot` into `bus` at visual index `pos` (`0..=len`).
    pub fn insert_at(&mut self, bus: usize, pos: usize, slot: usize) -> bool {
        if bus >= NUM_FX_BUSES || slot >= NUM_FX_SLOTS {
            return false;
        }
        self.explicit = true;
        let len = (self.counts[bus] as usize).min(NUM_FX_SLOTS);
        if len >= NUM_FX_SLOTS || self.lanes[bus][..len].contains(&(slot as u8)) {
            return false;
        }
        let target = pos.min(len);
        for i in (target..len).rev() {
            self.lanes[bus][i + 1] = self.lanes[bus][i];
        }
        self.lanes[bus][target] = slot as u8;
        self.counts[bus] = (len + 1) as u8;
        true
    }

    /// Removes backing `slot` from `bus`, compacting subsequent cards upward.
    pub fn remove_slot(&mut self, bus: usize, slot: usize) -> bool {
        if bus >= NUM_FX_BUSES || slot >= NUM_FX_SLOTS {
            return false;
        }
        let len = (self.counts[bus] as usize).min(NUM_FX_SLOTS);
        let Some(pos) = self.lanes[bus][..len].iter().position(|&s| s == slot as u8) else {
            return false;
        };
        for i in pos..(len - 1) {
            self.lanes[bus][i] = self.lanes[bus][i + 1];
        }
        self.lanes[bus][len - 1] = 0;
        self.counts[bus] = (len - 1) as u8;
        self.explicit = true;
        true
    }

    /// Moves the card at visual position `from_pos` to visual insertion index `to_pos` (`0..=len`)
    /// within the same lane. Returns `true` if the order actually changed.
    pub fn move_within_lane(&mut self, bus: usize, from_pos: usize, to_pos: usize) -> bool {
        if bus >= NUM_FX_BUSES {
            return false;
        }
        let len = (self.counts[bus] as usize).min(NUM_FX_SLOTS);
        if from_pos >= len || len <= 1 {
            return false;
        }
        let mut insert_idx = to_pos.min(len);
        if insert_idx > from_pos {
            insert_idx -= 1;
        }
        if insert_idx == from_pos {
            return false;
        }
        let slot = self.lanes[bus][from_pos];
        if insert_idx < from_pos {
            for i in (insert_idx..from_pos).rev() {
                self.lanes[bus][i + 1] = self.lanes[bus][i];
            }
        } else {
            for i in from_pos..insert_idx {
                self.lanes[bus][i] = self.lanes[bus][i + 1];
            }
        }
        self.lanes[bus][insert_idx] = slot;
        self.explicit = true;
        true
    }

    /// Ensures `order` matches live active (`fx_type != Off`) slots: drops any `Off` entries and
    /// appends any active slots that were not yet present (e.g. after legacy recall or host edit).
    pub fn reconcile_with_active_mask(&mut self, active_masks: [u32; NUM_FX_BUSES]) {
        for (bus, &mask) in active_masks.iter().enumerate() {
            let mut next = [0u8; NUM_FX_SLOTS];
            let mut count = 0usize;
            let mut seen = 0u32;
            if self.explicit {
                for &slot in self.active_slots(bus) {
                    let s = slot as usize;
                    if s < NUM_FX_SLOTS && ((mask >> s) & 1) != 0 && ((seen >> s) & 1) == 0 {
                        next[count] = slot;
                        count += 1;
                        seen |= 1u32 << s;
                    }
                }
            }
            for s in 0..NUM_FX_SLOTS {
                if ((mask >> s) & 1) != 0 && ((seen >> s) & 1) == 0 && count < NUM_FX_SLOTS {
                    next[count] = s as u8;
                    count += 1;
                    seen |= 1u32 << s;
                }
            }
            self.lanes[bus] = next;
            self.counts[bus] = count as u8;
        }
        self.explicit = true;
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct FxRackOrderWire {
    lanes: [Vec<u8>; NUM_FX_BUSES],
    explicit: bool,
}

impl Serialize for FxRackOrder {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let wire = FxRackOrderWire {
            lanes: std::array::from_fn(|b| self.active_slots(b).to_vec()),
            explicit: self.explicit,
        };
        wire.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FxRackOrder {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Some(wire) = Option::<FxRackOrderWire>::deserialize(deserializer)? else {
            return Ok(Self::default());
        };
        let mut out = Self {
            lanes: [[0; NUM_FX_SLOTS]; NUM_FX_BUSES],
            counts: [0; NUM_FX_BUSES],
            explicit: wire.explicit,
        };
        for (b, lane) in wire.lanes.into_iter().enumerate() {
            let mut count = 0usize;
            let mut seen = 0u32;
            for s in lane {
                let idx = s as usize;
                if idx < NUM_FX_SLOTS && ((seen >> idx) & 1) == 0 && count < NUM_FX_SLOTS {
                    out.lanes[b][count] = s;
                    count += 1;
                    seen |= 1u32 << idx;
                }
            }
            out.counts[b] = count as u8;
        }
        Ok(out)
    }
}

/// Command sent from non-RT threads to install a freshly constructed `ActiveFxEngine`.
pub struct FxSwapCommand {
    pub bus: u8,
    pub slot: u8,
    pub stack_idx: u8,
    pub engine: ActiveFxEngine,
}

const SWAP_RING_CAP: usize = 512;
const SWAP_RING_MASK: usize = SWAP_RING_CAP - 1;

struct LockFreeRing<T> {
    slots: Box<[UnsafeCell<Option<T>>]>,
    head: AtomicUsize,
    tail: AtomicUsize,
}

// SAFETY: Single-producer / single-consumer discipline is enforced by `FxEngineSwapQueue`
// (non-RT sides serialize behind their respective `Mutex<()>`, RT side is exclusively audio thread).
unsafe impl<T: Send> Send for LockFreeRing<T> {}
unsafe impl<T: Send> Sync for LockFreeRing<T> {}

impl<T> LockFreeRing<T> {
    fn new() -> Self {
        let slots = (0..SWAP_RING_CAP)
            .map(|_| UnsafeCell::new(None))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            slots,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    #[inline]
    fn has_space(&self) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        head.wrapping_sub(tail) < SWAP_RING_CAP
    }

    #[inline]
    fn try_push(&self, item: T) -> Result<(), T> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        if head.wrapping_sub(tail) >= SWAP_RING_CAP {
            return Err(item);
        }
        let idx = head & SWAP_RING_MASK;
        // SAFETY: Producer exclusively owns `slots[idx]` until `head` is published with Release.
        unsafe {
            *self.slots[idx].get() = Some(item);
        }
        self.head.store(head.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    #[inline]
    fn try_pop(&self) -> Option<T> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        if tail == head {
            return None;
        }
        let idx = tail & SWAP_RING_MASK;
        // SAFETY: Consumer exclusively owns `slots[idx]` after observing `head` with Acquire.
        let item = unsafe { (*self.slots[idx].get()).take() };
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        item
    }
}

/// Wait-free audio-thread bridge for swapping lazy `ActiveFxEngine` instances off the RT thread.
pub struct FxEngineSwapQueue {
    cmd_ring: LockFreeRing<FxSwapCommand>,
    garbage_ring: LockFreeRing<ActiveFxEngine>,
    cmd_producer_lock: Mutex<()>,
    garbage_consumer_lock: Mutex<()>,
    provision_mask: [AtomicU32; NUM_FX_BUSES],
    sample_rate_bits: AtomicU32,
}

impl FxEngineSwapQueue {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            cmd_ring: LockFreeRing::new(),
            garbage_ring: LockFreeRing::new(),
            cmd_producer_lock: Mutex::new(()),
            garbage_consumer_lock: Mutex::new(()),
            provision_mask: std::array::from_fn(|_| AtomicU32::new(0)),
            sample_rate_bits: AtomicU32::new(sample_rate.max(1_000.0).to_bits()),
        }
    }

    #[inline]
    pub fn set_sample_rate(&self, sample_rate: f32) {
        self.sample_rate_bits
            .store(sample_rate.max(1_000.0).to_bits(), Ordering::Relaxed);
    }

    #[inline]
    pub fn sample_rate(&self) -> f32 {
        f32::from_bits(self.sample_rate_bits.load(Ordering::Relaxed)).max(1_000.0)
    }

    /// Non-RT: drains & drops any engines previously retired by the audio thread.
    pub fn drain_garbage(&self) {
        if let Ok(_guard) = self.garbage_consumer_lock.lock() {
            while let Some(retired) = self.garbage_ring.try_pop() {
                drop(retired);
            }
        }
    }

    /// Non-RT: provisions `stack` active engines for `(bus, slot)` and retires unused stack stages.
    pub fn provision_slot(&self, bus: usize, slot: usize, fx_type: FxType, stack: usize) {
        if bus >= NUM_FX_BUSES || slot >= NUM_FX_SLOTS {
            return;
        }
        self.drain_garbage();
        let sr = self.sample_rate();
        let active_stages = if fx_type == FxType::Off {
            0
        } else {
            stack.clamp(1, MAX_FX_STACK)
        };
        if let Ok(_guard) = self.cmd_producer_lock.lock() {
            for stack_idx in 0..MAX_FX_STACK {
                let target_ty = if stack_idx < active_stages {
                    fx_type
                } else {
                    FxType::Off
                };
                let cmd = FxSwapCommand {
                    bus: bus as u8,
                    slot: slot as u8,
                    stack_idx: stack_idx as u8,
                    engine: ActiveFxEngine::new_for_type(target_ty, sr),
                };
                let _ = self.cmd_ring.try_push(cmd);
            }
        }
    }

    /// Audio thread (wait-free, zero alloc/free): checks if garbage ring can accept a retired engine.
    #[inline]
    pub(crate) fn can_retire(&self) -> bool {
        self.garbage_ring.has_space()
    }

    /// Audio thread (wait-free, zero alloc/free): pops next pending swap command.
    #[inline]
    pub(crate) fn try_pop_cmd(&self) -> Option<FxSwapCommand> {
        self.cmd_ring.try_pop()
    }

    /// Audio thread (wait-free, zero alloc/free): pushes a displaced non-`Off` engine to be dropped off-RT.
    #[inline]
    pub(crate) fn retire_engine(&self, engine: ActiveFxEngine) {
        if !engine.is_off() {
            // Caller verified `can_retire()` before popping, so `try_push` succeeds without dropping on RT.
            let _ = self.garbage_ring.try_push(engine);
        }
    }

    /// Audio thread (wait-free): marks `(bus, slot)` as needing non-RT engine provisioning.
    #[inline]
    pub(crate) fn mark_needs_provision(&self, bus: usize, slot: usize) {
        if bus < NUM_FX_BUSES && slot < NUM_FX_SLOTS {
            self.provision_mask[bus].fetch_or(1u32 << (slot as u32), Ordering::Release);
        }
    }

    /// Atomically reads & clears pending provision masks for background task dispatch.
    #[inline]
    pub fn take_provision_masks(&self) -> [u32; NUM_FX_BUSES] {
        [
            self.provision_mask[0].swap(0, Ordering::AcqRel),
            self.provision_mask[1].swap(0, Ordering::AcqRel),
            self.provision_mask[2].swap(0, Ordering::AcqRel),
        ]
    }
}
