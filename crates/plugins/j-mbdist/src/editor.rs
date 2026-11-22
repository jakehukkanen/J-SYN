//! Standalone `J-MBDist` Vizia editor built directly on `j-syn-ui` primitives and shared `MB Dist` FX Unit view.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use j_syn_dsp::mod_matrix::{FxScopeRing, FX_SCOPE_LEN};
use j_syn_dsp::FxType;
use j_syn_ui::fx_units::{
    build_multiband_dist_fx_unit_body, build_standalone_fx_shell, create_standalone_fx_editor,
    standalone_default_state, DistortionModeParam, MultibandDistUnitParams, MultibandTabHost,
    StandaloneFxHeaderSpec, StandaloneFxUiData,
};
use j_syn_ui::scopes::{FxSlotScopeSnapshot, MbBandScopeSnapshot};
use j_syn_ui::theme::ThemePalette;
use j_syn_ui::tokens;
use nih_plug::prelude::{BoolParam, Editor, EnumParam, FloatParam, GuiContext};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::ViziaState;

use crate::JMbDistParams;

const HEADER_SPEC: StandaloneFxHeaderSpec = StandaloneFxHeaderSpec {
    header_title: "J-MBDIST",
    card_title: "MB DIST",
    fx_type: FxType::MultibandDist,
};

pub fn mbdist_window_size() -> (u32, u32) {
    tokens::standalone_fx_window_size(FxType::MultibandDist)
}

pub fn default_state() -> Arc<ViziaState> {
    standalone_default_state(FxType::MultibandDist)
}

struct StandaloneMbTabHost {
    tab: AtomicU8,
}

impl StandaloneMbTabHost {
    fn new(initial_tab: u8) -> Self {
        Self {
            tab: AtomicU8::new(initial_tab.min(3)),
        }
    }
}

impl MultibandTabHost for StandaloneMbTabHost {
    fn mb_tab(&self) -> u8 {
        self.tab.load(Ordering::Relaxed)
    }

    fn set_mb_tab(&self, tab: u8) {
        self.tab.store(tab.min(3), Ordering::Relaxed);
    }
}

#[derive(Clone, Copy)]
struct StandaloneMbDistParams;

impl MultibandDistUnitParams<JMbDistParams> for StandaloneMbDistParams {
    fn enabled<'a>(&self, p: &'a JMbDistParams) -> &'a BoolParam {
        &p.enabled
    }
    fn low_xover<'a>(&self, p: &'a JMbDistParams) -> &'a FloatParam {
        &p.low_xover
    }
    fn high_xover<'a>(&self, p: &'a JMbDistParams) -> &'a FloatParam {
        &p.high_xover
    }
    fn out_gain<'a>(&self, p: &'a JMbDistParams) -> &'a FloatParam {
        &p.out_gain
    }
    fn mix<'a>(&self, p: &'a JMbDistParams) -> &'a FloatParam {
        &p.mix
    }
    fn band_enabled<'a>(&self, p: &'a JMbDistParams, band: usize) -> &'a BoolParam {
        &p.band(band).enabled
    }
    fn band_solo<'a>(&self, p: &'a JMbDistParams, band: usize) -> &'a BoolParam {
        &p.band(band).solo
    }
    fn band_mode<'a>(
        &self,
        p: &'a JMbDistParams,
        band: usize,
    ) -> &'a EnumParam<DistortionModeParam> {
        &p.band(band).mode
    }
    fn band_drive<'a>(&self, p: &'a JMbDistParams, band: usize) -> &'a FloatParam {
        &p.band(band).drive
    }
    fn band_bias<'a>(&self, p: &'a JMbDistParams, band: usize) -> &'a FloatParam {
        &p.band(band).bias
    }
    fn band_level<'a>(&self, p: &'a JMbDistParams, band: usize) -> &'a FloatParam {
        &p.band(band).level
    }
    fn band_mix<'a>(&self, p: &'a JMbDistParams, band: usize) -> &'a FloatParam {
        &p.band(band).mix
    }
}

/// Builds the full `J-MBDist` editor view tree on `cx` (shared between live DAW window and PNG preview harness).
#[allow(clippy::too_many_arguments)]
pub fn build_mbdist_ui(
    cx: &mut Context,
    params: Arc<JMbDistParams>,
    gui_context: Arc<dyn GuiContext>,
    scope_ring: Arc<FxScopeRing>,
    initial_theme: ThemePalette,
    initial_scale_pct: u32,
    initial_tab: u8,
    persist_settings: bool,
) {
    let p_body = params.clone();
    let tab_host = Arc::new(StandaloneMbTabHost::new(initial_tab));
    build_standalone_fx_shell(
        cx,
        HEADER_SPEC,
        params,
        |p| &p.enabled,
        Some(|p| &p.stack),
        initial_theme,
        initial_scale_pct,
        persist_settings,
        move |cx| {
            let p_scope = p_body.clone();
            let ring = scope_ring.clone();
            let tab_scope = tab_host.clone();
            build_multiband_dist_fx_unit_body(
                cx,
                StandaloneFxUiData::<JMbDistParams>::params,
                StandaloneFxUiData::<JMbDistParams>::ui_rev,
                p_body.clone(),
                gui_context.clone(),
                StandaloneMbDistParams,
                tab_host.clone(),
                None,
                move || {
                    let mut pre = [0.0_f32; FX_SCOPE_LEN];
                    let mut post = [0.0_f32; FX_SCOPE_LEN];
                    let act = ring.read_snapshot(&mut pre, &mut post);
                    FxSlotScopeSnapshot {
                        enabled: p_scope.enabled.value(),
                        fx_type: FxType::MultibandDist,
                        eff_p: [
                            p_scope.low_xover.value(),
                            p_scope.high_xover.value(),
                            p_scope.out_gain.value(),
                            p_scope.mix.value(),
                        ],
                        mb_xover_p1: p_scope.low_xover.value(),
                        mb_xover_p2: p_scope.high_xover.value(),
                        mb_tab: tab_scope.mb_tab(),
                        mb_bands: std::array::from_fn(|b| {
                            let mb = p_scope.band(b);
                            MbBandScopeSnapshot {
                                enabled: mb.enabled.value(),
                                solo: mb.solo.value(),
                                drive: mb.drive.value().clamp(0.0, 1.0),
                                level: mb.level.value(),
                            }
                        }),
                        pre,
                        post,
                        activity: act,
                        stack: p_scope.stack.value().clamp(1, 4) as u8,
                        ..FxSlotScopeSnapshot::default()
                    }
                },
                tokens::KNOB_FX_UNIT_2X2,
            );
        },
    );
}

pub fn create(
    params: Arc<JMbDistParams>,
    editor_state: Arc<ViziaState>,
    scope_ring: Arc<FxScopeRing>,
) -> Option<Box<dyn Editor>> {
    create_standalone_fx_editor(editor_state, move |cx, gui_context, pal, scale_pct| {
        build_mbdist_ui(
            cx,
            params.clone(),
            gui_context,
            scope_ring.clone(),
            pal,
            scale_pct,
            0,
            true,
        );
    })
}
