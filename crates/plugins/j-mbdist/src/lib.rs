//! `j-mbdist`: Standalone zero-allocation 3-band phase-aligned `LR4` Multiband Distortion FX plugin
//! (VST3 + CLAP), powered by `j-syn-dsp` (`MultiBandDistStereo`) and `j-syn-ui` (shared `MB Dist` FX Unit view).

use std::num::NonZeroU32;
use std::sync::Arc;

use j_syn_dsp::fx::multiband_dist::MultiBandDistStereo;
use j_syn_dsp::mod_matrix::FxScopeRing;
use j_syn_dsp::{
    FxType, MbDistBandParams, MAX_FX_STACK, MB_BAND_LEVEL_DEFAULT_NORM, MB_XOVER_HIGH_DEFAULT_NORM,
    MB_XOVER_LOW_DEFAULT_NORM,
};
use j_syn_ui::fx_units::{fx_slot_float_param, DistortionModeParam};
use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;

pub mod editor;

#[derive(Params)]
pub struct JMbDistBandParams {
    #[id = "en"]
    pub enabled: BoolParam,

    #[id = "solo"]
    pub solo: BoolParam,

    #[id = "mode"]
    pub mode: EnumParam<DistortionModeParam>,

    #[id = "drv"]
    pub drive: FloatParam,

    #[id = "bias"]
    pub bias: FloatParam,

    #[id = "lvl"]
    pub level: FloatParam,

    #[id = "mix"]
    pub mix: FloatParam,
}

impl JMbDistBandParams {
    pub fn new(band_label: &str) -> Self {
        Self {
            enabled: BoolParam::new(format!("{band_label} On"), true),
            solo: BoolParam::new(format!("{band_label} Solo"), false),
            mode: EnumParam::new(format!("{band_label} Mode"), DistortionModeParam::Tube),
            drive: fx_slot_float_param(
                format!("{band_label} Drive"),
                0.30,
                FxType::MultibandDist,
                false,
                1,
                0,
            ),
            bias: fx_slot_float_param(
                format!("{band_label} Bias"),
                0.50,
                FxType::MultibandDist,
                false,
                1,
                1,
            ),
            level: fx_slot_float_param(
                format!("{band_label} Level"),
                MB_BAND_LEVEL_DEFAULT_NORM,
                FxType::MultibandDist,
                false,
                1,
                2,
            ),
            mix: fx_slot_float_param(
                format!("{band_label} Mix"),
                1.00,
                FxType::MultibandDist,
                false,
                1,
                3,
            ),
        }
    }

    #[inline]
    pub fn to_dsp(&self) -> MbDistBandParams {
        MbDistBandParams {
            mode: self.mode.value().into(),
            enabled: self.enabled.value(),
            solo: self.solo.value(),
            drive: self.drive.value(),
            bias: self.bias.value(),
            level: self.level.value(),
            mix: self.mix.value(),
        }
    }
}

#[derive(Params)]
pub struct JMbDistParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    #[id = "en"]
    pub enabled: BoolParam,

    #[id = "stk"]
    pub stack: IntParam,

    #[id = "lo_xov"]
    pub low_xover: FloatParam,

    #[id = "hi_xov"]
    pub high_xover: FloatParam,

    #[id = "out_gn"]
    pub out_gain: FloatParam,

    #[id = "mix"]
    pub mix: FloatParam,

    #[nested(id_prefix = "lo", group = "Low Band")]
    pub low_band: JMbDistBandParams,

    #[nested(id_prefix = "mid", group = "Mid Band")]
    pub mid_band: JMbDistBandParams,

    #[nested(id_prefix = "hi", group = "High Band")]
    pub high_band: JMbDistBandParams,
}

impl JMbDistParams {
    #[inline]
    pub fn band(&self, idx: usize) -> &JMbDistBandParams {
        match idx {
            0 => &self.low_band,
            1 => &self.mid_band,
            _ => &self.high_band,
        }
    }
}

impl Default for JMbDistParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),
            enabled: BoolParam::new("Bypass / On", true),
            stack: IntParam::new("Stack", 1, IntRange::Linear { min: 1, max: MAX_FX_STACK as i32 }),
            low_xover: fx_slot_float_param(
                "Low Crossover",
                MB_XOVER_LOW_DEFAULT_NORM,
                FxType::MultibandDist,
                false,
                0,
                0,
            ),
            high_xover: fx_slot_float_param(
                "High Crossover",
                MB_XOVER_HIGH_DEFAULT_NORM,
                FxType::MultibandDist,
                false,
                0,
                1,
            ),
            out_gain: fx_slot_float_param("Output Gain", 0.50, FxType::MultibandDist, false, 0, 2),
            mix: fx_slot_float_param("Mix", 1.00, FxType::MultibandDist, false, 0, 3),
            low_band: JMbDistBandParams::new("Low"),
            mid_band: JMbDistBandParams::new("Mid"),
            high_band: JMbDistBandParams::new("High"),
        }
    }
}

pub struct JMbDistPlugin {
    pub params: Arc<JMbDistParams>,
    dsp: Box<[MultiBandDistStereo; MAX_FX_STACK]>,
    pub scope_ring: Arc<FxScopeRing>,
    scope_decim: u32,
}

impl Default for JMbDistPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(JMbDistParams::default()),
            dsp: Box::new(std::array::from_fn(|_| MultiBandDistStereo::new(44_100.0))),
            scope_ring: Arc::new(FxScopeRing::default()),
            scope_decim: 0,
        }
    }
}

impl Plugin for JMbDistPlugin {
    const NAME: &'static str = "J-MBDist";
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
        let p = [
            self.params.low_xover.value(),
            self.params.high_xover.value(),
            self.params.out_gain.value(),
            self.params.mix.value(),
        ];
        let bands = [
            self.params.low_band.to_dsp(),
            self.params.mid_band.to_dsp(),
            self.params.high_band.to_dsp(),
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
                    let (nl, nr, s_act) = stage.process(cur_l, cur_r, &bands, p);
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

impl ClapPlugin for JMbDistPlugin {
    const CLAP_ID: &'static str = "com.jakehukkanen.j-mbdist";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Zero-allocation 3-band phase-aligned LR4 Multiband Distortion processor");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Distortion,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for JMbDistPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"JMbDstFxPlug0001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Distortion];
}

nih_export_clap!(JMbDistPlugin);
nih_export_vst3!(JMbDistPlugin);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_plugin_parameter_ids_are_unique() {
        let params = JMbDistParams::default();
        let param_map = params.param_map();
        let mut seen = HashSet::new();
        for (id, _, _) in param_map {
            assert!(
                seen.insert(id.clone()),
                "Duplicate nih-plug parameter ID detected: '{id}'"
            );
        }
        // 6 top-level + 3 * 7 per-band = 27 parameters
        assert_eq!(seen.len(), 27);
        j_syn_ui::fx_units::assert_all_params_text_roundtrip(&params);
    }

    #[test]
    fn state_roundtrip_and_idle_processing() {
        let mut plugin = JMbDistPlugin::default();
        let serialized = plugin.params.serialize_fields();
        let restored = JMbDistParams::default();
        restored.deserialize_fields(&serialized);
        assert_eq!(restored.serialize_fields().len(), serialized.len());

        let bands = [
            MbDistBandParams::default(),
            MbDistBandParams::default(),
            MbDistBandParams::default(),
        ];
        let p = [
            MB_XOVER_LOW_DEFAULT_NORM,
            MB_XOVER_HIGH_DEFAULT_NORM,
            0.5,
            1.0,
        ];

        // Idle silence stays exact zero with zero allocations
        for _ in 0..512 {
            let (l, r, act) = plugin.dsp[0].process(0.0, 0.0, &bands, p);
            assert_eq!(l, 0.0);
            assert_eq!(r, 0.0);
            assert_eq!(act, 0.0);
        }

        // Verify window sizing stays aligned with FxType::MultibandDist design token
        let (w, h) = editor::mbdist_window_size();
        assert!(w >= j_syn_ui::tokens::FX_UNIT_W as u32);
        assert!(
            h > j_syn_ui::tokens::fx_unit_scope_height(j_syn_dsp::FxType::MultibandDist) as u32
        );
    }
}
