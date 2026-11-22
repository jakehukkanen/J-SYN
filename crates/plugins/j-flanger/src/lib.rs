//! `j-flanger`: Standalone zero-allocation stereo Flanger + MOD FX plugin
//! (VST3 + CLAP), powered by `j-syn-dsp` (`FlangerStereo`) and `j-syn-ui`.

use std::num::NonZeroU32;
use std::sync::Arc;

use j_syn_dsp::fx::modulation::FlangerStereo;
use j_syn_dsp::mod_matrix::FxScopeRing;
use j_syn_dsp::{FxSlotParams, FxType};
use j_syn_ui::fx_units::format_fx_slot_param_value;
use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;

pub mod editor;

#[derive(Params)]
pub struct JFlangerParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    #[id = "en"]
    pub enabled: BoolParam,

    #[id = "rate"]
    pub rate: FloatParam,

    #[id = "depth"]
    pub depth: FloatParam,

    #[id = "delay"]
    pub delay: FloatParam,

    #[id = "mix"]
    pub mix: FloatParam,

    #[id = "feedbk"]
    pub feedback: FloatParam,

    #[id = "phase"]
    pub phase: FloatParam,

    #[id = "lo_cut"]
    pub low_cut: FloatParam,

    #[id = "tone"]
    pub tone: FloatParam,
}

impl Default for JFlangerParams {
    fn default() -> Self {
        let main_defs = FxSlotParams::default_params_for_type(FxType::Flanger);
        let mod_defs = FxSlotParams::default_eq_params_for_type(FxType::Flanger);
        let fmt_main = |idx: usize| {
            Arc::new(move |v: f32| format_fx_slot_param_value(FxType::Flanger, false, 0, idx, v))
        };
        let fmt_mod = |idx: usize| {
            Arc::new(move |v: f32| format_fx_slot_param_value(FxType::Flanger, true, 0, idx, v))
        };
        Self {
            editor_state: editor::default_state(),
            enabled: BoolParam::new("Bypass / On", true),
            rate: FloatParam::new("Rate", main_defs[0], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt_main(0)),
            depth: FloatParam::new(
                "Depth",
                main_defs[1],
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(fmt_main(1)),
            delay: FloatParam::new(
                "Delay",
                main_defs[2],
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(fmt_main(2)),
            mix: FloatParam::new("Mix", main_defs[3], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt_main(3)),
            feedback: FloatParam::new(
                "Feedback",
                mod_defs[0],
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(fmt_mod(0)),
            phase: FloatParam::new("Phase", mod_defs[1], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt_mod(1)),
            low_cut: FloatParam::new(
                "Low Cut",
                mod_defs[2],
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(fmt_mod(2)),
            tone: FloatParam::new("Tone", mod_defs[3], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt_mod(3)),
        }
    }
}

pub struct JFlangerPlugin {
    pub params: Arc<JFlangerParams>,
    dsp: FlangerStereo,
    pub scope_ring: Arc<FxScopeRing>,
    scope_decim: u32,
}

impl Default for JFlangerPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(JFlangerParams::default()),
            dsp: FlangerStereo::new(44_100.0),
            scope_ring: Arc::new(FxScopeRing::default()),
            scope_decim: 0,
        }
    }
}

impl Plugin for JFlangerPlugin {
    const NAME: &'static str = "J-Flanger";
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
            self.params.rate.value(),
            self.params.depth.value(),
            self.params.delay.value(),
            self.params.mix.value(),
        ];
        let eq = [
            self.params.feedback.value(),
            self.params.phase.value(),
            self.params.low_cut.value(),
            self.params.tone.value(),
        ];

        for mut channel_samples in buffer.iter_samples() {
            let mut ch = channel_samples.iter_mut();
            let (Some(s_l), Some(s_r)) = (ch.next(), ch.next()) else {
                continue;
            };
            let (in_l, in_r) = (*s_l, *s_r);
            let (out_l, out_r, act) = if enabled {
                self.dsp.process(in_l, in_r, p, eq)
            } else {
                (in_l, in_r, 0.0)
            };
            *s_l = out_l;
            *s_r = out_r;

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

impl ClapPlugin for JFlangerPlugin {
    const CLAP_ID: &'static str = "com.jakehukkanen.j-flanger";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Zero-allocation stereo Flanger + MOD processor");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Flanger,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for JFlangerPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"JFlngFxPlugin001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Modulation];
}

nih_export_clap!(JFlangerPlugin);
nih_export_vst3!(JFlangerPlugin);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_plugin_parameter_ids_are_unique() {
        let params = JFlangerParams::default();
        let mut seen = HashSet::new();
        for (id, _, _) in params.param_map() {
            assert!(seen.insert(id.clone()), "Duplicate parameter ID: '{id}'");
        }
        assert_eq!(seen.len(), 9);
    }

    #[test]
    fn state_roundtrip_and_idle_processing() {
        let mut plugin = JFlangerPlugin::default();
        let serialized = plugin.params.serialize_fields();
        let restored = JFlangerParams::default();
        restored.deserialize_fields(&serialized);
        assert_eq!(restored.serialize_fields().len(), serialized.len());

        let p = FxSlotParams::default_params_for_type(FxType::Flanger);
        let eq = FxSlotParams::default_eq_params_for_type(FxType::Flanger);
        for _ in 0..512 {
            let (l, r, act) = plugin.dsp.process(0.0, 0.0, p, eq);
            assert_eq!(l, 0.0);
            assert_eq!(r, 0.0);
            assert_eq!(act, 0.0);
        }
    }
}
