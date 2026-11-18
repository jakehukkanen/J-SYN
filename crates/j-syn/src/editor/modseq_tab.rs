//! `MODSEQ` bottom drawer: 4 hybrid 16-step (`MODSEQ`) / freeform breakpoint (`LFO`) modulators
//! (`MSEQ 1`–`MSEQ 4`) and 4 modulation envelopes (`ENV 1`–`ENV 4`).

use std::sync::Arc;

use j_syn_dsp::mod_matrix::{ModDestination, ModMatrixSource};
use j_syn_dsp::{ModSeqStep, MAX_MODSEQ_STEPS};
use nih_plug::prelude::{FloatParam, GuiContext, Param};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

use super::controls::{
    bold_label, build_enum_dropdown, build_flex_pill_cell, build_knob_cell,
    build_knob_cell_mod_fmt, build_knob_cell_with_readout,
};
use super::mod_bar::{bottom_tab_tag, ModUiState};
use super::modseq_canvases::{
    AdsrPreviewScope, LfoPresetBar, ModSeqCurveCanvas, ModSeqPaletteGrid,
};
use super::tokens;
use super::{BottomTab, UiData, UiEvent, CORAL_RGB, GOLD_RGB};
use crate::{
    JSynPluginParams, ModSeqModeParam, ModSeqPlayParam, ModSeqRetrigParam, ModSeqSyncParam,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModSeqSubTab {
    MSeq(usize),
    Env(usize),
}

impl Data for ModSeqSubTab {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

const MODE_ITEMS: &[(&str, ModSeqModeParam)] = &[
    ("MODSEQ", ModSeqModeParam::Performer),
    ("LFO", ModSeqModeParam::Mseg),
];

const SYNC_ITEMS: &[(&str, ModSeqSyncParam)] = &[
    ("Free", ModSeqSyncParam::Free),
    ("Straight", ModSeqSyncParam::Straight),
    ("Triplet", ModSeqSyncParam::Triplet),
    ("Dotted", ModSeqSyncParam::Dotted),
];

const PLAY_ITEMS: &[(&str, ModSeqPlayParam)] = &[
    ("Loop", ModSeqPlayParam::Loop),
    ("1-Shot", ModSeqPlayParam::OneShot),
    ("PingPong", ModSeqPlayParam::PingPong),
];

const RETRIG_ITEMS: &[(&str, ModSeqRetrigParam)] = &[
    ("Poly", ModSeqRetrigParam::PolyVoice),
    ("Global", ModSeqRetrigParam::MonoGlobal),
];

pub(super) fn subtab_accent(sub: ModSeqSubTab) -> (u8, u8, u8) {
    let src = match sub {
        ModSeqSubTab::MSeq(0) => ModMatrixSource::ModSeq1,
        ModSeqSubTab::MSeq(1) => ModMatrixSource::ModSeq2,
        ModSeqSubTab::MSeq(2) => ModMatrixSource::ModSeq3,
        ModSeqSubTab::MSeq(_) => ModMatrixSource::ModSeq4,
        ModSeqSubTab::Env(0) => ModMatrixSource::Env1,
        ModSeqSubTab::Env(1) => ModMatrixSource::Env2,
        ModSeqSubTab::Env(2) => ModMatrixSource::Env3,
        ModSeqSubTab::Env(_) => ModMatrixSource::Env4,
    };
    src.color_rgb()
}

/// Interactive `femtovg` Performer/MSEG curve canvas with real-time playhead.

fn build_labeled_dropdown<E, P, FMap>(
    cx: &mut Context,
    title: &'static str,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    map_fn: FMap,
    items: &'static [(&'static str, E)],
) where
    E: nih_plug::prelude::Enum + Copy + PartialEq + Send + Sync + 'static,
    P: nih_plug::prelude::Param<Plain = E> + 'static,
    FMap: Fn(&Arc<JSynPluginParams>) -> &P + Copy + Send + Sync + 'static,
{
    VStack::new(cx, move |cx| {
        bold_label(cx, title, "mini-label")
            .width(Stretch(1.0))
            .height(Pixels(9.0))
            .child_space(Stretch(1.0));
        build_enum_dropdown(cx, params.clone(), gui_context.clone(), map_fn, items)
            .width(Stretch(1.0))
            .height(Pixels(18.0));
    })
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(0.0));
}

fn env_param_fns(
    env_idx: usize,
) -> (
    fn(&Arc<JSynPluginParams>) -> &FloatParam,
    fn(&Arc<JSynPluginParams>) -> &FloatParam,
    fn(&Arc<JSynPluginParams>) -> &FloatParam,
    fn(&Arc<JSynPluginParams>) -> &FloatParam,
) {
    match env_idx {
        0 => (
            |p| &p.amp_attack,
            |p| &p.amp_decay,
            |p| &p.amp_sustain,
            |p| &p.amp_release,
        ),
        1 => (
            |p| &p.mod_envs[0].attack,
            |p| &p.mod_envs[0].decay,
            |p| &p.mod_envs[0].sustain,
            |p| &p.mod_envs[0].release,
        ),
        2 => (
            |p| &p.mod_envs[1].attack,
            |p| &p.mod_envs[1].decay,
            |p| &p.mod_envs[1].sustain,
            |p| &p.mod_envs[1].release,
        ),
        _ => (
            |p| &p.mod_envs[2].attack,
            |p| &p.mod_envs[2].decay,
            |p| &p.mod_envs[2].sustain,
            |p| &p.mod_envs[2].release,
        ),
    }
}

pub(super) fn build_modseq_tab(
    cx: &mut Context,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
) {
    VStack::new(cx, move |cx| {
        let params_body = params.clone();
        let gui_ctx_body = gui_context.clone();
        let mod_ui_body = mod_ui.clone();
        Binding::new(cx, UiData::modseq_subtab, move |cx, sub| {
            let p_sub = params_body.clone();
            let g_sub = gui_ctx_body.clone();
            let m_sub = mod_ui_body.clone();
            match sub.get(cx) {
                ModSeqSubTab::MSeq(seq_idx) => {
                    VStack::new(cx, move |cx| {
                        // Top: full-width tall interactive curve canvas (no top label strip)
                        ModSeqCurveCanvas::new(cx, seq_idx, p_sub.clone(), m_sub.clone())
                            .width(Stretch(1.0))
                            .height(Stretch(1.0));

                        // Bottom bar: left utility/palette (2x9 stamp grid + FILL/CLEAR or LFO presets + RESET) + controls
                        HStack::new(cx, {
                            let p_bot = p_sub.clone();
                            let g_bot = g_sub.clone();
                            let u_bot = m_sub.clone();
                            move |cx| {
                                let p_pal = p_bot.clone();
                                let u_pal = u_bot.clone();
                                HStack::new(
                                    cx,
                                    ParamWidgetBase::build_view(
                                        UiData::params,
                                        move |pr| &pr.modseqs[seq_idx].mode,
                                        move |cx, param_data| {
                                            let is_mseg = param_data.make_lens(|pr| {
                                                pr.unmodulated_plain_value()
                                                    == ModSeqModeParam::Mseg
                                            });
                                            let p_c = p_pal.clone();
                                            let u_c = u_pal.clone();
                                            Binding::new(cx, is_mseg, move |cx, m| {
                                                let p_mode = p_c.clone();
                                                let u_mode = u_c.clone();
                                                if !m.get(cx) {
                                                    VStack::new(cx, move |cx| {
                                                        ModSeqPaletteGrid::new(
                                                            cx,
                                                            seq_idx,
                                                            p_mode.clone(),
                                                            u_mode.clone(),
                                                        )
                                                        .width(Stretch(1.0))
                                                        .height(Stretch(1.0));

                                                        HStack::new(cx, move |cx| {
                                                            let p_fill = p_mode.clone();
                                                            let u_fill = u_mode;
                                                            Button::new(
                                                                cx,
                                                                move |cx| {
                                                                    let shape = u_fill.active_stamp();
                                                                    let n = p_fill.modseqs[seq_idx]
                                                                        .num_steps
                                                                        .value()
                                                                        .clamp(2, MAX_MODSEQ_STEPS as i32)
                                                                        as usize;
                                                                    let mut bank =
                                                                        p_fill.modseq_steps.snapshot();
                                                                    for i in 0..n {
                                                                        bank.seqs[seq_idx][i] =
                                                                            ModSeqStep::stamped(
                                                                                shape, 1.0, 0.0,
                                                                            );
                                                                    }
                                                                    p_fill.modseq_steps.store_value(bank);
                                                                    cx.needs_redraw();
                                                                },
                                                                |cx| {
                                                                    bold_label(
                                                                        cx,
                                                                        "FILL",
                                                                        "preset-btn-label",
                                                                    )
                                                                    .child_space(Stretch(1.0))
                                                                    .hoverable(false)
                                                                },
                                                            )
                                                            .class("preset-btn")
                                                            .width(Stretch(1.0))
                                                            .height(Pixels(16.0));

                                                            let p_clr = p_mode;
                                                            Button::new(
                                                                cx,
                                                                move |cx| {
                                                                    let mut bank =
                                                                        p_clr.modseq_steps.snapshot();
                                                                    for step in
                                                                        bank.seqs[seq_idx].iter_mut()
                                                                    {
                                                                        *step = ModSeqStep::default();
                                                                    }
                                                                    p_clr.modseq_steps.store_value(bank);
                                                                    cx.needs_redraw();
                                                                },
                                                                |cx| {
                                                                    bold_label(
                                                                        cx,
                                                                        "CLEAR",
                                                                        "preset-btn-label",
                                                                    )
                                                                    .child_space(Stretch(1.0))
                                                                    .hoverable(false)
                                                                },
                                                            )
                                                            .class("preset-btn")
                                                            .width(Stretch(1.0))
                                                            .height(Pixels(16.0));
                                                        })
                                                        .col_between(Pixels(tokens::SPACE_SM))
                                                        .width(Stretch(1.0))
                                                        .height(Pixels(16.0));
                                                    })
                                                    .row_between(Pixels(tokens::SPACE_XS))
                                                    .width(Pixels(224.0))
                                                    .height(Stretch(1.0));
                                                } else {
                                                    VStack::new(cx, move |cx| {
                                                        LfoPresetBar::new(
                                                            cx,
                                                            seq_idx,
                                                            p_mode.clone(),
                                                        )
                                                        .width(Stretch(1.0))
                                                        .height(Pixels(28.0));

                                                        let p_rst = p_mode;
                                                        Button::new(
                                                            cx,
                                                            move |cx| {
                                                                let mut bank =
                                                                    p_rst.modseq_steps.snapshot();
                                                                bank.msegs[seq_idx] =
                                                                    j_syn_dsp::MsegCurve::default();
                                                                p_rst.modseq_steps.store_value(bank);
                                                                cx.emit(UiEvent::ModUiChanged);
                                                                cx.needs_redraw();
                                                            },
                                                            |cx| {
                                                                bold_label(
                                                                    cx,
                                                                    "RESET CURVE",
                                                                    "preset-btn-label",
                                                                )
                                                                .child_space(Stretch(1.0))
                                                                .hoverable(false)
                                                            },
                                                        )
                                                        .class("preset-btn")
                                                        .width(Stretch(1.0))
                                                        .height(Pixels(20.0));
                                                    })
                                                    .row_between(Pixels(tokens::SPACE_SM))
                                                    .top(Stretch(1.0))
                                                    .bottom(Stretch(1.0))
                                                    .width(Pixels(224.0))
                                                    .height(Auto);
                                                }
                                            });
                                        },
                                    ),
                                )
                                .width(Auto)
                                .height(Stretch(1.0));

                                let p_ctrl = p_bot;
                                let g_ctrl = g_bot;
                                let u_ctrl = u_bot;
                                let accent = subtab_accent(ModSeqSubTab::MSeq(seq_idx));
                                HStack::new(cx, move |cx| {
                                    VStack::new(cx, {
                                        let p = p_ctrl.clone();
                                        let g = g_ctrl.clone();
                                        move |cx| {
                                            build_labeled_dropdown(
                                                cx,
                                                "MODE",
                                                p.clone(),
                                                g.clone(),
                                                move |pr| &pr.modseqs[seq_idx].mode,
                                                MODE_ITEMS,
                                            );
                                            build_labeled_dropdown(
                                                cx,
                                                "PLAY",
                                                p,
                                                g,
                                                move |pr| &pr.modseqs[seq_idx].play_mode,
                                                PLAY_ITEMS,
                                            );
                                        }
                                    })
                                    .row_between(Pixels(tokens::SPACE_XS))
                                    .width(Pixels(62.0))
                                    .height(Auto)
                                    .top(Stretch(1.0))
                                    .bottom(Stretch(1.0));

                                    VStack::new(cx, {
                                        let p = p_ctrl.clone();
                                        let g = g_ctrl;
                                        move |cx| {
                                            build_labeled_dropdown(
                                                cx,
                                                "SYNC",
                                                p.clone(),
                                                g.clone(),
                                                move |pr| &pr.modseqs[seq_idx].sync_mode,
                                                SYNC_ITEMS,
                                            );
                                            build_labeled_dropdown(
                                                cx,
                                                "RETRIG",
                                                p,
                                                g,
                                                move |pr| &pr.modseqs[seq_idx].retrig,
                                                RETRIG_ITEMS,
                                            );
                                        }
                                    })
                                    .row_between(Pixels(tokens::SPACE_XS))
                                    .width(Pixels(62.0))
                                    .height(Auto)
                                    .top(Stretch(1.0))
                                    .bottom(Stretch(1.0));

                                    VStack::new(cx, {
                                        let u = u_ctrl.clone();
                                        move |cx| {
                                            HStack::new(
                                                cx,
                                                ParamWidgetBase::build_view(
                                                    UiData::params,
                                                    move |pr| &pr.modseqs[seq_idx].mode,
                                                    move |cx, param_data| {
                                                        let is_mseg = param_data.make_lens(|pr| {
                                                            pr.unmodulated_plain_value()
                                                                == ModSeqModeParam::Mseg
                                                        });
                                                        let u_s = u.clone();
                                                        Binding::new(cx, is_mseg, move |cx, m| {
                                                            if m.get(cx) {
                                                                let u_press = u_s.clone();
                                                                let u_chk = u_s.clone();
                                                                VStack::new(cx, move |cx| {
                                                                    bold_label(cx, "GRID", "mini-label")
                                                                        .width(Stretch(1.0))
                                                                        .height(Pixels(9.0))
                                                                        .child_space(Stretch(1.0));
                                                                    Button::new(
                                                                        cx,
                                                                        move |cx| {
                                                                            u_press.toggle_mseg_snap();
                                                                            cx.emit(UiEvent::ModUiChanged);
                                                                            cx.needs_redraw();
                                                                        },
                                                                        |cx| {
                                                                            bold_label(
                                                                                cx,
                                                                                "SNAP",
                                                                                "snap-btn-label",
                                                                            )
                                                                            .child_space(Stretch(1.0))
                                                                            .hoverable(false)
                                                                        },
                                                                    )
                                                                    .class("snap-btn")
                                                                    .checked(UiData::mod_ui_rev.map(
                                                                        move |_| u_chk.mseg_snap(),
                                                                    ))
                                                                    .width(Stretch(1.0))
                                                                    .height(Pixels(18.0));
                                                                })
                                                                .width(Stretch(1.0))
                                                                .height(Auto)
                                                                .row_between(Pixels(1.0));
                                                            } else {
                                                                build_flex_pill_cell(
                                                                    cx,
                                                                    UiData::params,
                                                                    move |pr| {
                                                                        &pr.modseqs[seq_idx].num_steps
                                                                    },
                                                                    "STEPS",
                                                                    14,
                                                                );
                                                            }
                                                        });
                                                    },
                                                ),
                                            )
                                            .width(Stretch(1.0))
                                            .height(Pixels(28.0));
                                        }
                                    })
                                    .width(Pixels(52.0))
                                    .height(Auto)
                                    .top(Stretch(1.0))
                                    .bottom(Stretch(1.0));

                                    let p_fmt = p_ctrl.clone();
                                    build_knob_cell_mod_fmt(
                                        cx,
                                        UiData::params,
                                        move |pr| &pr.modseqs[seq_idx].rate,
                                        "RATE",
                                        false,
                                        accent,
                                        tokens::KNOB_STD,
                                        46.0,
                                        ModDestination::modseq_rate(seq_idx),
                                        u_ctrl,
                                        bottom_tab_tag(BottomTab::ModSeq),
                                        Arc::new(move |norm, _opt_mod_offset| {
                                            let mode: j_syn_dsp::ModSeqSyncMode =
                                                p_fmt.modseqs[seq_idx].sync_mode.value().into();
                                            if mode == j_syn_dsp::ModSeqSyncMode::Free {
                                                format!(
                                                    "{:.2} Hz",
                                                    j_syn_dsp::modseq_free_norm_to_hz(norm)
                                                )
                                            } else {
                                                j_syn_dsp::modseq_division_label(mode, norm)
                                                    .to_string()
                                            }
                                        }),
                                    );

                                    build_knob_cell(
                                        cx,
                                        UiData::params,
                                        move |pr| &pr.modseqs[seq_idx].smooth,
                                        "SMOOTH",
                                        false,
                                        CORAL_RGB,
                                        tokens::KNOB_STD,
                                        44.0,
                                    );
                                })
                                .col_between(Pixels(tokens::SPACE_XS))
                                .child_top(Stretch(1.0))
                                .child_bottom(Stretch(1.0))
                                .width(Stretch(1.0))
                                .height(Stretch(1.0));
                            }
                        })
                        .col_between(Pixels(tokens::SPACE_MD))
                        .width(Stretch(1.0))
                        .height(Pixels(78.0));
                    })
                    .row_between(Pixels(tokens::SPACE_SM))
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
                }
                ModSeqSubTab::Env(env_idx) => {
                    let (atk_fn, dec_fn, sus_fn, rel_fn) = env_param_fns(env_idx);
                    VStack::new(cx, move |cx| {
                        AdsrPreviewScope::new(cx, env_idx, p_sub.clone(), m_sub.clone())
                            .width(Stretch(1.0))
                            .height(Stretch(1.0));

                        HStack::new(cx, move |cx| {
                            let accent = subtab_accent(ModSeqSubTab::Env(env_idx));
                            build_knob_cell_with_readout(
                                cx,
                                UiData::params,
                                atk_fn,
                                "ATTACK",
                                false,
                                accent,
                                tokens::KNOB_FLT,
                                84.0,
                            );
                            build_knob_cell_with_readout(
                                cx,
                                UiData::params,
                                dec_fn,
                                "DECAY",
                                false,
                                accent,
                                tokens::KNOB_FLT,
                                84.0,
                            );
                            build_knob_cell_with_readout(
                                cx,
                                UiData::params,
                                sus_fn,
                                "SUSTAIN",
                                false,
                                GOLD_RGB,
                                tokens::KNOB_FLT,
                                84.0,
                            );
                            build_knob_cell_with_readout(
                                cx,
                                UiData::params,
                                rel_fn,
                                "RELEASE",
                                false,
                                CORAL_RGB,
                                tokens::KNOB_FLT,
                                84.0,
                            );
                        })
                        .col_between(Pixels(tokens::SPACE_LG))
                        .child_left(Stretch(1.0))
                        .child_right(Stretch(1.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0))
                        .width(Stretch(1.0))
                        .height(Pixels(74.0));
                    })
                    .row_between(Pixels(tokens::SPACE_MD))
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
                }
            }
        });
    })
    .class("hardware-module")
    .class("drawer-module")
    .row_between(Pixels(2.0))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}
