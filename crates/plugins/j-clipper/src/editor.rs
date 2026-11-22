//! Standalone `J-Clipper` Vizia editor built directly on `j-syn-ui` shared FX Unit views.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::{FxScopeRing, FX_SCOPE_LEN};
use j_syn_dsp::FxType;
use j_syn_ui::fx_units::{
    build_four_knob_fx_unit_body, build_standalone_fx_shell, standalone_default_state,
    FourKnobUnitParams, StandaloneFxConfig, StandaloneFxHeaderSpec, StandaloneFxUiData,
};
use j_syn_ui::scopes::FxSlotScopeSnapshot;
use j_syn_ui::theme::{self, ThemePalette};
use j_syn_ui::tokens;
use nih_plug::prelude::{BoolParam, Editor, FloatParam, GuiContext};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};

use crate::JClipperParams;

const CONFIG_FILE: &str = "j-clipper.json";

const HEADER_SPEC: StandaloneFxHeaderSpec = StandaloneFxHeaderSpec {
    header_title: "J-CLIPPER",
    card_title: "CLIPPER",
    card_subtitle: "ADAA CLIP",
    fx_type: FxType::JSatClipper,
    config_file: CONFIG_FILE,
};

pub fn clipper_window_size() -> (u32, u32) {
    tokens::standalone_fx_window_size(FxType::JSatClipper)
}

pub fn default_state() -> Arc<ViziaState> {
    standalone_default_state(FxType::JSatClipper, CONFIG_FILE)
}

#[derive(Clone, Copy)]
struct StandaloneClipperParams;

impl FourKnobUnitParams<JClipperParams> for StandaloneClipperParams {
    fn enabled<'a>(&self, p: &'a JClipperParams) -> &'a BoolParam {
        &p.enabled
    }
    fn param<'a>(&self, p: &'a JClipperParams, idx: usize) -> &'a FloatParam {
        match idx {
            0 => &p.drive,
            1 => &p.ceil,
            2 => &p.knee,
            _ => &p.mix,
        }
    }
}

pub fn build_clipper_ui(
    cx: &mut Context,
    params: Arc<JClipperParams>,
    _gui_context: Arc<dyn GuiContext>,
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
            let ring = scope_ring;
            build_four_knob_fx_unit_body(
                cx,
                FxType::JSatClipper,
                [false; 4],
                StandaloneFxUiData::<JClipperParams>::params,
                p_body,
                StandaloneClipperParams,
                None,
                move || {
                    let mut pre = [0.0_f32; FX_SCOPE_LEN];
                    let mut post = [0.0_f32; FX_SCOPE_LEN];
                    let act = ring.read_snapshot(&mut pre, &mut post);
                    FxSlotScopeSnapshot {
                        enabled: p_scope.enabled.value(),
                        fx_type: FxType::JSatClipper,
                        eff_p: [
                            p_scope.drive.value(),
                            p_scope.ceil.value(),
                            p_scope.knee.value(),
                            p_scope.mix.value(),
                        ],
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
    params: Arc<JClipperParams>,
    editor_state: Arc<ViziaState>,
    scope_ring: Arc<FxScopeRing>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Builtin, move |cx, gui_context| {
        let cfg = StandaloneFxConfig::load(CONFIG_FILE);
        let initial_scale_pct = cfg.ui_scale.clamp(85, 200);
        let initial_theme = theme::find_theme_by_name(&cfg.theme);
        build_clipper_ui(
            cx,
            params.clone(),
            gui_context,
            scope_ring.clone(),
            initial_theme,
            initial_scale_pct,
            true,
        );
    })
}
