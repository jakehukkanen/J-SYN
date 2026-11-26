//! Full-screen `MATRIX` overview table for inspecting and editing all modulation slots.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::{
    ModDestination, ModMatrixSource, ModSlot, BASE_MOD_DESTINATIONS, NUM_MOD_SLOTS,
};
use j_syn_dsp::NUM_FX_BUSES;
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{
    bold_label, build_dropdown_trigger_labels, build_module_header, ArrowButton, ArrowDir,
    CrossButton,
};
use super::mod_bar::ModUiState;
use super::{UiData, UiEvent};

pub(super) const ALL_MOD_SOURCES: [ModMatrixSource; j_syn_dsp::NUM_ACTIVE_SOURCES] =
    ModMatrixSource::ALL_ACTIVE;

const CURVE_MODES: &[(&str, f32)] = &[
    ("LIN", 0.0),
    ("EXP", 0.55),
    ("LOG", -0.55),
    ("S-CURVE", 2.0),
    ("STEP", 3.0),
];

fn curve_mode_label(curve: f32) -> &'static str {
    if (curve - 2.0).abs() < 0.1 {
        "S-CURVE"
    } else if (curve - 3.0).abs() < 0.1 {
        "STEP"
    } else if curve > 0.05 {
        "EXP"
    } else if curve < -0.05 {
        "LOG"
    } else {
        "LIN"
    }
}

#[derive(Clone, Copy)]
enum MatrixField {
    Depth,
}

struct MatrixDragBox {
    slot_idx: usize,
    field: MatrixField,
    mod_ui: Arc<ModUiState>,
    dragging: bool,
    start_y: f32,
    start_val: f32,
}

impl MatrixDragBox {
    fn new<'a>(
        cx: &'a mut Context,
        slot_idx: usize,
        field: MatrixField,
        mod_ui: Arc<ModUiState>,
    ) -> Handle<'a, Self> {
        let ui_lbl = mod_ui.clone();
        Self {
            slot_idx,
            field,
            mod_ui,
            dragging: false,
            start_y: 0.0,
            start_val: 0.0,
        }
        .build(cx, move |cx| {
            let text_lens = UiData::mod_val_rev.map(move |_| {
                let slot = ui_lbl.params.mod_matrix.load().slots[slot_idx];
                match field {
                    MatrixField::Depth => format!("{:+.0}%", slot.depth * 100.0),
                }
            });
            bold_label(cx, text_lens, "matrix-depth-val")
                .child_space(Stretch(1.0))
                .hoverable(false);
        })
        .class("matrix-depth-box")
    }

    fn current_val(&self) -> f32 {
        let slot = self.mod_ui.params.mod_matrix.load().slots[self.slot_idx];
        match self.field {
            MatrixField::Depth => slot.depth,
        }
    }

    fn apply_val(&self, val: f32) {
        match self.field {
            MatrixField::Depth => self.mod_ui.set_slot_depth_at(self.slot_idx, val),
        }
    }
}

impl View for MatrixDragBox {
    fn element(&self) -> Option<&'static str> {
        Some("matrix-drag-box")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                self.dragging = true;
                self.start_y = cx.mouse().cursory;
                self.start_val = self.current_val();
                cx.capture();
                meta.consume();
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                let default_v = match self.field {
                    MatrixField::Depth => 0.25,
                };
                self.apply_val(default_v);
                cx.emit(UiEvent::ModValChanged);
                meta.consume();
            }
            WindowEvent::MouseMove(_, y) if self.dragging => {
                let sens = if cx.modifiers().contains(Modifiers::SHIFT) {
                    420.0
                } else {
                    150.0
                };
                let delta = (self.start_y - *y) / sens;
                self.apply_val((self.start_val + delta).clamp(-1.0, 1.0));
                cx.emit(UiEvent::ModValChanged);
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) if self.dragging => {
                self.dragging = false;
                cx.release();
                cx.emit(UiEvent::ModUiChanged);
                meta.consume();
            }
            WindowEvent::MouseScroll(_, y) => {
                let step = if *y > 0.0 { 0.02 } else { -0.02 };
                self.apply_val((self.current_val() + step).clamp(-1.0, 1.0));
                cx.emit(UiEvent::ModValChanged);
                meta.consume();
            }
            _ => {}
        });
    }
}

fn build_matrix_row(
    cx: &mut Context,
    row_num: usize,
    slot_idx: usize,
    slot: ModSlot,
    mod_ui: Arc<ModUiState>,
) {
    let (r, g, b) = slot.source.color_rgb();
    let num_text = format!("{:02}", row_num + 1);
    let src_text = mod_ui.source_display_name(slot.source);
    let dst_text = slot.destination.name().to_string();
    let pol_text = if slot.bipolar { "BIP" } else { "UNI" };
    let aux_text = mod_ui.aux_short_label(slot.aux_source);

    let ui_src = mod_ui.clone();
    let ui_dst = mod_ui.clone();
    let ui_depth = mod_ui.clone();
    let ui_curve = mod_ui.clone();
    let ui_pol = mod_ui.clone();
    let ui_aux = mod_ui.clone();
    let ui_up = mod_ui.clone();
    let ui_dn = mod_ui.clone();
    let ui_del = mod_ui.clone();

    HStack::new(cx, move |cx| {
        bold_label(cx, &num_text, "matrix-cell-idx")
            .width(Pixels(26.0))
            .height(Stretch(1.0))
            .child_space(Stretch(1.0));

        let ui_src_trig = ui_src.clone();
        let src_text_c = src_text.clone();
        Dropdown::new(
            cx,
            move |cx| {
                let ui_s = ui_src_trig.clone();
                let s_txt = src_text_c.clone();
                HStack::new(cx, move |cx| {
                    Element::new(cx)
                        .width(Pixels(4.0))
                        .height(Pixels(12.0))
                        .top(Stretch(1.0))
                        .bottom(Stretch(1.0))
                        .background_color(Color::rgb(r, g, b));
                    build_dropdown_trigger_labels(
                        cx,
                        UiData::mod_ui_rev.map(move |_| {
                            let s = ui_s.params.mod_matrix.load().slots[slot_idx];
                            if s.enabled {
                                ui_s.source_display_name(s.source)
                            } else {
                                s_txt.clone()
                            }
                        }),
                    );
                })
                .col_between(Pixels(4.0))
                .width(Stretch(1.0))
                .height(Stretch(1.0))
                .child_top(Pixels(0.0))
                .child_bottom(Pixels(0.0))
                .overflow(Overflow::Hidden)
            },
            move |cx| {
                let ui_src_list = ui_src.clone();
                ScrollView::new(cx, 0.0, 0.0, false, true, move |cx| {
                    let ui_src_inner = ui_src_list.clone();
                    VStack::new(cx, move |cx| {
                        for src in ALL_MOD_SOURCES {
                            let ui_set = ui_src_inner.clone();
                            let label = ui_set.source_display_name(src);
                            bold_label(cx, &label, "dropdown-item").on_press(move |cx| {
                                ui_set.set_slot_source_at(slot_idx, src);
                                cx.emit(UiEvent::ModUiChanged);
                                cx.emit(PopupEvent::Close);
                            });
                        }
                    })
                    .class("dropdown-scroll-content");
                })
                .class("dropdown-scroll");
            },
        )
        .width(Pixels(130.0))
        .height(Pixels(20.0))
        .top(Stretch(1.0))
        .bottom(Stretch(1.0));

        bold_label(cx, "->", "matrix-cell-arrow")
            .width(Pixels(20.0))
            .height(Stretch(1.0))
            .child_space(Stretch(1.0));

        let ui_dst_trig = ui_dst.clone();
        let dst_text_c = dst_text.clone();
        Dropdown::new(
            cx,
            move |cx| {
                let ui_d = ui_dst_trig.clone();
                let d_txt = dst_text_c.clone();
                HStack::new(cx, move |cx| {
                    build_dropdown_trigger_labels(
                        cx,
                        UiData::mod_ui_rev.map(move |_| {
                            let s = ui_d.params.mod_matrix.load().slots[slot_idx];
                            if s.enabled {
                                s.destination.name().to_string()
                            } else {
                                d_txt.clone()
                            }
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
                let ui_dst_list = ui_dst.clone();
                ScrollView::new(cx, 0.0, 0.0, false, true, move |cx| {
                    let ui_dst_inner = ui_dst_list.clone();
                    VStack::new(cx, move |cx| {
                        for dest in BASE_MOD_DESTINATIONS {
                            let ui_set = ui_dst_inner.clone();
                            let name = dest.name();
                            bold_label(cx, name.as_ref(), "dropdown-item").on_press(move |cx| {
                                ui_set.set_slot_destination_at(slot_idx, dest);
                                cx.emit(UiEvent::ModUiChanged);
                                cx.emit(PopupEvent::Close);
                            });
                        }
                        let order = ui_dst_inner.params.reconciled_fx_rack_order();
                        for bus in 0..NUM_FX_BUSES {
                            for &slot_u8 in order.active_slots(bus) {
                                let fx_slot = slot_u8 as usize;
                                for param_idx in 0..8 {
                                    let dest =
                                        ModDestination::fx_bus_slot_param(bus, fx_slot, param_idx);
                                    let ui_set = ui_dst_inner.clone();
                                    let name = dest.name();
                                    bold_label(cx, name.as_ref(), "dropdown-item").on_press(
                                        move |cx| {
                                            ui_set.set_slot_destination_at(slot_idx, dest);
                                            cx.emit(UiEvent::ModUiChanged);
                                            cx.emit(PopupEvent::Close);
                                        },
                                    );
                                }
                            }
                        }
                    })
                    .class("dropdown-scroll-content");
                })
                .class("dropdown-scroll");
            },
        )
        .width(Stretch(1.0))
        .height(Pixels(20.0))
        .top(Stretch(1.0))
        .bottom(Stretch(1.0));

        MatrixDragBox::new(cx, slot_idx, MatrixField::Depth, ui_depth)
            .width(Pixels(64.0))
            .height(Pixels(20.0))
            .top(Stretch(1.0))
            .bottom(Stretch(1.0));

        let ui_curve_trig = ui_curve.clone();
        Dropdown::new(
            cx,
            move |cx| {
                let ui_c = ui_curve_trig.clone();
                HStack::new(cx, move |cx| {
                    build_dropdown_trigger_labels(
                        cx,
                        UiData::mod_ui_rev.map(move |_| {
                            let s = ui_c.params.mod_matrix.load().slots[slot_idx];
                            curve_mode_label(s.curve).to_string()
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
                for &(label, val) in CURVE_MODES {
                    let ui_set = ui_curve.clone();
                    bold_label(cx, label, "dropdown-item").on_press(move |cx| {
                        ui_set.set_slot_curve_at(slot_idx, val);
                        cx.emit(UiEvent::ModUiChanged);
                        cx.emit(PopupEvent::Close);
                    });
                }
            },
        )
        .width(Pixels(68.0))
        .height(Pixels(20.0))
        .top(Stretch(1.0))
        .bottom(Stretch(1.0));

        Button::new(
            cx,
            move |cx| {
                ui_pol.toggle_slot_bipolar_at(slot_idx);
                cx.emit(UiEvent::ModUiChanged);
            },
            move |cx| {
                bold_label(cx, pol_text, "matrix-btn-text")
                    .child_space(Stretch(1.0))
                    .hoverable(false)
            },
        )
        .class("matrix-btn")
        .checked(slot.bipolar)
        .width(Pixels(46.0))
        .height(Pixels(20.0))
        .top(Stretch(1.0))
        .bottom(Stretch(1.0));

        let ui_aux_trig = ui_aux.clone();
        let aux_text_c = aux_text.clone();
        Dropdown::new(
            cx,
            move |cx| {
                let ui_a = ui_aux_trig.clone();
                let a_txt = aux_text_c.clone();
                HStack::new(cx, move |cx| {
                    build_dropdown_trigger_labels(
                        cx,
                        UiData::mod_ui_rev.map(move |_| {
                            let s = ui_a.params.mod_matrix.load().slots[slot_idx];
                            if s.enabled {
                                ui_a.aux_short_label(s.aux_source)
                            } else {
                                a_txt.clone()
                            }
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
                let ui_aux_list = ui_aux.clone();
                ScrollView::new(cx, 0.0, 0.0, false, true, move |cx| {
                    let ui_aux_inner = ui_aux_list.clone();
                    VStack::new(cx, move |cx| {
                        let ui_none = ui_aux_inner.clone();
                        bold_label(cx, "None", "dropdown-item").on_press(move |cx| {
                            ui_none.set_slot_aux_at(slot_idx, ModMatrixSource::None);
                            cx.emit(UiEvent::ModUiChanged);
                            cx.emit(PopupEvent::Close);
                        });
                        for src in ALL_MOD_SOURCES {
                            let ui_set = ui_aux_inner.clone();
                            let label = ui_set.source_display_name(src);
                            bold_label(cx, &label, "dropdown-item").on_press(move |cx| {
                                ui_set.set_slot_aux_at(slot_idx, src);
                                cx.emit(UiEvent::ModUiChanged);
                                cx.emit(PopupEvent::Close);
                            });
                        }
                    })
                    .class("dropdown-scroll-content");
                })
                .class("dropdown-scroll");
            },
        )
        .width(Pixels(78.0))
        .height(Pixels(20.0))
        .top(Stretch(1.0))
        .bottom(Stretch(1.0));

        HStack::new(cx, move |cx| {
            ArrowButton::new(
                cx,
                move |cx| {
                    ui_up.move_active_slot(slot_idx, -1);
                    cx.emit(UiEvent::ModUiChanged);
                },
                ArrowDir::Up,
            )
            .class("matrix-btn")
            .class("matrix-btn-text")
            .width(Pixels(20.0))
            .height(Pixels(20.0));

            ArrowButton::new(
                cx,
                move |cx| {
                    ui_dn.move_active_slot(slot_idx, 1);
                    cx.emit(UiEvent::ModUiChanged);
                },
                ArrowDir::Down,
            )
            .class("matrix-btn")
            .class("matrix-btn-text")
            .width(Pixels(20.0))
            .height(Pixels(20.0));
        })
        .col_between(Pixels(2.0))
        .width(Pixels(42.0))
        .height(Pixels(20.0))
        .top(Stretch(1.0))
        .bottom(Stretch(1.0));

        CrossButton::new(cx, move |cx| {
            ui_del.remove_slot_at(slot_idx);
            cx.emit(UiEvent::ModUiChanged);
        })
        .class("matrix-del-btn")
        .class("matrix-btn-text")
        .width(Pixels(26.0))
        .height(Pixels(20.0))
        .top(Stretch(1.0))
        .bottom(Stretch(1.0));
    })
    .class("matrix-row")
    .col_between(Pixels(6.0))
    .width(Stretch(1.0))
    .height(Pixels(26.0))
    .child_left(Pixels(6.0))
    .child_right(Pixels(6.0));
}

/// Builds the full-screen `MATRIX` view overview table showing all active modulation slots.
pub(crate) fn build_matrix_view(cx: &mut Context, mod_ui: Arc<ModUiState>) {
    let ui_header = mod_ui.clone();
    let ui_body = mod_ui;

    VStack::new(cx, move |cx| {
        // Header bar + active count + Clear All
        HStack::new(cx, move |cx| {
            HStack::new(cx, |cx| {
                build_module_header(cx, "MODULATION MATRIX OVERVIEW", 20.0);
            })
            .width(Pixels(220.0))
            .height(Pixels(20.0));

            let ui_cnt = ui_header.clone();
            Binding::new(cx, UiData::mod_ui_rev, move |cx, _| {
                let count = ui_cnt.active_slot_count();
                let badge = format!("ACTIVE SLOTS: {count} / {NUM_MOD_SLOTS}");
                bold_label(cx, &badge, "matrix-count-badge")
                    .width(Auto)
                    .height(Stretch(1.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0));
            });

            Element::new(cx).width(Stretch(1.0));

            let ui_clear = ui_header.clone();
            Button::new(
                cx,
                move |cx| {
                    ui_clear.clear_all_slots();
                    cx.emit(UiEvent::ModUiChanged);
                },
                |cx| {
                    bold_label(cx, "CLEAR ALL", "matrix-clear-text")
                        .child_space(Stretch(1.0))
                        .hoverable(false)
                },
            )
            .class("matrix-clear-btn")
            .width(Pixels(92.0))
            .height(Pixels(20.0));
        })
        .col_between(Pixels(12.0))
        .width(Stretch(1.0))
        .height(Pixels(22.0));

        // Table column header
        HStack::new(cx, |cx| {
            bold_label(cx, "#", "matrix-col-title")
                .width(Pixels(26.0))
                .height(Stretch(1.0))
                .child_space(Stretch(1.0));
            bold_label(cx, "SOURCE", "matrix-col-title")
                .width(Pixels(130.0))
                .height(Stretch(1.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0));
            bold_label(cx, " ", "matrix-col-title")
                .width(Pixels(20.0))
                .height(Stretch(1.0));
            bold_label(cx, "DESTINATION", "matrix-col-title")
                .width(Stretch(1.0))
                .height(Stretch(1.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0));
            bold_label(cx, "DEPTH", "matrix-col-title")
                .width(Pixels(64.0))
                .height(Stretch(1.0))
                .child_space(Stretch(1.0));
            bold_label(cx, "CURVE", "matrix-col-title")
                .width(Pixels(68.0))
                .height(Stretch(1.0))
                .child_space(Stretch(1.0));
            bold_label(cx, "POL", "matrix-col-title")
                .width(Pixels(46.0))
                .height(Stretch(1.0))
                .child_space(Stretch(1.0));
            bold_label(cx, "AUX", "matrix-col-title")
                .width(Pixels(78.0))
                .height(Stretch(1.0))
                .child_space(Stretch(1.0));
            bold_label(cx, "ORD", "matrix-col-title")
                .width(Pixels(42.0))
                .height(Stretch(1.0))
                .child_space(Stretch(1.0));
            bold_label(cx, "DEL", "matrix-col-title")
                .width(Pixels(26.0))
                .height(Stretch(1.0))
                .child_space(Stretch(1.0));
        })
        .class("matrix-col-header")
        .col_between(Pixels(6.0))
        .width(Stretch(1.0))
        .height(Pixels(20.0))
        .child_left(Pixels(6.0))
        .child_right(Pixels(6.0));

        // Scrollable matrix rows
        ScrollView::new(cx, 0.0, 0.0, false, true, move |cx| {
            let ui_rows = ui_body.clone();
            Binding::new(cx, UiData::mod_ui_rev, move |cx, _| {
                let entries = ui_rows.active_entries();
                if entries.is_empty() {
                    VStack::new(cx, |cx| {
                        bold_label(
                            cx,
                            "NO ACTIVE MODULATIONS — DRAG A SOURCE PILL BELOW ONTO ANY KNOB IN MAIN VIEW, OR RIGHT-CLICK A KNOB TO ASSIGN",
                            "matrix-empty-hint",
                        )
                        .width(Stretch(1.0))
                        .height(Pixels(28.0))
                        .child_space(Stretch(1.0));
                    })
                    .width(Stretch(1.0))
                    .height(Pixels(120.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0));
                } else {
                    let ui_for_list = ui_rows.clone();
                    VStack::new(cx, move |cx| {
                        for (row_num, (slot_idx, slot)) in entries.into_iter().enumerate() {
                            build_matrix_row(cx, row_num, slot_idx, slot, ui_for_list.clone());
                        }
                    })
                    .row_between(Pixels(3.0))
                    .width(Stretch(1.0))
                    .height(Auto);
                }
            });
        })
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })
    .class("hardware-module")
    .class("matrix-card")
    .row_between(Pixels(4.0))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}
