//! `j-syn-ui` FX Unit trait adapters bridging `JSynPluginParams` slots to shared FX Unit views.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::FxType;
use j_syn_ui::fx_units::{
    build_clipper_fx_unit_body, build_diopser_fx_unit_body, build_distortion_fx_unit_body,
    build_filter_fx_unit_body, build_four_knob_fx_unit_body, build_limiter_fx_unit_body,
    build_multiband_dist_fx_unit_body, build_tabbed_fx_unit_body, build_tube_res_fx_unit_body,
    build_utility_fx_unit_body, ClipperUnitParams, DiopserUnitParams, DistortionUnitParams,
    EightKnobUnitParams, FilterUnitParams, FourKnobUnitParams, FxSubTabHost, LimiterUnitParams,
    MultibandDistUnitParams, MultibandTabHost, TubeResonatorUnitParams, UtilityUnitParams,
};
use nih_plug::prelude::{BoolParam, EnumParam, FloatParam, GuiContext};
use nih_plug_vizia::vizia::prelude::*;

use super::super::fx_scopes::slot_scope_snapshot;
use super::super::mod_bar::ModUiState;
use super::super::{tokens, UiData};
use crate::{DiopserStagesParam, DistortionModeParam, FilterModeParam, FxFilterSlopeParam, JSynPluginParams};

struct JSynSubTabHost {
    bus_idx: usize,
    slot_idx: usize,
    mod_ui: Arc<ModUiState>,
}

impl FxSubTabHost for JSynSubTabHost {
    fn is_subtab(&self) -> bool {
        self.mod_ui.fx_eq_subtab(self.bus_idx, self.slot_idx)
    }
    fn set_subtab(&self, subtab: bool) {
        self.mod_ui
            .set_fx_eq_subtab(self.bus_idx, self.slot_idx, subtab);
    }
}

pub(super) fn build_fx_unit_body(
    cx: &mut Context,
    fx_type: FxType,
    bus_idx: usize,
    slot_idx: usize,
    p_unit: Arc<JSynPluginParams>,
    g_unit: Arc<dyn GuiContext>,
    m_unit: Arc<ModUiState>,
) {
                if fx_type == FxType::Utility {
                    #[derive(Clone, Copy)]
                    struct JSynUtilitySlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl UtilityUnitParams<JSynPluginParams> for JSynUtilitySlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn width<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p1
                        }
                        fn pan<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p2
                        }
                        fn gain<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p3
                        }
                        fn mix<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p4
                        }
                        fn inv_l<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].utility_inv_l
                        }
                        fn inv_r<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].utility_inv_r
                        }
                    }
                    let mod_dests: [ModDestination; 4] =
                        std::array::from_fn(|k| ModDestination::fx_bus_slot_param(bus_idx, slot_idx, k));
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    build_utility_fx_unit_body(
                        cx,
                        UiData::params,
                        p_unit,
                        g_unit,
                        JSynUtilitySlotParams { bus_idx, slot_idx },
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                    return;
                }
                if fx_type == FxType::JSatClipper {
                    #[derive(Clone, Copy)]
                    struct JSynClipperSlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl ClipperUnitParams<JSynPluginParams> for JSynClipperSlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn pcm<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].clipper_pcm
                        }
                        fn param<'a>(&self, p: &'a JSynPluginParams, idx: usize) -> &'a FloatParam {
                            p.fx_bus_slots(self.bus_idx)[self.slot_idx].param_at(idx)
                        }
                    }
                    let mod_dests: [ModDestination; 4] =
                        std::array::from_fn(|k| ModDestination::fx_bus_slot_param(bus_idx, slot_idx, k));
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    build_clipper_fx_unit_body(
                        cx,
                        UiData::params,
                        p_unit,
                        g_unit,
                        JSynClipperSlotParams { bus_idx, slot_idx },
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                    return;
                }
                if matches!(fx_type, FxType::Ott | FxType::Phaser) {
                    #[derive(Clone, Copy)]
                    struct JSynFourKnobSlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl FourKnobUnitParams<JSynPluginParams> for JSynFourKnobSlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn param<'a>(&self, p: &'a JSynPluginParams, idx: usize) -> &'a FloatParam {
                            p.fx_bus_slots(self.bus_idx)[self.slot_idx].param_at(idx)
                        }
                    }
                    let mod_dests: [ModDestination; 4] =
                        std::array::from_fn(|k| ModDestination::fx_bus_slot_param(bus_idx, slot_idx, k));
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    let bipolar = if fx_type == FxType::Ott {
                        [false, false, true, true]
                    } else {
                        [false; 4]
                    };
                    build_four_knob_fx_unit_body(
                        cx,
                        fx_type,
                        bipolar,
                        UiData::params,
                        p_unit,
                        JSynFourKnobSlotParams { bus_idx, slot_idx },
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                    return;
                }
                if fx_type == FxType::Distortion {
                    #[derive(Clone, Copy)]
                    struct JSynDistortionSlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl DistortionUnitParams<JSynPluginParams> for JSynDistortionSlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn mode<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                        ) -> &'a EnumParam<DistortionModeParam> {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].distortion_mode
                        }
                        fn param<'a>(&self, p: &'a JSynPluginParams, idx: usize) -> &'a FloatParam {
                            p.fx_bus_slots(self.bus_idx)[self.slot_idx].param_at(idx)
                        }
                    }
                    let mod_dests: [ModDestination; 4] =
                        std::array::from_fn(|k| ModDestination::fx_bus_slot_param(bus_idx, slot_idx, k));
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    build_distortion_fx_unit_body(
                        cx,
                        UiData::params,
                        p_unit,
                        g_unit,
                        JSynDistortionSlotParams { bus_idx, slot_idx },
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                    return;
                }
                if fx_type == FxType::Diopser {
                    #[derive(Clone, Copy)]
                    struct JSynDiopserSlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl DiopserUnitParams<JSynPluginParams> for JSynDiopserSlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn stages<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                        ) -> &'a EnumParam<DiopserStagesParam> {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].diopser_stages
                        }
                        fn param<'a>(&self, p: &'a JSynPluginParams, idx: usize) -> &'a FloatParam {
                            p.fx_bus_slots(self.bus_idx)[self.slot_idx].param_at(idx)
                        }
                    }
                    let mod_dests: [ModDestination; 4] =
                        std::array::from_fn(|k| ModDestination::fx_bus_slot_param(bus_idx, slot_idx, k));
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    build_diopser_fx_unit_body(
                        cx,
                        UiData::params,
                        p_unit,
                        g_unit,
                        JSynDiopserSlotParams { bus_idx, slot_idx },
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                    return;
                }
                if matches!(
                    fx_type,
                    FxType::Chorus | FxType::Delay | FxType::Reverb | FxType::Flanger
                ) {
                    #[derive(Clone, Copy)]
                    struct JSynEightKnobSlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl EightKnobUnitParams<JSynPluginParams> for JSynEightKnobSlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn param<'a>(&self, p: &'a JSynPluginParams, idx: usize) -> &'a FloatParam {
                            p.fx_bus_slots(self.bus_idx)[self.slot_idx].param_at(idx)
                        }
                    }
                    let mod_dests: [ModDestination; 8] =
                        std::array::from_fn(|k| ModDestination::fx_bus_slot_param(bus_idx, slot_idx, k));
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    let tab_host = Arc::new(JSynSubTabHost {
                        bus_idx,
                        slot_idx,
                        mod_ui: m_unit.clone(),
                    });
                    let (sub_lbl, sub_bipolar) = if fx_type == FxType::Flanger {
                        ("MOD", [true, false, false, false])
                    } else {
                        ("EQ", [false; 4])
                    };
                    build_tabbed_fx_unit_body(
                        cx,
                        fx_type,
                        sub_lbl,
                        sub_bipolar,
                        UiData::params,
                        UiData::mod_ui_rev,
                        p_unit,
                        JSynEightKnobSlotParams { bus_idx, slot_idx },
                        tab_host,
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                    return;
                }
                if fx_type == FxType::Limiter {
                    #[derive(Clone, Copy)]
                    struct JSynLimiterSlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl LimiterUnitParams<JSynPluginParams> for JSynLimiterSlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn true_peak<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].limiter_true_peak
                        }
                        fn param<'a>(&self, p: &'a JSynPluginParams, idx: usize) -> &'a FloatParam {
                            p.fx_bus_slots(self.bus_idx)[self.slot_idx].param_at(idx)
                        }
                    }
                    let mod_dests: [ModDestination; 8] =
                        std::array::from_fn(|k| ModDestination::fx_bus_slot_param(bus_idx, slot_idx, k));
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    let tab_host = Arc::new(JSynSubTabHost {
                        bus_idx,
                        slot_idx,
                        mod_ui: m_unit.clone(),
                    });
                    build_limiter_fx_unit_body(
                        cx,
                        UiData::params,
                        UiData::mod_ui_rev,
                        p_unit,
                        g_unit,
                        JSynLimiterSlotParams { bus_idx, slot_idx },
                        tab_host,
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                    return;
                }
                if fx_type == FxType::Filter {
                    #[derive(Clone, Copy)]
                    struct JSynFilterSlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl FilterUnitParams<JSynPluginParams> for JSynFilterSlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn mode<'a>(&self, p: &'a JSynPluginParams) -> &'a EnumParam<FilterModeParam> {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].filter_mode
                        }
                        fn slope<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                        ) -> &'a EnumParam<FxFilterSlopeParam> {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].filter_slope
                        }
                        fn param<'a>(&self, p: &'a JSynPluginParams, idx: usize) -> &'a FloatParam {
                            p.fx_bus_slots(self.bus_idx)[self.slot_idx].param_at(idx)
                        }
                        fn freq2<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p5
                        }
                    }
                    let mod_dests: [ModDestination; 5] =
                        std::array::from_fn(|k| ModDestination::fx_bus_slot_param(bus_idx, slot_idx, k));
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    build_filter_fx_unit_body(
                        cx,
                        UiData::params,
                        p_unit,
                        g_unit,
                        JSynFilterSlotParams { bus_idx, slot_idx },
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                    return;
                }
                if fx_type == FxType::TubeResonator {
                    #[derive(Clone, Copy)]
                    struct JSynTubeResSlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl TubeResonatorUnitParams<JSynPluginParams> for JSynTubeResSlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn filter_enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].tube_res_filter_enabled
                        }
                        fn dirty_lfo<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].tube_res_dirty_lfo
                        }
                        fn param<'a>(&self, p: &'a JSynPluginParams, idx: usize) -> &'a FloatParam {
                            p.fx_bus_slots(self.bus_idx)[self.slot_idx].param_at(idx)
                        }
                    }
                    let mod_dests: [ModDestination; 8] =
                        std::array::from_fn(|k| ModDestination::fx_bus_slot_param(bus_idx, slot_idx, k));
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    let tab_host = Arc::new(JSynSubTabHost {
                        bus_idx,
                        slot_idx,
                        mod_ui: m_unit.clone(),
                    });
                    build_tube_res_fx_unit_body(
                        cx,
                        UiData::params,
                        UiData::mod_ui_rev,
                        p_unit,
                        g_unit,
                        JSynTubeResSlotParams { bus_idx, slot_idx },
                        tab_host,
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                    return;
                }
                if fx_type == FxType::MultibandDist {
                    #[derive(Clone, Copy)]
                    struct JSynMbDistSlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl MultibandDistUnitParams<JSynPluginParams> for JSynMbDistSlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn low_xover<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p1
                        }
                        fn high_xover<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p2
                        }
                        fn out_gain<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p3
                        }
                        fn mix<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p4
                        }
                        fn band_enabled<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].enabled
                        }
                        fn band_solo<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].solo
                        }
                        fn band_mode<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a EnumParam<DistortionModeParam> {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].mode
                        }
                        fn band_drive<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].drive
                        }
                        fn band_bias<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].bias
                        }
                        fn band_level<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].level
                        }
                        fn band_mix<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].mix
                        }
                    }
                    struct JSynMbTabHost {
                        bus_idx: usize,
                        slot_idx: usize,
                        mod_ui: Arc<ModUiState>,
                    }
                    impl MultibandTabHost for JSynMbTabHost {
                        fn mb_tab(&self) -> u8 {
                            self.mod_ui.fx_mb_tab(self.bus_idx, self.slot_idx)
                        }
                        fn set_mb_tab(&self, tab: u8) {
                            self.mod_ui.set_fx_mb_tab(self.bus_idx, self.slot_idx, tab);
                        }
                    }
                    let mod_dests: [ModDestination; 16] =
                        std::array::from_fn(|k| ModDestination::fx_bus_slot_param(bus_idx, slot_idx, k));
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    let tab_host = Arc::new(JSynMbTabHost {
                        bus_idx,
                        slot_idx,
                        mod_ui: m_unit.clone(),
                    });
                    build_multiband_dist_fx_unit_body(
                        cx,
                        UiData::params,
                        UiData::mod_ui_rev,
                        p_unit,
                        g_unit,
                        JSynMbDistSlotParams { bus_idx, slot_idx },
                        tab_host,
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                }
}
