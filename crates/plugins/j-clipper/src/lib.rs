//! `j-clipper`: Standalone zero-allocation 4x oversampled ADAA Clipper FX plugin
//! (VST3 + CLAP), powered by `j-syn-dsp` (`JSatClipperStereo`) and `j-syn-ui`.

use std::num::NonZeroU32;
use std::sync::Arc;

use j_syn_dsp::fx::dynamics::JSatClipperStereo;
use j_syn_dsp::fx::MAX_FX_STACK;
use j_syn_dsp::mod_matrix::FxScopeRing;
use j_syn_dsp::{FxSlotParams, FxType};
use j_syn_ui::fx_units::fx_slot_float_param;
use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;

pub mod editor;

#[derive(Params)]
pub struct JClipperParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    #[id = "en"]
    pub enabled: BoolParam,

    #[id = "stk"]
    pub stack: IntParam,

    #[id = "drive"]
    pub drive: FloatParam,

    #[id = "ceil"]
    pub ceil: FloatParam,

    #[id = "knee"]
    pub knee: FloatParam,

    #[id = "mix"]
    pub mix: FloatParam,

    #[id = "pcm"]
    pub pcm: BoolParam,
}

impl Default for JClipperParams {
    fn default() -> Self {
        let defs = FxSlotParams::default_params_for_type(FxType::JSatClipper);
        Self {
            editor_state: editor::default_state(),
            enabled: BoolParam::new("Bypass / On", true),
            stack: IntParam::new("Stack", 1, IntRange::Linear { min: 1, max: 4 }),
            drive: fx_slot_float_param("Drive", defs[0], FxType::JSatClipper, false, 0, 0),
            ceil: fx_slot_float_param("Ceiling", defs[1], FxType::JSatClipper, false, 0, 1),
            knee: fx_slot_float_param("Knee", defs[2], FxType::JSatClipper, false, 0, 2),
            mix: fx_slot_float_param("Mix", defs[3], FxType::JSatClipper, false, 0, 3),
            pcm: BoolParam::new("PCM Clip", true),
        }
    }
}

pub struct JClipperPlugin {
    pub params: Arc<JClipperParams>,
    dsp: Box<[JSatClipperStereo; MAX_FX_STACK]>,
    pub scope_ring: Arc<FxScopeRing>,
    scope_decim: u32,
}

impl Default for JClipperPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(JClipperParams::default()),
            dsp: Box::new(std::array::from_fn(|_| JSatClipperStereo::new())),
            scope_ring: Arc::new(FxScopeRing::default()),
            scope_decim: 0,
        }
    }
}

impl Plugin for JClipperPlugin {
    const NAME: &'static str = "J-Clipper";
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
        let pcm = self.params.pcm.value();
        let p = [
            self.params.drive.value(),
            self.params.ceil.value(),
            self.params.knee.value(),
            self.params.mix.value(),
        ];

        for mut channel_samples in buffer.iter_samples() {
            let mut ch = channel_samples.iter_mut();
            let (Some(s_l), Some(s_r)) = (ch.next(), ch.next()) else {
                continue;
            };
            let (in_l, in_r) = (*s_l, *s_r);
            let (out_l, out_r, act) = if enabled {
                let mut l = in_l;
                let mut r = in_r;
                let mut max_act = 0.0_f32;
                for stage in self.dsp.iter_mut().take(stack_count) {
                    let (nl, nr, a) = stage.process(l, r, p, pcm);
                    l = nl;
                    r = nr;
                    if a > max_act {
                        max_act = a;
                    }
                }
                (l, r, max_act)
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

impl ClapPlugin for JClipperPlugin {
    const CLAP_ID: &'static str = "com.jakehukkanen.j-clipper";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Zero-allocation 4x oversampled ADAA soft/hard clipper");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Distortion,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for JClipperPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"JClipFxPlugin001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Distortion];
}

nih_export_clap!(JClipperPlugin);
nih_export_vst3!(JClipperPlugin);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_plugin_parameter_ids_are_unique() {
        let params = JClipperParams::default();
        let mut seen = HashSet::new();
        for (id, _, _) in params.param_map() {
            assert!(seen.insert(id.clone()), "Duplicate parameter ID: '{id}'");
        }
        assert_eq!(seen.len(), 7);
        j_syn_ui::fx_units::assert_all_params_text_roundtrip(&params);
    }

    #[test]
    fn state_roundtrip_and_idle_processing() {
        let mut plugin = JClipperPlugin::default();
        let serialized = plugin.params.serialize_fields();
        let restored = JClipperParams::default();
        restored.deserialize_fields(&serialized);
        assert_eq!(restored.serialize_fields().len(), serialized.len());

        let p = FxSlotParams::default_params_for_type(FxType::JSatClipper);
        for _ in 0..512 {
            let (l, r, act) = plugin.dsp[0].process(0.0, 0.0, p, true);
            assert_eq!(l, 0.0);
            assert_eq!(r, 0.0);
            assert_eq!(act, 0.0);
        }
    }
}
