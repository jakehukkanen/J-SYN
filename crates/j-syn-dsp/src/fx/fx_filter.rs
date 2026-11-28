//! Full-parity multimode stereo Filter FX reusing per-voice ZDF SVF + Comb + ADAA Scream engine.

use crate::filter::{
    FilterMode, FilterParams, FilterSlope, FilterState, cutoff_norm_to_hz,
    evaluate_filter_response_db,
};
use crate::oscillator::EngineMode;

use super::{FxFilterSlope, flush_denormal};

/// Full-parity multimode stereo Filter FX reusing per-voice ZDF SVF + Comb + ADAA Scream engine.
pub struct FxFilterStereo {
    state: FilterState,
    sample_rate: f32,
}

impl FxFilterStereo {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            state: FilterState::new(2_500.0),
            sample_rate: sample_rate.max(1_000.0),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1_000.0);
    }

    pub fn reset(&mut self) {
        self.state.reset();
    }

    #[inline]
    pub(super) fn filter_params(
        mode: FilterMode,
        slope: FxFilterSlope,
        p: [f32; 4],
        freq2_norm: f32,
    ) -> FilterParams {
        let fc = cutoff_norm_to_hz(p[0]);
        let eff_slope = if mode.has_fixed_12db_slope() {
            FilterSlope::Db12
        } else {
            slope.to_filter_slope()
        };
        let aux = match mode {
            FilterMode::Scream => p[2].clamp(0.0, 1.0),
            FilterMode::Comb => 0.35,
            m if m.is_multi() => freq2_norm.clamp(0.0, 1.0),
            _ => p[3].clamp(0.0, 1.0),
        };
        let drive = if mode == FilterMode::Scream {
            0.0
        } else {
            p[2].clamp(0.0, 1.0)
        };
        FilterParams {
            enabled: true,
            mode,
            slope: eff_slope,
            cutoff_hz: fc,
            resonance: p[1].clamp(0.0, 1.0),
            drive,
            keytrack: 0.0,
            aux,
        }
    }

    #[inline]
    pub fn process(
        &mut self,
        in_l: f32,
        in_r: f32,
        mode: FilterMode,
        slope: FxFilterSlope,
        p: [f32; 4],
        freq2_norm: f32,
    ) -> (f32, f32, f32) {
        let fp = Self::filter_params(mode, slope, p, freq2_norm);
        let mix = p[3].clamp(0.0, 1.0);
        let (wl, wr) =
            self.state
                .process_stereo(in_l, in_r, &fp, 60.0, self.sample_rate, EngineMode::Hq);
        // `Scream`, `Comb`, and multi-filters repurpose `fp.aux`, so apply Mix manually here.
        let (ol, or) = if mode == FilterMode::Scream || mode == FilterMode::Comb || mode.is_multi()
        {
            (in_l + (wl - in_l) * mix, in_r + (wr - in_r) * mix)
        } else {
            (wl, wr)
        };
        let act = ol.abs().max(or.abs()).clamp(0.0, 1.0) * mix;
        (flush_denormal(ol), flush_denormal(or), act)
    }
}

/// Evaluates steady-state magnitude response in dB (`-36.0` .. `+24.0` dB) for an FX Filter slot.
pub fn filter_response_db_from_slot(
    mode: FilterMode,
    slope: FxFilterSlope,
    p: [f32; 4],
    freq2_norm: f32,
    freq_hz: f32,
) -> f32 {
    let fp = FxFilterStereo::filter_params(mode, slope, p, freq2_norm);
    let mix = p[3].clamp(0.0, 1.0);
    let wet_db = evaluate_filter_response_db(&fp, freq_hz);
    if (mode == FilterMode::Scream || mode == FilterMode::Comb || mode.is_multi()) && mix < 0.999 {
        let wet_mag = 10.0_f32.powf(wet_db / 20.0);
        let mag = (1.0 - mix) + mix * wet_mag;
        (20.0 * mag.max(1e-4).log10()).clamp(-36.0, 24.0)
    } else {
        wet_db
    }
}
