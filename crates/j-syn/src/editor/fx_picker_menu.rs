//! Top-level floating overlays for the categorized 2-column FX Picker,
//! FX Unit right-click context menu, and FX Lane header context menu.

use std::sync::Arc;

use nih_plug::prelude::GuiContext;
use nih_plug_vizia::vizia::prelude::*;

use super::controls::bold_label;
use super::fx_rack::slot_ops::{
    FX_PICKER_LEFT_COL, FX_PICKER_RIGHT_COL, FxPickerCategory, add_fx_at, clear_lane,
    duplicate_fx_unit, lane_name, move_fx_across_lanes, remove_fx_slot, replace_fx_slot,
    reset_fx_slot_defaults, set_lane_all_bypassed,
};
use super::mod_bar::ModUiState;
use super::{BASE_HEIGHT, BASE_WIDTH, UiData, UiEvent, tokens};
use crate::JSynPluginParams;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FxPickerTarget {
    InsertAt(usize),
    ReplaceSlot(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum FxOverlayMenuKind {
    #[default]
    None,
    Picker {
        bus: usize,
        target: FxPickerTarget,
    },
    UnitContext {
        bus: usize,
        pos: usize,
        slot: usize,
    },
    LaneContext {
        bus: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct FxOverlayMenuState {
    pub kind: FxOverlayMenuKind,
    pub anchor_x: f32,
    pub anchor_y: f32,
    pub rev: u64,
}

const OUTER_CHROME_H: f32 = 14.0; // 6px top + 6px bottom padding + 2x 1px border
const OUTER_ROW_GAP: f32 = 2.0;
const HEADER_H: f32 = 18.0;
const SEP_H: f32 = 5.0; // 1px line + 2px top + 2px bottom
const CAT_HEADER_H: f32 = 18.0; // generous headroom for Plus Jakarta Sans Bold ascenders
const CAT_TOP_GAP: f32 = 4.0; // extra spacing above categories after the first
const PICKER_ITEM_H: f32 = 21.0;
const COL_ROW_GAP: f32 = 3.0;
const CTX_ITEM_H: f32 = 20.0;
const HEIGHT_SAFETY_CUSHION: f32 = 6.0;
const VIEWPORT_MARGIN_X: f32 = 8.0;
const VIEWPORT_MARGIN_BOTTOM: f32 = 10.0;
const UNIT_CTX_ITEM_COUNT: usize = 10;
const UNIT_CTX_SEP_COUNT: usize = 3;
const LANE_CTX_ITEM_COUNT: usize = 6;
const LANE_CTX_SEP_COUNT: usize = 3;

#[inline]
fn picker_column_height(categories: &[FxPickerCategory]) -> f32 {
    let num_cats = categories.len();
    let num_items: usize = categories.iter().map(|c| c.items.len()).sum();
    let total_rows = num_cats + num_items;
    let gaps = total_rows.saturating_sub(1) as f32 * COL_ROW_GAP;
    let inter_cat_pad = num_cats.saturating_sub(1) as f32 * CAT_TOP_GAP;
    num_cats as f32 * CAT_HEADER_H + num_items as f32 * PICKER_ITEM_H + gaps + inter_cat_pad
}

#[inline]
fn stacked_ctx_height(num_items: usize, num_seps: usize) -> f32 {
    let total_children = 1 + num_seps + num_items;
    let gaps = total_children.saturating_sub(1) as f32 * OUTER_ROW_GAP;
    OUTER_CHROME_H + HEADER_H + num_seps as f32 * SEP_H + num_items as f32 * CTX_ITEM_H + gaps
}

impl FxOverlayMenuState {
    #[inline]
    pub fn is_visible(&self) -> bool {
        !matches!(self.kind, FxOverlayMenuKind::None)
    }

    #[inline]
    fn menu_width(&self) -> f32 {
        match self.kind {
            FxOverlayMenuKind::Picker { .. } => tokens::FX_PICKER_W,
            _ => tokens::FX_CTX_MENU_W,
        }
    }

    #[inline]
    fn exact_height(&self) -> f32 {
        match self.kind {
            FxOverlayMenuKind::Picker { .. } => {
                let left_h = picker_column_height(FX_PICKER_LEFT_COL);
                let right_h = picker_column_height(FX_PICKER_RIGHT_COL);
                OUTER_CHROME_H + HEADER_H + SEP_H + 2.0 * OUTER_ROW_GAP + left_h.max(right_h)
            }
            FxOverlayMenuKind::UnitContext { .. } => {
                stacked_ctx_height(UNIT_CTX_ITEM_COUNT, UNIT_CTX_SEP_COUNT)
            }
            FxOverlayMenuKind::LaneContext { .. } => {
                stacked_ctx_height(LANE_CTX_ITEM_COUNT, LANE_CTX_SEP_COUNT)
            }
            FxOverlayMenuKind::None => 0.0,
        }
    }

    #[inline]
    fn est_height(&self) -> f32 {
        let exact = self.exact_height();
        if exact > 0.0 {
            exact + HEIGHT_SAFETY_CUSHION
        } else {
            0.0
        }
    }

    #[inline]
    fn clamped_left(&self) -> f32 {
        let w = self.menu_width();
        self.anchor_x
            .clamp(6.0, (BASE_WIDTH as f32 - w - VIEWPORT_MARGIN_X).max(6.0))
    }

    #[inline]
    fn clamped_top(&self) -> f32 {
        let h = self.est_height();
        self.anchor_y.clamp(
            6.0,
            (BASE_HEIGHT as f32 - h - VIEWPORT_MARGIN_BOTTOM).max(6.0),
        )
    }
}

impl Data for FxOverlayMenuState {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

fn build_picker_column(
    cx: &mut Context,
    categories: &'static [FxPickerCategory],
    bus: usize,
    target: FxPickerTarget,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    VStack::new(cx, move |cx| {
        for (cat_idx, cat) in categories.iter().enumerate() {
            let mut hdr = bold_label(cx, cat.title, "fx-picker-cat-header")
                .width(Stretch(1.0))
                .height(Pixels(CAT_HEADER_H))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0));
            if cat_idx > 0 {
                hdr = hdr.top(Pixels(CAT_TOP_GAP));
            }
            let _ = hdr;

            for &(label, fx_ty) in cat.items {
                let p = params.clone();
                let g = gui_context.clone();
                let m = mod_ui.clone();
                Button::new(
                    cx,
                    move |cx| {
                        match target {
                            FxPickerTarget::InsertAt(pos) => {
                                let _ = add_fx_at(&p, g.as_ref(), &m, bus, pos, fx_ty);
                            }
                            FxPickerTarget::ReplaceSlot(slot) => {
                                replace_fx_slot(&p, g.as_ref(), &m, bus, slot, fx_ty);
                            }
                        }
                        cx.emit(UiEvent::HideFxOverlayMenu);
                        cx.emit(UiEvent::ModUiChanged);
                    },
                    move |cx| {
                        bold_label(cx, label, "fx-picker-item-text")
                            .width(Stretch(1.0))
                            .height(Stretch(1.0))
                            .child_top(Stretch(1.0))
                            .child_bottom(Stretch(1.0))
                            .hoverable(false)
                    },
                )
                .class("fx-picker-item")
                .width(Stretch(1.0))
                .height(Pixels(PICKER_ITEM_H));
            }
        }
    })
    .row_between(Pixels(COL_ROW_GAP))
    .width(Stretch(1.0))
    .height(Auto);
}

fn build_ctx_item<F>(cx: &mut Context, label: &str, danger: bool, on_press: F)
where
    F: Fn(&mut EventContext) + 'static,
{
    let lbl = label.to_string();
    Button::new(cx, on_press, move |cx| {
        bold_label(cx, &lbl, "fx-ctx-item-text")
            .width(Stretch(1.0))
            .height(Stretch(1.0))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0))
            .hoverable(false)
    })
    .class(if danger {
        "fx-ctx-item-danger"
    } else {
        "fx-ctx-item"
    })
    .width(Stretch(1.0))
    .height(Pixels(CTX_ITEM_H));
}

fn build_ctx_sep(cx: &mut Context) {
    Element::new(cx)
        .class("fx-ctx-sep")
        .width(Stretch(1.0))
        .height(Pixels(1.0))
        .top(Pixels(2.0))
        .bottom(Pixels(2.0));
}

pub(crate) fn build_fx_overlay_menu(
    cx: &mut Context,
    mod_ui: Arc<ModUiState>,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
) {
    // Click-away transparent backdrop
    Element::new(cx)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(0.0))
        .top(Pixels(0.0))
        .width(Stretch(1.0))
        .height(Stretch(1.0))
        .z_index(355)
        .display(UiData::fx_menu.map(FxOverlayMenuState::is_visible))
        .on_press(|cx| {
            cx.emit(UiEvent::HideFxOverlayMenu);
        });

    let p_root = params;
    let g_root = gui_context;
    let m_root = mod_ui;

    VStack::new(cx, move |cx| {
        let p_bind = p_root.clone();
        let g_bind = g_root.clone();
        let m_bind = m_root.clone();
        Binding::new(cx, UiData::fx_menu, move |cx, menu_lens| {
            let menu = menu_lens.get(cx);
            match menu.kind {
                FxOverlayMenuKind::None => {}
                FxOverlayMenuKind::Picker { bus, target } => {
                    let header = match target {
                        FxPickerTarget::InsertAt(_) => {
                            format!("ADD EFFECT  --  {}", lane_name(bus))
                        }
                        FxPickerTarget::ReplaceSlot(_) => {
                            format!("REPLACE EFFECT  --  {}", lane_name(bus))
                        }
                    };
                    bold_label(cx, &header, "mod-context-header")
                        .width(Stretch(1.0))
                        .height(Pixels(HEADER_H))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0));
                    build_ctx_sep(cx);

                    let p_col = p_bind.clone();
                    let g_col = g_bind.clone();
                    let m_col = m_bind.clone();
                    HStack::new(cx, move |cx| {
                        build_picker_column(
                            cx,
                            FX_PICKER_LEFT_COL,
                            bus,
                            target,
                            p_col.clone(),
                            g_col.clone(),
                            m_col.clone(),
                        );
                        build_picker_column(
                            cx,
                            FX_PICKER_RIGHT_COL,
                            bus,
                            target,
                            p_col,
                            g_col,
                            m_col,
                        );
                    })
                    .col_between(Pixels(8.0))
                    .width(Stretch(1.0))
                    .height(Auto);
                }
                FxOverlayMenuKind::UnitContext { bus, pos, slot } => {
                    let fx_ty =
                        j_syn_dsp::FxType::from(p_bind.fx_bus_slots(bus)[slot].fx_type.value());
                    let hdr = format!("{}  ({})", fx_ty.label().to_uppercase(), lane_name(bus));
                    bold_label(cx, &hdr, "mod-context-header")
                        .width(Stretch(1.0))
                        .height(Pixels(HEADER_H))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0));
                    build_ctx_sep(cx);

                    let p_dup = p_bind.clone();
                    let g_dup = g_bind.clone();
                    let m_dup = m_bind.clone();
                    build_ctx_item(cx, "Duplicate", false, move |cx| {
                        let _ = duplicate_fx_unit(
                            &p_dup,
                            g_dup.as_ref(),
                            &m_dup,
                            bus,
                            slot,
                            bus,
                            pos + 1,
                        );
                        cx.emit(UiEvent::HideFxOverlayMenu);
                        cx.emit(UiEvent::ModUiChanged);
                    });

                    let ax = menu.anchor_x;
                    let ay = menu.anchor_y;
                    build_ctx_item(cx, "Replace Effect...", false, move |cx| {
                        cx.emit(UiEvent::ShowFxPicker(
                            bus,
                            FxPickerTarget::ReplaceSlot(slot),
                            ax,
                            ay,
                        ));
                    });
                    build_ctx_item(cx, "Insert Above...", false, move |cx| {
                        cx.emit(UiEvent::ShowFxPicker(
                            bus,
                            FxPickerTarget::InsertAt(pos),
                            ax,
                            ay,
                        ));
                    });
                    build_ctx_item(cx, "Insert Below...", false, move |cx| {
                        cx.emit(UiEvent::ShowFxPicker(
                            bus,
                            FxPickerTarget::InsertAt(pos + 1),
                            ax,
                            ay,
                        ));
                    });

                    let p_rst = p_bind.clone();
                    let g_rst = g_bind.clone();
                    let m_rst = m_bind.clone();
                    build_ctx_item(cx, "Reset to Default", false, move |cx| {
                        reset_fx_slot_defaults(&p_rst, g_rst.as_ref(), &m_rst, bus, slot);
                        cx.emit(UiEvent::HideFxOverlayMenu);
                        cx.emit(UiEvent::ModUiChanged);
                    });

                    build_ctx_sep(cx);

                    for target_bus in 0..3 {
                        if target_bus == bus {
                            continue;
                        }
                        let p_mv = p_bind.clone();
                        let g_mv = g_bind.clone();
                        let m_mv = m_bind.clone();
                        let lbl = format!("Move to {}", lane_name(target_bus));
                        build_ctx_item(cx, &lbl, false, move |cx| {
                            let end_pos = p_mv.reconciled_fx_rack_order().len(target_bus);
                            let _ = move_fx_across_lanes(
                                &p_mv,
                                g_mv.as_ref(),
                                &m_mv,
                                bus,
                                slot,
                                target_bus,
                                end_pos,
                            );
                            cx.emit(UiEvent::HideFxOverlayMenu);
                            cx.emit(UiEvent::ModUiChanged);
                        });
                    }

                    let m_ca = m_bind.clone();
                    build_ctx_item(cx, "Collapse All in Lane", false, move |cx| {
                        m_ca.set_lane_collapsed(bus, true);
                        cx.emit(UiEvent::HideFxOverlayMenu);
                        cx.emit(UiEvent::ModUiChanged);
                    });
                    let m_ea = m_bind.clone();
                    build_ctx_item(cx, "Expand All in Lane", false, move |cx| {
                        m_ea.set_lane_collapsed(bus, false);
                        cx.emit(UiEvent::HideFxOverlayMenu);
                        cx.emit(UiEvent::ModUiChanged);
                    });

                    build_ctx_sep(cx);

                    let p_del = p_bind.clone();
                    let g_del = g_bind.clone();
                    let m_del = m_bind.clone();
                    build_ctx_item(cx, "Delete", true, move |cx| {
                        remove_fx_slot(&p_del, g_del.as_ref(), &m_del, bus, slot);
                        cx.emit(UiEvent::HideFxOverlayMenu);
                        cx.emit(UiEvent::ModUiChanged);
                    });
                }
                FxOverlayMenuKind::LaneContext { bus } => {
                    let hdr = format!("{}  ACTIONS", lane_name(bus));
                    bold_label(cx, &hdr, "mod-context-header")
                        .width(Stretch(1.0))
                        .height(Pixels(HEADER_H))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0));
                    build_ctx_sep(cx);

                    let ax = menu.anchor_x;
                    let ay = menu.anchor_y;
                    let p_add = p_bind.clone();
                    build_ctx_item(cx, "Add Effect...", false, move |cx| {
                        let end_pos = p_add.reconciled_fx_rack_order().len(bus);
                        cx.emit(UiEvent::ShowFxPicker(
                            bus,
                            FxPickerTarget::InsertAt(end_pos),
                            ax,
                            ay,
                        ));
                    });

                    build_ctx_sep(cx);

                    let m_ca = m_bind.clone();
                    build_ctx_item(cx, "Collapse All", false, move |cx| {
                        m_ca.set_lane_collapsed(bus, true);
                        cx.emit(UiEvent::HideFxOverlayMenu);
                        cx.emit(UiEvent::ModUiChanged);
                    });
                    let m_ea = m_bind.clone();
                    build_ctx_item(cx, "Expand All", false, move |cx| {
                        m_ea.set_lane_collapsed(bus, false);
                        cx.emit(UiEvent::HideFxOverlayMenu);
                        cx.emit(UiEvent::ModUiChanged);
                    });

                    let p_byp = p_bind.clone();
                    let g_byp = g_bind.clone();
                    build_ctx_item(cx, "Bypass All FX", false, move |cx| {
                        set_lane_all_bypassed(&p_byp, g_byp.as_ref(), bus, true);
                        cx.emit(UiEvent::HideFxOverlayMenu);
                        cx.emit(UiEvent::ModUiChanged);
                    });
                    let p_en = p_bind.clone();
                    let g_en = g_bind.clone();
                    build_ctx_item(cx, "Enable All FX", false, move |cx| {
                        set_lane_all_bypassed(&p_en, g_en.as_ref(), bus, false);
                        cx.emit(UiEvent::HideFxOverlayMenu);
                        cx.emit(UiEvent::ModUiChanged);
                    });

                    build_ctx_sep(cx);

                    let p_clr = p_bind.clone();
                    let g_clr = g_bind.clone();
                    let m_clr = m_bind.clone();
                    build_ctx_item(cx, "Clear Lane", true, move |cx| {
                        clear_lane(&p_clr, g_clr.as_ref(), &m_clr, bus);
                        cx.emit(UiEvent::HideFxOverlayMenu);
                        cx.emit(UiEvent::ModUiChanged);
                    });
                }
            }
        });
    })
    .class("fx-picker-popover")
    .position_type(PositionType::SelfDirected)
    .left(UiData::fx_menu.map(|m| Pixels(m.clamped_left())))
    .top(UiData::fx_menu.map(|m| Pixels(m.clamped_top())))
    .width(UiData::fx_menu.map(|m| Pixels(m.menu_width())))
    .height(Auto)
    .row_between(Pixels(OUTER_ROW_GAP))
    .z_index(365)
    .display(UiData::fx_menu.map(FxOverlayMenuState::is_visible));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_menus_stay_strictly_inside_window_bounds() {
        let cases = [
            FxOverlayMenuKind::Picker {
                bus: 0,
                target: FxPickerTarget::InsertAt(0),
            },
            FxOverlayMenuKind::UnitContext {
                bus: 1,
                pos: 0,
                slot: 0,
            },
            FxOverlayMenuKind::LaneContext { bus: 2 },
        ];

        for kind in cases {
            let state = FxOverlayMenuState {
                kind,
                anchor_x: BASE_WIDTH as f32 + 200.0,
                anchor_y: BASE_HEIGHT as f32 + 200.0,
                rev: 1,
            };
            let exact_h = state.exact_height();
            let est_h = state.est_height();
            assert!(
                est_h >= exact_h + HEIGHT_SAFETY_CUSHION - 1e-4,
                "est_height ({est_h}) must include safety cushion above exact_height ({exact_h})"
            );
            let left = state.clamped_left();
            let top = state.clamped_top();
            assert!(
                left >= 6.0
                    && left + state.menu_width() <= BASE_WIDTH as f32 - VIEWPORT_MARGIN_X + 1e-4
            );
            assert!(
                top >= 6.0
                    && top + exact_h
                        <= BASE_HEIGHT as f32 - VIEWPORT_MARGIN_BOTTOM - HEIGHT_SAFETY_CUSHION
                            + 1e-4,
                "menu {:?} bottom {} overflows window height {}",
                kind,
                top + exact_h,
                BASE_HEIGHT
            );
        }
    }

    #[test]
    fn picker_height_accounts_for_all_categories_and_effects() {
        let picker = FxOverlayMenuState {
            kind: FxOverlayMenuKind::Picker {
                bus: 0,
                target: FxPickerTarget::InsertAt(0),
            },
            anchor_x: 0.0,
            anchor_y: 0.0,
            rev: 0,
        };
        let total_fx: usize = FX_PICKER_LEFT_COL
            .iter()
            .chain(FX_PICKER_RIGHT_COL.iter())
            .map(|c| c.items.len())
            .sum();
        assert_eq!(
            total_fx, 14,
            "FX picker should expose all 14 active FX types"
        );
        assert!(
            picker.exact_height() >= 295.0,
            "Picker exact_height ({}) should reflect full taller right column",
            picker.exact_height()
        );
    }
}
