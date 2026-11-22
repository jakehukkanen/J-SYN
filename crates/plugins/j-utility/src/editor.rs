//! Standalone `J-Utility` Vizia editor built directly on `j-syn-ui` primitives and shared `Utility` FX Unit view.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::{FxScopeRing, FX_SCOPE_LEN};
use j_syn_dsp::FxType;
use j_syn_ui::fx_units::{
    build_standalone_fx_shell, build_utility_fx_unit_body, create_standalone_fx_editor,
    standalone_default_state, StandaloneFxHeaderSpec, StandaloneFxUiData, UtilityUnitParams,
};
use j_syn_ui::scopes::FxSlotScopeSnapshot;
use j_syn_ui::theme::ThemePalette;
use j_syn_ui::tokens;
use nih_plug::prelude::{BoolParam, Editor, FloatParam, GuiContext};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::ViziaState;

use crate::JUtilityParams;

const HEADER_SPEC: StandaloneFxHeaderSpec = StandaloneFxHeaderSpec {
    header_title: "J-UTILITY",
    card_title: "UTILITY",
    card_subtitle: "M/S • GAIN • Ø",
    fx_type: FxType::Utility,
};

pub fn utility_window_size() -> (u32, u32) {
    tokens::standalone_fx_window_size(FxType::Utility)
}

pub fn default_state() -> Arc<ViziaState> {
    standalone_default_state(FxType::Utility)
}

#[derive(Clone, Copy)]
struct StandaloneUtilityParams;

impl UtilityUnitParams<JUtilityParams> for StandaloneUtilityParams {
    fn enabled<'a>(&self, p: &'a JUtilityParams) -> &'a BoolParam {
        &p.enabled
    }
    fn width<'a>(&self, p: &'a JUtilityParams) -> &'a FloatParam {
        &p.width
    }
    fn pan<'a>(&self, p: &'a JUtilityParams) -> &'a FloatParam {
        &p.pan
    }
    fn gain<'a>(&self, p: &'a JUtilityParams) -> &'a FloatParam {
        &p.gain
    }
    fn mix<'a>(&self, p: &'a JUtilityParams) -> &'a FloatParam {
        &p.mix
    }
    fn inv_l<'a>(&self, p: &'a JUtilityParams) -> &'a BoolParam {
        &p.inv_l
    }
    fn inv_r<'a>(&self, p: &'a JUtilityParams) -> &'a BoolParam {
        &p.inv_r
    }
}

/// Builds the full `J-Utility` editor view tree on `cx` (shared between live DAW window and PNG preview harness).
pub fn build_utility_ui(
    cx: &mut Context,
    params: Arc<JUtilityParams>,
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
            build_utility_fx_unit_body(
                cx,
                StandaloneFxUiData::<JUtilityParams>::params,
                p_body.clone(),
                gui_context.clone(),
                StandaloneUtilityParams,
                None,
                move || {
                    let mut pre = [0.0_f32; FX_SCOPE_LEN];
                    let mut post = [0.0_f32; FX_SCOPE_LEN];
                    let act = ring.read_snapshot(&mut pre, &mut post);
                    FxSlotScopeSnapshot {
                        enabled: p_scope.enabled.value(),
                        fx_type: FxType::Utility,
                        eff_p: [
                            p_scope.width.value(),
                            p_scope.pan.value(),
                            p_scope.gain.value(),
                            p_scope.mix.value(),
                        ],
                        utility_inv_l: p_scope.inv_l.value(),
                        utility_inv_r: p_scope.inv_r.value(),
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
    params: Arc<JUtilityParams>,
    editor_state: Arc<ViziaState>,
    scope_ring: Arc<FxScopeRing>,
) -> Option<Box<dyn Editor>> {
    create_standalone_fx_editor(editor_state, move |cx, gui_context, pal, scale_pct| {
        build_utility_ui(
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
