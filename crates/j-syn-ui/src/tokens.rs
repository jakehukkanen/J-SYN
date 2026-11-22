//! Central visual and layout design tokens for `J-SYN` and standalone FX plugins.
#![allow(dead_code)]

use j_syn_dsp::fx::FxType;

pub const WINDOW_W: u32 = 1760;
pub const WINDOW_H: u32 = 840;
pub const HEADER_H: f32 = 46.0;

pub const LEFT_WING_W: f32 = 352.0;
pub const RIGHT_WING_W: f32 = 834.0;

pub const SPACE_XS: f32 = 2.0;
pub const SPACE_SM: f32 = 4.0;
pub const SPACE_MD: f32 = 6.0;
pub const SPACE_LG: f32 = 8.0;
pub const SPACE_XL: f32 = 12.0;

pub const CARD_PAD: f32 = 6.0;
pub const HEADER_STRIP_H: f32 = 20.0;
pub const PILL_H: f32 = 18.0;
pub const CONTROL_H: f32 = 20.0;
pub const STEP_BTN_W: f32 = 16.0;
pub const READOUT_H: f32 = 17.0;
pub const RADIUS_CARD: f32 = 4.0;
pub const RADIUS_PILL: f32 = 3.0;
pub const BORDER_HAIRLINE: f32 = 1.0;
pub const STROKE_TRACE: f32 = 1.5;

pub const KNOB_OSC: f32 = 30.0;
pub const KNOB_FLT: f32 = 30.0;
pub const KNOB_STD: f32 = 28.0;
pub const KNOB_FX_UNIT: f32 = 27.0;
pub const KNOB_COMPACT: f32 = 24.0;
pub const KNOB_MINI: f32 = 18.0;
pub const KNOB_HEADER: f32 = 36.0;

pub const KNOB_TITLE_H: f32 = 14.0;
pub const KNOB_TITLE_COMPACT_H: f32 = 12.0;
pub const KNOB_READOUT_H: f32 = 14.0;
pub const TOOLTIP_H: f32 = 20.0;
pub const TOOLTIP_WRAP_W: f32 = 220.0;

// Left Wing: Oscillators & Noise
pub const OSC_LED_W: f32 = 52.0;
pub const OSC_SCOPE_W: f32 = 92.0;
pub const NOISE_LED_W: f32 = 58.0;
pub const NOISE_CARD_H: f32 = 170.0;

// Center Wing: Filters, Inserts, Voice & Macros
pub const FILTER_SECTION_H: f32 = 148.0;
pub const FILTER_LED_W: f32 = 62.0;
pub const FILTER_SLOPE_W: f32 = 60.0;
pub const FILTER_SCOPE_H: f32 = 52.0;
pub const INSERT_SECTION_H: f32 = 84.0;
pub const INSERT_LED_W: f32 = 48.0;
pub const INSERT_POS_W: f32 = 56.0;
pub const INSERT_KNOB_CLUSTER_W: f32 = 86.0;
pub const VOICE_VOICING_H: f32 = 118.0;
pub const VOICE_KT_W: f32 = 52.0;
pub const VOICE_COL_MODE_W: f32 = 72.0;
pub const VOICE_COL_MPE_W: f32 = 68.0;
pub const VOICE_FLT_KT_CELL_W: f32 = 58.0;
pub const MACROS_DOCK_H: f32 = 134.0;
pub const PERF_PILL_COL_W: f32 = 44.0;
pub const MACRO_PILL_W: f32 = 28.0;
pub const MACRO_HDR_H: f32 = 15.0;

// Right Wing: 3-Lane Scrollable FX Rack & Variable-Height FX Units
pub const FX_UNIT_W: f32 = 220.0;
pub const FX_LANE_HEADER_H: f32 = 54.0;
pub const FX_LANE_ROUTE_COL_W: f32 = 76.0;
pub const FX_LANE_KNOBS_W: f32 = 92.0;
pub const FX_LANE_SCROLL_GUTTER: f32 = 10.0;
pub const FX_UNIT_HEADER_H: f32 = 18.0;
pub const FX_UNIT_LED_W: f32 = 34.0;
pub const FX_UNIT_STACK_W: f32 = 38.0;
pub const FX_UNIT_ARROW_W: f32 = 13.0;
pub const FX_UNIT_ARROW_H: f32 = 16.0;
pub const FX_UNIT_SCOPE_COMPACT_H: f32 = 44.0;
pub const FX_UNIT_SCOPE_H: f32 = 58.0;
pub const FX_UNIT_SCOPE_TALL_H: f32 = 72.0;
pub const FX_UNIT_OVERLAY_PILL_H: f32 = 17.0;
pub const FX_UNIT_OVERLAY_PILL_W: f32 = 30.0;
pub const FX_UNIT_KNOB_ROW_H: f32 = 40.0;

// Standalone single-FX plugin shell metrics (built around FX_UNIT_W)
pub const STANDALONE_FX_PAD: f32 = 12.0;
pub const STANDALONE_FX_HEADER_H: f32 = 34.0;
pub const STANDALONE_FX_CARD_W: f32 = 260.0;

/// Variable scope height per effect unit so collapsed/simple/dense FX size proportionally inside scrollable lanes.
#[inline]
pub const fn fx_unit_scope_height(fx_type: FxType) -> f32 {
    match fx_type {
        FxType::Off => 0.0,
        FxType::Utility | FxType::JSatClipper | FxType::Distortion => FX_UNIT_SCOPE_COMPACT_H,
        FxType::Filter
        | FxType::Limiter
        | FxType::Reverb
        | FxType::TubeResonator
        | FxType::MultibandDist => FX_UNIT_SCOPE_TALL_H,
        FxType::Chorus
        | FxType::Flanger
        | FxType::Phaser
        | FxType::Delay
        | FxType::Ott
        | FxType::Diopser => FX_UNIT_SCOPE_H,
    }
}

/// Computes the deterministic logical window `(width, height)` for a standalone single-FX plugin.
#[inline]
pub const fn standalone_fx_window_size(fx_type: FxType) -> (u32, u32) {
    let scope_h = fx_unit_scope_height(fx_type) as u32;
    let w = (STANDALONE_FX_CARD_W + STANDALONE_FX_PAD * 2.0) as u32;
    let extra_h = match fx_type {
        // Band tab bar (18 + 4) + context/mode strip (18 + 4) + second knob row (40 + 4)
        FxType::MultibandDist => {
            ((PILL_H + SPACE_SM) as u32) * 2 + ((FX_UNIT_KNOB_ROW_H + SPACE_SM) as u32)
        }
        _ => 0,
    };
    // Header bar (34) + chassis top/bottom padding (20) + card padding (12) + card header (18)
    // + 2 gaps (8) + scope + knob row (40) + extra_h + bottom margin
    let h = (STANDALONE_FX_HEADER_H as u32)
        + 20
        + (CARD_PAD as u32 * 2)
        + (FX_UNIT_HEADER_H as u32)
        + (SPACE_SM as u32 * 2)
        + scope_h
        + (FX_UNIT_KNOB_ROW_H as u32)
        + extra_h
        + 4;
    (w, h)
}
