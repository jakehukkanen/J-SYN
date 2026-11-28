//! Steady-state magnitude response evaluation for UI filter scope curves.

use std::f32::consts::PI;

use super::{ComboBandKind, FilterMode, FilterParams, FilterSlope, aux_to_freq2_hz};

fn evaluate_combo_band_mag(
    kind: ComboBandKind,
    w: f32,
    slope: FilterSlope,
    res: f32,
    drive_gain: f32,
) -> f32 {
    let w2 = w * w;
    let one_minus_w2 = 1.0 - w2;
    if slope == FilterSlope::Db6 && !kind.is_peak_or_notch() {
        let den = (1.0 + w2).sqrt().max(1e-4);
        let base = match kind {
            ComboBandKind::Hp => w / den,
            _ => 1.0 / den,
        };
        return base * drive_gain;
    }

    let stages = slope.num_2pole_stages().max(1);
    let k0 = if kind.is_peak_or_notch() {
        (1.4 - 1.25 * res).max(0.08)
    } else {
        (2.0 - 1.95 * res).max(0.05)
    };
    let d0 = (one_minus_w2 * one_minus_w2 + (k0 * w).powi(2))
        .sqrt()
        .max(1e-4);
    let peak_boost = (0.6 + 1.8 * res) / (stages as f32).sqrt();
    let mut m = match kind {
        ComboBandKind::Hp => (w2 * drive_gain) / d0,
        ComboBandKind::Lp => drive_gain / d0,
        ComboBandKind::Peak => {
            let num = (one_minus_w2 * one_minus_w2 + ((1.0 + peak_boost) * k0 * w).powi(2)).sqrt();
            (num * drive_gain) / d0
        }
        ComboBandKind::Notch => (one_minus_w2.abs() * drive_gain) / d0,
    };
    if stages > 1 {
        let k_helper = (std::f32::consts::SQRT_2 - 0.45 * res).max(0.75);
        let d_h = (one_minus_w2 * one_minus_w2 + (k_helper * w).powi(2))
            .sqrt()
            .max(1e-4);
        let stage_gain = match kind {
            ComboBandKind::Hp => w2 / d_h,
            ComboBandKind::Lp => 1.0 / d_h,
            ComboBandKind::Peak => {
                let num = (one_minus_w2 * one_minus_w2
                    + ((1.0 + 0.5 * peak_boost) * k_helper * w).powi(2))
                .sqrt();
                num / d_h
            }
            ComboBandKind::Notch => one_minus_w2.abs() / d_h,
        };
        m *= stage_gain.powi((stages - 1) as i32);
    }
    m
}

/// Evaluates an approximate steady-state magnitude response in dB (`-36.0` .. `+24.0` dB)
/// for visualizing the filter curve in `FilterResponseScope`.
pub fn evaluate_filter_response_db(params: &FilterParams, freq_hz: f32) -> f32 {
    let fc = params.cutoff_hz.clamp(20.0, 20_000.0);
    let res = params.resonance.clamp(0.0, 1.0);
    let drive = params.drive.clamp(0.0, 1.0);
    let aux = params.aux.clamp(0.0, 1.0);
    let w = (freq_hz.max(1.0) / fc).max(1e-4);
    let w2 = w * w;
    let one_minus_w2 = 1.0 - w2;

    let mag = match params.mode {
        FilterMode::Lp | FilterMode::Bp | FilterMode::Hp | FilterMode::Notch | FilterMode::Peak => {
            let wet = if params.slope == FilterSlope::Db6
                && !matches!(params.mode, FilterMode::Notch | FilterMode::Peak)
            {
                let den = (1.0 + w2).sqrt().max(1e-4);
                let base = match params.mode {
                    FilterMode::Lp => 1.0 / den,
                    FilterMode::Hp => w / den,
                    FilterMode::Bp => (2.0 * w) / (1.0 + w2).max(1e-4),
                    _ => 1.0 / den,
                };
                base * (1.0 + 0.15 * drive)
            } else {
                let stages = params.slope.num_2pole_stages().max(1);
                let k0 = (2.0 - 1.95 * res).max(0.05);
                let d0 = (one_minus_w2 * one_minus_w2 + (k0 * w).powi(2))
                    .sqrt()
                    .max(1e-4);
                let peak_boost = 0.6 + 1.8 * res;
                let mut m = match params.mode {
                    FilterMode::Lp => (1.0 + 0.15 * drive) / d0,
                    FilterMode::Bp => ((1.0 + res * 0.5) * w) / d0,
                    FilterMode::Hp => (w2 * (1.0 + 0.15 * drive)) / d0,
                    FilterMode::Notch => (one_minus_w2.abs() * (1.0 + 0.15 * drive)) / d0,
                    FilterMode::Peak => {
                        let num = (one_minus_w2 * one_minus_w2
                            + ((1.0 + peak_boost) * k0 * w).powi(2))
                        .sqrt();
                        (num * (1.0 + 0.15 * drive)) / d0
                    }
                    _ => 1.0 / d0,
                };
                if stages > 1 {
                    let k_helper = (std::f32::consts::SQRT_2 - 0.45 * res).max(0.75);
                    let d_h = (one_minus_w2 * one_minus_w2 + (k_helper * w).powi(2))
                        .sqrt()
                        .max(1e-4);
                    let stage_gain = match params.mode {
                        FilterMode::Lp => 1.0 / d_h,
                        FilterMode::Bp => (k_helper * w) / d_h,
                        FilterMode::Hp => w2 / d_h,
                        FilterMode::Notch => one_minus_w2.abs() / d_h,
                        FilterMode::Peak => {
                            let num = (one_minus_w2 * one_minus_w2
                                + ((1.0 + 0.5 * peak_boost) * k_helper * w).powi(2))
                            .sqrt();
                            num / d_h
                        }
                        _ => 1.0 / d_h,
                    };
                    m *= stage_gain.powi((stages - 1) as i32);
                }
                m
            };
            (1.0 - aux) + aux * wet
        }
        FilterMode::Scream => {
            const MIDI_20HZ: f32 = 15.486_821;
            const MIDI_20KHZ: f32 = 135.076_23;
            const HP_CUTOFF_MIN: f32 = MIDI_20HZ - 12.0;

            let lp_midi = 69.0 + 12.0 * (fc / 440.0).log2();
            let hp_midi =
                HP_CUTOFF_MIN + aux * (MIDI_20KHZ - HP_CUTOFF_MIN) - (MIDI_20KHZ - lp_midi);
            let hp_fc = (440.0 * 2.0_f32.powf((hp_midi - 69.0) / 12.0)).clamp(20.0, 20_000.0);
            let w_hp = (freq_hz.max(1.0) / hp_fc).max(1e-4);
            let hp_res = (w_hp * w_hp)
                / ((1.0 - w_hp * w_hp).powi(2) + ((1.414 - 0.7 * res) * w_hp).powi(2))
                    .sqrt()
                    .max(1e-4);
            let k_lp = std::f32::consts::SQRT_2;
            let lp_den = (one_minus_w2 * one_minus_w2 + (k_lp * w).powi(2))
                .sqrt()
                .max(1e-4);
            let fb_lift = 1.0 + res * (0.4 + 0.8 * aux) * hp_res.min(2.5);
            ((1.0 + 0.25 * drive) * fb_lift) / lp_den
        }
        FilterMode::Daft => {
            let k0 = (1.65 - 1.3 * res).max(0.2);
            let k1 = (1.85 - 1.75 * res).max(0.08);
            let d0 = (one_minus_w2 * one_minus_w2 + (k0 * w).powi(2)).sqrt();
            let d1 = (one_minus_w2 * one_minus_w2 + (k1 * w).powi(2)).sqrt();
            let bass_shelf = 1.0 + 0.30 * res * (-w).exp();
            let wet = bass_shelf / (d0 * d1).max(1e-4);
            (1.0 - aux) + aux * wet
        }
        FilterMode::Comb => {
            let fb = res * 0.90;
            let phase = 2.0 * PI * (freq_hz.max(1.0) / fc);
            let hf_damp = (1.0 - aux * 0.65 * (freq_hz / 20_000.0)).max(0.2);
            let eff_fb = fb * hf_damp;
            let den = (1.0 + eff_fb * eff_fb - 2.0 * eff_fb * phase.cos())
                .max(0.02)
                .sqrt();
            (1.0 + 0.15 * drive) / den
        }
        FilterMode::Bandreject => {
            let k = 1.0 + 3.5 * res;
            let den = (one_minus_w2 * one_minus_w2 + (k * w).powi(2))
                .sqrt()
                .max(1e-4);
            let wet = (one_minus_w2.abs() * (1.0 + 0.15 * drive)) / den;
            (1.0 - aux) + aux * wet
        }
        FilterMode::HpPeak
        | FilterMode::HpNotch
        | FilterMode::LpNotch
        | FilterMode::LpPeak
        | FilterMode::PeakPeak
        | FilterMode::NotchNotch => {
            let (band1, band2) = ComboBandKind::from_mode(params.mode);
            let mag1 = evaluate_combo_band_mag(band1, w, params.slope, res, 1.0 + 0.15 * drive);
            let freq2 = aux_to_freq2_hz(aux);
            let w2_s = (freq_hz.max(1.0) / freq2).max(1e-4);
            let mag2 = evaluate_combo_band_mag(band2, w2_s, params.slope, res, 1.0);
            mag1 * mag2
        }
    };

    (20.0 * mag.max(1e-4).log10()).clamp(-36.0, 24.0)
}
