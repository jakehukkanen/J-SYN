use crate::envelope::AmpAdsr;
use crate::filter::{FilterParams, FilterState, FilterSvfCore};
use crate::insert::{
    FilterRouting, InsertPosition, InsertShaperState, RoutingParams, VoiceFeedbackState,
};
use crate::noise::{NoiseParams, NoiseState};
use crate::oscillator::{EngineMode, ModSource, OscParams, OscillatorState};
use crate::wavetable::Wavetable;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolyMode {
    Poly = 0,
    MonoLegato = 1,
}

/// Snapshot of all synthesizer DSP parameters for a processing block.
#[derive(Debug, Clone, Copy)]
pub struct SynthParams {
    pub master_gain: f32,
    pub engine_mode: EngineMode,
    pub poly_mode: PolyMode,
    pub glide_ms: f32,
    pub pitch_bend_range: f32,
    pub amp_attack_s: f32,
    pub amp_decay_s: f32,
    pub amp_sustain: f32,
    pub amp_release_s: f32,
    pub oscs: [OscParams; 4],
    pub noise: NoiseParams,
    pub filters: [FilterParams; 2],
    pub routing: RoutingParams,
}

impl Default for SynthParams {
    fn default() -> Self {
        let mut oscs = [OscParams::default(); 4];
        oscs[0].enabled = true;
        Self {
            master_gain: 0.707_106_77, // -3 dB
            engine_mode: EngineMode::Vintage2007,
            poly_mode: PolyMode::Poly,
            glide_ms: 0.0,
            pitch_bend_range: 2.0,
            amp_attack_s: 0.005,
            amp_decay_s: 0.25,
            amp_sustain: 1.0,
            amp_release_s: 0.20,
            oscs,
            noise: NoiseParams::default(),
            filters: [
                FilterParams::default_filter_1(),
                FilterParams::default_filter_2(),
            ],
            routing: RoutingParams::default(),
        }
    }
}

trait StereoFilterStage {
    fn process_filter(
        &mut self,
        in_l: f32,
        in_r: f32,
        params: &FilterParams,
        midi_note: f32,
        sample_rate: f32,
        engine_mode: EngineMode,
    ) -> (f32, f32);
}

impl StereoFilterStage for FilterState {
    #[inline]
    fn process_filter(
        &mut self,
        in_l: f32,
        in_r: f32,
        params: &FilterParams,
        midi_note: f32,
        sample_rate: f32,
        engine_mode: EngineMode,
    ) -> (f32, f32) {
        self.process_stereo(in_l, in_r, params, midi_note, sample_rate, engine_mode)
    }
}

impl StereoFilterStage for FilterSvfCore {
    #[inline]
    fn process_filter(
        &mut self,
        in_l: f32,
        in_r: f32,
        params: &FilterParams,
        midi_note: f32,
        sample_rate: f32,
        engine_mode: EngineMode,
    ) -> (f32, f32) {
        self.process_core_stereo(in_l, in_r, params, midi_note, sample_rate, engine_mode)
    }
}

#[inline]
fn split_filter_send(l: f32, r: f32, balance: f32, f1_l: &mut f32, f1_r: &mut f32, f2_l: &mut f32, f2_r: &mut f32) {
    let t = (balance.clamp(-1.0, 1.0) + 1.0) * 0.5;
    let w1 = 1.0 - t;
    let w2 = t;
    *f1_l += l * w1;
    *f1_r += r * w1;
    *f2_l += l * w2;
    *f2_r += r * w2;
}

#[inline]
fn run_inserts_at(
    pos: InsertPosition,
    inserts: &mut [InsertShaperState; 2],
    routing: &RoutingParams,
    engine_mode: EngineMode,
    sample_rate: f32,
    mut l: f32,
    mut r: f32,
) -> (f32, f32) {
    for idx in 0..2 {
        if routing.inserts[idx].position == pos {
            (l, r) = inserts[idx].process_stereo(
                l,
                r,
                &routing.inserts[idx],
                engine_mode,
                sample_rate,
            );
        }
    }
    (l, r)
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn process_voice_bus<F: StereoFilterStage>(
    oscillators: &mut [OscillatorState; 4],
    osc_out: &mut [f32; 4],
    prev_osc_out: &mut [f32; 4],
    noise: &mut NoiseState,
    filters: &mut [F; 2],
    inserts: &mut [InsertShaperState; 2],
    feedback: &mut VoiceFeedbackState,
    params: &SynthParams,
    tables: [&Wavetable; 4],
    effective_note: f32,
    sample_rate: f32,
) -> (f32, f32) {
    // C3 (MIDI 60) = 440 * 2^(-9/12) Hz
    const C3_HZ: f32 = 130.812_78;
    let base_hz = 440.0 * (2.0_f32).powf((effective_note - 69.0) / 12.0);
    let mut f1_l = 0.0_f32;
    let mut f1_r = 0.0_f32;
    let mut f2_l = 0.0_f32;
    let mut f2_r = 0.0_f32;
    let mut direct_l = 0.0_f32;
    let mut direct_r = 0.0_f32;

    // Evaluate oscillators 1 -> 4 sequentially for zero-latency forward / 1-sample feedback cross-PM.
    for a in 0..4 {
        let osc_p = &params.oscs[a];
        if !osc_p.enabled {
            oscillators[a].fb_lp_state = 0.0;
            osc_out[a] = 0.0;
            continue;
        }

        let (mod_sig, is_fb) = match osc_p.mod_source {
            ModSource::Off => (0.0, false),
            ModSource::Osc1 => {
                let b = 0;
                if b < a { (osc_out[b], false) } else { (prev_osc_out[b], true) }
            }
            ModSource::Osc2 => {
                let b = 1;
                if b < a { (osc_out[b], false) } else { (prev_osc_out[b], true) }
            }
            ModSource::Osc3 => {
                let b = 2;
                if b < a { (osc_out[b], false) } else { (prev_osc_out[b], true) }
            }
            ModSource::Osc4 => {
                let b = 3;
                if b < a { (osc_out[b], false) } else { (prev_osc_out[b], true) }
            }
        };

        let osc_hz = if osc_p.keytrack { base_hz } else { C3_HZ };
        let (raw, l, r) = oscillators[a].process_sample(
            a,
            osc_p,
            tables[a],
            osc_hz,
            sample_rate,
            params.engine_mode,
            mod_sig,
            is_fb,
        );
        osc_out[a] = raw;
        if params.routing.osc_direct[a] {
            direct_l += l;
            direct_r += r;
        } else {
            split_filter_send(
                l,
                r,
                params.routing.osc_filter_balance[a],
                &mut f1_l,
                &mut f1_r,
                &mut f2_l,
                &mut f2_r,
            );
        }
    }

    *prev_osc_out = *osc_out;

    // Noise Generator
    let (nl, nr) = noise.process_sample(&params.noise, sample_rate);
    if params.routing.noise_direct {
        direct_l += nl;
        direct_r += nr;
    } else {
        split_filter_send(
            nl,
            nr,
            params.routing.noise_filter_balance,
            &mut f1_l,
            &mut f1_r,
            &mut f2_l,
            &mut f2_r,
        );
    }

    // Voice Feedback Bus (1-sample delayed post-filter output back into pre-filter sum)
    let (fb_l, fb_r) = feedback.read_feedback(params.routing.feedback_amount, sample_rate);
    f1_l += fb_l;
    f1_r += fb_r;
    if params.routing.filter_routing == FilterRouting::Parallel && params.filters[1].enabled {
        f2_l += fb_l;
        f2_r += fb_r;
    }

    let f1_has = f1_l.abs() + f1_r.abs() > 0.0;
    let f2_has = f2_l.abs() + f2_r.abs() > 0.0;
    if f1_has || !f2_has {
        (f1_l, f1_r) = run_inserts_at(
            InsertPosition::PreFilter,
            inserts,
            &params.routing,
            params.engine_mode,
            sample_rate,
            f1_l,
            f1_r,
        );
    }
    if f2_has {
        (f2_l, f2_r) = run_inserts_at(
            InsertPosition::PreFilter,
            inserts,
            &params.routing,
            params.engine_mode,
            sample_rate,
            f2_l,
            f2_r,
        );
    }

    // Filter 1
    let (mut out1_l, mut out1_r) = filters[0].process_filter(
        f1_l,
        f1_r,
        &params.filters[0],
        effective_note,
        sample_rate,
        params.engine_mode,
    );

    (out1_l, out1_r) = run_inserts_at(
        InsertPosition::BetweenFilters,
        inserts,
        &params.routing,
        params.engine_mode,
        sample_rate,
        out1_l,
        out1_r,
    );

    // Filter 2 + Serial / Parallel Bus Routing
    let mix = params.routing.filter_mix.clamp(-1.0, 1.0);
    let gain_f1 = (1.0 - mix).clamp(0.0, 1.0);
    let gain_f2 = (1.0 + mix).clamp(0.0, 1.0);

    let (mut post_l, mut post_r) = match params.routing.filter_routing {
        FilterRouting::Serial => {
            if params.filters[1].enabled {
                let (out2_l, out2_r) = filters[1].process_filter(
                    out1_l + f2_l,
                    out1_r + f2_r,
                    &params.filters[1],
                    effective_note,
                    sample_rate,
                    params.engine_mode,
                );
                if mix >= 0.0 {
                    (out2_l, out2_r)
                } else {
                    let w1 = -mix;
                    let w2 = 1.0 + mix;
                    (out1_l * w1 + out2_l * w2, out1_r * w1 + out2_r * w2)
                }
            } else {
                (
                    out1_l * gain_f1 + f2_l * gain_f2,
                    out1_r * gain_f1 + f2_r * gain_f2,
                )
            }
        }
        FilterRouting::Parallel => {
            let (out2_l, out2_r) = filters[1].process_filter(
                f2_l,
                f2_r,
                &params.filters[1],
                effective_note,
                sample_rate,
                params.engine_mode,
            );
            (
                out1_l * gain_f1 + out2_l * gain_f2,
                out1_r * gain_f1 + out2_r * gain_f2,
            )
        }
    };

    (post_l, post_r) = run_inserts_at(
        InsertPosition::PostFilter,
        inserts,
        &params.routing,
        params.engine_mode,
        sample_rate,
        post_l,
        post_r,
    );

    feedback.store_post_filter(post_l, post_r);
    (post_l + direct_l, post_r + direct_r)
}

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
    pub oscillators: [OscillatorState; 4],
    pub noise: NoiseState,
    pub filters: [FilterState; 2],
    pub inserts: [InsertShaperState; 2],
    pub feedback: VoiceFeedbackState,
    pub amp_env: AmpAdsr,
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
            oscillators: [
                OscillatorState::new(),
                OscillatorState::new(),
                OscillatorState::new(),
                OscillatorState::new(),
            ],
            noise: NoiseState::new(),
            filters: [FilterState::new(20_000.0), FilterState::new(2_000.0)],
            inserts: [InsertShaperState::new(), InsertShaperState::new()],
            feedback: VoiceFeedbackState::new(),
            amp_env: AmpAdsr::new(),
            osc_out: [0.0; 4],
            prev_osc_out: [0.0; 4],
        }
    }

    pub fn reset(&mut self) {
        self.active = false;
        self.releasing = false;
        self.voice_id = None;
        self.velocity = 0.0;
        self.amp_env.reset();
        self.osc_out = [0.0; 4];
        self.prev_osc_out = [0.0; 4];
        self.noise.reset();
        self.inserts[0].reset();
        self.inserts[1].reset();
        self.feedback.reset();
        for f in &mut self.filters {
            f.reset();
        }
        for osc in &mut self.oscillators {
            osc.reset();
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
    ) -> (f32, f32) {
        if !self.active {
            return (0.0, 0.0);
        }

        // Portamento / Glide
        if (self.current_midi_note - self.target_midi_note).abs() > 1e-4 {
            if params.glide_ms <= 0.1 {
                self.current_midi_note = self.target_midi_note;
            } else {
                let glide_samples = (params.glide_ms * 0.001 * sample_rate).max(1.0);
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
            return (0.0, 0.0);
        }

        let effective_note = self.current_midi_note + pitch_bend_semitones;
        let (post_l, post_r) = process_voice_bus(
            &mut self.oscillators,
            &mut self.osc_out,
            &mut self.prev_osc_out,
            &mut self.noise,
            &mut self.filters,
            &mut self.inserts,
            &mut self.feedback,
            params,
            tables,
            effective_note,
            sample_rate,
        );

        let gain = env_level * self.velocity;
        (post_l * gain, post_r * gain)
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
    pub feedback: VoiceFeedbackState,
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
            feedback: VoiceFeedbackState::new(),
            osc_out: [0.0; 4],
            prev_osc_out: [0.0; 4],
        }
    }

    pub fn reset(&mut self) {
        self.active = false;
        self.fade_remaining = 0;
        self.amp_at_steal = 0.0;
        self.osc_out = [0.0; 4];
        self.prev_osc_out = [0.0; 4];
        self.noise.reset();
        self.inserts[0].reset();
        self.inserts[1].reset();
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
        self.amp_at_steal = voice.amp_env.level() * voice.velocity;
        self.fade_total = fade_samples.max(1);
        self.fade_remaining = self.fade_total;
        self.oscillators = voice.oscillators.clone();
        self.noise = voice.noise;
        self.filter_cores = [voice.filters[0].core, voice.filters[1].core];
        self.inserts = voice.inserts;
        self.feedback = voice.feedback;
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
    ) -> (f32, f32) {
        if !self.active || self.fade_remaining == 0 {
            self.active = false;
            return (0.0, 0.0);
        }

        let fade_gain = self.fade_remaining as f32 / self.fade_total as f32;
        self.fade_remaining -= 1;
        if self.fade_remaining == 0 {
            self.active = false;
        }

        let effective_note = self.current_midi_note + pitch_bend_semitones;
        let (post_l, post_r) = process_voice_bus(
            &mut self.oscillators,
            &mut self.osc_out,
            &mut self.prev_osc_out,
            &mut self.noise,
            &mut self.filter_cores,
            &mut self.inserts,
            &mut self.feedback,
            params,
            tables,
            effective_note,
            sample_rate,
        );

        let gain = self.amp_at_steal * fade_gain;
        (post_l * gain, post_r * gain)
    }
}
