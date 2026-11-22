//! `j-reverb`: Standalone zero-allocation algorithmic stereo Reverb + EQ FX plugin
//! (VST3 + CLAP), powered by `j-syn-dsp` (`ReverbStereo`) and `j-syn-ui`.

use std::num::NonZeroU32;
use std::sync::Arc;

use j_syn_dsp::fx::modulation::ReverbStereo;
use j_syn_dsp::mod_matrix::FxScopeRing;
use j_syn_dsp::{FxSlotParams, FxType};
use j_syn_ui::fx_units::format_fx_slot_param_value;
use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;

pub mod editor;

#[derive(Params)]
pub struct JReverbParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    #[id = "en"]
    pub enabled: BoolParam,

    #[id = "size"]
    pub size: FloatParam,

    #[id = "decay"]
    pub decay: FloatParam,

    #[id = "damp"]
    pub damp: FloatParam,

    #[id = "mix"]
    pub mix: FloatParam,

    #[id = "lo_cut"]
    pub low_cut: FloatParam,

    #[id = "hi_cut"]
    pub high_cut: FloatParam,

    #[id = "predly"]
    pub pre_delay: FloatParam,

    #[id = "width"]
    pub width: FloatParam,
}

impl Default for JReverbParams {
    fn default() -> Self {
        let main_defs = FxSlotParams::default_params_for_type(FxType::Reverb);
        let eq_defs = FxSlotParams::default_eq_params_for_type(FxType::Reverb);
        let fmt_main = |idx: usize| {
            Arc::new(move |v: f32| format_fx_slot_param_value(FxType::Reverb, false, 0, idx, v))
        };
        let fmt_eq = |idx: usize| {
            Arc::new(move |v: f32| format_fx_slot_param_value(FxType::Reverb, true, 0, idx, v))
        };
        Self {
            editor_state: editor::default_state(),
            enabled: BoolParam::new("Bypass / On", true),
            size: FloatParam::new("Size", main_defs[0], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt_main(0)),
            decay: FloatParam::new(
                "Decay",
                main_defs[1],
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(fmt_main(1)),
            damp: FloatParam::new("Damp", main_defs[2], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt_main(2)),
            mix: FloatParam::new("Mix", main_defs[3], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt_main(3)),
            low_cut: FloatParam::new(
                "Low Cut",
                eq_defs[0],
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(fmt_eq(0)),
            high_cut: FloatParam::new(
                "High Cut",
                eq_defs[1],
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(fmt_eq(1)),
            pre_delay: FloatParam::new(
                "Pre-Delay",
                eq_defs[2],
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(fmt_eq(2)),
            width: FloatParam::new("Width", eq_defs[3], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt_eq(3)),
        }
    }
}

pub struct JReverbPlugin {
    pub params: Arc<JReverbParams>,
    dsp: ReverbStereo,
    pub scope_ring: Arc<FxScopeRing>,
    scope_decim: u32,
}

impl Default for JReverbPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(JReverbParams::default()),
            dsp: ReverbStereo::new(44_100.0),
            scope_ring: Arc::new(FxScopeRing::default()),
            scope_decim: 0,
        }
    }
}

impl Plugin for JReverbPlugin {
    const NAME: &'static str = "J-Reverb";
    const VENDOR: &'static str = "Jake Hukkanen";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(
            self.params.clone(),
            self.params.editor_state.clone(),
            self.scope_ring.clone(),
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.dsp.set_sample_rate(buffer_config.sample_rate);
        self.dsp.reset();
        self.scope_decim = 0;
        true
    }

    fn reset(&mut self) {
        self.dsp.reset();
        self.scope_decim = 0;
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        if buffer.samples() == 0 {
            return ProcessStatus::Normal;
        }
        let enabled = self.params.enabled.value();
        let p = [
            self.params.size.value(),
            self.params.decay.value(),
            self.params.damp.value(),
            self.params.mix.value(),
        ];
        let eq = [
            self.params.low_cut.value(),
            self.params.high_cut.value(),
            self.params.pre_delay.value(),
            self.params.width.value(),
        ];

        for mut channel_samples in buffer.iter_samples() {
            let in_l = *channel_samples.get_mut(0).unwrap();
            let in_r = *channel_samples.get_mut(1).unwrap();
            let (out_l, out_r, act) = if enabled {
                self.dsp.process(in_l, in_r, p, eq)
            } else {
                (in_l, in_r, 0.0)
            };
            *channel_samples.get_mut(0).unwrap() = out_l;
            *channel_samples.get_mut(1).unwrap() = out_r;

            self.scope_decim = self.scope_decim.wrapping_add(1);
            if self.scope_decim & 127 == 0 {
                let pre_mono = 0.5 * (in_l.abs() + in_r.abs());
                let post_mono = 0.5 * (out_l.abs() + out_r.abs());
                self.scope_ring.push(pre_mono, post_mono, act);
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for JReverbPlugin {
    const CLAP_ID: &'static str = "com.jakehukkanen.j-reverb";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Zero-allocation algorithmic stereo Reverb + EQ processor");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Reverb,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for JReverbPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"JRevbFxPlugin001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Reverb];
}

nih_export_clap!(JReverbPlugin);
nih_export_vst3!(JReverbPlugin);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_plugin_parameter_ids_are_unique() {
        let params = JReverbParams::default();
        let mut seen = HashSet::new();
        for (id, _, _) in params.param_map() {
            assert!(seen.insert(id.clone()), "Duplicate parameter ID: '{id}'");
        }
        assert_eq!(seen.len(), 9);
    }

    #[test]
    fn state_roundtrip_and_idle_processing() {
        let mut plugin = JReverbPlugin::default();
        let serialized = plugin.params.serialize_fields();
        let restored = JReverbParams::default();
        restored.deserialize_fields(&serialized);
        assert_eq!(restored.serialize_fields().len(), serialized.len());

        let p = FxSlotParams::default_params_for_type(FxType::Reverb);
        let eq = FxSlotParams::default_eq_params_for_type(FxType::Reverb);
        for _ in 0..512 {
            let (l, r, act) = plugin.dsp.process(0.0, 0.0, p, eq);
            assert_eq!(l, 0.0);
            assert_eq!(r, 0.0);
            assert_eq!(act, 0.0);
        }
    }
}
