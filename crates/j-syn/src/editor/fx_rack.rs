//! Parallel Dual-Bus Master FX Rack UI (`Bus A` & `Bus B`, `Sub Bus`, `MAIN` Bus Switcher,
//! full `4×4` `FX` view, `ZStack` translucent floating overlays, and `MAIN | EQ` sub-tabs).

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::{FxBusTopology, FxType, MB_BAND_LEVEL_DEFAULT_NORM};
use j_syn_ui::fx_units::{
    build_clipper_fx_unit_body, build_diopser_fx_unit_body, build_distortion_fx_unit_body,
    build_filter_fx_unit_body, build_four_knob_fx_unit_body, build_limiter_fx_unit_body,
    build_multiband_dist_fx_unit_body, build_tabbed_fx_unit_body, build_tube_res_fx_unit_body,
    build_utility_fx_unit_body, ClipperUnitParams, DiopserUnitParams, DistortionUnitParams,
    EightKnobUnitParams, FilterUnitParams, FourKnobUnitParams, FxSubTabHost, LimiterUnitParams,
    MultibandDistUnitParams, MultibandTabHost, TubeResonatorUnitParams, UtilityUnitParams,
};
use j_syn_ui::widgets::{notify_scroll_lane_wheeled, LedToggleSwitch, LedToggleSwitchExt};
use nih_plug::prelude::{BoolParam, EnumParam, FloatParam, GuiContext, Param, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::util::ModifiersExt;

use super::controls::{
    bold_label, build_dropdown_trigger_labels, build_flex_knob_cell_mod, build_inline_pill_cell,
    ArrowButton, ArrowDir, CollapseButton, ScrollDropdown,
};
use super::fx_scopes::slot_scope_snapshot;
use super::mod_bar::ModUiState;
use super::tokens;
use super::{AMBER_RGB, GOLD_RGB, UiData, UiEvent};
use crate::{
    DiopserStagesParam, DistortionModeParam, FilterModeParam, FxBusTopologyParam,
    FxFilterSlopeParam, FxTypeParam, JSynPluginParams,
};

const FX_TYPE_ITEMS: &[(&str, FxTypeParam)] = &[
    ("Empty", FxTypeParam::Off),
    ("Utility", FxTypeParam::Utility),
    ("OTT", FxTypeParam::Ott),
    ("Distortion", FxTypeParam::Distortion),
    ("MB Dist", FxTypeParam::MultibandDist),
    ("Clipper", FxTypeParam::JSatClipper),
    ("Limiter", FxTypeParam::Limiter),
    ("Chorus", FxTypeParam::Chorus),
    ("Flanger", FxTypeParam::Flanger),
    ("Phaser", FxTypeParam::Phaser),
    ("Delay", FxTypeParam::Delay),
    ("Reverb", FxTypeParam::Reverb),
    ("Filter", FxTypeParam::Filter),
    ("Dispersion", FxTypeParam::Diopser),
    ("Tube Res.", FxTypeParam::TubeResonator),
];

#[inline]
fn slot_enabled_for_bus(p: &JSynPluginParams, bus_idx: usize, slot_idx: usize) -> bool {
    p.fx_bus_slots(bus_idx)[slot_idx].enabled.value()
}

fn swap_mod_destinations_for_slots(
    params: &JSynPluginParams,
    bus_idx: usize,
    slot_a: usize,
    slot_b: usize,
) {
    let mut mat = params.mod_matrix.snapshot();
    let mut changed = false;
    for slot in &mut mat.slots {
        if let Some((b, s, p)) = slot.destination.fx_slot_param_coords() {
            if b == bus_idx && s == slot_a {
                slot.destination = ModDestination::fx_bus_slot_param(bus_idx, slot_b, p);
                changed = true;
            } else if b == bus_idx && s == slot_b {
                slot.destination = ModDestination::fx_bus_slot_param(bus_idx, slot_a, p);
                changed = true;
            }
        }
    }
    if changed {
        params.mod_matrix.store_value(mat);
    }
}

struct SlotSnapshot {
    en: bool,
    ty: FxTypeParam,
    stk: i32,
    p_main: [f32; 4],
    p_eq: [f32; 4],
    flt_m: FilterModeParam,
    flt_s: FxFilterSlopeParam,
    diop_s: DiopserStagesParam,
    tr_flt: bool,
    tr_dirty: bool,
    util_inv_l: bool,
    util_inv_r: bool,
    lim_tp: bool,
    clip_pcm: bool,
    dist_m: DistortionModeParam,
    mb_bands: [(DistortionModeParam, bool, bool, f32, f32, f32, f32); 3],
}

/// Swaps all settings, EQ sub-tabs, and modulation assignments between two slots of `bus_idx`.
fn swap_fx_slots(
    params: &Arc<JSynPluginParams>,
    gui_context: &dyn GuiContext,
    mod_ui: &ModUiState,
    bus_idx: usize,
    slot_a: usize,
    slot_b: usize,
) {
    if slot_a == slot_b || slot_a >= 8 || slot_b >= 8 {
        return;
    }

    let slots = params.fx_bus_slots(bus_idx);
    let capture = |idx: usize| -> SlotSnapshot {
        let s = &slots[idx];
        SlotSnapshot {
            en: s.enabled.value(),
            ty: s.fx_type.value(),
            stk: s.stack.value(),
            p_main: [s.p1.value(), s.p2.value(), s.p3.value(), s.p4.value()],
            p_eq: [s.p5.value(), s.p6.value(), s.p7.value(), s.p8.value()],
            flt_m: s.filter_mode.value(),
            flt_s: s.filter_slope.value(),
            diop_s: s.diopser_stages.value(),
            tr_flt: s.tube_res_filter_enabled.value(),
            tr_dirty: s.tube_res_dirty_lfo.value(),
            util_inv_l: s.utility_inv_l.value(),
            util_inv_r: s.utility_inv_r.value(),
            lim_tp: s.limiter_true_peak.value(),
            clip_pcm: s.clipper_pcm.value(),
            dist_m: s.distortion_mode.value(),
            mb_bands: std::array::from_fn(|b| {
                let mb = &s.mb_bands[b];
                (
                    mb.mode.value(),
                    mb.enabled.value(),
                    mb.solo.value(),
                    mb.drive.value(),
                    mb.bias.value(),
                    mb.level.value(),
                    mb.mix.value(),
                )
            }),
        }
    };
    let a = capture(slot_a);
    let b = capture(slot_b);

    let setter = ParamSetter::new(gui_context);
    let write = |idx: usize, snap: SlotSnapshot| {
        let s = &slots[idx];
        setter.begin_set_parameter(&s.enabled);
        setter.set_parameter(&s.enabled, snap.en);
        setter.end_set_parameter(&s.enabled);

        setter.begin_set_parameter(&s.fx_type);
        setter.set_parameter(&s.fx_type, snap.ty);
        setter.end_set_parameter(&s.fx_type);

        setter.begin_set_parameter(&s.stack);
        setter.set_parameter(&s.stack, snap.stk);
        setter.end_set_parameter(&s.stack);

        for (k, param) in s.params_array().into_iter().enumerate() {
            setter.begin_set_parameter(param);
            setter.set_parameter(param, snap.p_main[k]);
            setter.end_set_parameter(param);
        }
        for (k, param) in s.eq_params_array().into_iter().enumerate() {
            setter.begin_set_parameter(param);
            setter.set_parameter(param, snap.p_eq[k]);
            setter.end_set_parameter(param);
        }

        setter.begin_set_parameter(&s.filter_mode);
        setter.set_parameter(&s.filter_mode, snap.flt_m);
        setter.end_set_parameter(&s.filter_mode);

        setter.begin_set_parameter(&s.filter_slope);
        setter.set_parameter(&s.filter_slope, snap.flt_s);
        setter.end_set_parameter(&s.filter_slope);

        setter.begin_set_parameter(&s.diopser_stages);
        setter.set_parameter(&s.diopser_stages, snap.diop_s);
        setter.end_set_parameter(&s.diopser_stages);

        setter.begin_set_parameter(&s.tube_res_filter_enabled);
        setter.set_parameter(&s.tube_res_filter_enabled, snap.tr_flt);
        setter.end_set_parameter(&s.tube_res_filter_enabled);

        setter.begin_set_parameter(&s.tube_res_dirty_lfo);
        setter.set_parameter(&s.tube_res_dirty_lfo, snap.tr_dirty);
        setter.end_set_parameter(&s.tube_res_dirty_lfo);

        setter.begin_set_parameter(&s.utility_inv_l);
        setter.set_parameter(&s.utility_inv_l, snap.util_inv_l);
        setter.end_set_parameter(&s.utility_inv_l);

        setter.begin_set_parameter(&s.utility_inv_r);
        setter.set_parameter(&s.utility_inv_r, snap.util_inv_r);
        setter.end_set_parameter(&s.utility_inv_r);

        setter.begin_set_parameter(&s.limiter_true_peak);
        setter.set_parameter(&s.limiter_true_peak, snap.lim_tp);
        setter.end_set_parameter(&s.limiter_true_peak);

        setter.begin_set_parameter(&s.clipper_pcm);
        setter.set_parameter(&s.clipper_pcm, snap.clip_pcm);
        setter.end_set_parameter(&s.clipper_pcm);

        setter.begin_set_parameter(&s.distortion_mode);
        setter.set_parameter(&s.distortion_mode, snap.dist_m);
        setter.end_set_parameter(&s.distortion_mode);

        for (b, (m, en, solo, drv, bias, lvl, mix)) in snap.mb_bands.into_iter().enumerate() {
            let mb = &s.mb_bands[b];
            setter.begin_set_parameter(&mb.mode);
            setter.set_parameter(&mb.mode, m);
            setter.end_set_parameter(&mb.mode);
            setter.begin_set_parameter(&mb.enabled);
            setter.set_parameter(&mb.enabled, en);
            setter.end_set_parameter(&mb.enabled);
            setter.begin_set_parameter(&mb.solo);
            setter.set_parameter(&mb.solo, solo);
            setter.end_set_parameter(&mb.solo);
            for (fp, val) in [
                (&mb.drive, drv),
                (&mb.bias, bias),
                (&mb.level, lvl),
                (&mb.mix, mix),
            ] {
                setter.begin_set_parameter(fp);
                setter.set_parameter(fp, val);
                setter.end_set_parameter(fp);
            }
        }
    };

    write(slot_a, b);
    write(slot_b, a);
    mod_ui.swap_fx_eq_subtab(bus_idx, slot_a, slot_b);
    mod_ui.swap_fx_mb_tab(bus_idx, slot_a, slot_b);
    mod_ui.swap_fx_collapsed(bus_idx, slot_a, slot_b);
    swap_mod_destinations_for_slots(params, bus_idx, slot_a, slot_b);
}

fn set_fx_slot_type_with_defaults(
    params: &JSynPluginParams,
    gui_context: &Arc<dyn GuiContext>,
    mod_ui: &ModUiState,
    bus_idx: usize,
    slot_idx: usize,
    val: FxTypeParam,
) {
    let setter = ParamSetter::new(gui_context.as_ref());
    let slot = &params.fx_bus_slots(bus_idx)[slot_idx];
    let prev = slot.fx_type.value();
    setter.begin_set_parameter(&slot.fx_type);
    setter.set_parameter(&slot.fx_type, val);
    setter.end_set_parameter(&slot.fx_type);
    if prev != val {
        let ty: FxType = val.into();
        let defs = j_syn_dsp::FxSlotParams::default_params_for_type(ty);
        for (p, def_v) in slot.params_array().into_iter().zip(defs.into_iter()) {
            setter.begin_set_parameter(p);
            setter.set_parameter(p, def_v);
            setter.end_set_parameter(p);
        }
        let eq_defs = j_syn_dsp::FxSlotParams::default_eq_params_for_type(ty);
        for (p, def_v) in slot.eq_params_array().into_iter().zip(eq_defs.into_iter()) {
            setter.begin_set_parameter(p);
            setter.set_parameter(p, def_v);
            setter.end_set_parameter(p);
        }
        setter.begin_set_parameter(&slot.tube_res_filter_enabled);
        setter.set_parameter(&slot.tube_res_filter_enabled, true);
        setter.end_set_parameter(&slot.tube_res_filter_enabled);
        setter.begin_set_parameter(&slot.tube_res_dirty_lfo);
        setter.set_parameter(&slot.tube_res_dirty_lfo, true);
        setter.end_set_parameter(&slot.tube_res_dirty_lfo);
        setter.begin_set_parameter(&slot.utility_inv_l);
        setter.set_parameter(&slot.utility_inv_l, false);
        setter.end_set_parameter(&slot.utility_inv_l);
        setter.begin_set_parameter(&slot.utility_inv_r);
        setter.set_parameter(&slot.utility_inv_r, false);
        setter.end_set_parameter(&slot.utility_inv_r);
        setter.begin_set_parameter(&slot.limiter_true_peak);
        setter.set_parameter(&slot.limiter_true_peak, false);
        setter.end_set_parameter(&slot.limiter_true_peak);
        setter.begin_set_parameter(&slot.clipper_pcm);
        setter.set_parameter(&slot.clipper_pcm, true);
        setter.end_set_parameter(&slot.clipper_pcm);
        for mb in &slot.mb_bands {
            setter.begin_set_parameter(&mb.mode);
            setter.set_parameter(&mb.mode, DistortionModeParam::Tube);
            setter.end_set_parameter(&mb.mode);
            setter.begin_set_parameter(&mb.enabled);
            setter.set_parameter(&mb.enabled, true);
            setter.end_set_parameter(&mb.enabled);
            setter.begin_set_parameter(&mb.solo);
            setter.set_parameter(&mb.solo, false);
            setter.end_set_parameter(&mb.solo);
            for (fp, val) in [
                (&mb.drive, 0.30),
                (&mb.bias, 0.50),
                (&mb.level, MB_BAND_LEVEL_DEFAULT_NORM),
                (&mb.mix, 1.00),
            ] {
                setter.begin_set_parameter(fp);
                setter.set_parameter(fp, val);
                setter.end_set_parameter(fp);
            }
        }
        mod_ui.set_fx_eq_subtab(bus_idx, slot_idx, false);
        mod_ui.set_fx_mb_tab(bus_idx, slot_idx, 0);
        mod_ui.set_fx_collapsed(bus_idx, slot_idx, false);
    }
}

/// Hover-passthrough `ZStack` overlay over an FX card header that illuminates with a subtle
/// accent tint + rim whenever the pointer hovers the header while holding `Cmd` or `Ctrl`.
struct FxHeaderCmdHighlight {
    bus_idx: usize,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
}

impl FxHeaderCmdHighlight {
    fn new(
        cx: &mut Context,
        bus_idx: usize,
        slot_idx: usize,
        params: Arc<JSynPluginParams>,
    ) -> Handle<'_, Self> {
        Self {
            bus_idx,
            slot_idx,
            params,
        }
        .build(cx, |_| {})
        .hoverable(false)
    }
}

impl View for FxHeaderCmdHighlight {
    fn element(&self) -> Option<&'static str> {
        Some("fx-header-cmd-highlight")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        if self.params.fx_bus_slots(self.bus_idx)[self.slot_idx]
            .fx_type
            .value()
            == FxTypeParam::Off
        {
            return;
        }
        let mods = cx.modifiers();
        if !(mods.command() || mods.contains(Modifiers::CTRL)) {
            return;
        }
        let b = cx.bounds();
        if b.w <= 1.0 || b.h <= 1.0 {
            return;
        }
        let mx = cx.mouse().cursorx;
        let my = cx.mouse().cursory;
        if mx < b.x || mx > b.x + b.w || my < b.y || my > b.y + b.h {
            return;
        }
        let dpx = cx.scale_factor();
        let pal = j_syn_ui::theme::current_palette(cx);
        let (ar, ag, ab) = pal.accent_rgb();
        let r = tokens::RADIUS_PILL * dpx;
        let mut rect = vg::Path::new();
        rect.rounded_rect(b.x, b.y, b.w, b.h, r);
        canvas.fill_path(&rect, &vg::Paint::color(vg::Color::rgba(ar, ag, ab, 32)));
        let mut border = vg::Paint::color(vg::Color::rgba(ar, ag, ab, 195));
        border.set_line_width(1.0 * dpx);
        canvas.stroke_path(&rect, &border);
    }
}

#[inline]
fn toggle_slot_collapse_if_active(
    cx: &mut EventContext,
    params: &JSynPluginParams,
    mod_ui: &ModUiState,
    bus_idx: usize,
    slot_idx: usize,
) {
    if params.fx_bus_slots(bus_idx)[slot_idx].fx_type.value() != FxTypeParam::Off {
        mod_ui.toggle_fx_collapsed(bus_idx, slot_idx);
        cx.emit(UiEvent::ModUiChanged);
    }
}

/// Scoped row container that intercepts `Cmd/Ctrl + Left-Click` only within its own bounds to
/// toggle FX card minimize/maximize without leaking globally across the window.
struct FxHeaderCmdClickZone {
    bus_idx: usize,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    mod_ui: Arc<ModUiState>,
    direct_only: bool,
}

impl FxHeaderCmdClickZone {
    fn new<'a, F>(
        cx: &'a mut Context,
        bus_idx: usize,
        slot_idx: usize,
        params: Arc<JSynPluginParams>,
        mod_ui: Arc<ModUiState>,
        direct_only: bool,
        content: F,
    ) -> Handle<'a, Self>
    where
        F: FnOnce(&mut Context),
    {
        Self {
            bus_idx,
            slot_idx,
            params,
            mod_ui,
            direct_only,
        }
        .build(cx, content)
        .layout_type(LayoutType::Row)
    }
}

impl View for FxHeaderCmdClickZone {
    fn element(&self) -> Option<&'static str> {
        Some("fx-header-cmd-click-zone")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            let cmd_or_ctrl =
                cx.modifiers().command() || cx.modifiers().contains(Modifiers::CTRL);
            if !cmd_or_ctrl {
                return;
            }
            if self.direct_only && meta.target != cx.current() {
                return;
            }
            let b = cx.bounds();
            let mx = cx.mouse().cursorx;
            let my = cx.mouse().cursory;
            if mx < b.x || mx > b.x + b.w || my < b.y || my > b.y + b.h {
                return;
            }
            match window_event {
                WindowEvent::MouseDown(MouseButton::Left)
                | WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                    toggle_slot_collapse_if_active(
                        cx,
                        &self.params,
                        &self.mod_ui,
                        self.bus_idx,
                        self.slot_idx,
                    );
                    meta.consume();
                }
                WindowEvent::Press { .. } | WindowEvent::PressDown { .. }
                    if !self.direct_only =>
                {
                    meta.consume();
                }
                _ => {}
            }
        });
    }
}

/// Scoped column container inside each FX lane `ScrollView` that observes unconsumed `MouseScroll`
/// events bubbling up to `ScrollView` and arms the `250 ms` / `≥ 2 px` pointer scroll-wheel guard.
struct FxLaneScrollGuard;

impl FxLaneScrollGuard {
    fn new<'a, F>(cx: &'a mut Context, content: F) -> Handle<'a, Self>
    where
        F: FnOnce(&mut Context),
    {
        Self.build(cx, content).layout_type(LayoutType::Column)
    }
}

impl View for FxLaneScrollGuard {
    fn element(&self) -> Option<&'static str> {
        Some("fx-lane-scroll-guard")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, _meta| {
            if let WindowEvent::MouseScroll(_, sy) = window_event {
                if *sy != 0.0 {
                    notify_scroll_lane_wheeled(cx.mouse().cursorx, cx.mouse().cursory);
                }
            }
        });
    }
}

fn build_fx_type_dropdown(
    cx: &mut Context,
    bus_idx: usize,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

    let p_scroll = params.clone();
    let g_scroll = gui_context.clone();
    let m_scroll = mod_ui.clone();
    ScrollDropdown::new(
        cx,
        move |_cx, delta| {
            let cur = p_scroll.fx_bus_slots(bus_idx)[slot_idx].fx_type.value();
            let pos = FX_TYPE_ITEMS
                .iter()
                .position(|&(_, v)| v == cur)
                .unwrap_or(0);
            let next = (pos as isize + delta).clamp(0, FX_TYPE_ITEMS.len() as isize - 1) as usize;
            if next != pos {
                set_fx_slot_type_with_defaults(
                    &p_scroll,
                    &g_scroll,
                    &m_scroll,
                    bus_idx,
                    slot_idx,
                    FX_TYPE_ITEMS[next].1,
                );
            }
        },
        move |cx| {
            let p_trig = params.clone();
            let m_trig = mod_ui.clone();
            Dropdown::new(
                cx,
                move |cx| {
                    let text_lens = ParamWidgetBase::make_lens(
                        UiData::params,
                        move |p| &p.fx_bus_slots(bus_idx)[slot_idx].fx_type,
                        |param| {
                            param.normalized_value_to_string(
                                param.modulated_normalized_value(),
                                true,
                            )
                        },
                    );
                    let p_cmd = p_trig.clone();
                    let m_cmd = m_trig.clone();
                    FxHeaderCmdClickZone::new(
                        cx,
                        bus_idx,
                        slot_idx,
                        p_cmd,
                        m_cmd,
                        false,
                        move |cx| {
                            build_dropdown_trigger_labels(cx, text_lens);
                        },
                    )
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_top(Pixels(0.0))
                    .child_bottom(Pixels(0.0))
                    .overflow(Overflow::Hidden)
                },
                move |cx| {
                    for &(label, val) in FX_TYPE_ITEMS {
                        let p_ref = params.clone();
                        let gui_ctx = gui_context.clone();
                        let m_ref = mod_ui.clone();
                        bold_label(cx, label, "dropdown-item").on_press(move |cx| {
                            set_fx_slot_type_with_defaults(
                                &p_ref,
                                &gui_ctx,
                                &m_ref,
                                bus_idx,
                                slot_idx,
                                val,
                            );
                            cx.emit(PopupEvent::Close);
                        });
                    }
                },
            )
        },
    )
    .width(Stretch(1.0))
    .height(Pixels(tokens::FX_UNIT_HEADER_H));
}

fn build_fx_slot_header(
    cx: &mut Context,
    bus_idx: usize,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    let p_hl = params.clone();
    ZStack::new(cx, move |cx| {
        let p_hdr_cmd = params.clone();
        let m_hdr_cmd = mod_ui.clone();
        FxHeaderCmdClickZone::new(
            cx,
            bus_idx,
            slot_idx,
            p_hdr_cmd,
            m_hdr_cmd,
            true,
            move |cx| {

            let p_led_cmd = params.clone();
            let m_led_cmd = mod_ui.clone();
            LedToggleSwitch::new(
                cx,
                UiData::params,
                move |p| &p.fx_bus_slots(bus_idx)[slot_idx].enabled,
                "",
                "",
                AMBER_RGB,
            )
            .on_cmd_click(move |cx| {
                toggle_slot_collapse_if_active(cx, &p_led_cmd, &m_led_cmd, bus_idx, slot_idx);
            })
            .width(Pixels(tokens::FX_UNIT_LED_W))
            .height(Pixels(tokens::FX_UNIT_ARROW_H))
            .top(Stretch(1.0))
            .bottom(Stretch(1.0));

            build_fx_type_dropdown(
                cx,
                bus_idx,
                slot_idx,
                params.clone(),
                gui_context.clone(),
                mod_ui.clone(),
            );

            HStack::new(cx, move |cx| {
                build_inline_pill_cell(
                    cx,
                    UiData::params,
                    move |p| &p.fx_bus_slots(bus_idx)[slot_idx].stack,
                    "STK",
                    4,
                    1.0,
                )
                .class("stk-pill");
            })
            .width(Pixels(tokens::FX_UNIT_STACK_W))
            .height(Pixels(tokens::FX_UNIT_HEADER_H));

            let p_up = params.clone();
            let g_up = gui_context.clone();
            let m_up = mod_ui.clone();
            ArrowButton::new(
                cx,
                move |cx| {
                    if slot_idx > 0 {
                        swap_fx_slots(&p_up, g_up.as_ref(), &m_up, bus_idx, slot_idx, slot_idx - 1);
                        cx.emit(UiEvent::ModUiChanged);
                    }
                },
                ArrowDir::Up,
            )
            .class("fx-move-btn")
            .class("fx-move-label")
            .toggle_class("fx-move-disabled", UiData::mod_ui_rev.map(move |_| slot_idx == 0))
            .width(Pixels(tokens::FX_UNIT_ARROW_W))
            .height(Pixels(tokens::FX_UNIT_ARROW_H))
            .top(Stretch(1.0))
            .bottom(Stretch(1.0));

            let p_dn = params.clone();
            let g_dn = gui_context;
            let m_dn = mod_ui.clone();
            ArrowButton::new(
                cx,
                move |cx| {
                    if slot_idx < 7 {
                        swap_fx_slots(
                            &p_dn,
                            g_dn.as_ref(),
                            &m_dn,
                            bus_idx,
                            slot_idx,
                            slot_idx + 1,
                        );
                        cx.emit(UiEvent::ModUiChanged);
                    }
                },
                ArrowDir::Down,
            )
            .class("fx-move-btn")
            .class("fx-move-label")
            .toggle_class("fx-move-disabled", UiData::mod_ui_rev.map(move |_| slot_idx == 7))
            .width(Pixels(tokens::FX_UNIT_ARROW_W))
            .height(Pixels(tokens::FX_UNIT_ARROW_H))
            .top(Stretch(1.0))
            .bottom(Stretch(1.0));

            let p_col_press = params.clone();
            let m_col_press = mod_ui.clone();
            let p_col_state = params.clone();
            let m_col_state = mod_ui;
            CollapseButton::reactive(
                cx,
                move |cx| {
                    toggle_slot_collapse_if_active(
                        cx,
                        &p_col_press,
                        &m_col_press,
                        bus_idx,
                        slot_idx,
                    );
                },
                UiData::mod_ui_rev,
                move |_| {
                    let is_off = p_col_state.fx_bus_slots(bus_idx)[slot_idx].fx_type.value()
                        == FxTypeParam::Off;
                    is_off || m_col_state.fx_collapsed(bus_idx, slot_idx)
                },
            )
            .class("fx-move-btn")
            .class("fx-move-label")
            .toggle_class(
                "fx-move-disabled",
                UiData::params.map(move |p| {
                    p.fx_bus_slots(bus_idx)[slot_idx].fx_type.value() == FxTypeParam::Off
                }),
            )
            .width(Pixels(tokens::FX_UNIT_COLLAPSE_W))
            .height(Pixels(tokens::FX_UNIT_ARROW_H))
            .top(Stretch(1.0))
            .bottom(Stretch(1.0));
        })
        .col_between(Pixels(tokens::SPACE_XS))
        .height(Pixels(tokens::FX_UNIT_HEADER_H))
        .width(Stretch(1.0));

        FxHeaderCmdHighlight::new(cx, bus_idx, slot_idx, p_hl)
            .position_type(PositionType::SelfDirected)
            .left(Pixels(0.0))
            .top(Pixels(0.0))
            .width(Stretch(1.0))
            .height(Stretch(1.0));
    })
    .height(Pixels(tokens::FX_UNIT_HEADER_H))
    .width(Stretch(1.0));
}

struct JSynSubTabHost {
    bus_idx: usize,
    slot_idx: usize,
    mod_ui: Arc<ModUiState>,
}

impl FxSubTabHost for JSynSubTabHost {
    fn is_subtab(&self) -> bool {
        self.mod_ui.fx_eq_subtab(self.bus_idx, self.slot_idx)
    }
    fn set_subtab(&self, subtab: bool) {
        self.mod_ui
            .set_fx_eq_subtab(self.bus_idx, self.slot_idx, subtab);
    }
}

fn set_fx_bus_topology(
    params: &JSynPluginParams,
    gui_context: &Arc<dyn GuiContext>,
    topo: FxBusTopologyParam,
) {
    let setter = ParamSetter::new(gui_context.as_ref());
    setter.begin_set_parameter(&params.fx_bus_topology);
    setter.set_parameter(&params.fx_bus_topology, topo);
    setter.end_set_parameter(&params.fx_bus_topology);
}

#[inline]
fn lane_target_label(target: u8) -> &'static str {
    match target {
        1 => "-> B",
        2 => "-> C",
        _ => "-> MST",
    }
}

fn build_lane_route_dropdown(
    cx: &mut Context,
    bus_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
) {
    const BUS_A_TARGETS: &[(&str, u8)] = &[("-> MST", 0), ("-> B", 1), ("-> C", 2)];
    const BUS_B_TARGETS: &[(&str, u8)] = &[("-> MST", 0), ("-> C", 2)];

    let items: &'static [(&'static str, u8)] = if bus_idx == 0 {
        BUS_A_TARGETS
    } else {
        BUS_B_TARGETS
    };
    let cur_target = move |topo: FxBusTopology| -> u8 {
        if bus_idx == 0 {
            topo.bus_a_target()
        } else {
            topo.bus_b_target()
        }
    };
    let next_topo_for = move |topo: FxBusTopology, target: u8| -> FxBusTopologyParam {
        let next = if bus_idx == 0 {
            FxBusTopology::from_lane_targets(target, topo.bus_b_target())
        } else {
            FxBusTopology::from_lane_targets(topo.bus_a_target(), target)
        };
        FxBusTopologyParam::from(next)
    };

    let p_scroll = params.clone();
    let g_scroll = gui_context.clone();
    let p_trig = params.clone();
    let p_pop = params.clone();
    let g_pop = gui_context;

    ScrollDropdown::new(
        cx,
        move |cx, delta| {
            let cur: FxBusTopology = p_scroll.fx_bus_topology.value().into();
            let target = cur_target(cur);
            let pos = items.iter().position(|&(_, t)| t == target).unwrap_or(0);
            let next = (pos as isize + delta).clamp(0, items.len() as isize - 1) as usize;
            if next != pos {
                set_fx_bus_topology(&p_scroll, &g_scroll, next_topo_for(cur, items[next].1));
                cx.emit(UiEvent::ModUiChanged);
            }
        },
        move |cx| {
            let p_trig = p_trig.clone();
            let p_pop = p_pop.clone();
            let g_pop = g_pop.clone();
            Dropdown::new(
                cx,
                move |cx| {
                    let p_lbl = p_trig.clone();
                    HStack::new(cx, move |cx| {
                        build_dropdown_trigger_labels(
                            cx,
                            UiData::params.map(move |p| {
                                let topo: FxBusTopology = p.fx_bus_topology.value().into();
                                let _ = &p_lbl;
                                lane_target_label(cur_target(topo)).to_string()
                            }),
                        );
                    })
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_top(Pixels(0.0))
                    .child_bottom(Pixels(0.0))
                    .overflow(Overflow::Hidden)
                },
                move |cx| {
                    for &(label, target) in items {
                        let p_set = p_pop.clone();
                        let g_set = g_pop.clone();
                        bold_label(cx, label, "dropdown-item").on_press(move |cx| {
                            let cur: FxBusTopology = p_set.fx_bus_topology.value().into();
                            set_fx_bus_topology(&p_set, &g_set, next_topo_for(cur, target));
                            cx.emit(UiEvent::ModUiChanged);
                            cx.emit(PopupEvent::Close);
                        });
                    }
                },
            )
        },
    )
    .toggle_class(
        "routing-pill-active",
        UiData::params.map(move |p| {
            let topo: FxBusTopology = p.fx_bus_topology.value().into();
            cur_target(topo) != 0
        }),
    )
    .width(Stretch(1.0))
    .height(Pixels(tokens::PILL_H));
}

fn build_lane_header(
    cx: &mut Context,
    bus_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    let title = match bus_idx {
        0 => "BUS A",
        1 => "BUS B",
        _ => "BUS C",
    };
    let (lvl_dest, pan_dest) = match bus_idx {
        0 => (ModDestination::BusALevel, ModDestination::BusAPan),
        1 => (ModDestination::BusBLevel, ModDestination::BusBPan),
        _ => (ModDestination::BusCLevel, ModDestination::BusCPan),
    };

    HStack::new(cx, move |cx| {
        let p_hdr = params.clone();
        let g_hdr = gui_context;
        VStack::new(cx, move |cx| {
            bold_label(cx, title, "module-title")
                .width(Stretch(1.0))
                .height(Pixels(tokens::MACRO_HDR_H))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0));

            if bus_idx < 2 {
                build_lane_route_dropdown(cx, bus_idx, p_hdr, g_hdr);
            } else {
                bold_label(cx, "-> MST", "mini-label")
                    .width(Stretch(1.0))
                    .height(Pixels(tokens::PILL_H))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0));
            }
        })
        .row_between(Pixels(tokens::SPACE_SM))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .width(Pixels(tokens::FX_LANE_ROUTE_COL_W))
        .height(Stretch(1.0));

        Element::new(cx).width(Stretch(1.0));

        let m_lvl = mod_ui.clone();
        HStack::new(cx, move |cx| {
            build_flex_knob_cell_mod(
                cx,
                UiData::params,
                move |p| p.fx_bus_level_param(bus_idx),
                "LEVEL",
                false,
                AMBER_RGB,
                tokens::KNOB_OSC,
                lvl_dest,
                m_lvl,
                0,
            );

            build_flex_knob_cell_mod(
                cx,
                UiData::params,
                move |p| p.fx_bus_pan_param(bus_idx),
                "PAN",
                true,
                GOLD_RGB,
                tokens::KNOB_OSC,
                pan_dest,
                mod_ui,
                0,
            );
        })
        .col_between(Pixels(tokens::SPACE_SM))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .width(Pixels(tokens::FX_LANE_KNOBS_W))
        .height(Stretch(1.0));
    })
    .class("fx-bus-header")
    .col_between(Pixels(tokens::SPACE_SM))
    .height(Pixels(tokens::FX_LANE_HEADER_H))
    .width(Stretch(1.0));
}

fn build_fx_unit_card(
    cx: &mut Context,
    bus_idx: usize,
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    let p_dim = params.clone();
    VStack::new(cx, move |cx| {
        build_fx_slot_header(
            cx,
            bus_idx,
            slot_idx,
            params.clone(),
            gui_context.clone(),
            mod_ui.clone(),
        );

        let p_body = params.clone();
        let g_body = gui_context.clone();
        let m_body = mod_ui.clone();
        let m_col_key = mod_ui.clone();
        Binding::new(
            cx,
            UiData::root.map(move |data| {
                (
                    data.params.fx_bus_slots(bus_idx)[slot_idx].fx_type.value() as usize,
                    m_col_key.fx_collapsed(bus_idx, slot_idx),
                )
            }),
            move |cx, slot_view_lens| {
                let (fx_idx, is_collapsed) = slot_view_lens.get(cx);
                let fx_type = FxType::from_index(fx_idx);
                if fx_type == FxType::Off || is_collapsed {
                    return;
                }
                let p_unit = p_body.clone();
                let g_unit = g_body.clone();
                let m_unit = m_body.clone();
                if fx_type == FxType::Utility {
                    #[derive(Clone, Copy)]
                    struct JSynUtilitySlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl UtilityUnitParams<JSynPluginParams> for JSynUtilitySlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn width<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p1
                        }
                        fn pan<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p2
                        }
                        fn gain<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p3
                        }
                        fn mix<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p4
                        }
                        fn inv_l<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].utility_inv_l
                        }
                        fn inv_r<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].utility_inv_r
                        }
                    }
                    let mod_dests: [ModDestination; 4] =
                        std::array::from_fn(|k| ModDestination::fx_bus_slot_param(bus_idx, slot_idx, k));
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    build_utility_fx_unit_body(
                        cx,
                        UiData::params,
                        p_unit,
                        g_unit,
                        JSynUtilitySlotParams { bus_idx, slot_idx },
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                    return;
                }
                if fx_type == FxType::JSatClipper {
                    #[derive(Clone, Copy)]
                    struct JSynClipperSlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl ClipperUnitParams<JSynPluginParams> for JSynClipperSlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn pcm<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].clipper_pcm
                        }
                        fn param<'a>(&self, p: &'a JSynPluginParams, idx: usize) -> &'a FloatParam {
                            p.fx_bus_slots(self.bus_idx)[self.slot_idx].param_at(idx)
                        }
                    }
                    let mod_dests: [ModDestination; 4] =
                        std::array::from_fn(|k| ModDestination::fx_bus_slot_param(bus_idx, slot_idx, k));
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    build_clipper_fx_unit_body(
                        cx,
                        UiData::params,
                        p_unit,
                        g_unit,
                        JSynClipperSlotParams { bus_idx, slot_idx },
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                    return;
                }
                if matches!(fx_type, FxType::Ott | FxType::Phaser) {
                    #[derive(Clone, Copy)]
                    struct JSynFourKnobSlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl FourKnobUnitParams<JSynPluginParams> for JSynFourKnobSlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn param<'a>(&self, p: &'a JSynPluginParams, idx: usize) -> &'a FloatParam {
                            p.fx_bus_slots(self.bus_idx)[self.slot_idx].param_at(idx)
                        }
                    }
                    let mod_dests: [ModDestination; 4] =
                        std::array::from_fn(|k| ModDestination::fx_bus_slot_param(bus_idx, slot_idx, k));
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    let bipolar = if fx_type == FxType::Ott {
                        [false, false, true, true]
                    } else {
                        [false; 4]
                    };
                    build_four_knob_fx_unit_body(
                        cx,
                        fx_type,
                        bipolar,
                        UiData::params,
                        p_unit,
                        JSynFourKnobSlotParams { bus_idx, slot_idx },
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                    return;
                }
                if fx_type == FxType::Distortion {
                    #[derive(Clone, Copy)]
                    struct JSynDistortionSlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl DistortionUnitParams<JSynPluginParams> for JSynDistortionSlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn mode<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                        ) -> &'a EnumParam<DistortionModeParam> {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].distortion_mode
                        }
                        fn param<'a>(&self, p: &'a JSynPluginParams, idx: usize) -> &'a FloatParam {
                            p.fx_bus_slots(self.bus_idx)[self.slot_idx].param_at(idx)
                        }
                    }
                    let mod_dests: [ModDestination; 4] =
                        std::array::from_fn(|k| ModDestination::fx_bus_slot_param(bus_idx, slot_idx, k));
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    build_distortion_fx_unit_body(
                        cx,
                        UiData::params,
                        p_unit,
                        g_unit,
                        JSynDistortionSlotParams { bus_idx, slot_idx },
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                    return;
                }
                if fx_type == FxType::Diopser {
                    #[derive(Clone, Copy)]
                    struct JSynDiopserSlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl DiopserUnitParams<JSynPluginParams> for JSynDiopserSlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn stages<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                        ) -> &'a EnumParam<DiopserStagesParam> {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].diopser_stages
                        }
                        fn param<'a>(&self, p: &'a JSynPluginParams, idx: usize) -> &'a FloatParam {
                            p.fx_bus_slots(self.bus_idx)[self.slot_idx].param_at(idx)
                        }
                    }
                    let mod_dests: [ModDestination; 4] =
                        std::array::from_fn(|k| ModDestination::fx_bus_slot_param(bus_idx, slot_idx, k));
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    build_diopser_fx_unit_body(
                        cx,
                        UiData::params,
                        p_unit,
                        g_unit,
                        JSynDiopserSlotParams { bus_idx, slot_idx },
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                    return;
                }
                if matches!(
                    fx_type,
                    FxType::Chorus | FxType::Delay | FxType::Reverb | FxType::Flanger
                ) {
                    #[derive(Clone, Copy)]
                    struct JSynEightKnobSlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl EightKnobUnitParams<JSynPluginParams> for JSynEightKnobSlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn param<'a>(&self, p: &'a JSynPluginParams, idx: usize) -> &'a FloatParam {
                            p.fx_bus_slots(self.bus_idx)[self.slot_idx].param_at(idx)
                        }
                    }
                    let mod_dests: [ModDestination; 8] =
                        std::array::from_fn(|k| ModDestination::fx_bus_slot_param(bus_idx, slot_idx, k));
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    let tab_host = Arc::new(JSynSubTabHost {
                        bus_idx,
                        slot_idx,
                        mod_ui: m_unit.clone(),
                    });
                    let (sub_lbl, sub_bipolar) = if fx_type == FxType::Flanger {
                        ("MOD", [true, false, false, false])
                    } else {
                        ("EQ", [false; 4])
                    };
                    build_tabbed_fx_unit_body(
                        cx,
                        fx_type,
                        sub_lbl,
                        sub_bipolar,
                        UiData::params,
                        UiData::mod_ui_rev,
                        p_unit,
                        JSynEightKnobSlotParams { bus_idx, slot_idx },
                        tab_host,
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                    return;
                }
                if fx_type == FxType::Limiter {
                    #[derive(Clone, Copy)]
                    struct JSynLimiterSlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl LimiterUnitParams<JSynPluginParams> for JSynLimiterSlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn true_peak<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].limiter_true_peak
                        }
                        fn param<'a>(&self, p: &'a JSynPluginParams, idx: usize) -> &'a FloatParam {
                            p.fx_bus_slots(self.bus_idx)[self.slot_idx].param_at(idx)
                        }
                    }
                    let mod_dests: [ModDestination; 8] =
                        std::array::from_fn(|k| ModDestination::fx_bus_slot_param(bus_idx, slot_idx, k));
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    let tab_host = Arc::new(JSynSubTabHost {
                        bus_idx,
                        slot_idx,
                        mod_ui: m_unit.clone(),
                    });
                    build_limiter_fx_unit_body(
                        cx,
                        UiData::params,
                        UiData::mod_ui_rev,
                        p_unit,
                        g_unit,
                        JSynLimiterSlotParams { bus_idx, slot_idx },
                        tab_host,
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                    return;
                }
                if fx_type == FxType::Filter {
                    #[derive(Clone, Copy)]
                    struct JSynFilterSlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl FilterUnitParams<JSynPluginParams> for JSynFilterSlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn mode<'a>(&self, p: &'a JSynPluginParams) -> &'a EnumParam<FilterModeParam> {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].filter_mode
                        }
                        fn slope<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                        ) -> &'a EnumParam<FxFilterSlopeParam> {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].filter_slope
                        }
                        fn param<'a>(&self, p: &'a JSynPluginParams, idx: usize) -> &'a FloatParam {
                            p.fx_bus_slots(self.bus_idx)[self.slot_idx].param_at(idx)
                        }
                        fn freq2<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p5
                        }
                    }
                    let mod_dests: [ModDestination; 5] =
                        std::array::from_fn(|k| ModDestination::fx_bus_slot_param(bus_idx, slot_idx, k));
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    build_filter_fx_unit_body(
                        cx,
                        UiData::params,
                        p_unit,
                        g_unit,
                        JSynFilterSlotParams { bus_idx, slot_idx },
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                    return;
                }
                if fx_type == FxType::TubeResonator {
                    #[derive(Clone, Copy)]
                    struct JSynTubeResSlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl TubeResonatorUnitParams<JSynPluginParams> for JSynTubeResSlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn filter_enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].tube_res_filter_enabled
                        }
                        fn dirty_lfo<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].tube_res_dirty_lfo
                        }
                        fn param<'a>(&self, p: &'a JSynPluginParams, idx: usize) -> &'a FloatParam {
                            p.fx_bus_slots(self.bus_idx)[self.slot_idx].param_at(idx)
                        }
                    }
                    let mod_dests: [ModDestination; 8] =
                        std::array::from_fn(|k| ModDestination::fx_bus_slot_param(bus_idx, slot_idx, k));
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    let tab_host = Arc::new(JSynSubTabHost {
                        bus_idx,
                        slot_idx,
                        mod_ui: m_unit.clone(),
                    });
                    build_tube_res_fx_unit_body(
                        cx,
                        UiData::params,
                        UiData::mod_ui_rev,
                        p_unit,
                        g_unit,
                        JSynTubeResSlotParams { bus_idx, slot_idx },
                        tab_host,
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                    return;
                }
                if fx_type == FxType::MultibandDist {
                    #[derive(Clone, Copy)]
                    struct JSynMbDistSlotParams {
                        bus_idx: usize,
                        slot_idx: usize,
                    }
                    impl MultibandDistUnitParams<JSynPluginParams> for JSynMbDistSlotParams {
                        fn enabled<'a>(&self, p: &'a JSynPluginParams) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].enabled
                        }
                        fn low_xover<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p1
                        }
                        fn high_xover<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p2
                        }
                        fn out_gain<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p3
                        }
                        fn mix<'a>(&self, p: &'a JSynPluginParams) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].p4
                        }
                        fn band_enabled<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].enabled
                        }
                        fn band_solo<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a BoolParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].solo
                        }
                        fn band_mode<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a EnumParam<DistortionModeParam> {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].mode
                        }
                        fn band_drive<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].drive
                        }
                        fn band_bias<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].bias
                        }
                        fn band_level<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].level
                        }
                        fn band_mix<'a>(
                            &self,
                            p: &'a JSynPluginParams,
                            band: usize,
                        ) -> &'a FloatParam {
                            &p.fx_bus_slots(self.bus_idx)[self.slot_idx].mb_bands[band].mix
                        }
                    }
                    struct JSynMbTabHost {
                        bus_idx: usize,
                        slot_idx: usize,
                        mod_ui: Arc<ModUiState>,
                    }
                    impl MultibandTabHost for JSynMbTabHost {
                        fn mb_tab(&self) -> u8 {
                            self.mod_ui.fx_mb_tab(self.bus_idx, self.slot_idx)
                        }
                        fn set_mb_tab(&self, tab: u8) {
                            self.mod_ui.set_fx_mb_tab(self.bus_idx, self.slot_idx, tab);
                        }
                    }
                    let mod_dests: [ModDestination; 16] =
                        std::array::from_fn(|k| ModDestination::fx_bus_slot_param(bus_idx, slot_idx, k));
                    let p_scope = p_unit.clone();
                    let m_scope = m_unit.clone();
                    let tab_host = Arc::new(JSynMbTabHost {
                        bus_idx,
                        slot_idx,
                        mod_ui: m_unit.clone(),
                    });
                    build_multiband_dist_fx_unit_body(
                        cx,
                        UiData::params,
                        UiData::mod_ui_rev,
                        p_unit,
                        g_unit,
                        JSynMbDistSlotParams { bus_idx, slot_idx },
                        tab_host,
                        Some((mod_dests, m_unit, 0)),
                        move || slot_scope_snapshot(bus_idx, slot_idx, &p_scope, &m_scope),
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                }
            },
        );
    })
    .class("hardware-module")
    .class("fx-full-card")
    .toggle_class(
        "module-disabled",
        UiData::mod_ui_rev.map(move |_| !slot_enabled_for_bus(&p_dim, bus_idx, slot_idx)),
    )
    .width(Pixels(tokens::FX_UNIT_W))
    .height(Auto)
    .row_between(Pixels(tokens::SPACE_SM));
}

/// Right Wing: 3-lane scrollable FX Rack (`Bus A`, `Bus B`, `Bus C`, 8 slots each).
pub(crate) fn build_three_lane_fx_rack(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    HStack::new(cx, move |cx| {
        for bus_idx in 0..3 {
            let p_lane = params.clone();
            let g_lane = gui_context.clone();
            let m_lane = mod_ui.clone();
            VStack::new(cx, move |cx| {
                build_lane_header(
                    cx,
                    bus_idx,
                    p_lane.clone(),
                    g_lane.clone(),
                    m_lane.clone(),
                );

                ScrollView::new(cx, 0.0, 0.0, false, true, move |cx| {
                    FxLaneScrollGuard::new(cx, move |cx| {
                        for slot_idx in 0..8 {
                            build_fx_unit_card(
                                cx,
                                bus_idx,
                                slot_idx,
                                p_lane.clone(),
                                g_lane.clone(),
                                m_lane.clone(),
                            );
                        }
                    })
                    .row_between(Pixels(tokens::SPACE_SM))
                    .child_right(Pixels(tokens::FX_LANE_SCROLL_GUTTER))
                    .width(Stretch(1.0))
                    .height(Auto);
                })
                .width(Stretch(1.0))
                .height(Stretch(1.0));
            })
            .row_between(Pixels(tokens::SPACE_SM))
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        }
    })
    .col_between(Pixels(tokens::SPACE_MD))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}
