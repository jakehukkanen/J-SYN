//! Standalone `J-Filter` Vizia editor built directly on `j-syn-ui` shared FX Unit views.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::{FxScopeRing, FX_SCOPE_LEN};
use j_syn_dsp::FxType;
use j_syn_ui::fx_units::{
    build_filter_fx_unit_body, build_standalone_fx_shell, create_standalone_fx_editor,
    standalone_default_state, FilterModeParam, FilterUnitParams, FxFilterSlopeParam,
    StandaloneFxHeaderSpec, StandaloneFxUiData,
};
use j_syn_ui::scopes::FxSlotScopeSnapshot;
use j_syn_ui::theme::ThemePalette;
use j_syn_ui::tokens;
use nih_plug::prelude::{BoolParam, Editor, EnumParam, FloatParam, GuiContext};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::ViziaState;

use crate::JFilterParams;

const HEADER_SPEC: StandaloneFxHeaderSpec = StandaloneFxHeaderSpec {
    header_title: "J-FILTER",
    card_title: "FILTER",
    fx_type: FxType::Filter,
};

pub fn filter_window_size() -> (u32, u32) {
    tokens::standalone_fx_window_size(FxType::Filter)
}

pub fn default_state() -> Arc<ViziaState> {
    standalone_default_state(FxType::Filter)
}

#[derive(Clone, Copy)]
struct StandaloneFilterParams;

impl FilterUnitParams<JFilterParams> for StandaloneFilterParams {
    fn enabled<'a>(&self, p: &'a JFilterParams) -> &'a BoolParam {
        &p.enabled
    }
    fn mode<'a>(&self, p: &'a JFilterParams) -> &'a EnumParam<FilterModeParam> {
        &p.mode
    }
    fn slope<'a>(&self, p: &'a JFilterParams) -> &'a EnumParam<FxFilterSlopeParam> {
        &p.slope
    }
    fn param<'a>(&self, p: &'a JFilterParams, idx: usize) -> &'a FloatParam {
        match idx {
            0 => &p.cutoff,
            1 => &p.resonance,
            2 => &p.drive,
            _ => &p.mix,
        }
    }
    fn freq2<'a>(&self, p: &'a JFilterParams) -> &'a FloatParam {
        &p.freq2
    }
}

pub fn build_filter_ui(
    cx: &mut Context,
    params: Arc<JFilterParams>,
    gui_context: Arc<dyn GuiContext>,
    scope_ring: Arc<FxScopeRing>,
    initial_theme: ThemePalette,
    initial_scale_pct: u32,
    persist_settings: bool,
) {
    let p_body = params.clone();
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
            build_filter_fx_unit_body(
                cx,
                StandaloneFxUiData::<JFilterParams>::params,
                p_body.clone(),
                gui_context.clone(),
                StandaloneFilterParams,
                None,
                move || {
                    let mut pre = [0.0_f32; FX_SCOPE_LEN];
                    let mut post = [0.0_f32; FX_SCOPE_LEN];
                    let act = ring.read_snapshot(&mut pre, &mut post);
                    FxSlotScopeSnapshot {
                        enabled: p_scope.enabled.value(),
                        fx_type: FxType::Filter,
                        eff_p: [
                            p_scope.cutoff.value(),
                            p_scope.resonance.value(),
                            p_scope.drive.value(),
                            p_scope.mix.value(),
                        ],
                        eff_eq: [p_scope.freq2.value(), 0.0, 0.0, 0.0],
                        filter_mode: p_scope.mode.value().into(),
                        filter_slope: p_scope.slope.value().into(),
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
    params: Arc<JFilterParams>,
    editor_state: Arc<ViziaState>,
    scope_ring: Arc<FxScopeRing>,
) -> Option<Box<dyn Editor>> {
    create_standalone_fx_editor(editor_state, move |cx, gui_context, pal, scale_pct| {
        build_filter_ui(
            cx,
            params.clone(),
            gui_context,
            scope_ring.clone(),
            pal,
            scale_pct,
            true,
        );
    })
}
