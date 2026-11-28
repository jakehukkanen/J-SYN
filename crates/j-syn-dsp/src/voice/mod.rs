//! Pre-allocated polyphonic `Voice` and voice-stealing `TailVoice` DSP engines.

mod params;
mod routing;

pub use params::*;
pub use routing::apply_filter_modulation;

use crate::envelope::AmpAdsr;
use crate::filter::{FilterState, FilterSvfCore};
use crate::insert::{InsertShaperState, VoiceFeedbackState};
use crate::mod_matrix::{ModDestination, ModMatrixSource, NUM_ACTIVE_SOURCES, NUM_VOICE_MOD_DESTS};
use crate::modseq::{ModSeqRetrig, ModSeqState, NUM_MOD_ENVS, NUM_MODSEQS};
use crate::noise::NoiseState;
use crate::oscillator::OscillatorState;
use crate::wavetable::Wavetable;

use self::routing::process_voice_bus;

/// Pre-allocated polyphonic voice.
#[derive(Debug, Clone)]
pub struct Voice {
    pub active: bool,
    pub releasing: bool,
    pub note: u8,
    pub channel: u8,
    pub voice_id: Option<i32>,
    pub velocity: f32,
    pub age: u64,
    pub current_midi_note: f32,
    pub target_midi_note: f32,
    pub pitch_bend_norm: f32,
    pub pitch_bend_smoothed: f32,
    pub poly_tuning_semi: f32,
    pub poly_tuning_smoothed: f32,
    pub aftertouch: f32,
    pub aftertouch_smoothed: f32,
    pub slide: f32,
    pub slide_smoothed: f32,
    pub oscillators: [OscillatorState; 4],
    pub noise: NoiseState,
    pub filters: [FilterState; 2],
    pub inserts: [InsertShaperState; 2],
    pub inserts_aux: [InsertShaperState; 2],
    pub feedback: VoiceFeedbackState,
    pub amp_env: AmpAdsr,
    pub mod_envs: [AmpAdsr; NUM_MOD_ENVS],
    pub modseqs: [ModSeqState; NUM_MODSEQS],
    pub mod_offsets: [f32; NUM_VOICE_MOD_DESTS],
    pub target_mod_offsets: [f32; NUM_VOICE_MOD_DESTS],
    pub last_sources: [f32; NUM_ACTIVE_SOURCES],
    pub osc_out: [f32; 4],
    pub prev_osc_out: [f32; 4],
}

impl Default for Voice {
    fn default() -> Self {
        Self::new()
    }
}

impl Voice {
    pub fn new() -> Self {
        Self {
            active: false,
            releasing: false,
            note: 0,
            channel: 0,
            voice_id: None,
            velocity: 0.0,
            age: 0,
            current_midi_note: 60.0,
            target_midi_note: 60.0,
            pitch_bend_norm: 0.0,
            pitch_bend_smoothed: 0.0,
            poly_tuning_semi: 0.0,
            poly_tuning_smoothed: 0.0,
            aftertouch: 0.0,
            aftertouch_smoothed: 0.0,
            slide: 0.0,
            slide_smoothed: 0.0,
            oscillators: [
                OscillatorState::new(),
                OscillatorState::new(),
                OscillatorState::new(),
                OscillatorState::new(),
            ],
            noise: NoiseState::new(),
            filters: [FilterState::new(20_000.0), FilterState::new(2_000.0)],
            inserts: [InsertShaperState::new(), InsertShaperState::new()],
            inserts_aux: [InsertShaperState::new(), InsertShaperState::new()],
            feedback: VoiceFeedbackState::new(),
            amp_env: AmpAdsr::new(),
            mod_envs: [AmpAdsr::new(); NUM_MOD_ENVS],
            modseqs: [ModSeqState::new(); NUM_MODSEQS],
            mod_offsets: [0.0; NUM_VOICE_MOD_DESTS],
            target_mod_offsets: [0.0; NUM_VOICE_MOD_DESTS],
            last_sources: [0.0; NUM_ACTIVE_SOURCES],
            osc_out: [0.0; 4],
            prev_osc_out: [0.0; 4],
        }
    }

    pub fn reset(&mut self) {
        self.active = false;
        self.releasing = false;
        self.voice_id = None;
        self.velocity = 0.0;
        self.pitch_bend_norm = 0.0;
        self.pitch_bend_smoothed = 0.0;
        self.poly_tuning_semi = 0.0;
        self.poly_tuning_smoothed = 0.0;
        self.aftertouch = 0.0;
        self.aftertouch_smoothed = 0.0;
        self.slide = 0.0;
        self.slide_smoothed = 0.0;
        self.amp_env.reset();
        for env in &mut self.mod_envs {
            env.reset();
        }
        for ms in &mut self.modseqs {
            ms.reset();
        }
        self.mod_offsets = [0.0; NUM_VOICE_MOD_DESTS];
        self.target_mod_offsets = [0.0; NUM_VOICE_MOD_DESTS];
        self.last_sources = [0.0; NUM_ACTIVE_SOURCES];
        self.osc_out = [0.0; 4];
        self.prev_osc_out = [0.0; 4];
        self.noise.reset();
        self.inserts[0].reset();
        self.inserts[1].reset();
        self.inserts_aux[0].reset();
        self.inserts_aux[1].reset();
        self.feedback.reset();
        for f in &mut self.filters {
            f.reset();
        }
        for osc in &mut self.oscillators {
            osc.reset();
        }
    }

    #[inline]
    pub fn release_modulators(&mut self) {
        for env in &mut self.mod_envs {
            env.release();
        }
    }

    /// Evaluates modulation sources and updates destination offsets for this voice.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub fn update_modulation(
        &mut self,
        dt_seconds: f32,
        params: &SynthParams,
        global_modseq_vals: &[f32; NUM_MODSEQS],
        mod_wheel: f32,
        global_aftertouch: f32,
        global_slide: f32,
        snap: bool,
        is_telemetry_voice: bool,
    ) {
        if !self.active {
            return;
        }
        if snap && dt_seconds == 0.0 {
            self.pitch_bend_smoothed = self.pitch_bend_norm;
            self.poly_tuning_smoothed = self.poly_tuning_semi;
            self.aftertouch_smoothed = self.aftertouch;
            self.slide_smoothed = self.slide;
        }
        let mut sources = [0.0_f32; NUM_ACTIVE_SOURCES];
        sources[0] = self.amp_env.level();
        let sr_equiv = if dt_seconds > 1e-6 {
            1.0 / dt_seconds
        } else {
            48_000.0
        };
        for i in 0..NUM_MOD_ENVS {
            let src = ModMatrixSource::ENVS[1 + i];
            let Some(src_idx) = src.active_index() else {
                continue;
            };
            if !is_telemetry_voice && !params.mod_matrix.is_source_routed(src) {
                sources[src_idx] = 0.0;
                continue;
            }
            let env_p = &params.mod_envs[i];
            if dt_seconds > 0.0 {
                sources[src_idx] = self.mod_envs[i].process(
                    sr_equiv,
                    env_p.attack_s,
                    env_p.decay_s,
                    env_p.sustain,
                    env_p.release_s,
                );
            } else {
                sources[src_idx] = self.mod_envs[i].level();
            }
        }
        for i in 0..NUM_MODSEQS {
            let src = ModMatrixSource::MODSEQS[i];
            let Some(src_idx) = src.active_index() else {
                continue;
            };
            if !is_telemetry_voice && !params.mod_matrix.is_source_routed(src) {
                sources[src_idx] = 0.0;
                continue;
            }
            let mut ms_p = params.modseqs[i];
            ms_p.rate_mod = self.mod_offsets[ModDestination::modseq_rate(i).index()];
            ms_p.row_mod = self.mod_offsets[ModDestination::modseq_row(i).index()];
            sources[src_idx] = if ms_p.retrig == ModSeqRetrig::PolyVoice {
                self.modseqs[i].advance_with_mseg(
                    dt_seconds,
                    params.tempo_bpm,
                    &ms_p,
                    &params.modseq_steps.seqs[i],
                    &params.modseq_steps.msegs[i],
                )
            } else {
                global_modseq_vals[i]
            };
        }
        sources[8] = self.velocity.clamp(0.0, 1.0);
        sources[9] = ((self.current_midi_note - 12.0) / 96.0).clamp(0.0, 1.0);
        sources[10] = mod_wheel.clamp(0.0, 1.0);
        sources[11] = global_aftertouch
            .max(self.aftertouch_smoothed)
            .clamp(0.0, 1.0);
        for m in 0..8 {
            sources[12 + m] = params.macros[m].clamp(0.0, 1.0);
        }
        sources[20] = global_slide.max(self.slide_smoothed).clamp(0.0, 1.0);

        self.last_sources = sources;
        params
            .mod_matrix
            .evaluate_voice_destinations(&self.last_sources, &mut self.target_mod_offsets);

        if snap {
            self.mod_offsets = self.target_mod_offsets;
        } else {
            for i in 0..NUM_VOICE_MOD_DESTS {
                self.mod_offsets[i] += (self.target_mod_offsets[i] - self.mod_offsets[i]) * 0.25;
            }
        }
    }

    /// Processes 1 stereo sample for this voice.
    #[inline]
    pub fn process_sample(
        &mut self,
        params: &SynthParams,
        tables: [&Wavetable; 4],
        sample_rate: f32,
        pitch_bend_semitones: f32,
    ) -> VoiceBusOutput {
        let overrides = VoiceControlOverrides::from_params(params);
        self.process_sample_with_overrides(
            params,
            &overrides,
            tables,
            sample_rate,
            pitch_bend_semitones,
        )
    }

    /// Processes 1 stereo sample using smoothed control-rate overrides without copying `SynthParams`.
    #[inline]
    pub fn process_sample_with_overrides(
        &mut self,
        params: &SynthParams,
        overrides: &VoiceControlOverrides,
        tables: [&Wavetable; 4],
        sample_rate: f32,
        pitch_bend_semitones: f32,
    ) -> VoiceBusOutput {
        if !self.active {
            return VoiceBusOutput::default();
        }

        // Portamento / Glide (with GlideMs modulation support)
        let eff_glide_ms = (params.glide_ms
            + self.mod_offsets[ModDestination::GlideMs.index()] * 500.0)
            .clamp(0.0, 500.0);
        if (self.current_midi_note - self.target_midi_note).abs() > 1e-4 {
            if eff_glide_ms <= 0.1 {
                self.current_midi_note = self.target_midi_note;
            } else {
                let glide_samples = (eff_glide_ms * 0.001 * sample_rate).max(1.0);
                let coeff = 1.0 - (-4.6 / glide_samples).exp();
                self.current_midi_note += (self.target_midi_note - self.current_midi_note) * coeff;
            }
        }

        let env_level = self.amp_env.process(
            sample_rate,
            params.amp_attack_s,
            params.amp_decay_s,
            params.amp_sustain,
            params.amp_release_s,
        );

        if !self.amp_env.is_active() {
            self.active = false;
            self.releasing = false;
            return VoiceBusOutput::default();
        }

        // 1.5 ms per-voice 1-pole smoothing for MPE pitch bend, tuning, pressure, and slide.
        let expr_coeff = 1.0 - (-1.0 / (0.0015 * sample_rate.max(1000.0))).exp();
        self.pitch_bend_smoothed += (self.pitch_bend_norm - self.pitch_bend_smoothed) * expr_coeff;
        self.poly_tuning_smoothed +=
            (self.poly_tuning_semi - self.poly_tuning_smoothed) * expr_coeff;
        self.aftertouch_smoothed += (self.aftertouch - self.aftertouch_smoothed) * expr_coeff;
        self.slide_smoothed += (self.slide - self.slide_smoothed) * expr_coeff;

        let mpe_pb_semi = if params.mpe_enabled && self.channel > 0 {
            self.pitch_bend_smoothed * params.mpe_pitch_bend_range
        } else {
            0.0
        };
        let effective_note =
            self.current_midi_note + pitch_bend_semitones + mpe_pb_semi + self.poly_tuning_smoothed;
        let out = process_voice_bus(
            &mut self.oscillators,
            &mut self.osc_out,
            &mut self.prev_osc_out,
            &mut self.noise,
            &mut self.filters,
            &mut self.inserts,
            &mut self.inserts_aux,
            &mut self.feedback,
            params,
            overrides,
            &self.mod_offsets,
            tables,
            effective_note,
            sample_rate,
        );

        out.scale(env_level)
    }
}

/// Pre-allocated 3 ms fast-fade "tail" voice for click-free voice stealing.
#[derive(Debug, Clone)]
pub struct TailVoice {
    pub active: bool,
    pub age: u64,
    pub current_midi_note: f32,
    pub amp_at_steal: f32,
    pub fade_remaining: u32,
    pub fade_total: u32,
    pub oscillators: [OscillatorState; 4],
    pub noise: NoiseState,
    pub filter_cores: [FilterSvfCore; 2],
    pub inserts: [InsertShaperState; 2],
    pub inserts_aux: [InsertShaperState; 2],
    pub feedback: VoiceFeedbackState,
    pub mod_offsets: [f32; NUM_VOICE_MOD_DESTS],
    pub osc_out: [f32; 4],
    pub prev_osc_out: [f32; 4],
}

impl Default for TailVoice {
    fn default() -> Self {
        Self::new()
    }
}

impl TailVoice {
    pub fn new() -> Self {
        Self {
            active: false,
            age: 0,
            current_midi_note: 60.0,
            amp_at_steal: 0.0,
            fade_remaining: 0,
            fade_total: 1,
            oscillators: [
                OscillatorState::new(),
                OscillatorState::new(),
                OscillatorState::new(),
                OscillatorState::new(),
            ],
            noise: NoiseState::new(),
            filter_cores: [FilterSvfCore::new(20_000.0), FilterSvfCore::new(2_000.0)],
            inserts: [InsertShaperState::new(), InsertShaperState::new()],
            inserts_aux: [InsertShaperState::new(), InsertShaperState::new()],
            feedback: VoiceFeedbackState::new(),
            mod_offsets: [0.0; NUM_VOICE_MOD_DESTS],
            osc_out: [0.0; 4],
            prev_osc_out: [0.0; 4],
        }
    }

    pub fn reset(&mut self) {
        self.active = false;
        self.fade_remaining = 0;
        self.amp_at_steal = 0.0;
        self.mod_offsets = [0.0; NUM_VOICE_MOD_DESTS];
        self.osc_out = [0.0; 4];
        self.prev_osc_out = [0.0; 4];
        self.noise.reset();
        self.inserts[0].reset();
        self.inserts[1].reset();
        self.inserts_aux[0].reset();
        self.inserts_aux[1].reset();
        self.feedback.reset();
        for fc in &mut self.filter_cores {
            fc.reset_states();
        }
        for osc in &mut self.oscillators {
            osc.reset();
        }
    }

    pub fn steal_from(&mut self, voice: &Voice, fade_samples: u32, age: u64) {
        self.active = true;
        self.age = age;
        self.current_midi_note = voice.current_midi_note;
        self.amp_at_steal = voice.amp_env.level();
        self.fade_total = fade_samples.max(1);
        self.fade_remaining = self.fade_total;
        self.oscillators = voice.oscillators.clone();
        self.noise = voice.noise;
        self.filter_cores = [voice.filters[0].core, voice.filters[1].core];
        self.inserts = voice.inserts;
        self.inserts_aux = voice.inserts_aux;
        self.feedback = voice.feedback;
        self.mod_offsets = voice.mod_offsets;
        self.osc_out = voice.osc_out;
        self.prev_osc_out = voice.prev_osc_out;
    }

    #[inline]
    pub fn process_sample(
        &mut self,
        params: &SynthParams,
        tables: [&Wavetable; 4],
        sample_rate: f32,
        pitch_bend_semitones: f32,
    ) -> VoiceBusOutput {
        let overrides = VoiceControlOverrides::from_params(params);
        self.process_sample_with_overrides(
            params,
            &overrides,
            tables,
            sample_rate,
            pitch_bend_semitones,
        )
    }

    #[inline]
    pub fn process_sample_with_overrides(
        &mut self,
        params: &SynthParams,
        overrides: &VoiceControlOverrides,
        tables: [&Wavetable; 4],
        sample_rate: f32,
        pitch_bend_semitones: f32,
    ) -> VoiceBusOutput {
        if !self.active || self.fade_remaining == 0 {
            self.active = false;
            return VoiceBusOutput::default();
        }

        let fade_gain = self.fade_remaining as f32 / self.fade_total as f32;
        self.fade_remaining -= 1;
        if self.fade_remaining == 0 {
            self.active = false;
        }

        let effective_note = self.current_midi_note + pitch_bend_semitones;
        let out = process_voice_bus(
            &mut self.oscillators,
            &mut self.osc_out,
            &mut self.prev_osc_out,
            &mut self.noise,
            &mut self.filter_cores,
            &mut self.inserts,
            &mut self.inserts_aux,
            &mut self.feedback,
            params,
            overrides,
            &self.mod_offsets,
            tables,
            effective_note,
            sample_rate,
        );

        let gain = self.amp_at_steal * fade_gain;
        out.scale(gain)
    }
}
