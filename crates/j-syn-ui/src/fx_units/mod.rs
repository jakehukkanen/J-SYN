//! Shared FX Unit views, parameter labels, and value formatters consumed by both `J-SYN`
//! and standalone single-FX plugins (`crates/plugins/*`).

use std::sync::Arc;

use j_syn_dsp::filter::cutoff_norm_to_hz;
use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::{
    diopser_freq_hz, flanger_delay_ms, flanger_feedback, flanger_phase_deg, flanger_rate_hz,
    format_tube_res_tune, format_utility_pan, fx_high_cut_hz, fx_low_cut_hz, limiter_ceil_db,
    limiter_gain_db, limiter_lookahead_ms, limiter_release_ms, mb_band_level_db, mb_out_gain_db,
    mb_xover_high_hz, mb_xover_low_hz, tube_res_decay_ms, tube_res_flt_freq_hz,
    tube_res_flt_width_oct, tube_res_lfo_rate_hz, utility_gain_db_from_norm,
    utility_width_from_norm, FxRackState, FxType, MB_BAND_LEVEL_DEFAULT_NORM,
};
use nih_plug::prelude::{BoolParam, FloatParam, GuiContext, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;

use crate::controls::bold_label;
use crate::scopes::{FxSlotScopeSnapshot, FxSlotVisualizer};
use crate::shell::{ModUiHost, UiKitEvent};
use crate::tokens;

pub mod filter;
pub mod limiter;
pub mod multiband_dist;
pub mod params;
pub mod simple_units;
pub mod standalone_shell;
pub mod tube_res;

pub use filter::{build_filter_fx_unit_body, FilterUnitParams};
pub use limiter::{build_limiter_fx_unit_body, LimiterUnitParams};
pub use multiband_dist::{
    build_multiband_dist_fx_unit_body, MultibandDistUnitParams, MultibandTabHost,
};
pub use params::{
    DiopserStagesParam, DistortionModeParam, FilterModeParam, FxFilterSlopeParam,
    DIOPSER_STAGE_ITEMS, FX_FILTER_MODE_ITEMS, FX_FILTER_SLOPE_ITEMS, MB_DIST_MODE_ITEMS,
};
pub use simple_units::{
    build_clipper_fx_unit_body, build_diopser_fx_unit_body, build_distortion_fx_unit_body,
    build_four_knob_fx_unit_body, build_tabbed_fx_unit_body, ClipperUnitParams,
    DiopserUnitParams, DistortionUnitParams, EightKnobUnitParams, FourKnobUnitParams,
    FxSubTabHost,
};
pub use standalone_shell::{
    build_standalone_fx_shell, create_standalone_fx_editor, standalone_default_state,
    StandaloneFxConfig, StandaloneFxHeaderSpec, StandaloneFxUiData, StandaloneShellEvent,
};
pub use tube_res::{build_tube_res_fx_unit_body, TubeResonatorUnitParams};

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
pub fn fx_slot_param_label(
    fx_ty: FxType,
    is_eq: bool,
    is_scream: bool,
    mb_tab: u8,
    param_idx: usize,
) -> &'static str {
    if fx_ty == FxType::MultibandDist {
        return if mb_tab == 0 {
            match param_idx {
                0 => "LOW XOV",
                1 => "HIGH XOV",
                2 => "OUT GN",
                _ => "MIX",
            }
        } else {
            match param_idx {
                0 => "DRIVE",
                1 => "BIAS",
                2 => "LEVEL",
                _ => "MIX",
            }
        };
    }
    if is_eq {
        return match fx_ty {
            FxType::Chorus => match param_idx {
                0 => "LOW CUT",
                1 => "HIGH CUT",
                2 => "SPREAD",
                _ => "FEEDBK",
            },
            FxType::Delay => match param_idx {
                0 => "LOW CUT",
                1 => "HIGH CUT",
                2 => "PING-PNG",
                _ => "WIDTH",
            },
            FxType::Reverb => match param_idx {
                0 => "LOW CUT",
                1 => "HIGH CUT",
                2 => "PRE-DLY",
                _ => "WIDTH",
            },
            FxType::TubeResonator => match param_idx {
                0 => "LFO AMT",
                1 => "LFO SPD",
                2 => "FEEDBK",
                _ => "DAMP",
            },
            FxType::Limiter => match param_idx {
                0 => "RELEASE",
                1 => "KNEE",
                2 => "LINK",
                _ => "PUNCH",
            },
            FxType::Flanger => match param_idx {
                0 => "FEEDBK",
                1 => "PHASE",
                2 => "LOW CUT",
                _ => "TONE",
            },
            FxType::Filter => match param_idx {
                0 => "FREQ 2",
                _ => "--",
            },
            _ => "--",
        };
    }

    match fx_ty {
        FxType::Off => "--",
        FxType::Ott => match param_idx {
            0 => "DEPTH",
            1 => "TIME",
            2 => "IN GAIN",
            _ => "OUT GN",
        },
        FxType::Distortion => match param_idx {
            0 => "BIAS",
            1 => "DRIVE",
            2 => "TONE",
            _ => "MIX",
        },
        FxType::Chorus => match param_idx {
            0 => "RATE",
            1 => "DEPTH",
            2 => "DIM",
            _ => "MIX",
        },
        FxType::Delay => match param_idx {
            0 => "TIME",
            1 => "FEEDBK",
            2 => "MOD",
            _ => "MIX",
        },
        FxType::Reverb => match param_idx {
            0 => "SIZE",
            1 => "DECAY",
            2 => "DAMP",
            _ => "MIX",
        },
        FxType::JSatClipper => match param_idx {
            0 => "DRIVE",
            1 => "CEIL",
            2 => "KNEE",
            _ => "MIX",
        },
        FxType::Filter => match param_idx {
            0 => "CUTOFF",
            1 => "RES",
            2 => {
                if is_scream {
                    "SCREAM"
                } else {
                    "DRIVE"
                }
            }
            _ => "MIX",
        },
        FxType::Phaser => match param_idx {
            0 => "RATE",
            1 => "DEPTH",
            2 => "FEEDBK",
            _ => "MIX",
        },
        FxType::Diopser => match param_idx {
            0 => "FREQ",
            1 => "RES",
            2 => "SPREAD",
            _ => "MIX",
        },
        FxType::TubeResonator => match param_idx {
            0 => "TUNE",
            1 => "FLT FREQ",
            2 => "FLT WDTH",
            _ => "MIX",
        },
        FxType::Utility => match param_idx {
            0 => "WIDTH",
            1 => "PAN",
            2 => "GAIN",
            _ => "MIX",
        },
        FxType::Limiter => match param_idx {
            0 => "GAIN",
            1 => "CEIL",
            2 => "LOOKAHD",
            _ => "MIX",
        },
        FxType::Flanger => match param_idx {
            0 => "RATE",
            1 => "DEPTH",
            2 => "DELAY",
            _ => "MIX",
        },
        FxType::MultibandDist => "--",
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

/// Parameter projection trait for the shared `Utility` FX Unit view.
/// Implemented as a tiny `Copy` marker/coordinate struct by both standalone `j-utility`
/// and `J-SYN`'s `(bus_idx, slot_idx)` rack slots.
pub trait UtilityUnitParams<Params: 'static>: Copy + Send + Sync + 'static {
    fn enabled<'a>(&self, p: &'a Params) -> &'a BoolParam;
    fn width<'a>(&self, p: &'a Params) -> &'a FloatParam;
    fn pan<'a>(&self, p: &'a Params) -> &'a FloatParam;
    fn gain<'a>(&self, p: &'a Params) -> &'a FloatParam;
    fn mix<'a>(&self, p: &'a Params) -> &'a FloatParam;
    fn inv_l<'a>(&self, p: &'a Params) -> &'a BoolParam;
    fn inv_r<'a>(&self, p: &'a Params) -> &'a BoolParam;
}

pub(crate) fn build_overlay_bool_pill<L, Params, FPick>(
    cx: &mut Context,
    params_lens: L,
    params: Arc<Params>,
    gui_context: Arc<dyn GuiContext>,
    label: &'static str,
    pick: FPick,
) where
    L: Lens<Target = Arc<Params>> + Clone,
    Params: Send + Sync + 'static,
    FPick: Fn(&Arc<Params>) -> &BoolParam + Copy + Send + Sync + 'static,
{
    Button::new(
        cx,
        move |cx| {
            let setter = ParamSetter::new(gui_context.as_ref());
            let p = pick(&params);
            let next = !p.value();
            setter.begin_set_parameter(p);
            setter.set_parameter(p, next);
            setter.end_set_parameter(p);
            cx.emit(UiKitEvent::ModUiChanged);
        },
        move |cx| bold_label(cx, label, "fx-subtab-lbl").hoverable(false),
    )
    .class("fx-scope-pill")
    .class("fx-scope-flt-btn")
    .toggle_class(
        "fx-subtab-active",
        params_lens.map(move |p| pick(p).value()),
    )
    .width(Pixels(tokens::FX_UNIT_OVERLAY_PILL_W))
    .height(Pixels(tokens::FX_UNIT_OVERLAY_PILL_H));
}

/// Shared `Utility` FX Unit body (`ZStack` stereo cone scope + `Ø L`/`Ø R` corner pills + 4-knob footer),
/// used identically by both `J-SYN`'s 3-lane FX rack and the standalone `j-utility` plugin.
#[allow(clippy::too_many_arguments)]
pub fn build_utility_fx_unit_body<L, Params, U, FScope>(
    cx: &mut Context,
    params_lens: L,
    params: Arc<Params>,
    gui_context: Arc<dyn GuiContext>,
    unit_params: U,
    mod_targets: Option<([ModDestination; 4], Arc<dyn ModUiHost>, u8)>,
    scope_provider: FScope,
    knob_sz: f32,
) where
    L: Lens<Target = Arc<Params>> + Clone,
    Params: Send + Sync + 'static,
    U: UtilityUnitParams<Params>,
    FScope: Fn() -> FxSlotScopeSnapshot + Send + Sync + 'static,
{
    let scope_h = tokens::fx_unit_scope_height(FxType::Utility);
    let defs = j_syn_dsp::FxSlotParams::default_params_for_type(FxType::Utility);

    let lens_scope = params_lens.clone();
    let p_scope = params.clone();
    let g_scope = gui_context;

    VStack::new(cx, move |cx| {
        // Scope ZStack with translucent corner Ø L / Ø R buttons
        ZStack::new(cx, move |cx| {
            FxSlotVisualizer::new(cx, scope_provider)
                .hoverable(false)
                .width(Stretch(1.0))
                .height(Stretch(1.0));

            HStack::new(cx, move |cx| {
                build_overlay_bool_pill(
                    cx,
                    lens_scope.clone(),
                    p_scope.clone(),
                    g_scope.clone(),
                    "Ø L",
                    move |p: &Arc<Params>| unit_params.inv_l(p),
                );
                Element::new(cx).width(Stretch(1.0)).hoverable(false);
                build_overlay_bool_pill(cx, lens_scope, p_scope, g_scope, "Ø R", move |p: &Arc<Params>| {
                    unit_params.inv_r(p)
                });
            })
            .col_between(Pixels(tokens::SPACE_SM))
            .position_type(PositionType::SelfDirected)
            .width(Stretch(1.0))
            .height(Pixels(tokens::FX_UNIT_OVERLAY_PILL_H))
            .top(Pixels(tokens::SPACE_SM))
            .left(Pixels(tokens::SPACE_SM))
            .right(Pixels(tokens::SPACE_SM));
        })
        .width(Stretch(1.0))
        .height(Pixels(scope_h));

        // 2×2 knob grid: WIDTH, PAN, GAIN, MIX
        simple_units::build_2x2_knob_grid(
            cx,
            FxType::Utility,
            false,
            [false, true, true, false],
            defs,
            knob_sz,
            params_lens,
            params,
            move |p| unit_params.enabled(p),
            move |p, idx| match idx {
                0 => unit_params.width(p),
                1 => unit_params.pan(p),
                2 => unit_params.gain(p),
                _ => unit_params.mix(p),
            },
            mod_targets,
        );
    })
    .row_between(Pixels(tokens::SPACE_SM))
    .width(Stretch(1.0))
    .height(Auto);
}

#[inline]
pub fn mb_band_default_norm(param_idx: usize) -> f32 {
    match param_idx {
        0 => 0.30,
        1 => 0.50,
        2 => MB_BAND_LEVEL_DEFAULT_NORM,
        _ => 1.00,
    }
}
