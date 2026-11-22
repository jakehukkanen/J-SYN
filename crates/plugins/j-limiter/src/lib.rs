//! `j-limiter`: Standalone zero-allocation Lookahead S-Curve + True Peak Limiter FX plugin
//! (VST3 + CLAP), powered by `j-syn-dsp` (`LimiterStereo`) and `j-syn-ui`.

use std::num::NonZeroU32;
use std::sync::Arc;

use j_syn_dsp::fx::limiter::LimiterStereo;
use j_syn_dsp::mod_matrix::FxScopeRing;
use j_syn_dsp::{limiter_lookahead_samples, FxSlotParams, FxType};
use j_syn_ui::fx_units::format_fx_slot_param_value;
use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;

pub mod editor;

#[derive(Params)]
pub struct JLimiterParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    #[id = "en"]
    pub enabled: BoolParam,

    #[id = "gain"]
    pub gain: FloatParam,

    #[id = "ceil"]
    pub ceil: FloatParam,

    #[id = "lookhd"]
    pub lookahead: FloatParam,

    #[id = "mix"]
    pub mix: FloatParam,

    #[id = "rel"]
    pub release: FloatParam,

    #[id = "knee"]
    pub knee: FloatParam,

    #[id = "link"]
    pub link: FloatParam,

    #[id = "punch"]
    pub punch: FloatParam,

    #[id = "tp"]
    pub true_peak: BoolParam,
}

impl Default for JLimiterParams {
    fn default() -> Self {
        let main_defs = FxSlotParams::default_params_for_type(FxType::Limiter);
        let adv_defs = FxSlotParams::default_eq_params_for_type(FxType::Limiter);
        let fmt_main = |idx: usize| {
            Arc::new(move |v: f32| format_fx_slot_param_value(FxType::Limiter, false, 0, idx, v))
        };
        let fmt_adv = |idx: usize| {
            Arc::new(move |v: f32| format_fx_slot_param_value(FxType::Limiter, true, 0, idx, v))
        };
        Self {
            editor_state: editor::default_state(),
            enabled: BoolParam::new("Bypass / On", true),
            gain: FloatParam::new("Gain", main_defs[0], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt_main(0)),
            ceil: FloatParam::new("Ceiling", main_defs[1], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt_main(1)),
            lookahead: FloatParam::new(
                "Lookahead",
                main_defs[2],
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(fmt_main(2)),
            mix: FloatParam::new("Mix", main_defs[3], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt_main(3)),
            release: FloatParam::new(
                "Release",
                adv_defs[0],
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(fmt_adv(0)),
            knee: FloatParam::new("Knee", adv_defs[1], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt_adv(1)),
            link: FloatParam::new("Link", adv_defs[2], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt_adv(2)),
            punch: FloatParam::new("Punch", adv_defs[3], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt_adv(3)),
            true_peak: BoolParam::new("True Peak", false),
        }
    }
}

pub struct JLimiterPlugin {
    pub params: Arc<JLimiterParams>,
    dsp: LimiterStereo,
    sample_rate: f32,
    last_latency_samples: u32,
    pub scope_ring: Arc<FxScopeRing>,
    scope_decim: u32,
}

impl Default for JLimiterPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(JLimiterParams::default()),
            dsp: LimiterStereo::new(44_100.0),
            sample_rate: 44_100.0,
            last_latency_samples: 0,
            scope_ring: Arc::new(FxScopeRing::default()),
            scope_decim: 0,
        }
    }
}

impl Plugin for JLimiterPlugin {
    const NAME: &'static str = "J-Limiter";
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
        context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate.max(1_000.0);
        self.dsp.set_sample_rate(self.sample_rate);
        self.dsp.reset();
        self.scope_decim = 0;

        let lat = if self.params.enabled.value() {
            limiter_lookahead_samples(self.sample_rate, self.params.lookahead.value())
        } else {
            0
        };
        self.last_latency_samples = lat;
        context.set_latency_samples(lat);
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
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        if buffer.samples() == 0 {
            return ProcessStatus::Normal;
        }
        let enabled = self.params.enabled.value();
        let true_peak = self.params.true_peak.value();
        let p = [
            self.params.gain.value(),
            self.params.ceil.value(),
            self.params.lookahead.value(),
            self.params.mix.value(),
        ];
        let eq = [
            self.params.release.value(),
            self.params.knee.value(),
            self.params.link.value(),
            self.params.punch.value(),
        ];

        let lat = if enabled {
            limiter_lookahead_samples(self.sample_rate, p[2])
        } else {
            0
        };
        if lat != self.last_latency_samples {
            self.last_latency_samples = lat;
            context.set_latency_samples(lat);
        }

        for mut channel_samples in buffer.iter_samples() {
            let in_l = *channel_samples.get_mut(0).unwrap();
            let in_r = *channel_samples.get_mut(1).unwrap();
            let (out_l, out_r, act) = if enabled {
                self.dsp.process(in_l, in_r, p, eq, true_peak)
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

impl ClapPlugin for JLimiterPlugin {
    const CLAP_ID: &'static str = "com.jakehukkanen.j-limiter";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Zero-allocation Lookahead + True Peak Limiter");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Limiter,
        ClapFeature::Mastering,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for JLimiterPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"JLimtFxPlugin001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Dynamics];
}

nih_export_clap!(JLimiterPlugin);
nih_export_vst3!(JLimiterPlugin);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_plugin_parameter_ids_are_unique() {
        let params = JLimiterParams::default();
        let mut seen = HashSet::new();
        for (id, _, _) in params.param_map() {
            assert!(seen.insert(id.clone()), "Duplicate parameter ID: '{id}'");
        }
        assert_eq!(seen.len(), 10);
    }

    #[test]
    fn state_roundtrip_pdc_and_idle_processing() {
        let mut plugin = JLimiterPlugin::default();
        let serialized = plugin.params.serialize_fields();
        let restored = JLimiterParams::default();
        restored.deserialize_fields(&serialized);
        assert_eq!(restored.serialize_fields().len(), serialized.len());

        assert_eq!(limiter_lookahead_samples(48_000.0, 0.0), 0);
        assert_eq!(limiter_lookahead_samples(48_000.0, 1.0), 144);

        let p = FxSlotParams::default_params_for_type(FxType::Limiter);
        let eq = FxSlotParams::default_eq_params_for_type(FxType::Limiter);
        for _ in 0..1024 {
            let (l, r, act) = plugin.dsp.process(0.0, 0.0, p, eq, false);
            assert_eq!(l, 0.0);
            assert_eq!(r, 0.0);
            assert_eq!(act, 0.0);
        }
    }
}
