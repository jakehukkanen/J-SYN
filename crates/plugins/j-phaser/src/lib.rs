//! `j-phaser`: Standalone zero-allocation 6-stage allpass Phaser FX plugin
//! (VST3 + CLAP), powered by `j-syn-dsp` (`PhaserStereo`) and `j-syn-ui`.

use std::num::NonZeroU32;
use std::sync::Arc;

use j_syn_dsp::fx::modulation::PhaserStereo;
use j_syn_dsp::mod_matrix::FxScopeRing;
use j_syn_dsp::{FxSlotParams, FxType};
use j_syn_ui::fx_units::format_fx_slot_param_value;
use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;

pub mod editor;

#[derive(Params)]
pub struct JPhaserParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    #[id = "en"]
    pub enabled: BoolParam,

    #[id = "rate"]
    pub rate: FloatParam,

    #[id = "depth"]
    pub depth: FloatParam,

    #[id = "feedbk"]
    pub feedback: FloatParam,

    #[id = "mix"]
    pub mix: FloatParam,
}

impl Default for JPhaserParams {
    fn default() -> Self {
        let defs = FxSlotParams::default_params_for_type(FxType::Phaser);
        let fmt = |idx: usize| {
            Arc::new(move |v: f32| format_fx_slot_param_value(FxType::Phaser, false, 0, idx, v))
        };
        Self {
            editor_state: editor::default_state(),
            enabled: BoolParam::new("Bypass / On", true),
            rate: FloatParam::new("Rate", defs[0], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt(0)),
            depth: FloatParam::new("Depth", defs[1], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt(1)),
            feedback: FloatParam::new(
                "Feedback",
                defs[2],
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(fmt(2)),
            mix: FloatParam::new("Mix", defs[3], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt(3)),
        }
    }
}

pub struct JPhaserPlugin {
    pub params: Arc<JPhaserParams>,
    dsp: PhaserStereo,
    pub scope_ring: Arc<FxScopeRing>,
    scope_decim: u32,
}

impl Default for JPhaserPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(JPhaserParams::default()),
            dsp: PhaserStereo::new(44_100.0),
            scope_ring: Arc::new(FxScopeRing::default()),
            scope_decim: 0,
        }
    }
}

impl Plugin for JPhaserPlugin {
    const NAME: &'static str = "J-Phaser";
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
            self.params.feedback.value(),
            self.params.mix.value(),
        ];

        for mut channel_samples in buffer.iter_samples() {
            let mut ch = channel_samples.iter_mut();
            let (Some(s_l), Some(s_r)) = (ch.next(), ch.next()) else {
                continue;
            };
            let (in_l, in_r) = (*s_l, *s_r);
            let (out_l, out_r, act) = if enabled {
                self.dsp.process(in_l, in_r, p)
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

impl ClapPlugin for JPhaserPlugin {
    const CLAP_ID: &'static str = "com.jakehukkanen.j-phaser";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Zero-allocation 6-stage allpass Phaser with quadrature LFO");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Phaser,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for JPhaserPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"JPhasFxPlugin001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Modulation];
}

nih_export_clap!(JPhaserPlugin);
nih_export_vst3!(JPhaserPlugin);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_plugin_parameter_ids_are_unique() {
        let params = JPhaserParams::default();
        let mut seen = HashSet::new();
        for (id, _, _) in params.param_map() {
            assert!(seen.insert(id.clone()), "Duplicate parameter ID: '{id}'");
        }
        assert_eq!(seen.len(), 5);
    }

    #[test]
    fn state_roundtrip_and_idle_processing() {
        let mut plugin = JPhaserPlugin::default();
        let serialized = plugin.params.serialize_fields();
        let restored = JPhaserParams::default();
        restored.deserialize_fields(&serialized);
        assert_eq!(restored.serialize_fields().len(), serialized.len());

        let p = FxSlotParams::default_params_for_type(FxType::Phaser);
        for _ in 0..512 {
            let (l, r, _act) = plugin.dsp.process(0.0, 0.0, p);
            assert_eq!(l, 0.0);
            assert_eq!(r, 0.0);
        }
    }
}
