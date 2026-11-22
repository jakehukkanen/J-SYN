//! Standalone `J-Reverb` Vizia editor built directly on `j-syn-ui` shared FX Unit views.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use j_syn_dsp::mod_matrix::{FxScopeRing, FX_SCOPE_LEN};
use j_syn_dsp::FxType;
use j_syn_ui::fx_units::{
    build_standalone_fx_shell, build_tabbed_fx_unit_body, create_standalone_fx_editor,
    standalone_default_state, EightKnobUnitParams, FxSubTabHost, StandaloneFxHeaderSpec,
    StandaloneFxUiData,
};
use j_syn_ui::scopes::FxSlotScopeSnapshot;
use j_syn_ui::theme::ThemePalette;
use j_syn_ui::tokens;
use nih_plug::prelude::{BoolParam, Editor, FloatParam, GuiContext};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::ViziaState;

use crate::JReverbParams;

const HEADER_SPEC: StandaloneFxHeaderSpec = StandaloneFxHeaderSpec {
    header_title: "J-REVERB",
    card_title: "REVERB",
    card_subtitle: "FDN • EQ",
    fx_type: FxType::Reverb,
};

pub fn reverb_window_size() -> (u32, u32) {
    tokens::standalone_fx_window_size(FxType::Reverb)
}

pub fn default_state() -> Arc<ViziaState> {
    standalone_default_state(FxType::Reverb)
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
struct StandaloneReverbParams;

impl EightKnobUnitParams<JReverbParams> for StandaloneReverbParams {
    fn enabled<'a>(&self, p: &'a JReverbParams) -> &'a BoolParam {
        &p.enabled
    }
    fn param<'a>(&self, p: &'a JReverbParams, idx: usize) -> &'a FloatParam {
        match idx {
            0 => &p.size,
            1 => &p.decay,
            2 => &p.damp,
            3 => &p.mix,
            4 => &p.low_cut,
            5 => &p.high_cut,
            6 => &p.pre_delay,
            _ => &p.width,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build_reverb_ui(
    cx: &mut Context,
    params: Arc<JReverbParams>,
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
            let ring = scope_ring.clone();
            build_tabbed_fx_unit_body(
                cx,
                FxType::Reverb,
                "EQ",
                [false; 4],
                StandaloneFxUiData::<JReverbParams>::params,
                StandaloneFxUiData::<JReverbParams>::ui_rev,
                p_body.clone(),
                StandaloneReverbParams,
                tab_host.clone(),
                None,
                move || {
                    let mut pre = [0.0_f32; FX_SCOPE_LEN];
                    let mut post = [0.0_f32; FX_SCOPE_LEN];
                    let act = ring.read_snapshot(&mut pre, &mut post);
                    FxSlotScopeSnapshot {
                        enabled: p_scope.enabled.value(),
                        fx_type: FxType::Reverb,
                        eff_p: [
                            p_scope.size.value(),
                            p_scope.decay.value(),
                            p_scope.damp.value(),
                            p_scope.mix.value(),
                        ],
                        eff_eq: [
                            p_scope.low_cut.value(),
                            p_scope.high_cut.value(),
                            p_scope.pre_delay.value(),
                            p_scope.width.value(),
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
    params: Arc<JReverbParams>,
    editor_state: Arc<ViziaState>,
    scope_ring: Arc<FxScopeRing>,
) -> Option<Box<dyn Editor>> {
    create_standalone_fx_editor(editor_state, move |cx, gui_context, pal, scale_pct| {
        build_reverb_ui(
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
