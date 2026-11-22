//! Standalone `J-Dispersion` Vizia editor built directly on `j-syn-ui` shared FX Unit views.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::{FxScopeRing, FX_SCOPE_LEN};
use j_syn_dsp::FxType;
use j_syn_ui::fx_units::{
    build_diopser_fx_unit_body, build_standalone_fx_shell, create_standalone_fx_editor,
    standalone_default_state, DiopserStagesParam, DiopserUnitParams, StandaloneFxHeaderSpec,
    StandaloneFxUiData,
};
use j_syn_ui::scopes::FxSlotScopeSnapshot;
use j_syn_ui::theme::ThemePalette;
use j_syn_ui::tokens;
use nih_plug::prelude::{BoolParam, Editor, EnumParam, FloatParam, GuiContext};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::ViziaState;

use crate::JDispersionParams;

const HEADER_SPEC: StandaloneFxHeaderSpec = StandaloneFxHeaderSpec {
    header_title: "J-DISPERSION",
    card_title: "DISPERSION",
    card_subtitle: "ALLPASS",
    fx_type: FxType::Diopser,
};

pub fn dispersion_window_size() -> (u32, u32) {
    tokens::standalone_fx_window_size(FxType::Diopser)
}

pub fn default_state() -> Arc<ViziaState> {
    standalone_default_state(FxType::Diopser)
}

#[derive(Clone, Copy)]
struct StandaloneDispersionParams;

impl DiopserUnitParams<JDispersionParams> for StandaloneDispersionParams {
    fn enabled<'a>(&self, p: &'a JDispersionParams) -> &'a BoolParam {
        &p.enabled
    }
    fn stages<'a>(&self, p: &'a JDispersionParams) -> &'a EnumParam<DiopserStagesParam> {
        &p.stages
    }
    fn param<'a>(&self, p: &'a JDispersionParams, idx: usize) -> &'a FloatParam {
        match idx {
            0 => &p.freq,
            1 => &p.res,
            2 => &p.spread,
            _ => &p.mix,
        }
    }
}

pub fn build_dispersion_ui(
    cx: &mut Context,
    params: Arc<JDispersionParams>,
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
            build_diopser_fx_unit_body(
                cx,
                StandaloneFxUiData::<JDispersionParams>::params,
                p_body.clone(),
                gui_context.clone(),
                StandaloneDispersionParams,
                None,
                move || {
                    let mut pre = [0.0_f32; FX_SCOPE_LEN];
                    let mut post = [0.0_f32; FX_SCOPE_LEN];
                    let act = ring.read_snapshot(&mut pre, &mut post);
                    FxSlotScopeSnapshot {
                        enabled: p_scope.enabled.value(),
                        fx_type: FxType::Diopser,
                        eff_p: [
                            p_scope.freq.value(),
                            p_scope.res.value(),
                            p_scope.spread.value(),
                            p_scope.mix.value(),
                        ],
                        diopser_stages_idx: p_scope.stages.value().to_index() as usize,
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
    params: Arc<JDispersionParams>,
    editor_state: Arc<ViziaState>,
    scope_ring: Arc<FxScopeRing>,
) -> Option<Box<dyn Editor>> {
    create_standalone_fx_editor(editor_state, move |cx, gui_context, pal, scale_pct| {
        build_dispersion_ui(
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
