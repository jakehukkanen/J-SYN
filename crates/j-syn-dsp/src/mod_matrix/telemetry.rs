//! Lock-free atomic UI telemetry rings and snapshots for scopes, meters, and modulation arcs.

use std::sync::atomic::{AtomicU32, Ordering};

use super::destinations::{ModDestination, ModMatrixSource, NUM_ACTIVE_SOURCES, NUM_MOD_DESTS};

pub const FX_SCOPE_LEN: usize = 256;

/// Lock-free atomic ring buffer storing downsampled pre/post peak waveforms & activity for an FX slot.
pub struct FxScopeRing {
    pre_peaks: [AtomicU32; FX_SCOPE_LEN],
    post_peaks: [AtomicU32; FX_SCOPE_LEN],
    write_idx: AtomicU32,
    activity_bits: AtomicU32,
}

impl Default for FxScopeRing {
    fn default() -> Self {
        Self::new()
    }
}

impl FxScopeRing {
    pub fn new() -> Self {
        Self {
            pre_peaks: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            post_peaks: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            write_idx: AtomicU32::new(0),
            activity_bits: AtomicU32::new(0.0_f32.to_bits()),
        }
    }

    #[inline]
    pub fn push(&self, pre_peak: f32, post_peak: f32, activity: f32) {
        let idx = (self.write_idx.load(Ordering::Relaxed) as usize) % FX_SCOPE_LEN;
        self.pre_peaks[idx].store(pre_peak.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
        self.post_peaks[idx].store(post_peak.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
        self.write_idx
            .store(((idx + 1) % FX_SCOPE_LEN) as u32, Ordering::Relaxed);
        self.activity_bits
            .store(activity.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    #[inline]
    pub fn clear(&self) {
        for v in &self.pre_peaks {
            v.store(0.0_f32.to_bits(), Ordering::Relaxed);
        }
        for v in &self.post_peaks {
            v.store(0.0_f32.to_bits(), Ordering::Relaxed);
        }
        self.activity_bits
            .store(0.0_f32.to_bits(), Ordering::Relaxed);
    }

    pub fn read_snapshot(
        &self,
        pre_out: &mut [f32; FX_SCOPE_LEN],
        post_out: &mut [f32; FX_SCOPE_LEN],
    ) -> f32 {
        let head = (self.write_idx.load(Ordering::Relaxed) as usize) % FX_SCOPE_LEN;
        for i in 0..FX_SCOPE_LEN {
            let idx = (head + i) % FX_SCOPE_LEN;
            pre_out[i] = f32::from_bits(self.pre_peaks[idx].load(Ordering::Relaxed));
            post_out[i] = f32::from_bits(self.post_peaks[idx].load(Ordering::Relaxed));
        }
        f32::from_bits(self.activity_bits.load(Ordering::Relaxed))
    }
}

/// Lock-free atomic telemetry published by the audio thread for 60 Hz UI mod-ring animations.
pub struct ModUiTelemetry {
    pub dest_offsets: [AtomicU32; NUM_MOD_DESTS],
    pub source_levels: [AtomicU32; NUM_ACTIVE_SOURCES],
    pub modseq_phases: [AtomicU32; 4],
    pub env_levels: [AtomicU32; 4],
    /// Post-modulation `[cutoff_hz, resonance, drive, aux]` for `FILTER 1` and `FILTER 2`.
    pub filter_live: [[AtomicU32; 4]; 2],
    /// Lock-free FX scopes for `Bus A` (`0..8`), `Bus B` (`8..16`), and `Bus C` (`16..24`).
    pub fx_scopes: [FxScopeRing; 24],
    /// Lock-free Sub Bus clipper scope (`pre_driven`, `post_clipped`, `clip_activity`).
    pub sub_scope: FxScopeRing,
    /// Lock-free Master Output / Limiter scope (`pre_limit`, `post_out`, `limit_activity`).
    pub master_scope: FxScopeRing,
    pub master_peak_l: AtomicU32,
    pub master_peak_r: AtomicU32,
    pub master_rms_l: AtomicU32,
    pub master_rms_r: AtomicU32,
    pub master_clip_l: AtomicU32,
    pub master_clip_r: AtomicU32,
}

impl Default for ModUiTelemetry {
    fn default() -> Self {
        Self::new()
    }
}

impl ModUiTelemetry {
    pub fn new() -> Self {
        Self {
            dest_offsets: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            source_levels: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            modseq_phases: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            env_levels: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            filter_live: std::array::from_fn(|_| {
                [
                    AtomicU32::new(0.0_f32.to_bits()),
                    AtomicU32::new(0.0_f32.to_bits()),
                    AtomicU32::new(0.0_f32.to_bits()),
                    AtomicU32::new(0.0_f32.to_bits()),
                ]
            }),
            fx_scopes: std::array::from_fn(|_| FxScopeRing::new()),
            sub_scope: FxScopeRing::new(),
            master_scope: FxScopeRing::new(),
            master_peak_l: AtomicU32::new(0.0_f32.to_bits()),
            master_peak_r: AtomicU32::new(0.0_f32.to_bits()),
            master_rms_l: AtomicU32::new(0.0_f32.to_bits()),
            master_rms_r: AtomicU32::new(0.0_f32.to_bits()),
            master_clip_l: AtomicU32::new(0),
            master_clip_r: AtomicU32::new(0),
        }
    }

    #[inline]
    pub fn store_master_meters(
        &self,
        peak_l: f32,
        peak_r: f32,
        rms_l: f32,
        rms_r: f32,
        clip_l: bool,
        clip_r: bool,
    ) {
        self.master_peak_l.store(peak_l.to_bits(), Ordering::Relaxed);
        self.master_peak_r.store(peak_r.to_bits(), Ordering::Relaxed);
        self.master_rms_l.store(rms_l.to_bits(), Ordering::Relaxed);
        self.master_rms_r.store(rms_r.to_bits(), Ordering::Relaxed);
        if clip_l {
            self.master_clip_l.store(1, Ordering::Relaxed);
        }
        if clip_r {
            self.master_clip_r.store(1, Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn master_meters(&self) -> (f32, f32, f32, f32, bool, bool) {
        (
            f32::from_bits(self.master_peak_l.load(Ordering::Relaxed)),
            f32::from_bits(self.master_peak_r.load(Ordering::Relaxed)),
            f32::from_bits(self.master_rms_l.load(Ordering::Relaxed)),
            f32::from_bits(self.master_rms_r.load(Ordering::Relaxed)),
            self.master_clip_l.load(Ordering::Relaxed) != 0,
            self.master_clip_r.load(Ordering::Relaxed) != 0,
        )
    }

    #[inline]
    pub fn reset_master_clip(&self) {
        self.master_clip_l.store(0, Ordering::Relaxed);
        self.master_clip_r.store(0, Ordering::Relaxed);
    }

    #[inline]
    pub fn store_filter_live(
        &self,
        idx: usize,
        cutoff_hz: f32,
        resonance: f32,
        drive: f32,
        aux: f32,
    ) {
        if idx < 2 {
            self.filter_live[idx][0].store(cutoff_hz.to_bits(), Ordering::Relaxed);
            self.filter_live[idx][1].store(resonance.to_bits(), Ordering::Relaxed);
            self.filter_live[idx][2].store(drive.to_bits(), Ordering::Relaxed);
            self.filter_live[idx][3].store(aux.to_bits(), Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn filter_live(&self, idx: usize) -> [f32; 4] {
        if idx < 2 {
            [
                f32::from_bits(self.filter_live[idx][0].load(Ordering::Relaxed)),
                f32::from_bits(self.filter_live[idx][1].load(Ordering::Relaxed)),
                f32::from_bits(self.filter_live[idx][2].load(Ordering::Relaxed)),
                f32::from_bits(self.filter_live[idx][3].load(Ordering::Relaxed)),
            ]
        } else {
            [0.0; 4]
        }
    }

    #[inline]
    pub fn store_voice_snapshot(
        &self,
        dest_offsets: &[f32; NUM_MOD_DESTS],
        source_levels: &[f32; NUM_ACTIVE_SOURCES],
        modseq_phases: &[f32; 4],
        env_levels: &[f32; 4],
    ) {
        for (dst, &val) in self.dest_offsets.iter().zip(dest_offsets.iter()) {
            dst.store(val.to_bits(), Ordering::Relaxed);
        }
        for (dst, &val) in self.source_levels.iter().zip(source_levels.iter()) {
            dst.store(val.to_bits(), Ordering::Relaxed);
        }
        for (dst, &val) in self.modseq_phases.iter().zip(modseq_phases.iter()) {
            dst.store(val.to_bits(), Ordering::Relaxed);
        }
        for (dst, &val) in self.env_levels.iter().zip(env_levels.iter()) {
            dst.store(val.to_bits(), Ordering::Relaxed);
        }
    }

    #[inline]
    pub fn fx_scope(&self, bus: usize, slot: usize) -> &FxScopeRing {
        let idx = bus.min(2) * 8 + slot.min(7);
        &self.fx_scopes[idx]
    }

    #[inline]
    pub fn dest_offset(&self, dest: ModDestination) -> f32 {
        f32::from_bits(self.dest_offsets[dest.index()].load(Ordering::Relaxed))
    }

    #[inline]
    pub fn source_level(&self, src: ModMatrixSource) -> f32 {
        if let Some(idx) = src.active_index() {
            f32::from_bits(self.source_levels[idx].load(Ordering::Relaxed))
        } else {
            0.0
        }
    }

    #[inline]
    pub fn modseq_phase(&self, idx: usize) -> f32 {
        if idx < 4 {
            f32::from_bits(self.modseq_phases[idx].load(Ordering::Relaxed))
        } else {
            0.0
        }
    }

    #[inline]
    pub fn env_level(&self, idx: usize) -> f32 {
        if idx < 4 {
            f32::from_bits(self.env_levels[idx].load(Ordering::Relaxed))
        } else {
            0.0
        }
    }
}
