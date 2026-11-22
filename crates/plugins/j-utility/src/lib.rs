//! `j-utility`: Standalone zero-allocation stereo Utility FX plugin (VST3 + CLAP)
//! powered by `j-syn-dsp` (`UtilityStereo`) and `j-syn-ui` (shared `Utility` FX Unit view).

use std::num::NonZeroU32;
use std::sync::Arc;

use j_syn_dsp::fx::utility::UtilityStereo;
use j_syn_dsp::mod_matrix::FxScopeRing;
use j_syn_dsp::{
    format_utility_pan, utility_gain_db_from_norm, utility_width_from_norm, MAX_FX_STACK,
    UTILITY_WIDTH_DEFAULT_NORM,
};
use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;

pub mod editor;

#[derive(Params)]
pub struct JUtilityParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    #[id = "en"]
    pub enabled: BoolParam,

    #[id = "stk"]
    pub stack: IntParam,

    #[id = "width"]
    pub width: FloatParam,

    #[id = "pan"]
    pub pan: FloatParam,

    #[id = "gain"]
    pub gain: FloatParam,

    #[id = "mix"]
    pub mix: FloatParam,

    #[id = "inv_l"]
    pub inv_l: BoolParam,

    #[id = "inv_r"]
    pub inv_r: BoolParam,
}

impl Default for JUtilityParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),
            enabled: BoolParam::new("Bypass / On", true),
            stack: IntParam::new("Stack", 1, IntRange::Linear { min: 1, max: MAX_FX_STACK as i32 }),
            width: FloatParam::new(
                "Width",
                UTILITY_WIDTH_DEFAULT_NORM,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(Arc::new(|v| {
                format!("{:.0}%", utility_width_from_norm(v) * 100.0)
            })),
            pan: FloatParam::new("Balance", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(Arc::new(format_utility_pan)),
            gain: FloatParam::new("Gain", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(Arc::new(|v| {
                    let db = utility_gain_db_from_norm(v);
                    let clean = if db.abs() < 0.05 { 0.0 } else { db };
                    if clean == 0.0 {
                        "0.0 dB".to_string()
                    } else {
                        format!("{clean:+.1} dB")
                    }
                })),
            mix: FloatParam::new("Mix", 1.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit(" %")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            inv_l: BoolParam::new("Invert Left", false),
            inv_r: BoolParam::new("Invert Right", false),
        }
    }
}

pub struct JUtilityPlugin {
    pub params: Arc<JUtilityParams>,
    dsp: Box<[UtilityStereo; MAX_FX_STACK]>,
    pub scope_ring: Arc<FxScopeRing>,
    scope_decim: u32,
}

impl Default for JUtilityPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(JUtilityParams::default()),
            dsp: Box::new(std::array::from_fn(|_| UtilityStereo::new(44_100.0))),
            scope_ring: Arc::new(FxScopeRing::default()),
            scope_decim: 0,
        }
    }
}

impl Plugin for JUtilityPlugin {
    const NAME: &'static str = "J-Utility";
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
        let num_samples = buffer.samples();
        if num_samples == 0 {
            return ProcessStatus::Normal;
        }

        let enabled = self.params.enabled.value();
        let stack_count = (self.params.stack.value() as usize).clamp(1, MAX_FX_STACK);
        let inv_l = self.params.inv_l.value();
        let inv_r = self.params.inv_r.value();
        let p = [
            self.params.width.value(),
            self.params.pan.value(),
            self.params.gain.value(),
            self.params.mix.value(),
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
                    let (nl, nr, s_act) = stage.process(cur_l, cur_r, p, inv_l, inv_r);
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

impl ClapPlugin for JUtilityPlugin {
    const CLAP_ID: &'static str = "com.jakehukkanen.j-utility";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Zero-allocation Mid/Side Width, Stereo Balance, Gain & Phase Invert utility");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Utility,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for JUtilityPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"JUtilFxPlugin001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Spatial];
}

nih_export_clap!(JUtilityPlugin);
nih_export_vst3!(JUtilityPlugin);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_plugin_parameter_ids_are_unique() {
        let params = JUtilityParams::default();
        let param_map = params.param_map();
        let mut seen = HashSet::new();
        for (id, _, _) in param_map {
            assert!(
                seen.insert(id.clone()),
                "Duplicate nih-plug parameter ID detected: '{id}'"
            );
        }
        assert!(seen.len() >= 8);
    }

    #[test]
    fn state_roundtrip_and_idle_processing() {
        let mut plugin = JUtilityPlugin::default();
        let serialized = plugin.params.serialize_fields();
        let restored = JUtilityParams::default();
        restored.deserialize_fields(&serialized);
        assert_eq!(restored.serialize_fields().len(), serialized.len());

        // Idle silence stays exact zero with zero allocations
        for _ in 0..512 {
            let (l, r, act) = plugin.dsp[0].process(
                0.0,
                0.0,
                [UTILITY_WIDTH_DEFAULT_NORM, 0.5, 0.5, 1.0],
                false,
                false,
            );
            assert_eq!(l, 0.0);
            assert_eq!(r, 0.0);
            assert_eq!(act, 0.0);
        }

        // Verify window sizing stays aligned with FxType::Utility design token
        let (w, h) = editor::utility_window_size();
        assert!(w >= j_syn_ui::tokens::FX_UNIT_W as u32);
        assert!(h > j_syn_ui::tokens::fx_unit_scope_height(j_syn_dsp::FxType::Utility) as u32);
    }
}
