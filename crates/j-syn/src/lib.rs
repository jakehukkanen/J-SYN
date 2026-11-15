//! `j-syn`: Zero-allocation 16-voice polyphonic wavetable synthesizer (VST3 + CLAP).

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use j_syn_dsp::{
    EngineMode, FactoryWavetable, ModSource, ModType, OscParams, PolyMode, SynthEngine,
    SynthParams, WarpMode, Wavetable, WavetableBank,
};
use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;

mod editor;

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavetableChoice {
    #[name = "Basic Shapes"]
    BasicShapes,
    #[name = "Digital Formant"]
    DigitalFormant,
    #[name = "Harmonic Sync"]
    HarmonicSync,
    #[name = "Aggressive PWM"]
    AggressivePwm,
    #[name = "Custom .WAV"]
    Custom,
}

impl WavetableChoice {
    #[inline]
    pub fn factory_preset(self) -> Option<FactoryWavetable> {
        match self {
            Self::BasicShapes => Some(FactoryWavetable::BasicShapes),
            Self::DigitalFormant => Some(FactoryWavetable::DigitalFormant),
            Self::HarmonicSync => Some(FactoryWavetable::HarmonicSync),
            Self::AggressivePwm => Some(FactoryWavetable::AggressivePwm),
            Self::Custom => None,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarpModeParam {
    #[name = "None"]
    None,
    #[name = "Bend-"]
    BendMinus,
    #[name = "Bend+"]
    BendPlus,
    #[name = "Spectrum"]
    Spectrum,
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSourceParam {
    #[name = "Off"]
    Off,
    #[name = "Osc 1"]
    Osc1,
    #[name = "Osc 2"]
    Osc2,
    #[name = "Osc 3"]
    Osc3,
    #[name = "Osc 4"]
    Osc4,
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModTypeParam {
    #[name = "PM"]
    Pm,
    #[name = "RM"]
    Rm,
}

#[derive(Params)]
pub struct OscPluginParams {
    #[id = "enabled"]
    pub enabled: BoolParam,

    #[id = "wt_select"]
    pub wt_select: EnumParam<WavetableChoice>,

    #[id = "wt_pos"]
    pub wt_pos: FloatParam,

    #[id = "octave"]
    pub octave: IntParam,

    #[id = "semi"]
    pub semi: IntParam,

    #[id = "fine"]
    pub fine: FloatParam,

    #[id = "phase"]
    pub phase: FloatParam,

    #[id = "retrig"]
    pub retrig: BoolParam,

    #[id = "warp_mode"]
    pub warp_mode: EnumParam<WarpModeParam>,

    #[id = "warp_amt"]
    pub warp_amount: FloatParam,

    #[id = "mod_src"]
    pub mod_source: EnumParam<ModSourceParam>,

    #[id = "mod_type"]
    pub mod_type: EnumParam<ModTypeParam>,

    #[id = "mod_depth"]
    pub mod_depth: FloatParam,

    #[id = "unison"]
    pub unison: IntParam,

    #[id = "detune"]
    pub detune: FloatParam,

    #[id = "spread"]
    pub spread: FloatParam,

    #[id = "pan"]
    pub pan: FloatParam,

    #[id = "level"]
    pub level: FloatParam,
}

impl OscPluginParams {
    fn new(osc_idx: usize) -> Self {
        let default_wt = match osc_idx {
            0 => WavetableChoice::BasicShapes,
            1 => WavetableChoice::DigitalFormant,
            2 => WavetableChoice::HarmonicSync,
            _ => WavetableChoice::AggressivePwm,
        };

        Self {
            enabled: BoolParam::new("Enabled", osc_idx == 0),
            wt_select: EnumParam::new("Wavetable", default_wt),
            wt_pos: FloatParam::new("WT Pos", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit(" %")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            octave: IntParam::new("Octave", 0, IntRange::Linear { min: -3, max: 3 }),
            semi: IntParam::new("Semi", 0, IntRange::Linear { min: -12, max: 12 }),
            fine: FloatParam::new(
                "Fine",
                0.0,
                FloatRange::Linear {
                    min: -100.0,
                    max: 100.0,
                },
            )
            .with_unit(" ct")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            phase: FloatParam::new(
                "Phase",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 360.0,
                },
            )
            .with_unit("°")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            retrig: BoolParam::new("Retrig", true),
            warp_mode: EnumParam::new("Warp Mode", WarpModeParam::None),
            warp_amount: FloatParam::new(
                "Warp Amt",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),
            mod_source: EnumParam::new("Mod Src", ModSourceParam::Off),
            mod_type: EnumParam::new("Mod Type", ModTypeParam::Pm),
            mod_depth: FloatParam::new(
                "Mod Depth",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),
            unison: IntParam::new("Unison", 1, IntRange::Linear { min: 1, max: 8 }),
            detune: FloatParam::new("Detune", 0.2, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit(" %")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            spread: FloatParam::new("Spread", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit(" %")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            pan: FloatParam::new(
                "Pan",
                0.0,
                FloatRange::Linear {
                    min: -1.0,
                    max: 1.0,
                },
            )
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),
            level: FloatParam::new("Level", 0.75, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit(" %")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
        }
    }

    fn to_dsp(&self) -> OscParams {
        OscParams {
            enabled: self.enabled.value(),
            wt_pos: self.wt_pos.value(),
            octave: self.octave.value(),
            semi: self.semi.value(),
            fine_cents: self.fine.value(),
            phase_offset: (self.phase.value() / 360.0).rem_euclid(1.0).fract(),
            retrig: self.retrig.value(),
            warp_mode: match self.warp_mode.value() {
                WarpModeParam::None => WarpMode::None,
                WarpModeParam::BendMinus => WarpMode::BendMinus,
                WarpModeParam::BendPlus => WarpMode::BendPlus,
                WarpModeParam::Spectrum => WarpMode::Spectrum,
            },
            warp_amount: self.warp_amount.value(),
            mod_source: match self.mod_source.value() {
                ModSourceParam::Off => ModSource::Off,
                ModSourceParam::Osc1 => ModSource::Osc1,
                ModSourceParam::Osc2 => ModSource::Osc2,
                ModSourceParam::Osc3 => ModSource::Osc3,
                ModSourceParam::Osc4 => ModSource::Osc4,
            },
            mod_type: match self.mod_type.value() {
                ModTypeParam::Pm => ModType::Pm,
                ModTypeParam::Rm => ModType::Rm,
            },
            mod_depth: self.mod_depth.value(),
            unison: self.unison.value().clamp(1, 8) as usize,
            detune: self.detune.value(),
            spread: self.spread.value(),
            pan: self.pan.value(),
            level: self.level.value(),
        }
    }
}

#[derive(Params)]
pub struct JSynPluginParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    #[persist = "wavetable_paths"]
    pub wavetable_paths: Arc<Mutex<[String; 4]>>,

    #[id = "master_vol"]
    pub master_volume_db: FloatParam,

    #[id = "hq_mode"]
    pub hq_mode: BoolParam,

    #[id = "mono_legato"]
    pub mono_legato: BoolParam,

    #[id = "glide_ms"]
    pub glide_ms: FloatParam,

    #[id = "pb_range"]
    pub pitch_bend_range: IntParam,

    #[id = "amp_atk"]
    pub amp_attack: FloatParam,

    #[id = "amp_dec"]
    pub amp_decay: FloatParam,

    #[id = "amp_sus"]
    pub amp_sustain: FloatParam,

    #[id = "amp_rel"]
    pub amp_release: FloatParam,

    #[nested(array, group = "Oscillators")]
    pub oscs: [OscPluginParams; 4],
}

impl Default for JSynPluginParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),
            wavetable_paths: Arc::new(Mutex::new([
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ])),
            master_volume_db: FloatParam::new(
                "Master Vol",
                -3.0,
                FloatRange::Linear {
                    min: -24.0,
                    max: 6.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            hq_mode: BoolParam::new("HQ Mode", false),
            mono_legato: BoolParam::new("Mono Legato", false),

            glide_ms: FloatParam::new(
                "Glide",
                0.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 500.0,
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            pitch_bend_range: IntParam::new(
                "PB Range",
                2,
                IntRange::Linear { min: 1, max: 24 },
            )
            .with_unit(" st"),

            amp_attack: FloatParam::new(
                "Attack",
                0.005,
                FloatRange::Skewed {
                    min: 0.001,
                    max: 5.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_unit(" s")
            .with_value_to_string(formatters::v2s_f32_rounded(3)),

            amp_decay: FloatParam::new(
                "Decay",
                0.25,
                FloatRange::Skewed {
                    min: 0.002,
                    max: 5.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_unit(" s")
            .with_value_to_string(formatters::v2s_f32_rounded(3)),

            amp_sustain: FloatParam::new(
                "Sustain",
                1.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            amp_release: FloatParam::new(
                "Release",
                0.20,
                FloatRange::Skewed {
                    min: 0.002,
                    max: 8.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_unit(" s")
            .with_value_to_string(formatters::v2s_f32_rounded(3)),

            oscs: [
                OscPluginParams::new(0),
                OscPluginParams::new(1),
                OscPluginParams::new(2),
                OscPluginParams::new(3),
            ],
        }
    }
}

pub struct JSynPlugin {
    params: Arc<JSynPluginParams>,
    engine: SynthEngine,
    wavetable_bank: Arc<WavetableBank>,
}

impl Default for JSynPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(JSynPluginParams::default()),
            engine: SynthEngine::new(44100.0),
            wavetable_bank: Arc::new(WavetableBank::new()),
        }
    }
}

impl JSynPlugin {
    fn reload_persisted_wavetables(&self) {
        let paths = {
            let guard = self.params.wavetable_paths.lock().unwrap();
            guard.clone()
        };
        for (idx, path_str) in paths.into_iter().enumerate() {
            if !path_str.is_empty() && self.params.oscs[idx].wt_select.value() == WavetableChoice::Custom {
                let bank = self.wavetable_bank.clone();
                let path = PathBuf::from(path_str);
                std::thread::spawn(move || {
                    if let Ok(wt) = Wavetable::load_from_wav(&path) {
                        bank.swap_custom(idx, Arc::new(wt));
                    }
                });
            }
        }
    }
}

impl Plugin for JSynPlugin {
    const NAME: &'static str = "J-SYN";
    const VENDOR: &'static str = "Jake Hukkanen";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(
            self.params.clone(),
            self.params.editor_state.clone(),
            self.wavetable_bank.clone(),
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.engine.set_sample_rate(buffer_config.sample_rate);
        self.engine.reset();
        self.reload_persisted_wavetables();
        true
    }

    fn reset(&mut self) {
        self.engine.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let num_samples = buffer.samples();
        if num_samples == 0 {
            return ProcessStatus::Normal;
        }

        let synth_params = SynthParams {
            master_gain: util::db_to_gain(self.params.master_volume_db.value()),
            engine_mode: if self.params.hq_mode.value() {
                EngineMode::Hq
            } else {
                EngineMode::Vintage2007
            },
            poly_mode: if self.params.mono_legato.value() {
                PolyMode::MonoLegato
            } else {
                PolyMode::Poly
            },
            glide_ms: self.params.glide_ms.value(),
            pitch_bend_range: self.params.pitch_bend_range.value() as f32,
            amp_attack_s: self.params.amp_attack.value(),
            amp_decay_s: self.params.amp_decay.value(),
            amp_sustain: self.params.amp_sustain.value(),
            amp_release_s: self.params.amp_release.value(),
            oscs: [
                self.params.oscs[0].to_dsp(),
                self.params.oscs[1].to_dsp(),
                self.params.oscs[2].to_dsp(),
                self.params.oscs[3].to_dsp(),
            ],
        };

        let choices = [
            self.params.oscs[0].wt_select.value(),
            self.params.oscs[1].wt_select.value(),
            self.params.oscs[2].wt_select.value(),
            self.params.oscs[3].wt_select.value(),
        ];
        let g0 = if choices[0].factory_preset().is_none() {
            Some(nih_plug::util::permit_alloc(|| {
                self.wavetable_bank.osc_tables[0].load()
            }))
        } else {
            None
        };
        let g1 = if choices[1].factory_preset().is_none() {
            Some(nih_plug::util::permit_alloc(|| {
                self.wavetable_bank.osc_tables[1].load()
            }))
        } else {
            None
        };
        let g2 = if choices[2].factory_preset().is_none() {
            Some(nih_plug::util::permit_alloc(|| {
                self.wavetable_bank.osc_tables[2].load()
            }))
        } else {
            None
        };
        let g3 = if choices[3].factory_preset().is_none() {
            Some(nih_plug::util::permit_alloc(|| {
                self.wavetable_bank.osc_tables[3].load()
            }))
        } else {
            None
        };
        let tables: [&Wavetable; 4] = [
            match choices[0].factory_preset() {
                Some(p) => self.wavetable_bank.factory_table(p),
                None => g0.as_deref().unwrap(),
            },
            match choices[1].factory_preset() {
                Some(p) => self.wavetable_bank.factory_table(p),
                None => g1.as_deref().unwrap(),
            },
            match choices[2].factory_preset() {
                Some(p) => self.wavetable_bank.factory_table(p),
                None => g2.as_deref().unwrap(),
            },
            match choices[3].factory_preset() {
                Some(p) => self.wavetable_bank.factory_table(p),
                None => g3.as_deref().unwrap(),
            },
        ];

        let channels = buffer.as_slice();
        if channels.len() < 2 {
            return ProcessStatus::Normal;
        }
        let (left_slice, right_rest) = channels.split_at_mut(1);
        let left = &mut left_slice[0][..num_samples];
        let right = &mut right_rest[0][..num_samples];

        let mut next_event = context.next_event();
        for sample_idx in 0..num_samples {
            while let Some(event) = next_event {
                if event.timing() as usize > sample_idx {
                    break;
                }

                match event {
                    NoteEvent::NoteOn {
                        channel,
                        note,
                        velocity,
                        voice_id,
                        ..
                    } => {
                        self.engine
                            .note_on(channel, note, velocity, voice_id, &synth_params);
                    }
                    NoteEvent::NoteOff {
                        channel,
                        note,
                        voice_id,
                        ..
                    } => {
                        self.engine
                            .note_off(channel, note, voice_id, &synth_params);
                    }
                    NoteEvent::Choke {
                        channel,
                        note,
                        voice_id,
                        ..
                    } => {
                        self.engine.choke(channel, note, voice_id);
                    }
                    NoteEvent::MidiPitchBend { value, .. } => {
                        self.engine.set_pitch_bend(value);
                    }
                    NoteEvent::MidiCC { cc, value, .. } => {
                        // CC 120: All Sound Off, CC 123: All Notes Off
                        if cc == 120 && value >= 0.0 {
                            self.engine.all_sound_off();
                        } else if cc == 123 && value >= 0.0 {
                            self.engine.all_notes_off();
                        }
                    }
                    _ => {}
                }

                next_event = context.next_event();
            }

            let (l, r) = self.engine.process_sample_with_tables(&synth_params, tables);
            left[sample_idx] = l;
            right[sample_idx] = r;
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for JSynPlugin {
    const CLAP_ID: &'static str = "com.jakehukkanen.j-syn";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("16-voice polyphonic wavetable synthesizer with 4 oscillators and cross-PM");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for JSynPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"JH_JSynWavetable";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Instrument,
        Vst3SubCategory::Synth,
    ];
}

nih_export_clap!(JSynPlugin);
nih_export_vst3!(JSynPlugin);
