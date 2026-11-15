use std::sync::Arc;

use crate::fx::FxRackState;
use crate::mod_matrix::{ModDestination, ModUiTelemetry, NUM_ACTIVE_SOURCES, NUM_MOD_DESTS};
use crate::modseq::{ModSeqRetrig, ModSeqState};
use crate::oscillator::EngineMode;
use crate::voice::{PolyMode, SynthParams, TailVoice, Voice};
use crate::wavetable::{Wavetable, WavetableBank};

pub const MAX_VOICES: usize = 16;
pub const MAX_TAIL_VOICES: usize = 8;

/// 16-voice polyphonic + Mono Legato synthesizer engine with 1.5 kHz control-rate tick
/// and click-free 3 ms tail-voice stealing. Zero heap allocations during processing.
pub struct SynthEngine {
    pub voices: [Voice; MAX_VOICES],
    pub tail_voices: [TailVoice; MAX_TAIL_VOICES],
    sample_rate: f32,
    voice_age_counter: u64,
    last_note: Option<f32>,
    mono_stack: [(u8, u8); 16],
    mono_stack_len: usize,
    pitch_bend_norm: f32,
    pitch_bend_smoothed: f32,
    mod_wheel: f32,
    aftertouch: f32,
    global_modseqs: [ModSeqState; 4],
    global_modseq_vals: [f32; 4],
    telemetry: Arc<ModUiTelemetry>,
    control_tick_counter: u32,
    control_tick_period: u32,
    smoothed_wt_pos: [f32; 4],
    smoothed_mod_depth: [f32; 4],
    smoothed_warp_amount: [f32; 4],
    smoothed_filter_cutoff: [f32; 2],
    smoothed_filter_res: [f32; 2],
    smoothed_filter_drive: [f32; 2],
    smoothed_master_gain: f32,
    rng_state: u32,
    fx_rack: FxRackState,
    idle_mod_offsets: [f32; NUM_MOD_DESTS],
}

impl SynthEngine {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1000.0);
        let mut engine = Self {
            voices: std::array::from_fn(|_| Voice::new()),
            tail_voices: std::array::from_fn(|_| TailVoice::new()),
            sample_rate: sr,
            voice_age_counter: 0,
            last_note: None,
            mono_stack: [(0, 0); 16],
            mono_stack_len: 0,
            pitch_bend_norm: 0.0,
            pitch_bend_smoothed: 0.0,
            mod_wheel: 0.0,
            aftertouch: 0.0,
            global_modseqs: [ModSeqState::new(); 4],
            global_modseq_vals: [0.0; 4],
            telemetry: Arc::new(ModUiTelemetry::new()),
            control_tick_counter: 0,
            control_tick_period: (sr / 1500.0).round().max(1.0) as u32,
            smoothed_wt_pos: [0.0; 4],
            smoothed_mod_depth: [0.0; 4],
            smoothed_warp_amount: [0.0; 4],
            smoothed_filter_cutoff: [20_000.0, 2_000.0],
            smoothed_filter_res: [0.0; 2],
            smoothed_filter_drive: [0.0; 2],
            smoothed_master_gain: 0.707_106_77,
            rng_state: 0x9E37_79B9,
            fx_rack: FxRackState::new(sr),
            idle_mod_offsets: [0.0; NUM_MOD_DESTS],
        };
        engine.set_sample_rate(sr);
        engine
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(1000.0);
        self.sample_rate = sr;
        self.control_tick_period = (sr / 1500.0).round().max(1.0) as u32;
        self.fx_rack.set_sample_rate(sr);
        for v in &mut self.voices {
            v.inserts[0].set_sample_rate(sr);
            v.inserts[1].set_sample_rate(sr);
        }
        for tv in &mut self.tail_voices {
            tv.inserts[0].set_sample_rate(sr);
            tv.inserts[1].set_sample_rate(sr);
        }
    }

    pub fn reset(&mut self) {
        self.fx_rack.reset();
        for v in &mut self.voices {
            v.reset();
        }
        for tv in &mut self.tail_voices {
            tv.reset();
        }
        for ms in &mut self.global_modseqs {
            ms.reset();
        }
        self.global_modseq_vals = [0.0; 4];
        self.mono_stack_len = 0;
        self.last_note = None;
        self.pitch_bend_norm = 0.0;
        self.pitch_bend_smoothed = 0.0;
        self.mod_wheel = 0.0;
        self.aftertouch = 0.0;
        self.control_tick_counter = 0;
        self.idle_mod_offsets = [0.0; NUM_MOD_DESTS];
    }

    #[inline]
    pub fn set_pitch_bend(&mut self, value_0_to_1: f32) {
        self.pitch_bend_norm = (value_0_to_1.clamp(0.0, 1.0) * 2.0) - 1.0;
    }

    #[inline]
    pub fn set_mod_wheel(&mut self, value_0_to_1: f32) {
        self.mod_wheel = value_0_to_1.clamp(0.0, 1.0);
    }

    #[inline]
    pub fn set_aftertouch(&mut self, value_0_to_1: f32) {
        self.aftertouch = value_0_to_1.clamp(0.0, 1.0);
    }

    pub fn telemetry(&self) -> Arc<ModUiTelemetry> {
        Arc::clone(&self.telemetry)
    }

    pub fn note_on(
        &mut self,
        channel: u8,
        note: u8,
        velocity: f32,
        voice_id: Option<i32>,
        params: &SynthParams,
    ) {
        let vel = velocity.clamp(0.0, 1.0);
        if vel <= 0.0 {
            self.note_off(channel, note, voice_id, params);
            return;
        }

        // Snap control-rate smoothers when starting from silence so newly triggered notes
        // immediately reflect current knob positions on sample 0.
        if !self.voices.iter().any(|v| v.active) {
            self.smoothed_master_gain = params.master_gain;
            for i in 0..4 {
                self.smoothed_wt_pos[i] = params.oscs[i].wt_pos;
                self.smoothed_mod_depth[i] = params.oscs[i].mod_depth;
                self.smoothed_warp_amount[i] = params.oscs[i].warp_amount;
            }
            for i in 0..2 {
                self.smoothed_filter_cutoff[i] = params.filters[i].cutoff_hz;
                self.smoothed_filter_res[i] = params.filters[i].resonance;
                self.smoothed_filter_drive[i] = params.filters[i].drive;
            }
        }

        self.voice_age_counter = self.voice_age_counter.wrapping_add(1);
        let age = self.voice_age_counter;
        let fade_samples = (self.sample_rate * 0.003).round().max(1.0) as u32;

        if params.poly_mode == PolyMode::MonoLegato {
            // Release any polyphonic voices other than slot 0
            for v in self.voices.iter_mut().skip(1) {
                if v.active {
                    Self::push_tail_voice(&mut self.tail_voices, v, fade_samples, age);
                    v.reset();
                }
            }

            // Push onto mono note stack (avoiding duplicate entry)
            self.remove_from_mono_stack(channel, note);
            if self.mono_stack_len < self.mono_stack.len() {
                self.mono_stack[self.mono_stack_len] = (channel, note);
                self.mono_stack_len += 1;
            }

            let v0 = &mut self.voices[0];
            if v0.active && !v0.releasing && self.mono_stack_len > 1 {
                // Legato transition: keep envelope & oscillator phases running, glide pitch.
                v0.note = note;
                v0.channel = channel;
                v0.voice_id = voice_id;
                v0.velocity = vel;
                v0.age = age;
                v0.target_midi_note = note as f32;
                self.last_note = Some(note as f32);
                return;
            }

            if v0.active {
                Self::push_tail_voice(&mut self.tail_voices, v0, fade_samples, age);
            }

            let start_note = if params.glide_ms > 0.1 {
                self.last_note.unwrap_or(note as f32)
            } else {
                note as f32
            };
            Self::init_voice(
                &mut self.voices[0],
                channel,
                note,
                vel,
                voice_id,
                age,
                start_note,
                note as f32,
                params,
                &self.global_modseq_vals,
                self.mod_wheel,
                self.aftertouch,
                &mut self.rng_state,
            );
            self.last_note = Some(note as f32);
            return;
        }

        // Polyphonic mode: find a free voice, or steal oldest/quietest voice into TailVoice pool.
        let slot = if let Some(idx) = self.voices.iter().position(|v| !v.active) {
            idx
        } else {
            let steal_idx = self.find_steal_candidate();
            let stolen = &self.voices[steal_idx];
            Self::push_tail_voice(&mut self.tail_voices, stolen, fade_samples, age);
            steal_idx
        };

        let start_note = if params.glide_ms > 0.1 {
            self.last_note.unwrap_or(note as f32)
        } else {
            note as f32
        };

        Self::init_voice(
            &mut self.voices[slot],
            channel,
            note,
            vel,
            voice_id,
            age,
            start_note,
            note as f32,
            params,
            &self.global_modseq_vals,
            self.mod_wheel,
            self.aftertouch,
            &mut self.rng_state,
        );
        self.last_note = Some(note as f32);
    }

    pub fn note_off(
        &mut self,
        channel: u8,
        note: u8,
        voice_id: Option<i32>,
        params: &SynthParams,
    ) {
        if params.poly_mode == PolyMode::MonoLegato {
            self.remove_from_mono_stack(channel, note);
            if self.mono_stack_len > 0 {
                let (top_ch, top_note) = self.mono_stack[self.mono_stack_len - 1];
                let v0 = &mut self.voices[0];
                if v0.active {
                    v0.channel = top_ch;
                    v0.note = top_note;
                    v0.target_midi_note = top_note as f32;
                    self.last_note = Some(top_note as f32);
                }
            } else {
                let v0 = &mut self.voices[0];
                if v0.active {
                    v0.releasing = true;
                    v0.amp_env.release();
                    v0.release_modulators();
                }
            }
            return;
        }

        for v in &mut self.voices {
            if !v.active || v.releasing {
                continue;
            }
            let id_match = match (voice_id, v.voice_id) {
                (Some(ev_id), Some(v_id)) => ev_id == v_id,
                _ => v.channel == channel && v.note == note,
            };
            if id_match {
                v.releasing = true;
                v.amp_env.release();
                v.release_modulators();
            }
        }
    }

    pub fn choke(&mut self, channel: u8, note: u8, voice_id: Option<i32>) {
        self.remove_from_mono_stack(channel, note);
        self.voice_age_counter = self.voice_age_counter.wrapping_add(1);
        let age = self.voice_age_counter;
        let fade_samples = (self.sample_rate * 0.003).round().max(1.0) as u32;

        for v in &mut self.voices {
            if !v.active {
                continue;
            }
            let id_match = match (voice_id, v.voice_id) {
                (Some(ev_id), Some(v_id)) => ev_id == v_id,
                _ => v.channel == channel && v.note == note,
            };
            if id_match {
                Self::push_tail_voice(&mut self.tail_voices, v, fade_samples, age);
                v.reset();
            }
        }
    }

    pub fn all_notes_off(&mut self) {
        self.mono_stack_len = 0;
        for v in &mut self.voices {
            if v.active && !v.releasing {
                v.releasing = true;
                v.amp_env.release();
                v.release_modulators();
            }
        }
    }

    pub fn all_sound_off(&mut self) {
        self.mono_stack_len = 0;
        self.voice_age_counter = self.voice_age_counter.wrapping_add(1);
        let age = self.voice_age_counter;
        let fade_samples = (self.sample_rate * 0.003).round().max(1.0) as u32;

        for v in &mut self.voices {
            if v.active {
                Self::push_tail_voice(&mut self.tail_voices, v, fade_samples, age);
                v.reset();
            }
        }
    }

    #[inline]
    pub fn active_voice_count(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    /// Processes a single stereo sample using pre-loaded wavetable references.
    #[inline]
    pub fn process_sample(&mut self, params: &SynthParams, bank: &WavetableBank) -> (f32, f32) {
        let g0 = bank.osc_tables[0].load();
        let g1 = bank.osc_tables[1].load();
        let g2 = bank.osc_tables[2].load();
        let g3 = bank.osc_tables[3].load();
        let tables: [&Wavetable; 4] = [&g0, &g1, &g2, &g3];
        self.process_sample_with_tables(params, tables)
    }

    /// Processes a single stereo sample with already-guarded wavetable references.
    #[inline]
    pub fn process_sample_with_tables(
        &mut self,
        params: &SynthParams,
        tables: [&Wavetable; 4],
    ) -> (f32, f32) {
        // 1.5 kHz control-rate modulation tick with lightweight 1-pole smoothing
        let is_control_tick = self.control_tick_counter == 0;
        if is_control_tick {
            const CR_COEFF: f32 = 0.28;
            self.pitch_bend_smoothed +=
                (self.pitch_bend_norm - self.pitch_bend_smoothed) * CR_COEFF;
            self.smoothed_master_gain +=
                (params.master_gain - self.smoothed_master_gain) * CR_COEFF;
            for i in 0..4 {
                self.smoothed_wt_pos[i] +=
                    (params.oscs[i].wt_pos - self.smoothed_wt_pos[i]) * CR_COEFF;
                self.smoothed_mod_depth[i] +=
                    (params.oscs[i].mod_depth - self.smoothed_mod_depth[i]) * CR_COEFF;
                self.smoothed_warp_amount[i] +=
                    (params.oscs[i].warp_amount - self.smoothed_warp_amount[i]) * CR_COEFF;
            }
            for i in 0..2 {
                let log_cur = self.smoothed_filter_cutoff[i].max(20.0).ln();
                let log_tgt = params.filters[i].cutoff_hz.max(20.0).ln();
                self.smoothed_filter_cutoff[i] = (log_cur + (log_tgt - log_cur) * CR_COEFF).exp();
                self.smoothed_filter_res[i] +=
                    (params.filters[i].resonance - self.smoothed_filter_res[i]) * CR_COEFF;
                self.smoothed_filter_drive[i] +=
                    (params.filters[i].drive - self.smoothed_filter_drive[i]) * CR_COEFF;
            }
        }
        self.control_tick_counter += 1;
        if self.control_tick_counter >= self.control_tick_period {
            self.control_tick_counter = 0;
        }

        let any_voices = self.voices.iter().any(|v| v.active);
        let any_tail = self.tail_voices.iter().any(|tv| tv.active);

        if params.engine_mode == EngineMode::Hq {
            let dt_sample = 1.0 / self.sample_rate;
            let rate_mods = self.current_global_modseq_rate_mods();
            for i in 0..4 {
                let mut ms_p = params.modseqs[i];
                ms_p.rate_mod = rate_mods[i];
                self.global_modseq_vals[i] = self.global_modseqs[i].advance_with_mseg(
                    dt_sample,
                    params.tempo_bpm,
                    &ms_p,
                    &params.modseq_steps.seqs[i],
                    &params.modseq_steps.msegs[i],
                );
            }
            if any_voices {
                for v in &mut self.voices {
                    if v.active {
                        v.update_modulation(
                            dt_sample,
                            params,
                            &self.global_modseq_vals,
                            self.mod_wheel,
                            self.aftertouch,
                            false,
                        );
                    }
                }
            } else {
                self.idle_mod_offsets = self.evaluate_idle_offsets(params);
            }
        } else if is_control_tick {
            let dt_tick = (self.control_tick_period as f32) / self.sample_rate;
            let rate_mods = self.current_global_modseq_rate_mods();
            for i in 0..4 {
                let mut ms_p = params.modseqs[i];
                ms_p.rate_mod = rate_mods[i];
                self.global_modseq_vals[i] = self.global_modseqs[i].advance_with_mseg(
                    dt_tick,
                    params.tempo_bpm,
                    &ms_p,
                    &params.modseq_steps.seqs[i],
                    &params.modseq_steps.msegs[i],
                );
            }
            if any_voices {
                for v in &mut self.voices {
                    if v.active {
                        v.update_modulation(
                            dt_tick,
                            params,
                            &self.global_modseq_vals,
                            self.mod_wheel,
                            self.aftertouch,
                            true,
                        );
                    }
                }
            } else {
                self.idle_mod_offsets = self.evaluate_idle_offsets(params);
            }
        }

        let mut mix_l = 0.0_f32;
        let mut mix_r = 0.0_f32;

        if any_voices || any_tail {
            let mut cr_params = *params;
            for i in 0..4 {
                cr_params.oscs[i].wt_pos = self.smoothed_wt_pos[i];
                cr_params.oscs[i].mod_depth = self.smoothed_mod_depth[i];
                cr_params.oscs[i].warp_amount = self.smoothed_warp_amount[i];
            }
            for i in 0..2 {
                cr_params.filters[i].cutoff_hz = self.smoothed_filter_cutoff[i];
                cr_params.filters[i].resonance = self.smoothed_filter_res[i];
                cr_params.filters[i].drive = self.smoothed_filter_drive[i];
            }

            let pb_semi = self.pitch_bend_smoothed * params.pitch_bend_range;

            if any_voices {
                for v in &mut self.voices {
                    if v.active {
                        let (vl, vr) =
                            v.process_sample(&cr_params, tables, self.sample_rate, pb_semi);
                        mix_l += vl;
                        mix_r += vr;
                    }
                }
            }

            if any_tail {
                for tv in &mut self.tail_voices {
                    if tv.active {
                        let (tl, tr) =
                            tv.process_sample(&cr_params, tables, self.sample_rate, pb_semi);
                        mix_l += tl;
                        mix_r += tr;
                    }
                }
            }
        }

        if is_control_tick {
            self.publish_telemetry(params);
        }

        // Polyphonic headroom scaling (~-9 dB) -> Master FX Rack -> master gain
        const HEADROOM: f32 = 0.35;
        let pre_l = mix_l * HEADROOM;
        let pre_r = mix_r * HEADROOM;
        let fx_mod = Self::current_global_fx_mod_offsets(&self.voices, &self.idle_mod_offsets);
        let (fx_l, fx_r) = self.fx_rack.process_sample(
            pre_l,
            pre_r,
            &params.fx_slots,
            fx_mod,
            params.tempo_bpm,
            Some(&self.telemetry),
        );
        (fx_l * self.smoothed_master_gain, fx_r * self.smoothed_master_gain)
    }

    #[inline]
    fn evaluate_idle_offsets(&self, params: &SynthParams) -> [f32; NUM_MOD_DESTS] {
        let mut idle_sources = [0.0_f32; NUM_ACTIVE_SOURCES];
        for i in 0..4 {
            if params.modseqs[i].retrig == ModSeqRetrig::MonoGlobal {
                idle_sources[4 + i] = self.global_modseq_vals[i];
            }
        }
        idle_sources[10] = self.mod_wheel.clamp(0.0, 1.0);
        idle_sources[11] = self.aftertouch.clamp(0.0, 1.0);
        for m in 0..8 {
            idle_sources[12 + m] = params.macros[m].clamp(0.0, 1.0);
        }
        let mut idle_offsets = [0.0_f32; NUM_MOD_DESTS];
        params
            .mod_matrix
            .evaluate_destinations(&idle_sources, &mut idle_offsets);
        idle_offsets
    }

    #[inline]
    fn current_global_fx_mod_offsets<'a>(
        voices: &'a [Voice; MAX_VOICES],
        idle_mod_offsets: &'a [f32; NUM_MOD_DESTS],
    ) -> &'a [f32; NUM_MOD_DESTS] {
        if let Some(v) = voices.iter().filter(|v| v.active).max_by_key(|v| v.age) {
            return &v.mod_offsets;
        }
        idle_mod_offsets
    }

    #[inline]
    fn current_global_modseq_rate_mods(&self) -> [f32; 4] {
        let offsets = Self::current_global_fx_mod_offsets(&self.voices, &self.idle_mod_offsets);
        std::array::from_fn(|i| offsets[ModDestination::modseq_rate(i).index()])
    }

    fn publish_telemetry(&self, params: &SynthParams) {
        if let Some(v) = self.voices.iter().filter(|v| v.active).max_by_key(|v| v.age) {
            let mut modseq_phases = [0.0_f32; 4];
            for i in 0..4 {
                modseq_phases[i] = if params.modseqs[i].retrig == ModSeqRetrig::PolyVoice {
                    v.modseqs[i].phase
                } else {
                    self.global_modseqs[i].phase
                };
            }
            let env_levels = [
                v.amp_env.level(),
                v.mod_envs[0].level(),
                v.mod_envs[1].level(),
                v.mod_envs[2].level(),
            ];
            self.telemetry.store_voice_snapshot(
                &v.mod_offsets,
                &v.last_sources,
                &modseq_phases,
                &env_levels,
            );
            for i in 0..2 {
                let mf = crate::voice::apply_filter_modulation(&params.filters[i], i, &v.mod_offsets);
                let kt_semi = (v.current_midi_note - 60.0) * mf.keytrack.clamp(0.0, 1.0);
                let fc = (mf.cutoff_hz * 2.0_f32.powf(kt_semi / 12.0)).clamp(20.0, 20_000.0);
                self.telemetry
                    .store_filter_live(i, fc, mf.resonance, mf.drive, mf.aux);
            }
        } else {
            let mut idle_sources = [0.0_f32; NUM_ACTIVE_SOURCES];
            let mut modseq_phases = [0.0_f32; 4];
            for i in 0..4 {
                if params.modseqs[i].retrig == ModSeqRetrig::MonoGlobal {
                    idle_sources[4 + i] = self.global_modseq_vals[i];
                    modseq_phases[i] = self.global_modseqs[i].phase;
                }
            }
            idle_sources[10] = self.mod_wheel.clamp(0.0, 1.0);
            idle_sources[11] = self.aftertouch.clamp(0.0, 1.0);
            for m in 0..8 {
                idle_sources[12 + m] = params.macros[m].clamp(0.0, 1.0);
            }
            let mut idle_offsets = [0.0_f32; NUM_MOD_DESTS];
            params
                .mod_matrix
                .evaluate_destinations(&idle_sources, &mut idle_offsets);
            self.telemetry.store_voice_snapshot(
                &idle_offsets,
                &idle_sources,
                &modseq_phases,
                &[0.0; 4],
            );
            for i in 0..2 {
                let mf = crate::voice::apply_filter_modulation(&params.filters[i], i, &idle_offsets);
                self.telemetry.store_filter_live(
                    i,
                    mf.cutoff_hz,
                    mf.resonance,
                    mf.drive,
                    mf.aux,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn init_voice(
        v: &mut Voice,
        channel: u8,
        note: u8,
        velocity: f32,
        voice_id: Option<i32>,
        age: u64,
        start_note: f32,
        target_note: f32,
        params: &SynthParams,
        global_modseq_vals: &[f32; 4],
        mod_wheel: f32,
        aftertouch: f32,
        rng_state: &mut u32,
    ) {
        v.active = true;
        v.releasing = false;
        v.channel = channel;
        v.note = note;
        v.voice_id = voice_id;
        v.velocity = velocity;
        v.age = age;
        v.current_midi_note = start_note;
        v.target_midi_note = target_note;
        v.amp_env.reset_and_trigger();
        for env in &mut v.mod_envs {
            env.reset_and_trigger();
        }
        for (i, ms) in v.modseqs.iter_mut().enumerate() {
            if params.modseqs[i].retrig == ModSeqRetrig::PolyVoice {
                ms.trigger();
            } else {
                ms.reset();
            }
        }

        for (i, osc) in v.oscillators.iter_mut().enumerate() {
            if params.oscs[i].retrig {
                v.osc_out[i] = 0.0;
                v.prev_osc_out[i] = 0.0;
            }
            osc.on_note_on(params.oscs[i].retrig, rng_state);
        }

        v.noise.on_note_on(rng_state);
        v.inserts[0].reset();
        v.inserts[1].reset();
        v.feedback.reset();
        for (i, flt) in v.filters.iter_mut().enumerate() {
            flt.on_note_on(
                params.filters[i].cutoff_hz,
                params.filters[i].resonance,
                params.filters[i].drive,
            );
        }
        v.update_modulation(
            0.0,
            params,
            global_modseq_vals,
            mod_wheel,
            aftertouch,
            true,
        );
    }

    fn find_steal_candidate(&self) -> usize {
        // Prefer stealing a releasing voice with the lowest amplitude, otherwise oldest voice.
        let mut best_idx = 0;
        let mut best_score = f32::MAX;
        for (i, v) in self.voices.iter().enumerate() {
            let rel_bonus = if v.releasing { 0.0 } else { 10.0 };
            let score = rel_bonus + v.amp_env.level() + (v.age as f32 * 1e-9);
            if score < best_score {
                best_score = score;
                best_idx = i;
            }
        }
        best_idx
    }

    fn push_tail_voice(
        tail_voices: &mut [TailVoice; MAX_TAIL_VOICES],
        voice: &Voice,
        fade_samples: u32,
        age: u64,
    ) {
        if voice.amp_env.level() <= 1e-4 {
            return;
        }
        let slot = if let Some(idx) = tail_voices.iter().position(|tv| !tv.active) {
            idx
        } else {
            tail_voices
                .iter()
                .enumerate()
                .min_by_key(|(_, tv)| tv.age)
                .map(|(idx, _)| idx)
                .unwrap_or(0)
        };
        tail_voices[slot].steal_from(voice, fade_samples, age);
    }

    fn remove_from_mono_stack(&mut self, channel: u8, note: u8) {
        let mut write = 0;
        for read in 0..self.mono_stack_len {
            let entry = self.mono_stack[read];
            if entry.0 != channel || entry.1 != note {
                self.mono_stack[write] = entry;
                write += 1;
            }
        }
        self.mono_stack_len = write;
    }
}
