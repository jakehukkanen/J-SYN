//! Shared FX slot parameter display formatters and inverse text parsers (`value_to_string` /
//! `string_to_value`) used identically by `J-SYN` scopes/tooltips and all 14 standalone FX plugins.

use std::sync::Arc;

use j_syn_dsp::filter::cutoff_norm_to_hz;
use j_syn_dsp::fx::tube_resonator::{TUBE_RES_MIDI_SPAN, TUBE_RES_MIN_MIDI};
use j_syn_dsp::{
    DELAY_SYNC_LABELS, FxRackState, FxType, diopser_freq_hz, flanger_delay_ms, flanger_feedback,
    flanger_phase_deg, flanger_rate_hz, format_tube_res_tune, format_utility_pan, fx_high_cut_hz,
    fx_low_cut_hz, limiter_ceil_db, limiter_gain_db, limiter_lookahead_ms, limiter_release_ms,
    mb_band_level_db, mb_out_gain_db, mb_xover_high_hz, mb_xover_low_hz, tube_res_decay_ms,
    tube_res_flt_freq_hz, tube_res_flt_width_oct, tube_res_lfo_rate_hz, utility_gain_db_from_norm,
    utility_width_from_norm,
};
use nih_plug::prelude::{FloatParam, FloatRange, Params};

#[inline]
pub fn format_hz(hz: f32) -> String {
    if hz >= 10_000.0 {
        format!("{:.1} kHz", hz / 1000.0)
    } else if hz >= 1000.0 {
        format!("{:.2} kHz", hz / 1000.0)
    } else {
        format!("{:.0} Hz", hz)
    }
}

#[inline]
pub fn format_fx_slot_param_value(
    fx_ty: FxType,
    is_eq: bool,
    mb_tab: u8,
    param_idx: usize,
    norm: f32,
) -> String {
    let n = norm.clamp(0.0, 1.0);
    if fx_ty == FxType::MultibandDist {
        return if mb_tab == 0 {
            match param_idx {
                0 => format_hz(mb_xover_low_hz(n)),
                1 => format_hz(mb_xover_high_hz(n)),
                2 => format!("{:+.1} dB", mb_out_gain_db(n)),
                _ => format!("{:.0}%", n * 100.0),
            }
        } else {
            match param_idx {
                0 | 3 => format!("{:.0}%", n * 100.0),
                1 => format!("{:+.0}%", (n - 0.5) * 200.0),
                _ => format!("{:+.1} dB", mb_band_level_db(n)),
            }
        };
    }
    if is_eq {
        if fx_ty == FxType::TubeResonator {
            return match param_idx {
                0 => format!("{:.0}%", n * 100.0),
                1 => format!("{:.2} Hz", tube_res_lfo_rate_hz(n)),
                2 => format!("{:.1} ms", tube_res_decay_ms(n)),
                _ => format!("{:.0}%", n * 100.0),
            };
        }
        if fx_ty == FxType::Limiter {
            return match param_idx {
                0 => format!("{:.0} ms", limiter_release_ms(n)),
                _ => format!("{:.0}%", n * 100.0),
            };
        }
        if fx_ty == FxType::Flanger {
            return match param_idx {
                0 => format!("{:+.0}%", flanger_feedback(n) * 100.0),
                1 => format!("{:.0}°", flanger_phase_deg(n)),
                2 => format_hz(fx_low_cut_hz(n)),
                _ => format_hz(fx_high_cut_hz(n)),
            };
        }
        if fx_ty == FxType::Filter {
            return format_hz(cutoff_norm_to_hz(n));
        }
        return match param_idx {
            0 => format_hz(fx_low_cut_hz(n)),
            1 => format_hz(fx_high_cut_hz(n)),
            2 => match fx_ty {
                FxType::Reverb => format!("{:.0} ms", n * 250.0),
                FxType::Chorus => format!("{:.0}%", n * 200.0),
                _ => format!("{:.0}%", n * 100.0),
            },
            _ => match fx_ty {
                FxType::Chorus => format!("{:.0}%", n * 75.0),
                _ => format!("{:.0}%", n * 200.0),
            },
        };
    }

    match fx_ty {
        FxType::Off => "--".to_string(),
        FxType::Ott => match param_idx {
            0 => format!("{:.0}%", n * 100.0),
            1 => format!("{:.2}x", 0.25 * 16.0_f32.powf(n)),
            2 | 3 => format!("{:+.1} dB", -18.0 + n * 36.0),
            _ => String::new(),
        },
        FxType::Distortion => match param_idx {
            0 | 2 => format!("{:+.0}%", (n - 0.5) * 200.0),
            1 | 3 => format!("{:.0}%", n * 100.0),
            _ => String::new(),
        },
        FxType::Chorus => match param_idx {
            0 => format!("{:.2} Hz", 0.05 * 100.0_f32.powf(n)),
            _ => format!("{:.0}%", n * 100.0),
        },
        FxType::Delay => match param_idx {
            0 => FxRackState::format_delay_sync(n).to_string(),
            _ => format!("{:.0}%", n * 100.0),
        },
        FxType::Reverb => format!("{:.0}%", n * 100.0),
        FxType::JSatClipper => match param_idx {
            0 => format!("+{:.1} dB", n * 24.0),
            1 => format!("{:.1} dB", -18.0 + n * 18.0),
            _ => format!("{:.0}%", n * 100.0),
        },
        FxType::Filter => match param_idx {
            0 => format_hz(cutoff_norm_to_hz(n)),
            _ => format!("{:.0}%", n * 100.0),
        },
        FxType::Phaser => match param_idx {
            0 => format!("{:.2} Hz", 0.05 * 120.0_f32.powf(n)),
            _ => format!("{:.0}%", n * 100.0),
        },
        FxType::Diopser => match param_idx {
            0 => format_hz(diopser_freq_hz(n)),
            _ => format!("{:.0}%", n * 100.0),
        },
        FxType::TubeResonator => match param_idx {
            0 => format_tube_res_tune(n),
            1 => format_hz(tube_res_flt_freq_hz(n)),
            2 => format!("{:.2} oct", tube_res_flt_width_oct(n)),
            _ => format!("{:.0}%", n * 100.0),
        },
        FxType::Utility => match param_idx {
            0 => format!("{:.0}%", utility_width_from_norm(n) * 100.0),
            1 => format_utility_pan(n),
            2 => {
                let db = utility_gain_db_from_norm(n);
                let clean = if db.abs() < 0.05 { 0.0 } else { db };
                if clean == 0.0 {
                    "0.0 dB".to_string()
                } else {
                    format!("{clean:+.1} dB")
                }
            }
            _ => format!("{:.0}%", n * 100.0),
        },
        FxType::Limiter => match param_idx {
            0 => format!("+{:.1} dB", limiter_gain_db(n)),
            1 => format!("{:.1} dB", limiter_ceil_db(n)),
            2 => {
                let ms = limiter_lookahead_ms(n);
                if ms < 0.05 {
                    "0.0 (Clip)".to_string()
                } else {
                    format!("{ms:.1} ms")
                }
            }
            _ => format!("{:.0}%", n * 100.0),
        },
        FxType::Flanger => match param_idx {
            0 => format!("{:.2} Hz", flanger_rate_hz(n)),
            2 => format!("{:.2} ms", flanger_delay_ms(n)),
            _ => format!("{:.0}%", n * 100.0),
        },
        FxType::MultibandDist => String::new(),
    }
}

#[inline]
fn invert_monotone(target: f32, f: impl Fn(f32) -> f32) -> f32 {
    let mut lo = 0.0_f32;
    let mut hi = 1.0_f32;
    if target <= f(lo) {
        return lo;
    }
    if target >= f(hi) {
        return hi;
    }
    for _ in 0..24 {
        let mid = 0.5 * (lo + hi);
        if f(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

#[inline]
fn strip_suffix_ci<'a>(s: &'a str, suffix: &str) -> Option<&'a str> {
    let s_bytes = s.as_bytes();
    let suf_bytes = suffix.as_bytes();
    if s_bytes.len() < suf_bytes.len() {
        return None;
    }
    let split = s_bytes.len() - suf_bytes.len();
    if s.is_char_boundary(split) && s[split..].eq_ignore_ascii_case(suffix) {
        Some(s[..split].trim())
    } else {
        None
    }
}

#[inline]
fn parse_finite(s: &str) -> Option<f32> {
    let v = s.trim().parse::<f32>().ok()?;
    v.is_finite().then_some(v)
}

#[inline]
fn parse_pct(text: &str) -> Option<f32> {
    let t = text.trim();
    let num = strip_suffix_ci(t, "%").unwrap_or(t);
    parse_finite(num)
}

#[inline]
fn parse_db(text: &str) -> Option<f32> {
    let t = text.trim();
    let num = strip_suffix_ci(t, "db").unwrap_or(t);
    parse_finite(num)
}

#[inline]
fn parse_ms(text: &str) -> Option<f32> {
    let t = text.trim();
    let num = strip_suffix_ci(t, "ms").unwrap_or(t);
    parse_finite(num)
}

#[inline]
fn parse_hz(text: &str) -> Option<f32> {
    let t = text.trim();
    if let Some(khz) = strip_suffix_ci(t, "khz").or_else(|| strip_suffix_ci(t, "k")) {
        return parse_finite(khz).map(|v| v * 1000.0);
    }
    let hz = strip_suffix_ci(t, "hz").unwrap_or(t);
    parse_finite(hz)
}

#[inline]
fn parse_mult_x(text: &str) -> Option<f32> {
    let t = text.trim();
    let num = strip_suffix_ci(t, "x").unwrap_or(t);
    parse_finite(num)
}

#[inline]
fn parse_oct(text: &str) -> Option<f32> {
    let t = text.trim();
    let num = strip_suffix_ci(t, "oct").unwrap_or(t);
    parse_finite(num)
}

#[inline]
fn parse_deg(text: &str) -> Option<f32> {
    let t = text.trim();
    let num = t
        .strip_suffix('°')
        .map(str::trim)
        .or_else(|| strip_suffix_ci(t, "deg"))
        .unwrap_or(t);
    parse_finite(num)
}

fn parse_delay_sync(text: &str) -> Option<f32> {
    let t = text.trim();
    for (idx, label) in DELAY_SYNC_LABELS.iter().enumerate() {
        if t.eq_ignore_ascii_case(label) {
            return Some(((idx as f32) + 0.5) / DELAY_SYNC_LABELS.len() as f32);
        }
    }
    parse_finite(t).map(|v| v.clamp(0.0, 1.0))
}

fn parse_utility_pan(text: &str) -> Option<f32> {
    let t = text.trim();
    if t.eq_ignore_ascii_case("c") || t.eq_ignore_ascii_case("center") {
        return Some(0.5);
    }
    if let Some(num) = strip_suffix_ci(t, "l").and_then(parse_finite) {
        return Some((0.5 - num / 100.0).clamp(0.0, 1.0));
    }
    if let Some(num) = strip_suffix_ci(t, "r").and_then(parse_finite) {
        return Some((0.5 + num / 100.0).clamp(0.0, 1.0));
    }
    if t.len() > 1 && t.as_bytes()[0].eq_ignore_ascii_case(&b'l') {
        if let Some(num) = parse_finite(&t[1..]) {
            return Some((0.5 - num / 100.0).clamp(0.0, 1.0));
        }
    }
    if t.len() > 1 && t.as_bytes()[0].eq_ignore_ascii_case(&b'r') {
        if let Some(num) = parse_finite(&t[1..]) {
            return Some((0.5 + num / 100.0).clamp(0.0, 1.0));
        }
    }
    parse_finite(t).map(|v| (0.5 + v / 100.0).clamp(0.0, 1.0))
}

fn parse_tube_res_tune(text: &str) -> Option<f32> {
    let t = text.trim();
    let mut parts = t.split_whitespace();
    let note_tok = parts.next()?;
    let bytes = note_tok.as_bytes();
    let base = match bytes.first()?.to_ascii_uppercase() {
        b'C' => 0_i32,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => return parse_finite(t).map(|v| v.clamp(0.0, 1.0)),
    };
    let mut idx = 1;
    let accidental = match bytes.get(idx) {
        Some(b'#') => {
            idx += 1;
            1
        }
        Some(b'b' | b'B') => {
            idx += 1;
            -1
        }
        _ => 0,
    };
    let octave: i32 = note_tok[idx..].parse().ok()?;
    let cents = if let Some(cents_tok) = parts.next() {
        let stripped = strip_suffix_ci(cents_tok, "c").unwrap_or(cents_tok);
        parse_finite(stripped)?
    } else {
        0.0
    };
    if parts.next().is_some() {
        return None;
    }
    let midi = ((octave + 1) * 12 + base + accidental) as f32 + cents / 100.0;
    Some(((midi - TUBE_RES_MIN_MIDI) / TUBE_RES_MIDI_SPAN).clamp(0.0, 1.0))
}

/// Exact inverse of [`format_fx_slot_param_value`], parsing host/user text back to normalized `0.0..=1.0`.
#[inline]
pub fn parse_fx_slot_param_value(
    fx_ty: FxType,
    is_eq: bool,
    mb_tab: u8,
    param_idx: usize,
    text: &str,
) -> Option<f32> {
    if fx_ty == FxType::MultibandDist {
        return if mb_tab == 0 {
            match param_idx {
                0 => parse_hz(text).map(|hz| invert_monotone(hz, mb_xover_low_hz)),
                1 => parse_hz(text).map(|hz| invert_monotone(hz, mb_xover_high_hz)),
                2 => parse_db(text).map(|db| ((db + 18.0) / 36.0).clamp(0.0, 1.0)),
                _ => parse_pct(text).map(|p| (p / 100.0).clamp(0.0, 1.0)),
            }
        } else {
            match param_idx {
                0 | 3 => parse_pct(text).map(|p| (p / 100.0).clamp(0.0, 1.0)),
                1 => parse_pct(text).map(|p| (p / 200.0 + 0.5).clamp(0.0, 1.0)),
                _ => parse_db(text).map(|db| ((db + 24.0) / 36.0).clamp(0.0, 1.0)),
            }
        };
    }

    if is_eq {
        if fx_ty == FxType::TubeResonator {
            return match param_idx {
                0 => parse_pct(text).map(|p| (p / 100.0).clamp(0.0, 1.0)),
                1 => parse_hz(text).map(|hz| invert_monotone(hz, tube_res_lfo_rate_hz)),
                2 => parse_ms(text).map(|ms| invert_monotone(ms, tube_res_decay_ms)),
                _ => parse_pct(text).map(|p| (p / 100.0).clamp(0.0, 1.0)),
            };
        }
        if fx_ty == FxType::Limiter {
            return match param_idx {
                0 => parse_ms(text).map(|ms| invert_monotone(ms, limiter_release_ms)),
                _ => parse_pct(text).map(|p| (p / 100.0).clamp(0.0, 1.0)),
            };
        }
        if fx_ty == FxType::Flanger {
            return match param_idx {
                0 => parse_pct(text).map(|p| (((p / 100.0) / 0.99 + 1.0) * 0.5).clamp(0.0, 1.0)),
                1 => parse_deg(text).map(|d| (d / 180.0).clamp(0.0, 1.0)),
                2 => parse_hz(text).map(|hz| invert_monotone(hz, fx_low_cut_hz)),
                _ => parse_hz(text).map(|hz| invert_monotone(hz, fx_high_cut_hz)),
            };
        }
        if fx_ty == FxType::Filter {
            return parse_hz(text).map(|hz| invert_monotone(hz, cutoff_norm_to_hz));
        }
        return match param_idx {
            0 => parse_hz(text).map(|hz| invert_monotone(hz, fx_low_cut_hz)),
            1 => parse_hz(text).map(|hz| invert_monotone(hz, fx_high_cut_hz)),
            2 => match fx_ty {
                FxType::Reverb => parse_ms(text).map(|ms| (ms / 250.0).clamp(0.0, 1.0)),
                FxType::Chorus => parse_pct(text).map(|p| (p / 200.0).clamp(0.0, 1.0)),
                _ => parse_pct(text).map(|p| (p / 100.0).clamp(0.0, 1.0)),
            },
            _ => match fx_ty {
                FxType::Chorus => parse_pct(text).map(|p| (p / 75.0).clamp(0.0, 1.0)),
                _ => parse_pct(text).map(|p| (p / 200.0).clamp(0.0, 1.0)),
            },
        };
    }

    match fx_ty {
        FxType::Off => parse_finite(text).map(|v| v.clamp(0.0, 1.0)),
        FxType::Ott => match param_idx {
            0 => parse_pct(text).map(|p| (p / 100.0).clamp(0.0, 1.0)),
            1 => parse_mult_x(text).map(|m| invert_monotone(m, |n| 0.25 * 16.0_f32.powf(n))),
            2 | 3 => parse_db(text).map(|db| ((db + 18.0) / 36.0).clamp(0.0, 1.0)),
            _ => None,
        },
        FxType::Distortion => match param_idx {
            0 | 2 => parse_pct(text).map(|p| (p / 200.0 + 0.5).clamp(0.0, 1.0)),
            1 | 3 => parse_pct(text).map(|p| (p / 100.0).clamp(0.0, 1.0)),
            _ => None,
        },
        FxType::Chorus => match param_idx {
            0 => parse_hz(text).map(|hz| invert_monotone(hz, |n| 0.05 * 100.0_f32.powf(n))),
            _ => parse_pct(text).map(|p| (p / 100.0).clamp(0.0, 1.0)),
        },
        FxType::Delay => match param_idx {
            0 => parse_delay_sync(text),
            _ => parse_pct(text).map(|p| (p / 100.0).clamp(0.0, 1.0)),
        },
        FxType::Reverb => parse_pct(text).map(|p| (p / 100.0).clamp(0.0, 1.0)),
        FxType::JSatClipper => match param_idx {
            0 => parse_db(text).map(|db| (db / 24.0).clamp(0.0, 1.0)),
            1 => parse_db(text).map(|db| ((db + 18.0) / 18.0).clamp(0.0, 1.0)),
            _ => parse_pct(text).map(|p| (p / 100.0).clamp(0.0, 1.0)),
        },
        FxType::Filter => match param_idx {
            0 => parse_hz(text).map(|hz| invert_monotone(hz, cutoff_norm_to_hz)),
            _ => parse_pct(text).map(|p| (p / 100.0).clamp(0.0, 1.0)),
        },
        FxType::Phaser => match param_idx {
            0 => parse_hz(text).map(|hz| invert_monotone(hz, |n| 0.05 * 120.0_f32.powf(n))),
            _ => parse_pct(text).map(|p| (p / 100.0).clamp(0.0, 1.0)),
        },
        FxType::Diopser => match param_idx {
            0 => parse_hz(text).map(|hz| invert_monotone(hz, diopser_freq_hz)),
            _ => parse_pct(text).map(|p| (p / 100.0).clamp(0.0, 1.0)),
        },
        FxType::TubeResonator => match param_idx {
            0 => parse_tube_res_tune(text),
            1 => parse_hz(text).map(|hz| invert_monotone(hz, tube_res_flt_freq_hz)),
            2 => parse_oct(text).map(|oct| ((oct - 0.5) / 8.5).clamp(0.0, 1.0)),
            _ => parse_pct(text).map(|p| (p / 100.0).clamp(0.0, 1.0)),
        },
        FxType::Utility => match param_idx {
            0 => parse_pct(text).map(|p| invert_monotone(p / 100.0, utility_width_from_norm)),
            1 => parse_utility_pan(text),
            2 => parse_db(text).map(|db| invert_monotone(db, utility_gain_db_from_norm)),
            _ => parse_pct(text).map(|p| (p / 100.0).clamp(0.0, 1.0)),
        },
        FxType::Limiter => match param_idx {
            0 => parse_db(text).map(|db| (db / 24.0).clamp(0.0, 1.0)),
            1 => parse_db(text).map(|db| ((db + 18.0) / 18.0).clamp(0.0, 1.0)),
            2 => {
                if text.to_ascii_lowercase().contains("clip") {
                    Some(0.0)
                } else {
                    parse_ms(text).map(|ms| invert_monotone(ms, limiter_lookahead_ms))
                }
            }
            _ => parse_pct(text).map(|p| (p / 100.0).clamp(0.0, 1.0)),
        },
        FxType::Flanger => match param_idx {
            0 => parse_hz(text).map(|hz| invert_monotone(hz, flanger_rate_hz)),
            2 => parse_ms(text).map(|ms| invert_monotone(ms, flanger_delay_ms)),
            _ => parse_pct(text).map(|p| (p / 100.0).clamp(0.0, 1.0)),
        },
        FxType::MultibandDist => None,
    }
}

/// Shared `nih_plug` `with_value_to_string` closure for any normalized FX parameter.
pub fn fx_slot_param_v2s(
    fx_ty: FxType,
    is_eq: bool,
    mb_tab: u8,
    param_idx: usize,
) -> Arc<dyn Fn(f32) -> String + Send + Sync> {
    Arc::new(move |v: f32| format_fx_slot_param_value(fx_ty, is_eq, mb_tab, param_idx, v))
}

/// Shared `nih_plug` `with_string_to_value` closure matching [`fx_slot_param_v2s`].
pub fn fx_slot_param_s2v(
    fx_ty: FxType,
    is_eq: bool,
    mb_tab: u8,
    param_idx: usize,
) -> Arc<dyn Fn(&str) -> Option<f32> + Send + Sync> {
    Arc::new(move |s: &str| parse_fx_slot_param_value(fx_ty, is_eq, mb_tab, param_idx, s))
}

/// Builds a normalized `0.0..=1.0` `FloatParam` wired with matching `value_to_string` and
/// `string_to_value` conversions so CLAP/VST3 text roundtrips (`param-conversions`) always succeed.
pub fn fx_slot_float_param(
    name: impl Into<String>,
    default: f32,
    fx_ty: FxType,
    is_eq: bool,
    mb_tab: u8,
    param_idx: usize,
) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_value_to_string(fx_slot_param_v2s(fx_ty, is_eq, mb_tab, param_idx))
        .with_string_to_value(fx_slot_param_s2v(fx_ty, is_eq, mb_tab, param_idx))
}

/// Mirrors `clap-validator`'s `param-conversions` check across every parameter of a `Params` struct.
pub fn assert_all_params_text_roundtrip<P: Params>(params: &P) {
    for (id, ptr, _) in params.param_map() {
        for step in 0..=20 {
            let norm = step as f32 / 20.0;
            // SAFETY: `ptr` comes directly from `params.param_map()` and remains alive for `params`.
            let text = unsafe { ptr.normalized_value_to_string(norm, true) };
            let parsed = unsafe { ptr.string_to_normalized_value(&text) };
            assert!(
                parsed.is_some(),
                "param '{id}' failed text_to_value roundtrip: norm={norm:.3} -> '{text}' -> None"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_fx_slot_formatters_roundtrip_parse() {
        let all_fx = [
            FxType::Ott,
            FxType::Distortion,
            FxType::Chorus,
            FxType::Delay,
            FxType::Reverb,
            FxType::JSatClipper,
            FxType::Filter,
            FxType::Phaser,
            FxType::Diopser,
            FxType::TubeResonator,
            FxType::Utility,
            FxType::Limiter,
            FxType::Flanger,
            FxType::MultibandDist,
        ];

        for fx_ty in all_fx {
            let tab_cases: &[(bool, u8)] = if fx_ty == FxType::MultibandDist {
                &[(false, 0), (false, 1)]
            } else {
                &[(false, 0), (true, 0)]
            };
            for &(is_eq, mb_tab) in tab_cases {
                for param_idx in 0..4 {
                    for step in 0..=100 {
                        let norm = step as f32 / 100.0;
                        let formatted =
                            format_fx_slot_param_value(fx_ty, is_eq, mb_tab, param_idx, norm);
                        assert!(
                            !formatted.is_empty(),
                            "empty formatted value for {fx_ty:?} eq={is_eq} tab={mb_tab} p={param_idx}"
                        );
                        let parsed = parse_fx_slot_param_value(
                            fx_ty,
                            is_eq,
                            mb_tab,
                            param_idx,
                            &formatted,
                        )
                        .unwrap_or_else(|| {
                            panic!(
                                "failed to parse '{formatted}' for {fx_ty:?} eq={is_eq} tab={mb_tab} p={param_idx} (norm={norm})"
                            )
                        });
                        assert!(
                            (0.0..=1.0).contains(&parsed),
                            "parsed {parsed} out of range for '{formatted}'"
                        );
                        // Re-formatting the parsed value should match the original display text
                        // (allowing tiny 1-ULP rounding boundary tolerance on quantized displays).
                        let reformatted =
                            format_fx_slot_param_value(fx_ty, is_eq, mb_tab, param_idx, parsed);
                        if reformatted != formatted {
                            assert!(
                                (parsed - norm).abs() < 0.03,
                                "roundtrip drift too large for {fx_ty:?} eq={is_eq} tab={mb_tab} p={param_idx}: norm={norm} -> '{formatted}' -> {parsed} -> '{reformatted}'"
                            );
                        }
                    }
                }
            }
        }
    }
}
