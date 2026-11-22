//! Shared `200 px` variable-height FX Unit views for Tier 1 + Tier 2 effects:
//! - Plain 4-knob `2×2` (`JSatClipper`, `Ott`, `Phaser`)
//! - 4-knob `2×2` + `20 px` context dropdown (`Distortion`, `Diopser`)
//! - 8-knob 2-tab `2×2` (`Chorus`, `Reverb`, `Flanger`)

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::FxType;
use nih_plug::prelude::{BoolParam, EnumParam, FloatParam, GuiContext};
use nih_plug_vizia::vizia::prelude::*;

use crate::controls::{bold_label, build_enum_dropdown, build_prefixed_enum_dropdown};
use crate::scopes::{fx_accent_rgb, FxSlotScopeSnapshot, FxSlotVisualizer};
use crate::shell::{ModUiHost, UiKitEvent};
use crate::tokens;
use crate::widgets::{KnobDimExt, RotaryKnob, ValueFormatter};

use super::params::{
    DiopserStagesParam, DistortionModeParam, DIOPSER_STAGE_ITEMS, MB_DIST_MODE_ITEMS,
};
use super::{build_overlay_bool_pill, format_fx_slot_param_value, fx_slot_param_label};

/// Shared equidistant `2×2` knob grid (`46 px` knobs at `W/3` & `2W/3`, `9.5 pt` titles)
/// matching `docs/FX_UNIT_SPEC.md` §4.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_2x2_knob_grid<L, Params, FEnabled, FPick>(
    cx: &mut Context,
    fx_type: FxType,
    is_subtab: bool,
    bipolar_mask: [bool; 4],
    defs: [f32; 4],
    accent: (u8, u8, u8),
    knob_sz: f32,
    params_lens: L,
    params: Arc<Params>,
    pick_enabled: FEnabled,
    pick_param: FPick,
    mod_targets: Option<([ModDestination; 4], Arc<dyn ModUiHost>, u8)>,
) where
    L: Lens<Target = Arc<Params>> + Clone,
    Params: Send + Sync + 'static,
    FEnabled: Fn(&Params) -> &BoolParam + Copy + Send + Sync + 'static,
    FPick: Fn(&Params, usize) -> &FloatParam + Copy + Send + Sync + 'static,
{
    VStack::new(cx, move |cx| {
        for row in 0..2 {
            let lens_r = params_lens.clone();
            let p_r = params.clone();
            let mod_r = mod_targets.clone();
            HStack::new(cx, move |cx| {
                for col in 0..2 {
                    let p_idx = row * 2 + col;
                    let title = fx_slot_param_label(fx_type, is_subtab, false, 0, p_idx);
                    let formatter: ValueFormatter = Arc::new(move |norm: f32, _off: Option<f32>| {
                        format_fx_slot_param_value(fx_type, is_subtab, 0, p_idx, norm)
                    });
                    let lens_k = lens_r.clone();
                    let p_dim = p_r.clone();
                    let mod_cell = mod_r
                        .as_ref()
                        .map(|(dests, host, tag)| (dests[p_idx], host.clone(), *tag));

                    VStack::new(cx, move |cx| {
                        bold_label(cx, title, "knob-title")
                            .width(Stretch(1.0))
                            .height(Pixels(tokens::KNOB_TITLE_H))
                            .child_space(Stretch(1.0));

                        RotaryKnob::new_with_opt_mod_formatter(
                            cx,
                            lens_k,
                            move |p: &Arc<Params>| pick_param(p.as_ref(), p_idx),
                            bipolar_mask[p_idx],
                            accent,
                            mod_cell,
                            Some(formatter),
                        )
                        .default_norm(Some(defs[p_idx]))
                        .enabled_by(p_dim, move |p| pick_enabled(p).value())
                        .width(Pixels(knob_sz))
                        .height(Pixels(knob_sz))
                        .left(Stretch(1.0))
                        .right(Stretch(1.0));
                    })
                    .class("knob-cell")
                    .width(Stretch(1.0))
                    .height(Auto)
                    .row_between(Pixels(tokens::SPACE_XS));
                }
            })
            .col_between(Pixels(0.0))
            .child_left(Pixels(tokens::FX_UNIT_2X2_INSET_X))
            .child_right(Pixels(tokens::FX_UNIT_2X2_INSET_X))
            .width(Stretch(1.0))
            .height(Auto);
        }
    })
    .class("fx-unit-2x2")
    .row_between(Pixels(tokens::SPACE_MD))
    .top(Pixels(tokens::FX_UNIT_2X2_TOP_PAD))
    .bottom(Pixels(tokens::FX_UNIT_2X2_BOTTOM_PAD))
    .width(Stretch(1.0))
    .height(Auto);
}

/// Parameter projection trait for the shared `Clipper` FX Unit view (with `PCM` scope toggle).
pub trait ClipperUnitParams<Params: 'static>: Copy + Send + Sync + 'static {
    fn enabled<'a>(&self, p: &'a Params) -> &'a BoolParam;
    fn pcm<'a>(&self, p: &'a Params) -> &'a BoolParam;
    fn param<'a>(&self, p: &'a Params, idx: usize) -> &'a FloatParam;
}

/// Shared `Clipper` FX Unit body (`ZStack` scope + top-left `PCM` toggle pill + `2×2` knob grid).
#[allow(clippy::too_many_arguments)]
pub fn build_clipper_fx_unit_body<L, Params, U, FScope>(
    cx: &mut Context,
    params_lens: L,
    params: Arc<Params>,
    gui_context: Arc<dyn GuiContext>,
    unit_params: U,
    mod_targets: Option<([ModDestination; 4], Arc<dyn ModUiHost>, u8)>,
    scope_provider: FScope,
    knob_sz: f32,
) where
    L: Lens<Target = Arc<Params>> + Clone,
    Params: Send + Sync + 'static,
    U: ClipperUnitParams<Params>,
    FScope: Fn() -> FxSlotScopeSnapshot + Send + Sync + 'static,
{
    let scope_h = tokens::fx_unit_scope_height(FxType::JSatClipper);
    let defs = j_syn_dsp::FxSlotParams::default_params_for_type(FxType::JSatClipper);
    let accent = fx_accent_rgb(FxType::JSatClipper);

    let lens_scope = params_lens.clone();
    let p_scope = params.clone();

    VStack::new(cx, move |cx| {
        ZStack::new(cx, move |cx| {
            FxSlotVisualizer::new(cx, scope_provider)
                .hoverable(false)
                .width(Stretch(1.0))
                .height(Stretch(1.0));

            HStack::new(cx, move |cx| {
                build_overlay_bool_pill(
                    cx,
                    lens_scope,
                    p_scope,
                    gui_context,
                    "PCM",
                    move |p: &Arc<Params>| unit_params.pcm(p),
                );
                Element::new(cx).width(Stretch(1.0)).hoverable(false);
            })
            .position_type(PositionType::SelfDirected)
            .width(Stretch(1.0))
            .height(Pixels(tokens::FX_UNIT_OVERLAY_PILL_H))
            .top(Pixels(tokens::SPACE_SM))
            .left(Pixels(tokens::SPACE_SM))
            .right(Pixels(tokens::SPACE_SM));
        })
        .width(Stretch(1.0))
        .height(Pixels(scope_h));

        build_2x2_knob_grid(
            cx,
            FxType::JSatClipper,
            false,
            [false; 4],
            defs,
            accent,
            knob_sz,
            params_lens,
            params,
            move |p| unit_params.enabled(p),
            move |p, idx| unit_params.param(p, idx),
            mod_targets,
        );
    })
    .row_between(Pixels(tokens::SPACE_SM))
    .width(Stretch(1.0))
    .height(Auto);
}

/// Parameter projection trait for plain 4-knob FX Units (`OTT`, `Phaser`).
pub trait FourKnobUnitParams<Params: 'static>: Copy + Send + Sync + 'static {
    fn enabled<'a>(&self, p: &'a Params) -> &'a BoolParam;
    fn param<'a>(&self, p: &'a Params, idx: usize) -> &'a FloatParam;
}

/// Shared plain 4-knob FX Unit body (`FxSlotVisualizer` scope + equidistant `2×2` knob grid).
#[allow(clippy::too_many_arguments)]
pub fn build_four_knob_fx_unit_body<L, Params, U, FScope>(
    cx: &mut Context,
    fx_type: FxType,
    bipolar_mask: [bool; 4],
    params_lens: L,
    params: Arc<Params>,
    unit_params: U,
    mod_targets: Option<([ModDestination; 4], Arc<dyn ModUiHost>, u8)>,
    scope_provider: FScope,
    knob_sz: f32,
) where
    L: Lens<Target = Arc<Params>> + Clone,
    Params: Send + Sync + 'static,
    U: FourKnobUnitParams<Params>,
    FScope: Fn() -> FxSlotScopeSnapshot + Send + Sync + 'static,
{
    let scope_h = tokens::fx_unit_scope_height(fx_type);
    let defs = j_syn_dsp::FxSlotParams::default_params_for_type(fx_type);
    let accent = fx_accent_rgb(fx_type);

    VStack::new(cx, move |cx| {
        FxSlotVisualizer::new(cx, scope_provider)
            .hoverable(false)
            .width(Stretch(1.0))
            .height(Pixels(scope_h));

        build_2x2_knob_grid(
            cx,
            fx_type,
            false,
            bipolar_mask,
            defs,
            accent,
            knob_sz,
            params_lens,
            params,
            move |p| unit_params.enabled(p),
            move |p, idx| unit_params.param(p, idx),
            mod_targets,
        );
    })
    .row_between(Pixels(tokens::SPACE_SM))
    .width(Stretch(1.0))
    .height(Auto);
}

/// Parameter projection trait for the shared `Distortion` FX Unit view.
pub trait DistortionUnitParams<Params: 'static>: Copy + Send + Sync + 'static {
    fn enabled<'a>(&self, p: &'a Params) -> &'a BoolParam;
    fn mode<'a>(&self, p: &'a Params) -> &'a EnumParam<DistortionModeParam>;
    fn param<'a>(&self, p: &'a Params, idx: usize) -> &'a FloatParam;
}

/// Shared `Distortion` FX Unit body (compact `44 px` scope + `20 px` mode dropdown + `2×2` grid).
#[allow(clippy::too_many_arguments)]
pub fn build_distortion_fx_unit_body<L, Params, U, FScope>(
    cx: &mut Context,
    params_lens: L,
    params: Arc<Params>,
    gui_context: Arc<dyn GuiContext>,
    unit_params: U,
    mod_targets: Option<([ModDestination; 4], Arc<dyn ModUiHost>, u8)>,
    scope_provider: FScope,
    knob_sz: f32,
) where
    L: Lens<Target = Arc<Params>> + Clone,
    Params: Send + Sync + 'static,
    U: DistortionUnitParams<Params>,
    FScope: Fn() -> FxSlotScopeSnapshot + Send + Sync + 'static,
{
    let scope_h = tokens::fx_unit_scope_height(FxType::Distortion);
    let defs = j_syn_dsp::FxSlotParams::default_params_for_type(FxType::Distortion);
    let accent = fx_accent_rgb(FxType::Distortion);

    VStack::new(cx, move |cx| {
        FxSlotVisualizer::new(cx, scope_provider)
            .hoverable(false)
            .width(Stretch(1.0))
            .height(Pixels(scope_h));

        build_enum_dropdown(
            cx,
            params_lens.clone(),
            params.clone(),
            gui_context,
            move |p: &Arc<Params>| unit_params.mode(p),
            MB_DIST_MODE_ITEMS,
        )
        .width(Stretch(1.0))
        .height(Pixels(tokens::CONTROL_H));

        build_2x2_knob_grid(
            cx,
            FxType::Distortion,
            false,
            [true, false, true, false],
            defs,
            accent,
            knob_sz,
            params_lens,
            params,
            move |p| unit_params.enabled(p),
            move |p, idx| unit_params.param(p, idx),
            mod_targets,
        );
    })
    .row_between(Pixels(tokens::SPACE_SM))
    .width(Stretch(1.0))
    .height(Auto);
}

/// Parameter projection trait for the shared `Dispersion` (`Diopser`) FX Unit view.
pub trait DiopserUnitParams<Params: 'static>: Copy + Send + Sync + 'static {
    fn enabled<'a>(&self, p: &'a Params) -> &'a BoolParam;
    fn stages<'a>(&self, p: &'a Params) -> &'a EnumParam<DiopserStagesParam>;
    fn param<'a>(&self, p: &'a Params, idx: usize) -> &'a FloatParam;
}

/// Shared `Dispersion` FX Unit body (`58 px` scope + `20 px` stage count dropdown + `2×2` grid).
#[allow(clippy::too_many_arguments)]
pub fn build_diopser_fx_unit_body<L, Params, U, FScope>(
    cx: &mut Context,
    params_lens: L,
    params: Arc<Params>,
    gui_context: Arc<dyn GuiContext>,
    unit_params: U,
    mod_targets: Option<([ModDestination; 4], Arc<dyn ModUiHost>, u8)>,
    scope_provider: FScope,
    knob_sz: f32,
) where
    L: Lens<Target = Arc<Params>> + Clone,
    Params: Send + Sync + 'static,
    U: DiopserUnitParams<Params>,
    FScope: Fn() -> FxSlotScopeSnapshot + Send + Sync + 'static,
{
    let scope_h = tokens::fx_unit_scope_height(FxType::Diopser);
    let defs = j_syn_dsp::FxSlotParams::default_params_for_type(FxType::Diopser);
    let accent = fx_accent_rgb(FxType::Diopser);

    VStack::new(cx, move |cx| {
        FxSlotVisualizer::new(cx, scope_provider)
            .hoverable(false)
            .width(Stretch(1.0))
            .height(Pixels(scope_h));

        build_prefixed_enum_dropdown(
            cx,
            params_lens.clone(),
            params.clone(),
            gui_context,
            move |p: &Arc<Params>| unit_params.stages(p),
            DIOPSER_STAGE_ITEMS,
            "STG ",
        )
        .width(Stretch(1.0))
        .height(Pixels(tokens::CONTROL_H));

        build_2x2_knob_grid(
            cx,
            FxType::Diopser,
            false,
            [false; 4],
            defs,
            accent,
            knob_sz,
            params_lens,
            params,
            move |p| unit_params.enabled(p),
            move |p, idx| unit_params.param(p, idx),
            mod_targets,
        );
    })
    .row_between(Pixels(tokens::SPACE_SM))
    .width(Stretch(1.0))
    .height(Auto);
}

/// Host abstraction for 2-tab FX Units (`MAIN` vs `EQ` / `MOD`).
pub trait FxSubTabHost: Send + Sync + 'static {
    fn is_subtab(&self) -> bool;
    fn set_subtab(&self, subtab: bool);
}

/// Parameter projection trait for 8-knob 2-tab FX Units (`Chorus`, `Reverb`, `Flanger`).
pub trait EightKnobUnitParams<Params: 'static>: Copy + Send + Sync + 'static {
    fn enabled<'a>(&self, p: &'a Params) -> &'a BoolParam;
    /// `idx` in `0..4` = `MAIN` tab, `4..8` = `EQ` / `MOD` tab.
    fn param<'a>(&self, p: &'a Params, idx: usize) -> &'a FloatParam;
}

/// Shared 8-knob 2-tab FX Unit body (`FxSlotVisualizer` scope + `18 px` `MAIN | <SUB>` tab bar
/// + constant-height `2×2` knob grid).
#[allow(clippy::too_many_arguments)]
pub fn build_tabbed_fx_unit_body<L, R, Params, U, FScope>(
    cx: &mut Context,
    fx_type: FxType,
    subtab_label: &'static str,
    subtab_bipolar_mask: [bool; 4],
    params_lens: L,
    ui_rev_lens: R,
    params: Arc<Params>,
    unit_params: U,
    tab_host: Arc<dyn FxSubTabHost>,
    mod_targets: Option<([ModDestination; 8], Arc<dyn ModUiHost>, u8)>,
    scope_provider: FScope,
    knob_sz: f32,
) where
    L: Lens<Target = Arc<Params>> + Clone,
    R: Lens<Target = u64> + Clone,
    Params: Send + Sync + 'static,
    U: EightKnobUnitParams<Params>,
    FScope: Fn() -> FxSlotScopeSnapshot + Send + Sync + 'static,
{
    let scope_h = tokens::fx_unit_scope_height(fx_type);
    let main_defs = j_syn_dsp::FxSlotParams::default_params_for_type(fx_type);
    let sub_defs = j_syn_dsp::FxSlotParams::default_eq_params_for_type(fx_type);
    let accent = fx_accent_rgb(fx_type);

    let rev_tabs = ui_rev_lens.clone();
    let rev_grid = ui_rev_lens;
    let host_tabs = tab_host.clone();
    let host_grid = tab_host;

    VStack::new(cx, move |cx| {
        FxSlotVisualizer::new(cx, scope_provider)
            .hoverable(false)
            .width(Stretch(1.0))
            .height(Pixels(scope_h));

        // 18 px MAIN | EQ (or MAIN | MOD) tab bar
        HStack::new(cx, move |cx| {
            let h_main_click = host_tabs.clone();
            let h_main_cls = host_tabs.clone();
            Button::new(
                cx,
                move |cx| {
                    h_main_click.set_subtab(false);
                    cx.emit(UiKitEvent::ModUiChanged);
                },
                |cx| bold_label(cx, "MAIN", "fx-subtab-lbl").hoverable(false),
            )
            .class("fx-mb-side-btn")
            .toggle_class(
                "fx-subtab-active",
                rev_tabs.clone().map(move |_| !h_main_cls.is_subtab()),
            )
            .width(Stretch(1.0))
            .height(Pixels(tokens::PILL_H));

            let h_sub_click = host_tabs.clone();
            let h_sub_cls = host_tabs;
            Button::new(
                cx,
                move |cx| {
                    h_sub_click.set_subtab(true);
                    cx.emit(UiKitEvent::ModUiChanged);
                },
                move |cx| bold_label(cx, subtab_label, "fx-subtab-lbl").hoverable(false),
            )
            .class("fx-mb-side-btn")
            .toggle_class(
                "fx-subtab-active",
                rev_tabs.map(move |_| h_sub_cls.is_subtab()),
            )
            .width(Stretch(1.0))
            .height(Pixels(tokens::PILL_H));
        })
        .col_between(Pixels(tokens::SPACE_XS))
        .width(Stretch(1.0))
        .height(Pixels(tokens::PILL_H));

        // 2×2 knob grid switching cleanly between MAIN (0..4) and EQ/MOD (4..8)
        Binding::new(
            cx,
            rev_grid.map(move |_| host_grid.is_subtab()),
            move |cx, sub_lens| {
                let is_sub = sub_lens.get(cx);
                let base = if is_sub { 4 } else { 0 };
                let defs = if is_sub { sub_defs } else { main_defs };
                let bipolar = if is_sub {
                    subtab_bipolar_mask
                } else {
                    [false; 4]
                };
                let mod_slice = mod_targets.as_ref().map(|(dests, host, tag)| {
                    (
                        [
                            dests[base],
                            dests[base + 1],
                            dests[base + 2],
                            dests[base + 3],
                        ],
                        host.clone(),
                        *tag,
                    )
                });
                build_2x2_knob_grid(
                    cx,
                    fx_type,
                    is_sub,
                    bipolar,
                    defs,
                    accent,
                    knob_sz,
                    params_lens.clone(),
                    params.clone(),
                    move |p| unit_params.enabled(p),
                    move |p, idx| unit_params.param(p, base + idx),
                    mod_slice,
                );
            },
        );
    })
    .row_between(Pixels(tokens::SPACE_SM))
    .width(Stretch(1.0))
    .height(Auto);
}
