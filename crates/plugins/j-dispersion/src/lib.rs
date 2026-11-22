//! `j-dispersion`: Standalone zero-allocation 8..128-stage allpass Dispersion FX plugin
//! (VST3 + CLAP), powered by `j-syn-dsp` (`DiopserStereo`) and `j-syn-ui`.

use std::num::NonZeroU32;
use std::sync::Arc;

use j_syn_dsp::fx::diopser::DiopserStereo;
use j_syn_dsp::mod_matrix::FxScopeRing;
use j_syn_dsp::{FxSlotParams, FxType};
use j_syn_ui::fx_units::{format_fx_slot_param_value, DiopserStagesParam};
use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;

pub mod editor;

#[derive(Params)]
pub struct JDispersionParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    #[id = "en"]
    pub enabled: BoolParam,

    #[id = "stages"]
    pub stages: EnumParam<DiopserStagesParam>,

    #[id = "freq"]
    pub freq: FloatParam,

    #[id = "res"]
    pub res: FloatParam,

    #[id = "spread"]
    pub spread: FloatParam,

    #[id = "mix"]
    pub mix: FloatParam,
}

impl Default for JDispersionParams {
    fn default() -> Self {
        let defs = FxSlotParams::default_params_for_type(FxType::Diopser);
        let fmt = |idx: usize| {
            Arc::new(move |v: f32| format_fx_slot_param_value(FxType::Diopser, false, 0, idx, v))
        };
        Self {
            editor_state: editor::default_state(),
            enabled: BoolParam::new("Bypass / On", true),
            stages: EnumParam::new("Stages", DiopserStagesParam::S32),
            freq: FloatParam::new("Frequency", defs[0], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt(0)),
            res: FloatParam::new("Resonance", defs[1], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt(1)),
            spread: FloatParam::new("Spread", defs[2], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt(2)),
            mix: FloatParam::new("Mix", defs[3], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt(3)),
        }
    }
}

pub struct JDispersionPlugin {
    pub params: Arc<JDispersionParams>,
    dsp: DiopserStereo,
    pub scope_ring: Arc<FxScopeRing>,
    scope_decim: u32,
}

impl Default for JDispersionPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(JDispersionParams::default()),
            dsp: DiopserStereo::new(44_100.0),
            scope_ring: Arc::new(FxScopeRing::default()),
            scope_decim: 0,
        }
    }
}

impl Plugin for JDispersionPlugin {
    const NAME: &'static str = "J-Dispersion";
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
        let stages_idx = self.params.stages.value().to_index() as usize;
        let p = [
            self.params.freq.value(),
            self.params.res.value(),
            self.params.spread.value(),
            self.params.mix.value(),
        ];

        for mut channel_samples in buffer.iter_samples() {
            let mut ch = channel_samples.iter_mut();
            let (Some(s_l), Some(s_r)) = (ch.next(), ch.next()) else {
                continue;
            };
            let (in_l, in_r) = (*s_l, *s_r);
            let (out_l, out_r, act) = if enabled {
                self.dsp.process(in_l, in_r, p, stages_idx)
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

impl ClapPlugin for JDispersionPlugin {
    const CLAP_ID: &'static str = "com.jakehukkanen.j-dispersion";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Zero-allocation 8..128-stage allpass Dispersion phase disperser");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Filter,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for JDispersionPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"JDiopFxPlugin001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Filter];
}

nih_export_clap!(JDispersionPlugin);
nih_export_vst3!(JDispersionPlugin);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_plugin_parameter_ids_are_unique() {
        let params = JDispersionParams::default();
        let mut seen = HashSet::new();
        for (id, _, _) in params.param_map() {
            assert!(seen.insert(id.clone()), "Duplicate parameter ID: '{id}'");
        }
        assert_eq!(seen.len(), 6);
    }

    #[test]
    fn state_roundtrip_and_idle_processing() {
        let mut plugin = JDispersionPlugin::default();
        let serialized = plugin.params.serialize_fields();
        let restored = JDispersionParams::default();
        restored.deserialize_fields(&serialized);
        assert_eq!(restored.serialize_fields().len(), serialized.len());

        let p = FxSlotParams::default_params_for_type(FxType::Diopser);
        for _ in 0..512 {
            let (l, r, act) = plugin.dsp.process(0.0, 0.0, p, 2);
            assert_eq!(l, 0.0);
            assert_eq!(r, 0.0);
            assert_eq!(act, 0.0);
        }
    }
}
