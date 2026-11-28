//! Oscillator strip (`OSC 1` - `OSC 4`) builder in compact 3-column layout.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::WavetableBank;
use nih_plug::prelude::{FloatParam, GuiContext, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{
    bold_label, build_dropdown_trigger_labels, build_enum_dropdown, build_flex_knob_cell_mod_dim,
    build_inline_pill_cell, build_inline_pill_cell_mod, build_pitch_tuning_group,
    build_ratio_tuning_row, build_wavetable_dropdown, step_wavetable, ArrowButton, ArrowDir,
    ScrollDropdown,
};
use super::mod_bar::ModUiState;
use super::scope::WavetableScope;
use super::tokens;
use super::widgets::LedToggleSwitch;
use super::UiData;
use crate::{
    JSynPluginParams, ModOrderParam, ModSourceParam, ModTypeParam, OscPluginParams, PitchModeParam,
    WarpModeParam,
};

fn osc0_enabled(p: &JSynPluginParams) -> bool {
    p.oscs[0].enabled.value()
}
fn osc1_enabled(p: &JSynPluginParams) -> bool {
    p.oscs[1].enabled.value()
}
fn osc2_enabled(p: &JSynPluginParams) -> bool {
    p.oscs[2].enabled.value()
}
fn osc3_enabled(p: &JSynPluginParams) -> bool {
    p.oscs[3].enabled.value()
}

#[inline]
pub(super) fn osc_enabled_fn(osc_idx: usize) -> fn(&JSynPluginParams) -> bool {
    match osc_idx {
        0 => osc0_enabled,
        1 => osc1_enabled,
        2 => osc2_enabled,
        _ => osc3_enabled,
    }
}

pub(super) fn build_osc_panel(
    cx: &mut Context,
    osc_idx: usize,
    bank: Arc<WavetableBank>,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    const WARP_ITEMS: &[(&str, WarpModeParam)] = &[
        ("None", WarpModeParam::None),
        ("B-", WarpModeParam::BendMinus),
        ("B+", WarpModeParam::BendPlus),
        ("SP", WarpModeParam::Spectrum),
    ];
    const UNIFIED_MOD_ITEMS: &[(&str, ModSourceParam, ModTypeParam)] = &[
        ("Off", ModSourceParam::Off, ModTypeParam::Pm),
        ("PM 1", ModSourceParam::Osc1, ModTypeParam::Pm),
        ("PM 2", ModSourceParam::Osc2, ModTypeParam::Pm),
        ("PM 3", ModSourceParam::Osc3, ModTypeParam::Pm),
        ("PM 4", ModSourceParam::Osc4, ModTypeParam::Pm),
        ("RM 1", ModSourceParam::Osc1, ModTypeParam::Rm),
        ("RM 2", ModSourceParam::Osc2, ModTypeParam::Rm),
        ("RM 3", ModSourceParam::Osc3, ModTypeParam::Rm),
        ("RM 4", ModSourceParam::Osc4, ModTypeParam::Rm),
    ];

    let osc_label = match osc_idx {
        0 => "OSC 1",
        1 => "OSC 2",
        2 => "OSC 3",
        _ => "OSC 4",
    };

    #[inline]
    fn get_osc(p: &Arc<JSynPluginParams>, idx: usize) -> &OscPluginParams {
        &p.oscs[idx]
    }

    let osc_check = osc_enabled_fn(osc_idx);
    let p_dim = params.clone();
    let p_dock = params.clone();
    let m_dock = mod_ui.clone();

    VStack::new(cx, move |cx| {
        // Header row: Power + [< Wavetable >] filling the full header bar
        let p_hdr = params.clone();
        let g_hdr = gui_context.clone();
        let b_hdr = bank.clone();
        let m_hdr = mod_ui.clone();
        HStack::new(cx, move |cx| {
            LedToggleSwitch::new(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).enabled,
                osc_label,
                osc_label,
            )
            .width(Pixels(tokens::OSC_LED_W))
            .height(Pixels(tokens::HEADER_STRIP_H));

            let p_prev = p_hdr.clone();
            let g_prev = g_hdr.clone();
            let m_prev = m_hdr.clone();
            let b_prev = b_hdr.clone();
            ArrowButton::new(
                cx,
                move |_| step_wavetable(&p_prev, &g_prev, &m_prev, &b_prev, osc_idx, -1),
                ArrowDir::Left,
            )
            .class("wt-step-btn")
            .class("wt-step-label")
            .width(Pixels(tokens::STEP_BTN_W))
            .height(Pixels(tokens::HEADER_STRIP_H));

            build_wavetable_dropdown(
                cx,
                osc_idx,
                b_hdr.clone(),
                p_hdr.clone(),
                g_hdr.clone(),
                m_hdr.clone(),
            )
            .width(Stretch(1.0))
            .height(Pixels(tokens::HEADER_STRIP_H));

            let p_next = p_hdr;
            let g_next = g_hdr;
            let m_next = m_hdr;
            let b_next = b_hdr;
            ArrowButton::new(
                cx,
                move |_| step_wavetable(&p_next, &g_next, &m_next, &b_next, osc_idx, 1),
                ArrowDir::Right,
            )
            .class("wt-step-btn")
            .class("wt-step-label")
            .width(Pixels(tokens::STEP_BTN_W))
            .height(Pixels(tokens::HEADER_STRIP_H));
        })
        .col_between(Pixels(tokens::SPACE_XS))
        .width(Stretch(1.0))
        .height(Pixels(tokens::HEADER_STRIP_H));

        // Row 2: [OCT | SEMI | FINE] (or [RATIO | HZ] in Ratio mode) + [RETRIG]
        HStack::new(cx, {
            let p_row2 = params.clone();
            let u_row2 = mod_ui.clone();
            move |cx| {
                let p_mode = p_row2.clone();
                let u_mode = u_row2.clone();
                HStack::new(cx, move |cx| {
                    Binding::new(
                        cx,
                        UiData::mod_ui_rev.map(move |_| {
                            p_mode.oscs[osc_idx].pitch_mode.value() == PitchModeParam::Ratio
                        }),
                        move |cx, is_ratio_lens| {
                            let mod_cell = u_mode.clone();
                            if !is_ratio_lens.get(cx) {
                                build_pitch_tuning_group(
                                    cx,
                                    osc_idx,
                                    UiData::params,
                                    move |p| &get_osc(p, osc_idx).pitch,
                                    ModDestination::osc_pitch(osc_idx),
                                    mod_cell,
                                    0,
                                );
                            } else {
                                build_ratio_tuning_row(
                                    cx,
                                    osc_idx,
                                    UiData::params,
                                    move |p| &get_osc(p, osc_idx).ratio,
                                    move |p| &get_osc(p, osc_idx).hz_offset,
                                    ModDestination::osc_ratio(osc_idx),
                                    ModDestination::osc_hz_offset(osc_idx),
                                    mod_cell,
                                    0,
                                );
                            }
                        },
                    );
                })
                .width(Stretch(3.0))
                .height(Stretch(1.0));

                LedToggleSwitch::new(
                    cx,
                    UiData::params,
                    move |p| &get_osc(p, osc_idx).retrig,
                    "RETRIG",
                    "RETRIG",
                )
                .width(Stretch(1.0))
                .height(Stretch(1.0));
            }
        })
        .col_between(Pixels(tokens::SPACE_XS))
        .width(Stretch(1.0))
        .height(Pixels(tokens::PILL_H));

        // Row 3: [WavetableScope] | [WT POS | WARP+dd | < > | MOD+dd | LEVEL]
        HStack::new(cx, move |cx| {
            WavetableScope::new(cx, osc_idx, bank, params.clone(), mod_ui.clone())
                .width(Pixels(tokens::OSC_SCOPE_W))
                .height(Stretch(1.0));

            let p_right = params;
            let ui_right = mod_ui;
            let gui_right = gui_context;
            VStack::new(cx, move |cx| {
                let add_knob = |cx: &mut Context,
                                field: fn(&OscPluginParams) -> &FloatParam,
                                title: &'static str,
                                dest: ModDestination,
                                p_k: Arc<JSynPluginParams>,
                                ui_k: Arc<ModUiState>| {
                    VStack::new(cx, move |cx| {
                        build_flex_knob_cell_mod_dim(
                            cx,
                            UiData::params,
                            move |p| field(&p.oscs[osc_idx]),
                            title,
                            false,
                            tokens::KNOB_OSC,
                            dest,
                            ui_k,
                            0,
                            p_k,
                            osc_check,
                        );
                    })
                    .width(Stretch(1.0))
                    .height(Auto);
                };

                // Row 1: WT POS | WARP | spacer(16) | MOD | LEVEL
                let p_r1 = p_right.clone();
                let u_r1 = ui_right.clone();
                HStack::new(cx, move |cx| {
                    add_knob(
                        cx,
                        |o| &o.wt_pos,
                        "WT POS",
                        ModDestination::osc_wt_pos(osc_idx),
                        p_r1.clone(),
                        u_r1.clone(),
                    );
                    add_knob(
                        cx,
                        |o| &o.warp_amount,
                        "WARP",
                        ModDestination::osc_warp(osc_idx),
                        p_r1.clone(),
                        u_r1.clone(),
                    );
                    Element::new(cx)
                        .width(Pixels(tokens::STEP_BTN_W))
                        .height(Stretch(1.0));
                    add_knob(
                        cx,
                        |o| &o.mod_depth,
                        "MOD",
                        ModDestination::osc_mod_depth(osc_idx),
                        p_r1.clone(),
                        u_r1.clone(),
                    );
                    add_knob(
                        cx,
                        |o| &o.level,
                        "LEVEL",
                        ModDestination::osc_level(osc_idx),
                        p_r1,
                        u_r1,
                    );
                })
                .col_between(Pixels(tokens::SPACE_XS))
                .width(Stretch(1.0))
                .height(Auto);

                // Row 2: [spacer] | WARP dropdown | < / > order button | MOD dropdown | [spacer]
                HStack::new(cx, move |cx| {
                    Element::new(cx)
                        .width(Stretch(1.0))
                        .height(Pixels(tokens::PILL_H));

                    build_enum_dropdown(
                        cx,
                        p_right.clone(),
                        gui_right.clone(),
                        move |p| &get_osc(p, osc_idx).warp_mode,
                        WARP_ITEMS,
                    )
                    .width(Stretch(1.0))
                    .height(Pixels(tokens::PILL_H));

                    let p_ord = p_right.clone();
                    let g_ord = gui_right.clone();
                    let p_ord_lbl = p_right.clone();
                    ArrowButton::reactive(
                        cx,
                        move |_| {
                            let cur = p_ord.oscs[osc_idx].mod_order.value();
                            let next = match cur {
                                ModOrderParam::WarpThenPm => ModOrderParam::PmThenWarp,
                                ModOrderParam::PmThenWarp => ModOrderParam::WarpThenPm,
                            };
                            let setter = ParamSetter::new(g_ord.as_ref());
                            let param = &p_ord.oscs[osc_idx].mod_order;
                            setter.begin_set_parameter(param);
                            setter.set_parameter(param, next);
                            setter.end_set_parameter(param);
                        },
                        UiData::params,
                        move |_| match p_ord_lbl.oscs[osc_idx].mod_order.value() {
                            ModOrderParam::WarpThenPm => ArrowDir::Right,
                            ModOrderParam::PmThenWarp => ArrowDir::Left,
                        },
                    )
                    .class("wt-step-btn")
                    .class("wt-step-label")
                    .width(Pixels(tokens::STEP_BTN_W))
                    .height(Pixels(tokens::PILL_H));

                    let p_mod_scroll = p_right.clone();
                    let g_mod_scroll = gui_right.clone();
                    let p_mod_trig = p_right.clone();
                    let p_mod_pop = p_right.clone();
                    let g_mod_pop = gui_right;
                    ScrollDropdown::new(
                        cx,
                        move |_cx, delta| {
                            let cur_src = p_mod_scroll.oscs[osc_idx].mod_source.value();
                            let cur_ty = p_mod_scroll.oscs[osc_idx].mod_type.value();
                            let pos = UNIFIED_MOD_ITEMS
                                .iter()
                                .position(|&(_, s, t)| {
                                    s == cur_src && (s == ModSourceParam::Off || t == cur_ty)
                                })
                                .unwrap_or(0);
                            let next = (pos as isize + delta)
                                .clamp(0, UNIFIED_MOD_ITEMS.len() as isize - 1)
                                as usize;
                            if next != pos {
                                let (_, src, ty) = UNIFIED_MOD_ITEMS[next];
                                let setter = ParamSetter::new(g_mod_scroll.as_ref());
                                let osc = &p_mod_scroll.oscs[osc_idx];
                                setter.begin_set_parameter(&osc.mod_type);
                                setter.set_parameter(&osc.mod_type, ty);
                                setter.end_set_parameter(&osc.mod_type);
                                setter.begin_set_parameter(&osc.mod_source);
                                setter.set_parameter(&osc.mod_source, src);
                                setter.end_set_parameter(&osc.mod_source);
                            }
                        },
                        move |cx| {
                            let p_mod_trig = p_mod_trig.clone();
                            let p_mod_pop = p_mod_pop.clone();
                            let g_mod_pop = g_mod_pop.clone();
                            Dropdown::new(
                                cx,
                                move |cx| {
                                    let p_m = p_mod_trig.clone();
                                    HStack::new(cx, move |cx| {
                                        build_dropdown_trigger_labels(
                                            cx,
                                            UiData::params.map(move |_| {
                                                let osc = &p_m.oscs[osc_idx];
                                                let src = osc.mod_source.value();
                                                let ty = osc.mod_type.value();
                                                let prefix = match ty {
                                                    ModTypeParam::Pm => "PM",
                                                    ModTypeParam::Rm => "RM",
                                                };
                                                match src {
                                                    ModSourceParam::Off => "Off".to_string(),
                                                    ModSourceParam::Osc1 => format!("{prefix} 1"),
                                                    ModSourceParam::Osc2 => format!("{prefix} 2"),
                                                    ModSourceParam::Osc3 => format!("{prefix} 3"),
                                                    ModSourceParam::Osc4 => format!("{prefix} 4"),
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
                                    for &(label, src, ty) in UNIFIED_MOD_ITEMS {
                                        let p_set = p_mod_pop.clone();
                                        let g_set = g_mod_pop.clone();
                                        bold_label(cx, label, "dropdown-item").on_press(move |cx| {
                                            let setter = ParamSetter::new(g_set.as_ref());
                                            let osc = &p_set.oscs[osc_idx];
                                            setter.begin_set_parameter(&osc.mod_type);
                                            setter.set_parameter(&osc.mod_type, ty);
                                            setter.end_set_parameter(&osc.mod_type);
                                            setter.begin_set_parameter(&osc.mod_source);
                                            setter.set_parameter(&osc.mod_source, src);
                                            setter.end_set_parameter(&osc.mod_source);
                                            cx.emit(PopupEvent::Close);
                                        });
                                    }
                                },
                            )
                        },
                    )
                    .width(Stretch(1.0))
                    .height(Pixels(tokens::PILL_H));

                    Element::new(cx)
                        .width(Stretch(1.0))
                        .height(Pixels(tokens::PILL_H));
                })
                .col_between(Pixels(tokens::SPACE_XS))
                .width(Stretch(1.0))
                .height(Pixels(tokens::PILL_H));
            })
            .row_between(Pixels(tokens::SPACE_XS))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0))
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        })
        .col_between(Pixels(tokens::SPACE_MD))
        .width(Stretch(1.0))
        .height(Stretch(1.0));

        // Row 4: [PHS] [UNI] [DET] [SPREAD] (wider SPREAD, equal remaining share on PHS/UNI/DET)
        HStack::new(cx, move |cx| {
            build_inline_pill_cell_mod(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).phase,
                "PHS",
                360,
                0.94,
                ModDestination::osc_phase(osc_idx),
                m_dock.clone(),
                0,
            )
            .class("stk-pill")
            .height(Pixels(tokens::PILL_H));

            build_inline_pill_cell(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).unison,
                "UNI",
                8,
                0.94,
            )
            .class("stk-pill")
            .height(Pixels(tokens::PILL_H));

            let p_det_dim = p_dock.clone();
            build_inline_pill_cell_mod(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).detune,
                "DET",
                100,
                0.94,
                ModDestination::osc_detune(osc_idx),
                m_dock.clone(),
                0,
            )
            .class("stk-pill")
            .height(Pixels(tokens::PILL_H))
            .toggle_class(
                "pill-dimmed",
                UiData::mod_ui_rev.map(move |_| p_det_dim.oscs[osc_idx].unison.value() <= 1),
            );

            build_inline_pill_cell_mod(
                cx,
                UiData::params,
                move |p| &get_osc(p, osc_idx).spread,
                "SPREAD",
                100,
                1.18,
                ModDestination::osc_spread(osc_idx),
                m_dock,
                0,
            )
            .class("stk-pill")
            .height(Pixels(tokens::PILL_H))
            .toggle_class(
                "pill-dimmed",
                UiData::mod_ui_rev.map(move |_| p_dock.oscs[osc_idx].unison.value() <= 1),
            );
        })
        .col_between(Pixels(tokens::SPACE_XS))
        .width(Stretch(1.0))
        .height(Pixels(tokens::PILL_H));
    })
    .class("hardware-module")
    .toggle_class(
        "module-disabled",
        UiData::mod_ui_rev.map(move |_| !osc_check(&p_dim)),
    )
    .row_between(Pixels(tokens::SPACE_XS))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}
