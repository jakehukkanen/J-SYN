//! Standalone `J-Flanger` Vizia editor built directly on `j-syn-ui` shared FX Unit views.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use j_syn_dsp::mod_matrix::{FxScopeRing, FX_SCOPE_LEN};
use j_syn_dsp::FxType;
use j_syn_ui::fx_units::{
    build_standalone_fx_shell, build_tabbed_fx_unit_body, standalone_default_state,
    EightKnobUnitParams, FxSubTabHost, StandaloneFxConfig, StandaloneFxHeaderSpec,
    StandaloneFxUiData,
};
use j_syn_ui::scopes::FxSlotScopeSnapshot;
use j_syn_ui::theme::{self, ThemePalette};
use j_syn_ui::tokens;
use nih_plug::prelude::{BoolParam, Editor, FloatParam, GuiContext};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};

use crate::JFlangerParams;

const CONFIG_FILE: &str = "j-flanger.json";

const HEADER_SPEC: StandaloneFxHeaderSpec = StandaloneFxHeaderSpec {
    header_title: "J-FLANGER",
    card_title: "FLANGER",
    card_subtitle: "STEREO • MOD",
    fx_type: FxType::Flanger,
    config_file: CONFIG_FILE,
};

pub fn flanger_window_size() -> (u32, u32) {
    tokens::standalone_fx_window_size(FxType::Flanger)
}

pub fn default_state() -> Arc<ViziaState> {
    standalone_default_state(FxType::Flanger, CONFIG_FILE)
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
struct StandaloneFlangerParams;

impl EightKnobUnitParams<JFlangerParams> for StandaloneFlangerParams {
    fn enabled<'a>(&self, p: &'a JFlangerParams) -> &'a BoolParam {
        &p.enabled
    }
    fn param<'a>(&self, p: &'a JFlangerParams, idx: usize) -> &'a FloatParam {
        match idx {
            0 => &p.rate,
            1 => &p.depth,
            2 => &p.delay,
            3 => &p.mix,
            4 => &p.feedback,
            5 => &p.phase,
            6 => &p.low_cut,
            _ => &p.tone,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build_flanger_ui(
    cx: &mut Context,
    params: Arc<JFlangerParams>,
    _gui_context: Arc<dyn GuiContext>,
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
        initial_theme,
        initial_scale_pct,
        persist_settings,
        move |cx| {
            let p_scope = p_body.clone();
            let ring = scope_ring;
            build_tabbed_fx_unit_body(
                cx,
                FxType::Flanger,
                "MOD",
                [true, false, false, false],
                StandaloneFxUiData::<JFlangerParams>::params,
                StandaloneFxUiData::<JFlangerParams>::ui_rev,
                p_body,
                StandaloneFlangerParams,
                tab_host,
                None,
                move || {
                    let mut pre = [0.0_f32; FX_SCOPE_LEN];
                    let mut post = [0.0_f32; FX_SCOPE_LEN];
                    let act = ring.read_snapshot(&mut pre, &mut post);
                    FxSlotScopeSnapshot {
                        enabled: p_scope.enabled.value(),
                        fx_type: FxType::Flanger,
                        eff_p: [
                            p_scope.rate.value(),
                            p_scope.depth.value(),
                            p_scope.delay.value(),
                            p_scope.mix.value(),
                        ],
                        eff_eq: [
                            p_scope.feedback.value(),
                            p_scope.phase.value(),
                            p_scope.low_cut.value(),
                            p_scope.tone.value(),
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
    params: Arc<JFlangerParams>,
    editor_state: Arc<ViziaState>,
    scope_ring: Arc<FxScopeRing>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Builtin, move |cx, gui_context| {
        let cfg = StandaloneFxConfig::load(CONFIG_FILE);
        let initial_scale_pct = cfg.ui_scale.clamp(85, 200);
        let initial_theme = theme::find_theme_by_name(&cfg.theme);
        build_flanger_ui(
            cx,
            params.clone(),
            gui_context,
            scope_ring.clone(),
            initial_theme,
            initial_scale_pct,
            0,
            true,
        );
    })
}
