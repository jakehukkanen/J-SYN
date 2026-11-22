//! Standalone `J-Limiter` Vizia editor built directly on `j-syn-ui` shared FX Unit views.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use j_syn_dsp::mod_matrix::{FxScopeRing, FX_SCOPE_LEN};
use j_syn_dsp::FxType;
use j_syn_ui::fx_units::{
    build_limiter_fx_unit_body, build_standalone_fx_shell, create_standalone_fx_editor,
    standalone_default_state, FxSubTabHost, LimiterUnitParams, StandaloneFxHeaderSpec,
    StandaloneFxUiData,
};
use j_syn_ui::scopes::FxSlotScopeSnapshot;
use j_syn_ui::theme::ThemePalette;
use j_syn_ui::tokens;
use nih_plug::prelude::{BoolParam, Editor, FloatParam, GuiContext};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::ViziaState;

use crate::JLimiterParams;

const HEADER_SPEC: StandaloneFxHeaderSpec = StandaloneFxHeaderSpec {
    header_title: "J-LIMITER",
    card_title: "LIMITER",
    fx_type: FxType::Limiter,
};

pub fn limiter_window_size() -> (u32, u32) {
    tokens::standalone_fx_window_size(FxType::Limiter)
}

pub fn default_state() -> Arc<ViziaState> {
    standalone_default_state(FxType::Limiter)
}

struct StandaloneSubTabHost {
    subtab: AtomicBool,
}

impl FxSubTabHost for StandaloneSubTabHost {
    fn is_subtab(&self) -> bool {
        self.subtab.load(Ordering::Relaxed)
    }
    fn set_subtab(&self, subtab: bool) {
        self.subtab.store(subtab, Ordering::Relaxed);
    }
}

#[derive(Clone, Copy)]
struct StandaloneLimiterParams;

impl LimiterUnitParams<JLimiterParams> for StandaloneLimiterParams {
    fn enabled<'a>(&self, p: &'a JLimiterParams) -> &'a BoolParam {
        &p.enabled
    }
    fn true_peak<'a>(&self, p: &'a JLimiterParams) -> &'a BoolParam {
        &p.true_peak
    }
    fn param<'a>(&self, p: &'a JLimiterParams, idx: usize) -> &'a FloatParam {
        match idx {
            0 => &p.gain,
            1 => &p.ceil,
            2 => &p.lookahead,
            3 => &p.mix,
            4 => &p.release,
            5 => &p.knee,
            6 => &p.link,
            _ => &p.punch,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build_limiter_ui(
    cx: &mut Context,
    params: Arc<JLimiterParams>,
    gui_context: Arc<dyn GuiContext>,
    scope_ring: Arc<FxScopeRing>,
    initial_theme: ThemePalette,
    initial_scale_pct: u32,
    initial_tab: u8,
    persist_settings: bool,
) {
    let p_body = params.clone();
    let tab_host = Arc::new(StandaloneSubTabHost {
        subtab: AtomicBool::new(initial_tab > 0),
    });
    build_standalone_fx_shell(
        cx,
        HEADER_SPEC,
        params,
        |p| &p.enabled,
        None,
        initial_theme,
        initial_scale_pct,
        persist_settings,
        move |cx| {
            let p_scope = p_body.clone();
            let ring = scope_ring.clone();
            build_limiter_fx_unit_body(
                cx,
                StandaloneFxUiData::<JLimiterParams>::params,
                StandaloneFxUiData::<JLimiterParams>::ui_rev,
                p_body.clone(),
                gui_context.clone(),
                StandaloneLimiterParams,
                tab_host.clone(),
                None,
                move || {
                    let mut pre = [0.0_f32; FX_SCOPE_LEN];
                    let mut post = [0.0_f32; FX_SCOPE_LEN];
                    let act = ring.read_snapshot(&mut pre, &mut post);
                    FxSlotScopeSnapshot {
                        enabled: p_scope.enabled.value(),
                        fx_type: FxType::Limiter,
                        eff_p: [
                            p_scope.gain.value(),
                            p_scope.ceil.value(),
                            p_scope.lookahead.value(),
                            p_scope.mix.value(),
                        ],
                        eff_eq: [
                            p_scope.release.value(),
                            p_scope.knee.value(),
                            p_scope.link.value(),
                            p_scope.punch.value(),
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
    params: Arc<JLimiterParams>,
    editor_state: Arc<ViziaState>,
    scope_ring: Arc<FxScopeRing>,
) -> Option<Box<dyn Editor>> {
    create_standalone_fx_editor(editor_state, move |cx, gui_context, pal, scale_pct| {
        build_limiter_ui(
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
