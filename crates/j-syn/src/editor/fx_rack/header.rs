//! FX Unit card header bar, type dropdown, reorder buttons, and Cmd/Ctrl+Click collapse zones.

use std::sync::Arc;

use j_syn_ui::widgets::{LedToggleSwitch, LedToggleSwitchExt};
use nih_plug::prelude::{GuiContext, Param};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::util::ModifiersExt;

use super::super::controls::{
    bold_label, build_dropdown_trigger_labels, build_inline_pill_cell, ArrowButton,
    ArrowButtonExt, ArrowDir, CollapseButton, PopupScrollCapture, ScrollDropdown,
};
use super::super::mod_bar::ModUiState;
use super::super::{tokens, UiData, UiEvent};
use super::slot_ops::{set_fx_slot_type_with_defaults, swap_fx_slots, FX_TYPE_ITEMS};
use crate::{FxTypeParam, JSynPluginParams};

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
    suppress_next_release: bool,
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
            suppress_next_release: false,
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
            if self.direct_only && meta.target != cx.current() {
                return;
            }
            let b = cx.bounds();
            let mx = cx.mouse().cursorx;
            let my = cx.mouse().cursory;
            let in_bounds = mx >= b.x && mx <= b.x + b.w && my >= b.y && my <= b.y + b.h;
            match window_event {
                WindowEvent::MouseDown(MouseButton::Left)
                | WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                    if cmd_or_ctrl && in_bounds {
                        self.suppress_next_release = true;
                        toggle_slot_collapse_if_active(
                            cx,
                            &self.params,
                            &self.mod_ui,
                            self.bus_idx,
                            self.slot_idx,
                        );
                        if !self.direct_only {
                            cx.emit(PopupEvent::Close);
                        }
                        meta.consume();
                    } else {
                        self.suppress_next_release = false;
                    }
                }
                WindowEvent::MouseUp(MouseButton::Left)
                | WindowEvent::Press { .. }
                | WindowEvent::PressDown { .. }
                    if !self.direct_only && (cmd_or_ctrl || self.suppress_next_release) =>
                {
                    cx.emit(PopupEvent::Close);
                    meta.consume();
                }
                _ => {}
            }
        });
    }
}

/// Scoped column container inside each FX lane `ScrollView` that observes unconsumed `MouseScroll`
/// events bubbling up to `ScrollView` and arms the `250 ms` / `≥ 2 px` pointer scroll-wheel guard.
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
                    PopupScrollCapture::new(cx, |cx| {
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
                    });
                },
            )
        },
    )
    .width(Stretch(1.0))
    .height(Pixels(tokens::FX_UNIT_HEADER_H));
}

pub(super) fn build_fx_slot_header(
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
            let p_up_cmd = params.clone();
            let m_up_cmd = mod_ui.clone();
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
            .on_cmd_click(move |cx| {
                toggle_slot_collapse_if_active(cx, &p_up_cmd, &m_up_cmd, bus_idx, slot_idx);
            })
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
            let p_dn_cmd = params.clone();
            let m_dn_cmd = mod_ui.clone();
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
            .on_cmd_click(move |cx| {
                toggle_slot_collapse_if_active(cx, &p_dn_cmd, &m_dn_cmd, bus_idx, slot_idx);
            })
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
