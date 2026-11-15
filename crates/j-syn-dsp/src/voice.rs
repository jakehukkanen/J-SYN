use crate::envelope::AmpAdsr;
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
        }
    }
}

#[inline]
fn process_osc_chain(
    oscillators: &mut [OscillatorState; 4],
    osc_out: &mut [f32; 4],
    prev_osc_out: &mut [f32; 4],
    params: &SynthParams,
    tables: [&Wavetable; 4],
    base_hz: f32,
    sample_rate: f32,
) -> (f32, f32) {
    // Evaluate oscillators sequentially 1 -> 2 -> 3 -> 4 so forward modulation
    // (B < A) is zero-latency from osc_out[B] while backward/self modulation (B >= A)
    // uses prev_osc_out[B] through the 1-pole feedback filter.
    let mut sum_l = 0.0_f32;
    let mut sum_r = 0.0_f32;

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
                if b < a {
                    (osc_out[b], false)
                } else {
                    (prev_osc_out[b], true)
                }
            }
            ModSource::Osc2 => {
                let b = 1;
                if b < a {
                    (osc_out[b], false)
                } else {
                    (prev_osc_out[b], true)
                }
            }
            ModSource::Osc3 => {
                let b = 2;
                if b < a {
                    (osc_out[b], false)
                } else {
                    (prev_osc_out[b], true)
                }
            }
            ModSource::Osc4 => {
                let b = 3;
                if b < a {
                    (osc_out[b], false)
                } else {
                    (prev_osc_out[b], true)
                }
            }
        };

        let (raw, l, r) = oscillators[a].process_sample(
            a,
            osc_p,
            tables[a],
            base_hz,
            sample_rate,
            params.engine_mode,
            mod_sig,
            is_fb,
        );
        osc_out[a] = raw;
        sum_l += l;
        sum_r += r;
    }

    *prev_osc_out = *osc_out;
    (sum_l, sum_r)
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
        let base_hz = 440.0 * (2.0_f32).powf((effective_note - 69.0) / 12.0);
        let (sum_l, sum_r) = process_osc_chain(
            &mut self.oscillators,
            &mut self.osc_out,
            &mut self.prev_osc_out,
            params,
            tables,
            base_hz,
            sample_rate,
        );

        let gain = env_level * self.velocity;
        (sum_l * gain, sum_r * gain)
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
        let base_hz = 440.0 * (2.0_f32).powf((effective_note - 69.0) / 12.0);
        let (sum_l, sum_r) = process_osc_chain(
            &mut self.oscillators,
            &mut self.osc_out,
            &mut self.prev_osc_out,
            params,
            tables,
            base_hz,
            sample_rate,
        );

        let gain = self.amp_at_steal * fade_gain;
        (sum_l * gain, sum_r * gain)
    }
}
