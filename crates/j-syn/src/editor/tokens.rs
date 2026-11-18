//! Central visual and layout design tokens for `J-SYN`'s `1760 × 840` 3-Wing UI.
#![allow(dead_code)]

use j_syn_dsp::fx::FxType;

pub(crate) const WINDOW_W: u32 = 1760;
pub(crate) const WINDOW_H: u32 = 840;
pub(crate) const HEADER_H: f32 = 46.0;

pub(crate) const LEFT_WING_W: f32 = 352.0;
pub(crate) const RIGHT_WING_W: f32 = 834.0;

pub(crate) const SPACE_XS: f32 = 2.0;
pub(crate) const SPACE_SM: f32 = 4.0;
pub(crate) const SPACE_MD: f32 = 6.0;
pub(crate) const SPACE_LG: f32 = 8.0;
pub(crate) const SPACE_XL: f32 = 12.0;

pub(crate) const CARD_PAD: f32 = 6.0;
pub(crate) const HEADER_STRIP_H: f32 = 20.0;
pub(crate) const PILL_H: f32 = 18.0;
pub(crate) const CONTROL_H: f32 = 20.0;
pub(crate) const STEP_BTN_W: f32 = 16.0;
pub(crate) const READOUT_H: f32 = 17.0;
pub(crate) const RADIUS_CARD: f32 = 4.0;
pub(crate) const RADIUS_PILL: f32 = 3.0;
pub(crate) const BORDER_HAIRLINE: f32 = 1.0;
pub(crate) const STROKE_TRACE: f32 = 1.5;

pub(crate) const KNOB_OSC: f32 = 30.0;
pub(crate) const KNOB_FLT: f32 = 30.0;
pub(crate) const KNOB_STD: f32 = 28.0;
pub(crate) const KNOB_FX_UNIT: f32 = 27.0;
pub(crate) const KNOB_COMPACT: f32 = 24.0;
pub(crate) const KNOB_MINI: f32 = 18.0;

// Left Wing: Oscillators & Noise
pub(crate) const OSC_LED_W: f32 = 52.0;
pub(crate) const OSC_SCOPE_W: f32 = 92.0;
pub(crate) const NOISE_LED_W: f32 = 58.0;
pub(crate) const NOISE_CARD_H: f32 = 170.0;

// Center Wing: Filters, Inserts, Voice & Macros
pub(crate) const FILTER_SECTION_H: f32 = 148.0;
pub(crate) const FILTER_LED_W: f32 = 62.0;
pub(crate) const FILTER_SLOPE_W: f32 = 60.0;
pub(crate) const FILTER_SCOPE_H: f32 = 52.0;
pub(crate) const INSERT_SECTION_H: f32 = 84.0;
pub(crate) const INSERT_LED_W: f32 = 48.0;
pub(crate) const INSERT_POS_W: f32 = 56.0;
pub(crate) const INSERT_KNOB_CLUSTER_W: f32 = 86.0;
pub(crate) const VOICE_VOICING_H: f32 = 118.0;
pub(crate) const VOICE_KT_W: f32 = 52.0;
pub(crate) const VOICE_COL_MODE_W: f32 = 72.0;
pub(crate) const VOICE_COL_MPE_W: f32 = 68.0;
pub(crate) const VOICE_FLT_KT_CELL_W: f32 = 58.0;
pub(crate) const MACROS_DOCK_H: f32 = 134.0;
pub(crate) const PERF_PILL_COL_W: f32 = 44.0;
pub(crate) const MACRO_PILL_W: f32 = 28.0;
pub(crate) const MACRO_HDR_H: f32 = 15.0;

// Right Wing: 3-Lane Scrollable FX Rack & Variable-Height FX Units
pub(crate) const FX_LANE_HEADER_H: f32 = 54.0;
pub(crate) const FX_LANE_ROUTE_COL_W: f32 = 76.0;
pub(crate) const FX_LANE_KNOBS_W: f32 = 92.0;
pub(crate) const FX_LANE_SCROLL_GUTTER: f32 = 10.0;
pub(crate) const FX_UNIT_HEADER_H: f32 = 18.0;
pub(crate) const FX_UNIT_LED_W: f32 = 34.0;
pub(crate) const FX_UNIT_STACK_W: f32 = 38.0;
pub(crate) const FX_UNIT_ARROW_W: f32 = 13.0;
pub(crate) const FX_UNIT_ARROW_H: f32 = 16.0;
pub(crate) const FX_UNIT_SCOPE_COMPACT_H: f32 = 44.0;
pub(crate) const FX_UNIT_SCOPE_H: f32 = 58.0;
pub(crate) const FX_UNIT_SCOPE_TALL_H: f32 = 72.0;

/// Variable scope height per effect unit so collapsed/simple/dense FX size proportionally inside scrollable lanes.
#[inline]
pub(crate) const fn fx_unit_scope_height(fx_type: FxType) -> f32 {
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
