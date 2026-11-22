//! Standalone `J-Phaser` Vizia editor built directly on `j-syn-ui` shared FX Unit views.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::{FxScopeRing, FX_SCOPE_LEN};
use j_syn_dsp::FxType;
use j_syn_ui::fx_units::{
    build_four_knob_fx_unit_body, build_standalone_fx_shell, create_standalone_fx_editor,
    standalone_default_state, FourKnobUnitParams, StandaloneFxHeaderSpec, StandaloneFxUiData,
};
use j_syn_ui::scopes::FxSlotScopeSnapshot;
use j_syn_ui::theme::ThemePalette;
use j_syn_ui::tokens;
use nih_plug::prelude::{BoolParam, Editor, FloatParam, GuiContext};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::ViziaState;

use crate::JPhaserParams;

const HEADER_SPEC: StandaloneFxHeaderSpec = StandaloneFxHeaderSpec {
    header_title: "J-PHASER",
    card_title: "PHASER",
    fx_type: FxType::Phaser,
};

pub fn phaser_window_size() -> (u32, u32) {
    tokens::standalone_fx_window_size(FxType::Phaser)
}

pub fn default_state() -> Arc<ViziaState> {
    standalone_default_state(FxType::Phaser)
}

#[derive(Clone, Copy)]
struct StandalonePhaserParams;

impl FourKnobUnitParams<JPhaserParams> for StandalonePhaserParams {
    fn enabled<'a>(&self, p: &'a JPhaserParams) -> &'a BoolParam {
        &p.enabled
    }
    fn param<'a>(&self, p: &'a JPhaserParams, idx: usize) -> &'a FloatParam {
        match idx {
            0 => &p.rate,
            1 => &p.depth,
            2 => &p.feedback,
            _ => &p.mix,
        }
    }
}

pub fn build_phaser_ui(
    cx: &mut Context,
    params: Arc<JPhaserParams>,
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
        Some(|p| &p.stack),
        initial_theme,
        initial_scale_pct,
        persist_settings,
        move |cx| {
            let p_scope = p_body.clone();
            let ring = scope_ring.clone();
            build_four_knob_fx_unit_body(
                cx,
                FxType::Phaser,
                [false; 4],
                StandaloneFxUiData::<JPhaserParams>::params,
                p_body.clone(),
                StandalonePhaserParams,
                None,
                move || {
                    let mut pre = [0.0_f32; FX_SCOPE_LEN];
                    let mut post = [0.0_f32; FX_SCOPE_LEN];
                    let act = ring.read_snapshot(&mut pre, &mut post);
                    FxSlotScopeSnapshot {
                        enabled: p_scope.enabled.value(),
                        fx_type: FxType::Phaser,
                        eff_p: [
                            p_scope.rate.value(),
                            p_scope.depth.value(),
                            p_scope.feedback.value(),
                            p_scope.mix.value(),
                        ],
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
    params: Arc<JPhaserParams>,
    editor_state: Arc<ViziaState>,
    scope_ring: Arc<FxScopeRing>,
) -> Option<Box<dyn Editor>> {
    create_standalone_fx_editor(editor_state, move |cx, gui_context, pal, scale_pct| {
        build_phaser_ui(
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
