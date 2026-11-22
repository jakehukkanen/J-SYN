//! `j-filter`: Standalone zero-allocation 15-mode ZDF SVF + Comb + Scream Filter FX plugin
//! (VST3 + CLAP), powered by `j-syn-dsp` (`FxFilterStereo`) and `j-syn-ui`.

use std::num::NonZeroU32;
use std::sync::Arc;

use j_syn_dsp::fx::FxFilterStereo;
use j_syn_dsp::mod_matrix::FxScopeRing;
use j_syn_dsp::{FxSlotParams, FxType, MAX_FX_STACK};
use j_syn_ui::fx_units::{
    format_fx_slot_param_value, FilterModeParam, FxFilterSlopeParam,
};
use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;

pub mod editor;

#[derive(Params)]
pub struct JFilterParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    #[id = "en"]
    pub enabled: BoolParam,

    #[id = "stk"]
    pub stack: IntParam,

    #[id = "mode"]
    pub mode: EnumParam<FilterModeParam>,

    #[id = "slope"]
    pub slope: EnumParam<FxFilterSlopeParam>,

    #[id = "cutoff"]
    pub cutoff: FloatParam,

    #[id = "res"]
    pub resonance: FloatParam,

    #[id = "drive"]
    pub drive: FloatParam,

    #[id = "mix"]
    pub mix: FloatParam,

    #[id = "freq2"]
    pub freq2: FloatParam,
}

impl Default for JFilterParams {
    fn default() -> Self {
        let defs = FxSlotParams::default_params_for_type(FxType::Filter);
        let eq_defs = FxSlotParams::default_eq_params_for_type(FxType::Filter);
        let fmt = |idx: usize| {
            Arc::new(move |v: f32| format_fx_slot_param_value(FxType::Filter, false, 0, idx, v))
        };
        let fmt_freq2 =
            Arc::new(|v: f32| format_fx_slot_param_value(FxType::Filter, true, 0, 0, v));
        Self {
            editor_state: editor::default_state(),
            enabled: BoolParam::new("Bypass / On", true),
            stack: IntParam::new("Stack", 1, IntRange::Linear { min: 1, max: MAX_FX_STACK as i32 }),
            mode: EnumParam::new("Mode", FilterModeParam::Lp),
            slope: EnumParam::new("Slope", FxFilterSlopeParam::Db12),
            cutoff: FloatParam::new("Cutoff", defs[0], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt(0)),
            resonance: FloatParam::new(
                "Resonance",
                defs[1],
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(fmt(1)),
            drive: FloatParam::new("Drive", defs[2], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt(2)),
            mix: FloatParam::new("Mix", defs[3], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt(3)),
            freq2: FloatParam::new("Freq 2", eq_defs[0], FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(fmt_freq2),
        }
    }
}

pub struct JFilterPlugin {
    pub params: Arc<JFilterParams>,
    dsp: Box<[FxFilterStereo; MAX_FX_STACK]>,
    pub scope_ring: Arc<FxScopeRing>,
    scope_decim: u32,
}

impl Default for JFilterPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(JFilterParams::default()),
            dsp: Box::new(std::array::from_fn(|_| FxFilterStereo::new(44_100.0))),
            scope_ring: Arc::new(FxScopeRing::default()),
            scope_decim: 0,
        }
    }
}

impl Plugin for JFilterPlugin {
    const NAME: &'static str = "J-Filter";
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
        let mode = self.params.mode.value().into();
        let slope = self.params.slope.value().into();
        let p = [
            self.params.cutoff.value(),
            self.params.resonance.value(),
            self.params.drive.value(),
            self.params.mix.value(),
        ];
        let freq2 = self.params.freq2.value();

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
                    let (nl, nr, s_act) = stage.process(cur_l, cur_r, mode, slope, p, freq2);
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

impl ClapPlugin for JFilterPlugin {
    const CLAP_ID: &'static str = "com.jakehukkanen.j-filter";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Zero-allocation 15-mode ZDF SVF + Comb + Scream Filter");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Filter,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for JFilterPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"JFiltFxPlugin001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Filter];
}

nih_export_clap!(JFilterPlugin);
nih_export_vst3!(JFilterPlugin);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_plugin_parameter_ids_are_unique() {
        let params = JFilterParams::default();
        let mut seen = HashSet::new();
        for (id, _, _) in params.param_map() {
            assert!(seen.insert(id.clone()), "Duplicate parameter ID: '{id}'");
        }
        assert_eq!(seen.len(), 9);
    }

    #[test]
    fn state_roundtrip_and_idle_processing() {
        let mut plugin = JFilterPlugin::default();
        let serialized = plugin.params.serialize_fields();
        let restored = JFilterParams::default();
        restored.deserialize_fields(&serialized);
        assert_eq!(restored.serialize_fields().len(), serialized.len());

        let p = FxSlotParams::default_params_for_type(FxType::Filter);
        let eq = FxSlotParams::default_eq_params_for_type(FxType::Filter);
        for _ in 0..512 {
            let (l, r, act) = plugin.dsp[0].process(
                0.0,
                0.0,
                j_syn_dsp::FilterMode::Lp,
                j_syn_dsp::FxFilterSlope::Db12,
                p,
                eq[0],
            );
            assert_eq!(l, 0.0);
            assert_eq!(r, 0.0);
            assert_eq!(act, 0.0);
        }
    }
}
