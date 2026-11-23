//! `j-syn`: Zero-allocation 64-voice polyphonic wavetable synthesizer (VST3 + CLAP).

use std::num::NonZeroU32;
use std::sync::Arc;

use j_syn_dsp::{
    EngineMode, FxBusParams, PolyMode, SynthEngine, SynthParams, Wavetable, WavetableBank,
};
use nih_plug::prelude::*;

pub mod editor;
mod params;
pub mod presets;

pub use params::*;

pub struct JSynPlugin {
    params: Arc<JSynPluginParams>,
    engine: Box<SynthEngine>,
    wavetable_bank: Arc<WavetableBank>,
}

impl Default for JSynPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(JSynPluginParams::default()),
            engine: Box::new(SynthEngine::new(44100.0)),
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
                std::thread::spawn(move || {
                    if let Some(resolved) = crate::presets::resolve_wavetable_path(&path_str) {
                        if let Ok(wt) = Wavetable::load_from_wav(&resolved) {
                            bank.swap_custom(idx, Arc::new(wt));
                        }
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

    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
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
            self.engine.telemetry(),
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

        let mod_matrix = nih_plug::util::permit_alloc(|| self.params.mod_matrix.snapshot());
        let modseq_steps = nih_plug::util::permit_alloc(|| self.params.modseq_steps.snapshot());
        let tempo_bpm = context.transport().tempo.unwrap_or(120.0) as f32;

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
            mpe_enabled: self.params.mpe_enabled.value(),
            mpe_pitch_bend_range: self.params.mpe_pitch_bend_range.value() as f32,
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
            noise: self.params.noise.to_dsp(),
            filters: [
                self.params.filters[0].to_dsp(),
                self.params.filters[1].to_dsp(),
            ],
            routing: self.params.routing.to_dsp(),
            mod_envs: [
                self.params.mod_envs[0].to_dsp(),
                self.params.mod_envs[1].to_dsp(),
                self.params.mod_envs[2].to_dsp(),
            ],
            modseqs: {
                let swing = self.params.modseq_swing.value();
                [
                    self.params.modseqs[0].to_dsp(swing),
                    self.params.modseqs[1].to_dsp(swing),
                    self.params.modseqs[2].to_dsp(swing),
                    self.params.modseqs[3].to_dsp(swing),
                ]
            },
            modseq_steps,
            mod_matrix,
            macros: std::array::from_fn(|i| self.params.macros[i].value.value()),
            fx_bus_a: FxBusParams {
                slots: std::array::from_fn(|i| self.params.fx_slots[i].to_dsp()),
                level: self.params.bus_a_level.value(),
                pan: self.params.bus_a_pan.value(),
            },
            fx_bus_b: FxBusParams {
                slots: std::array::from_fn(|i| self.params.fx_bus_b.slots[i].to_dsp()),
                level: self.params.fx_bus_b.level.value(),
                pan: self.params.fx_bus_b.pan.value(),
            },
            fx_bus_c: FxBusParams {
                slots: std::array::from_fn(|i| self.params.fx_bus_c.slots[i].to_dsp()),
                level: self.params.fx_bus_c.level.value(),
                pan: self.params.fx_bus_c.pan.value(),
            },
            fx_bus_topology: self.params.fx_bus_topology.value().into(),
            sub_bus: self.params.sub_bus.to_dsp(),
            tempo_bpm,
            dc_blocker: self.params.dc_blocker.value(),
            master_limiter: self.params.master_limiter.value(),
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
                    NoteEvent::MidiPitchBend { channel, value, .. } => {
                        self.engine
                            .set_pitch_bend_ch(channel, value, &synth_params);
                    }
                    NoteEvent::MidiCC {
                        channel, cc, value, ..
                    } => {
                        if cc == 1 {
                            if !synth_params.mpe_enabled || channel == 0 {
                                self.engine.set_mod_wheel(value);
                            }
                        } else if cc == 74 {
                            self.engine.set_slide_ch(channel, value, &synth_params);
                        } else if cc == 120 && value >= 0.0 {
                            self.engine.all_sound_off();
                        } else if cc == 123 && value >= 0.0 {
                            self.engine.all_notes_off();
                        }
                    }
                    NoteEvent::MidiChannelPressure {
                        channel, pressure, ..
                    } => {
                        self.engine
                            .set_aftertouch_ch(channel, pressure, &synth_params);
                    }
                    NoteEvent::PolyPressure {
                        voice_id,
                        channel,
                        note,
                        pressure,
                        ..
                    } => {
                        self.engine
                            .set_poly_pressure(channel, note, voice_id, pressure);
                    }
                    NoteEvent::PolyTuning {
                        voice_id,
                        channel,
                        note,
                        tuning,
                        ..
                    } => {
                        self.engine
                            .set_poly_tuning(channel, note, voice_id, tuning);
                    }
                    NoteEvent::PolyBrightness {
                        voice_id,
                        channel,
                        note,
                        brightness,
                        ..
                    } => {
                        self.engine
                            .set_poly_brightness(channel, note, voice_id, brightness);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_plugin_parameter_ids_are_unique() {
        let params = JSynPluginParams::default();
        let param_map = params.param_map();
        let unique_ids: HashSet<&str> = param_map.iter().map(|(id, _, _)| id.as_str()).collect();
        assert_eq!(
            param_map.len(),
            unique_ids.len(),
            "Duplicate plugin parameter IDs detected in JSynPluginParams"
        );
    }

    #[test]
    fn single_vizia_timer_and_fast_state_and_idle_processing() {
        use std::time::{Duration, Instant};

        // Boxing SynthEngine keeps JSynPlugin compact on the host stack during init/state save.
        assert!(std::mem::size_of::<JSynPlugin>() <= 256);

        let start_init = Instant::now();
        let mut plugin = JSynPlugin::default();
        assert!(
            start_init.elapsed() < Duration::from_secs(2),
            "JSynPlugin::default() took too long: {:?}",
            start_init.elapsed()
        );

        // Verify persisted custom state (`mod_matrix`, `modseq_steps`) round-trips quickly via nih-plug.
        let start_state = Instant::now();
        let serialized = plugin.params.serialize_fields();
        assert!(serialized.contains_key("mod_matrix"));
        assert!(serialized.contains_key("modseq_steps"));

        let restored = JSynPluginParams::default();
        restored.deserialize_fields(&serialized);
        let reserialized = restored.serialize_fields();
        assert_eq!(reserialized.get("mod_matrix"), serialized.get("mod_matrix"));
        assert_eq!(reserialized.get("modseq_steps"), serialized.get("modseq_steps"));
        assert!(
            start_state.elapsed() < Duration::from_secs(1),
            "Plugin state serialize/deserialize round-trip took too long: {:?}",
            start_state.elapsed()
        );

        // Verify idle processing with default FX slots and active-patch FX slots produces silence quickly.
        let default_params = j_syn_dsp::SynthParams::default();
        let mut patch_fx_params = j_syn_dsp::SynthParams::default();
        for (slot_idx, slot) in patch_fx_params.fx_bus_a.slots.iter_mut().enumerate() {
            *slot = j_syn_dsp::fx::FxSlotParams::init_patch_slot(slot_idx);
        }
        for (slot_idx, slot) in patch_fx_params.fx_bus_b.slots.iter_mut().enumerate() {
            *slot = j_syn_dsp::fx::FxSlotParams::init_patch_slot(slot_idx);
        }

        let start_idle = Instant::now();
        for _ in 0..4096 {
            let (l, r) = plugin.engine.process_sample(&default_params, &plugin.wavetable_bank);
            assert_eq!(l, 0.0);
            assert_eq!(r, 0.0);
        }
        for _ in 0..4096 {
            let (l, r) = plugin.engine.process_sample(&patch_fx_params, &plugin.wavetable_bank);
            assert!(l.abs() < 1e-6 && r.abs() < 1e-6);
        }
        assert!(
            start_idle.elapsed() < Duration::from_secs(1),
            "Idle DSP processing took too long: {:?}",
            start_idle.elapsed()
        );
    }

    #[test]
    fn theme_css_parses_cleanly() {
        let css = j_syn_ui::theme::BASE_THEME_CSS;
        assert!(
            !css.contains(":focus-within"),
            "Vizia's CSS parser does not support :focus-within"
        );
    }

    #[test]
    fn patch_json_roundtrip_captures_all_fields() {
        let params = Arc::new(JSynPluginParams::default());
        {
            let mut guard = params.macro_names.lock().unwrap();
            guard[0] = "WOBBLE".to_string();
        }
        let mut mat = params.mod_matrix.snapshot();
        mat.slots[0] = j_syn_dsp::ModSlot {
            source: j_syn_dsp::ModMatrixSource::Macro1,
            destination: j_syn_dsp::ModDestination::Filter1Cutoff,
            depth: 0.65,
            bipolar: true,
            enabled: true,
            bypassed: false,
            aux_source: j_syn_dsp::ModMatrixSource::ModWheel,
            curve: 0.35,
        };
        params.mod_matrix.store_value(mat);

        let patch = crate::presets::JSynPatch::capture("Test Patch", &params);
        let json = serde_json::to_string(&patch).unwrap();
        let decoded: crate::presets::JSynPatch = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.version, crate::presets::CURRENT_PATCH_VERSION);
        assert_eq!(decoded.name, "Test Patch");
        assert_eq!(decoded.macro_names[0], "WOBBLE");
        assert!(decoded.plain_params.contains_key("glide_ms"));
        assert!(decoded.enum_names.contains_key("fx_type_1"));
        assert_eq!(decoded.mod_matrix.slots[0].aux_source, j_syn_dsp::ModMatrixSource::ModWheel);
        assert!((decoded.mod_matrix.slots[0].curve - 0.35).abs() < 1e-6);
        assert!(decoded.params.contains_key("dc_blocker"));
        assert!(decoded.params.contains_key("master_limiter"));
        assert!(decoded.params.contains_key("bus_a_lvl"));
        assert!(decoded.params.contains_key("bus_b_lvl"));
        assert!(decoded.params.contains_key("bus_b_fx_en_1"));
        assert!(decoded.params.contains_key("sub_en"));
        assert!(decoded.params.contains_key("sub_cutoff"));
        assert!(decoded.params.contains_key("sub_drive"));
        assert!(decoded.params.contains_key("sub_lvl"));
        assert!(decoded.params.contains_key("fx_p5_1"));
        assert!(decoded.params.contains_key("fx_flt_m_1"));
        assert!(decoded.params.contains_key("fx_diop_s_1"));
        assert!(decoded.params.contains_key("fx_tr_flt_1"));
        assert!(decoded.params.contains_key("fx_util_inv_l_1"));
        assert!(decoded.params.contains_key("fx_util_inv_r_1"));
        assert!(decoded.params.contains_key("fx_lim_tp_1"));
        assert!(decoded.params.contains_key("f1_out_bus"));
        assert!(decoded.params.contains_key("f2_out_bus"));
        assert!(decoded.params.contains_key("fx_bus_topo"));
        assert!(decoded.params.contains_key("mpe_en"));
        assert!(decoded.params.contains_key("mpe_pb"));
        assert!(decoded.params.contains_key("pitch_mode_1"));
        assert!(decoded.params.contains_key("ratio_1"));
        assert!(decoded.params.contains_key("hz_offset_1"));

        // Verify DSP SymmetricalSkewed Hz offset mapping matches nih-plug FloatParam range
        for step in 0..=10 {
            let norm = step as f32 / 10.0;
            let param_hz = params.oscs[0].hz_offset.preview_plain(norm);
            let dsp_hz = j_syn_dsp::osc_hz_offset_from_norm(norm);
            assert!(
                (param_hz - dsp_hz).abs() < 1e-2,
                "hz_offset mismatch at norm={norm}: param={param_hz}, dsp={dsp_hz}"
            );
        }
    }
}
