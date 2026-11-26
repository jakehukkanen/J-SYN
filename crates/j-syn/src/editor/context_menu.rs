//! Floating right-click modulation & pitch-mode context menu overlay.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::{ModDestination, ModMatrixSource};
use nih_plug::prelude::{GuiContext, Param, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{bold_label, CheckIcon, CrossButton};
use super::mod_bar::ModUiState;
use super::{UiData, UiEvent, BASE_HEIGHT, BASE_WIDTH};
use crate::{JSynPluginParams, PitchModeParam};

pub(super) fn build_mod_context_menu_overlay(
    cx: &mut Context,
    mod_ui: Arc<ModUiState>,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
) {
    let params_menu = params;
    let gui_ctx_menu = gui_context;
        j_syn_ui::build_tooltip_overlay(cx, BASE_WIDTH as f32, BASE_HEIGHT as f32);

            // Click-away backdrop for floating Modulation Context Menu
            Element::new(cx)
                .position_type(PositionType::SelfDirected)
                .left(Pixels(0.0))
                .top(Pixels(0.0))
                .width(Stretch(1.0))
                .height(Stretch(1.0))
                .z_index(350)
                .display(UiData::mod_menu.map(|m| m.visible))
                .on_press(|cx| {
                    cx.emit(UiEvent::HideModContextMenu);
                });

            // Floating Modulation Context Menu overlay
            let mod_ui_menu = mod_ui;
            VStack::new(cx, move |cx| {
                let ui_menu = mod_ui_menu.clone();
                let p_menu = params_menu.clone();
                let g_menu = gui_ctx_menu.clone();
                Binding::new(cx, UiData::mod_menu, move |cx, menu_lens| {
                    let menu = menu_lens.get(cx);
                    if !menu.visible {
                        return;
                    }
                    let Some(dest) = ModDestination::from_index(menu.dest_idx) else {
                        return;
                    };
                    let slots = ui_menu.all_slots_for_dest(dest);
                    if slots.is_empty() && !menu.always_show_actions && menu.osc_pitch_idx.is_none()
                    {
                        return;
                    }

                    if let Some(osc_idx) = menu.osc_pitch_idx {
                        build_pitch_mode_menu_section(
                            cx,
                            osc_idx,
                            &p_menu,
                            &g_menu,
                        );
                        Element::new(cx)
                            .class("mod-context-sep")
                            .width(Stretch(1.0))
                            .height(Pixels(1.0));
                    }

                    let header = format!("MOD: {}", dest.name());
                    bold_label(cx, &header, "mod-context-header")
                        .width(Stretch(1.0))
                        .height(Pixels(16.0));

                    if menu.always_show_actions || menu.osc_pitch_idx.is_some() {
                        build_context_menu_actions_row(
                            cx,
                            dest,
                            !slots.is_empty(),
                            menu.show_mod_picker,
                            &ui_menu,
                            &p_menu,
                            &g_menu,
                        );
                        if menu.show_mod_picker {
                            build_context_mod_source_grid(cx, dest, &ui_menu);
                        }
                    }

                    for (slot_idx, slot) in slots {
                        let (r, g, b) = slot.source.color_rgb();
                        let src_name = ui_menu.source_display_name(slot.source);
                        let depth_str = format!("{:+.0}%", slot.depth * 100.0);
                        let ui_byp = ui_menu.clone();
                        let ui_pol = ui_menu.clone();
                        let ui_rem = ui_menu.clone();
                        let keep_open = menu.always_show_actions || menu.osc_pitch_idx.is_some();
                        HStack::new(cx, move |cx| {
                            Element::new(cx)
                                .width(Pixels(4.0))
                                .height(Pixels(12.0))
                                .top(Stretch(1.0))
                                .bottom(Stretch(1.0))
                                .background_color(Color::rgb(r, g, b));
                            bold_label(cx, &src_name, "mod-context-src")
                                .width(Stretch(1.0))
                                .height(Stretch(1.0))
                                .child_top(Stretch(1.0))
                                .child_bottom(Stretch(1.0));
                            bold_label(cx, &depth_str, "mod-context-depth")
                                .width(Pixels(36.0))
                                .height(Stretch(1.0))
                                .child_space(Stretch(1.0));
                            Button::new(
                                cx,
                                move |cx| {
                                    ui_byp.toggle_slot_bypass_at(slot_idx);
                                    cx.emit(UiEvent::ModUiChanged);
                                    cx.emit(UiEvent::ModMenuRefresh);
                                },
                                move |cx| {
                                    bold_label(
                                        cx,
                                        if slot.bypassed { "BYP" } else { "ON" },
                                        "mod-context-btn-text",
                                    )
                                    .child_space(Stretch(1.0))
                                    .hoverable(false)
                                },
                            )
                            .class("mod-context-btn")
                            .checked(!slot.bypassed)
                            .width(Pixels(30.0))
                            .height(Pixels(18.0));
                            Button::new(
                                cx,
                                move |cx| {
                                    ui_pol.toggle_slot_bipolar_at(slot_idx);
                                    cx.emit(UiEvent::ModUiChanged);
                                    cx.emit(UiEvent::ModMenuRefresh);
                                },
                                move |cx| {
                                    bold_label(
                                        cx,
                                        if slot.bipolar { "BIP" } else { "UNI" },
                                        "mod-context-btn-text",
                                    )
                                    .child_space(Stretch(1.0))
                                    .hoverable(false)
                                },
                            )
                            .class("mod-context-btn")
                            .checked(slot.bipolar)
                            .width(Pixels(30.0))
                            .height(Pixels(18.0));
                            CrossButton::new(cx, move |cx| {
                                ui_rem.remove_slot_at(slot_idx);
                                cx.emit(UiEvent::ModUiChanged);
                                if !keep_open && ui_rem.all_slots_for_dest(dest).is_empty() {
                                    cx.emit(UiEvent::HideModContextMenu);
                                } else {
                                    cx.emit(UiEvent::ModMenuRefresh);
                                }
                            })
                            .class("mod-context-btn")
                            .class("mod-context-btn-text")
                            .width(Pixels(20.0))
                            .height(Pixels(18.0));
                        })
                        .class("mod-context-row")
                        .col_between(Pixels(4.0))
                        .width(Stretch(1.0))
                        .height(Pixels(22.0));
                    }
                });
            })
            .class("mod-context-menu")
            .position_type(PositionType::SelfDirected)
            .left(UiData::mod_menu.map(|m| {
                Pixels(m.anchor_x.clamp(6.0, BASE_WIDTH as f32 - 236.0))
            }))
            .top(UiData::mod_menu.map(|m| {
                Pixels(m.anchor_y.clamp(6.0, BASE_HEIGHT as f32 - 270.0))
            }))
            .width(Pixels(228.0))
            .height(Auto)
            .z_index(360)
            .display(UiData::mod_menu.map(|m| m.visible));
}

fn build_pitch_mode_menu_section(
    cx: &mut Context,
    osc_idx: usize,
    params: &Arc<JSynPluginParams>,
    gui_context: &Arc<dyn GuiContext>,
) {
    bold_label(cx, "PITCH MODE", "mod-context-header")
        .width(Stretch(1.0))
        .height(Pixels(15.0));

    let cur_mode = params.oscs[osc_idx].pitch_mode.value();
    for (label, target_mode) in [
        ("Pitch (Semitones)", PitchModeParam::Semitones),
        ("Ratio (FM Harmonic)", PitchModeParam::Ratio),
    ] {
        let active = cur_mode == target_mode;
        let p_set = params.clone();
        let g_set = gui_context.clone();
        Button::new(
            cx,
            move |cx| {
                let setter = ParamSetter::new(g_set.as_ref());
                let param = &p_set.oscs[osc_idx].pitch_mode;
                setter.begin_set_parameter(param);
                setter.set_parameter(param, target_mode);
                setter.end_set_parameter(param);
                cx.emit(UiEvent::ModUiChanged);
                cx.emit(UiEvent::HideModContextMenu);
            },
            move |cx| {
                HStack::new(cx, move |cx| {
                    if active {
                        CheckIcon::new(cx)
                            .class("mod-context-check")
                            .width(Pixels(12.0))
                            .height(Pixels(12.0))
                            .top(Stretch(1.0))
                            .bottom(Stretch(1.0));
                    } else {
                        Element::new(cx)
                            .width(Pixels(12.0))
                            .height(Pixels(12.0))
                            .hoverable(false);
                    }
                    bold_label(cx, label, "mod-context-btn-text")
                        .width(Stretch(1.0))
                        .height(Stretch(1.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0))
                        .hoverable(false);
                })
                .col_between(Pixels(4.0))
                .width(Stretch(1.0))
                .height(Stretch(1.0))
                .hoverable(false)
            },
        )
        .class("mod-context-mode-btn")
        .checked(active)
        .width(Stretch(1.0))
        .height(Pixels(20.0));
    }
}

fn build_context_menu_actions_row(
    cx: &mut Context,
    dest: ModDestination,
    has_slots: bool,
    picker_open: bool,
    mod_ui: &Arc<ModUiState>,
    params: &Arc<JSynPluginParams>,
    gui_context: &Arc<dyn GuiContext>,
) {
    let ui_clr = mod_ui.clone();
    let p_rst = params.clone();
    let g_rst = gui_context.clone();
    HStack::new(cx, move |cx| {
        Button::new(
            cx,
            |cx| cx.emit(UiEvent::ToggleModMenuPicker),
            |cx| {
                bold_label(cx, "Modulate", "mod-context-btn-text")
                    .child_space(Stretch(1.0))
                    .hoverable(false)
            },
        )
        .class("mod-context-btn")
        .checked(picker_open)
        .width(Stretch(1.0))
        .height(Stretch(1.0));

        if has_slots {
            Button::new(
                cx,
                move |cx| {
                    ui_clr.remove_all_slots_for_dest(dest);
                    cx.emit(UiEvent::ModUiChanged);
                    cx.emit(UiEvent::ModMenuRefresh);
                },
                |cx| {
                    bold_label(cx, "Clear Mod", "mod-context-btn-text")
                        .child_space(Stretch(1.0))
                        .hoverable(false)
                },
            )
            .class("mod-context-btn")
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        }

        Button::new(
            cx,
            move |cx| {
                reset_dest_param_to_default(&p_rst, &g_rst, dest);
                cx.emit(UiEvent::ModUiChanged);
                cx.emit(UiEvent::HideModContextMenu);
            },
            |cx| {
                bold_label(cx, "Reset", "mod-context-btn-text")
                    .child_space(Stretch(1.0))
                    .hoverable(false)
            },
        )
        .class("mod-context-btn")
        .width(Pixels(48.0))
        .height(Stretch(1.0));
    })
    .col_between(Pixels(4.0))
    .width(Stretch(1.0))
    .height(Pixels(19.0));
}

fn build_context_mod_source_grid(cx: &mut Context, dest: ModDestination, mod_ui: &Arc<ModUiState>) {
    let rows: [&[ModMatrixSource]; 4] = [
        &ModMatrixSource::ENVS,
        &ModMatrixSource::MODSEQS,
        &ModMatrixSource::MACROS,
        &ModMatrixSource::PERF,
    ];
    VStack::new(cx, move |cx| {
        for row in rows {
            HStack::new(cx, move |cx| {
                for &src in row {
                    let short = mod_source_menu_short_label(src);
                    let ui_assign = mod_ui.clone();
                    Button::new(
                        cx,
                        move |cx| {
                            ui_assign.assign_drop(src, dest);
                            cx.emit(UiEvent::ModUiChanged);
                            cx.emit(UiEvent::ToggleModMenuPicker);
                        },
                        move |cx| {
                            bold_label(cx, short, "mod-context-pick-text")
                                .child_space(Stretch(1.0))
                                .hoverable(false)
                        },
                    )
                    .class("mod-context-pick-btn")
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
                }
            })
            .col_between(Pixels(2.0))
            .width(Stretch(1.0))
            .height(Pixels(17.0));
        }
    })
    .class("mod-context-pick-grid")
    .row_between(Pixels(2.0))
    .width(Stretch(1.0))
    .height(Auto);
}

#[inline]
fn mod_source_menu_short_label(src: ModMatrixSource) -> &'static str {
    src.short_label()
}

fn reset_dest_param_to_default(
    params: &Arc<JSynPluginParams>,
    gui_context: &Arc<dyn GuiContext>,
    dest: ModDestination,
) {
    let param = match dest {
        ModDestination::Osc1Pitch => Some(&params.oscs[0].pitch),
        ModDestination::Osc2Pitch => Some(&params.oscs[1].pitch),
        ModDestination::Osc3Pitch => Some(&params.oscs[2].pitch),
        ModDestination::Osc4Pitch => Some(&params.oscs[3].pitch),
        ModDestination::Osc1Ratio => Some(&params.oscs[0].ratio),
        ModDestination::Osc2Ratio => Some(&params.oscs[1].ratio),
        ModDestination::Osc3Ratio => Some(&params.oscs[2].ratio),
        ModDestination::Osc4Ratio => Some(&params.oscs[3].ratio),
        ModDestination::Osc1HzOffset => Some(&params.oscs[0].hz_offset),
        ModDestination::Osc2HzOffset => Some(&params.oscs[1].hz_offset),
        ModDestination::Osc3HzOffset => Some(&params.oscs[2].hz_offset),
        ModDestination::Osc4HzOffset => Some(&params.oscs[3].hz_offset),
        _ => None,
    };
    if let Some(p) = param {
        let setter = ParamSetter::new(gui_context.as_ref());
        setter.begin_set_parameter(p);
        setter.set_parameter(p, p.default_plain_value());
        setter.end_set_parameter(p);
    }
}
