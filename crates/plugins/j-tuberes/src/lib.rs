//! `j-tuberes`: Standalone zero-allocation Stereo Waveguide Tube Resonator FX plugin
//! (VST3 + CLAP), powered by `j-syn-dsp` (`TubeResonatorStereo`) and `j-syn-ui`.

use std::num::NonZeroU32;
use std::sync::Arc;

use j_syn_dsp::fx::tube_resonator::TubeResonatorStereo;
use j_syn_dsp::mod_matrix::FxScopeRing;
use j_syn_dsp::{FxSlotParams, FxType, MAX_FX_STACK};
use j_syn_ui::fx_units::format_fx_slot_param_value;
use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;

pub mod editor;

#[derive(Params)]
pub struct JTubeResParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    #[id = "en"]
    pub enabled: BoolParam,

    #[id = "stk"]
    pub stack: IntParam,

    #[id = "tune"]
    pub tune: FloatParam,

    #[id = "flt_en"]
    pub flt_en: BoolParam,

    #[id = "flt_fr"]
    pub flt_freq: FloatParam,

    #[id = "flt_wd"]
    pub flt_width: FloatParam,

    #[id = "mix"]
    pub mix: FloatParam,

    #[id = "lfo_am"]
    pub lfo_amt: FloatParam,

    #[id = "lfo_sp"]
    pub lfo_spd: FloatParam,

    #[id = "feedbk"]
    pub feedback: FloatParam,

    #[id = "damp"]
    pub damp: FloatParam,

    #[id = "dirty"]
    pub dirty: BoolParam,
}

impl Default for JTubeResParams {
    fn default() -> Self {
        let main_defs = FxSlotParams::default_params_for_type(FxType::TubeResonator);
        let lfo_defs = FxSlotParams::default_eq_params_for_type(FxType::TubeResonator);
        let fmt_main = |idx: usize| {
            Arc::new(move |v: f32| {
                format_fx_slot_param_value(FxType::TubeResonator, false, 0, idx, v)
            })
        };
        let fmt_lfo = |idx: usize| {
            Arc::new(move |v: f32| {
                format_fx_slot_param_value(FxType::TubeResonator, true, 0, idx, v)
            })
        };
        Self {
            editor_state: editor::default_state(),
            enabled: BoolParam::new("Bypass / On", true),
            stack: IntParam::new("Stack", 1, IntRange::Linear { min: 1, max: MAX_FX_STACK as i32 }),
            tune: FloatParam::new("Tune", main_defs[0], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt_main(0)),
            flt_en: BoolParam::new("Pre-Filter", true),
            flt_freq: FloatParam::new(
                "Filter Freq",
                main_defs[1],
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(fmt_main(1)),
            flt_width: FloatParam::new(
                "Filter Width",
                main_defs[2],
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(fmt_main(2)),
            mix: FloatParam::new("Mix", main_defs[3], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt_main(3)),
            lfo_amt: FloatParam::new(
                "LFO Amount",
                lfo_defs[0],
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(fmt_lfo(0)),
            lfo_spd: FloatParam::new(
                "LFO Speed",
                lfo_defs[1],
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(fmt_lfo(1)),
            feedback: FloatParam::new(
                "Feedback",
                lfo_defs[2],
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(fmt_lfo(2)),
            damp: FloatParam::new("Damping", lfo_defs[3], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt_lfo(3)),
            dirty: BoolParam::new("Dirty LFO", true),
        }
    }
}

pub struct JTubeResPlugin {
    pub params: Arc<JTubeResParams>,
    dsp: Box<[TubeResonatorStereo; MAX_FX_STACK]>,
    pub scope_ring: Arc<FxScopeRing>,
    scope_decim: u32,
}

impl Default for JTubeResPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(JTubeResParams::default()),
            dsp: Box::new(std::array::from_fn(|_| TubeResonatorStereo::new(44_100.0))),
            scope_ring: Arc::new(FxScopeRing::default()),
            scope_decim: 0,
        }
    }
}

impl Plugin for JTubeResPlugin {
    const NAME: &'static str = "J-TubeRes";
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
        for stage in self.dsp.iter_mut() {
            stage.set_sample_rate(buffer_config.sample_rate);
            stage.reset();
        }
        self.scope_decim = 0;
        true
    }

    fn reset(&mut self) {
        for stage in self.dsp.iter_mut() {
            stage.reset();
        }
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
        let stack_count = (self.params.stack.value() as usize).clamp(1, MAX_FX_STACK);
        let flt_en = self.params.flt_en.value();
        let dirty = self.params.dirty.value();
        let p = [
            self.params.tune.value(),
            self.params.flt_freq.value(),
            self.params.flt_width.value(),
            self.params.mix.value(),
        ];
        let eq = [
            self.params.lfo_amt.value(),
            self.params.lfo_spd.value(),
            self.params.feedback.value(),
            self.params.damp.value(),
        ];

        for mut channel_samples in buffer.iter_samples() {
            let mut ch = channel_samples.iter_mut();
            let (Some(s_l), Some(s_r)) = (ch.next(), ch.next()) else {
                continue;
            };
            let (in_l, in_r) = (*s_l, *s_r);
            let (out_l, out_r, act) = if enabled {
                let mut cur_l = in_l;
                let mut cur_r = in_r;
                let mut max_act = 0.0_f32;
                for stage in self.dsp.iter_mut().take(stack_count) {
                    let (nl, nr, s_act) = stage.process(cur_l, cur_r, p, eq, flt_en, dirty);
                    cur_l = nl;
                    cur_r = nr;
                    max_act = max_act.max(s_act);
                }
                (cur_l, cur_r, max_act)
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

impl ClapPlugin for JTubeResPlugin {
    const CLAP_ID: &'static str = "com.jakehukkanen.j-tuberes";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Zero-allocation Stereo Waveguide Tube Resonator");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Filter,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for JTubeResPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"JTubRFxPlugin001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Modulation];
}

nih_export_clap!(JTubeResPlugin);
nih_export_vst3!(JTubeResPlugin);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_plugin_parameter_ids_are_unique() {
        let params = JTubeResParams::default();
        let mut seen = HashSet::new();
        for (id, _, _) in params.param_map() {
            assert!(seen.insert(id.clone()), "Duplicate parameter ID: '{id}'");
        }
        assert_eq!(seen.len(), 12);
    }

    #[test]
    fn state_roundtrip_and_idle_processing() {
        let mut plugin = JTubeResPlugin::default();
        let serialized = plugin.params.serialize_fields();
        let restored = JTubeResParams::default();
        restored.deserialize_fields(&serialized);
        assert_eq!(restored.serialize_fields().len(), serialized.len());

        let p = FxSlotParams::default_params_for_type(FxType::TubeResonator);
        let eq = FxSlotParams::default_eq_params_for_type(FxType::TubeResonator);
        for _ in 0..512 {
            let (l, r, _act) = plugin.dsp[0].process(0.0, 0.0, p, eq, true, true);
            assert_eq!(l, 0.0);
            assert_eq!(r, 0.0);
        }
    }
}
