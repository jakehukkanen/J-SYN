//! Standalone `J-Dist` Vizia editor built directly on `j-syn-ui` shared FX Unit views.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::{FxScopeRing, FX_SCOPE_LEN};
use j_syn_dsp::FxType;
use j_syn_ui::fx_units::{
    build_distortion_fx_unit_body, build_standalone_fx_shell, create_standalone_fx_editor,
    standalone_default_state, DistortionModeParam, DistortionUnitParams, StandaloneFxHeaderSpec,
    StandaloneFxUiData,
};
use j_syn_ui::scopes::FxSlotScopeSnapshot;
use j_syn_ui::theme::ThemePalette;
use j_syn_ui::tokens;
use nih_plug::prelude::{BoolParam, Editor, EnumParam, FloatParam, GuiContext};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::ViziaState;

use crate::JDistParams;

const HEADER_SPEC: StandaloneFxHeaderSpec = StandaloneFxHeaderSpec {
    header_title: "J-DIST",
    card_title: "DISTORTION",
    card_subtitle: "4X OS",
    fx_type: FxType::Distortion,
};

pub fn dist_window_size() -> (u32, u32) {
    tokens::standalone_fx_window_size(FxType::Distortion)
}

pub fn default_state() -> Arc<ViziaState> {
    standalone_default_state(FxType::Distortion)
}

#[derive(Clone, Copy)]
struct StandaloneDistParams;

impl DistortionUnitParams<JDistParams> for StandaloneDistParams {
    fn enabled<'a>(&self, p: &'a JDistParams) -> &'a BoolParam {
        &p.enabled
    }
    fn mode<'a>(&self, p: &'a JDistParams) -> &'a EnumParam<DistortionModeParam> {
        &p.mode
    }
    fn param<'a>(&self, p: &'a JDistParams, idx: usize) -> &'a FloatParam {
        match idx {
            0 => &p.bias,
            1 => &p.drive,
            2 => &p.tone,
            _ => &p.mix,
        }
    }
}

pub fn build_dist_ui(
    cx: &mut Context,
    params: Arc<JDistParams>,
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
        initial_theme,
        initial_scale_pct,
        persist_settings,
        move |cx| {
            let p_scope = p_body.clone();
            let ring = scope_ring.clone();
            build_distortion_fx_unit_body(
                cx,
                StandaloneFxUiData::<JDistParams>::params,
                p_body.clone(),
                gui_context.clone(),
                StandaloneDistParams,
                None,
                move || {
                    let mut pre = [0.0_f32; FX_SCOPE_LEN];
                    let mut post = [0.0_f32; FX_SCOPE_LEN];
                    let act = ring.read_snapshot(&mut pre, &mut post);
                    FxSlotScopeSnapshot {
                        enabled: p_scope.enabled.value(),
                        fx_type: FxType::Distortion,
                        eff_p: [
                            p_scope.bias.value(),
                            p_scope.drive.value(),
                            p_scope.tone.value(),
                            p_scope.mix.value(),
                        ],
                        distortion_mode: p_scope.mode.value().into(),
                        pre,
                        post,
                        activity: act,
                        ..FxSlotScopeSnapshot::default()
                    }
                },
                tokens::KNOB_FX_UNIT_2X2,
            );
        },
    );
}

pub fn create(
    params: Arc<JDistParams>,
    editor_state: Arc<ViziaState>,
    scope_ring: Arc<FxScopeRing>,
) -> Option<Box<dyn Editor>> {
    create_standalone_fx_editor(editor_state, move |cx, gui_context, pal, scale_pct| {
        build_dist_ui(
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
