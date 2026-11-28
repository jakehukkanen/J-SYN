//! `MODSEQ` bottom drawer: 8 hybrid 32-step (`MODSEQ` 2×16) / freeform breakpoint (`LFO`) modulators
//! (`MSEQ 1`–`MSEQ 8`) and 8 envelopes (`ENV 1`–`ENV 8`).

mod bottom_bars;
mod curve_canvas;
mod mseg_canvas;
mod palette;

use std::sync::Arc;

use nih_plug::prelude::{GuiContext, Param};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

use self::curve_canvas::ModSeqCurveCanvas;
use super::mod_bar::ModUiState;
use super::tokens;
use super::UiData;
use crate::{JSynPluginParams, ModSeqModeParam};

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

pub(in super::super) fn build_modseq_tab(
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
                        ModSeqCurveCanvas::new(cx, seq_idx, p_sub.clone(), m_sub.clone())
                            .width(Stretch(1.0))
                            .height(Stretch(1.0));

                        let p_bot = p_sub.clone();
                        let g_bot = g_sub.clone();
                        let u_bot = m_sub.clone();
                        HStack::new(
                            cx,
                            ParamWidgetBase::build_view(
                                UiData::params,
                                move |pr| &pr.modseqs[seq_idx].mode,
                                move |cx, param_data| {
                                    let is_mseg = param_data.make_lens(|pr| {
                                        pr.unmodulated_plain_value() == ModSeqModeParam::Mseg
                                    });
                                    let p_m = p_bot.clone();
                                    let g_m = g_bot.clone();
                                    let u_m = u_bot.clone();
                                    Binding::new(cx, is_mseg, move |cx, m| {
                                        if !m.get(cx) {
                                            bottom_bars::build_performer_bottom_bar(
                                                cx,
                                                seq_idx,
                                                p_m.clone(),
                                                g_m.clone(),
                                                u_m.clone(),
                                            );
                                        } else {
                                            bottom_bars::build_lfo_bottom_bar(
                                                cx,
                                                seq_idx,
                                                p_m.clone(),
                                                g_m.clone(),
                                                u_m.clone(),
                                            );
                                        }
                                    });
                                },
                            ),
                        )
                        .width(Stretch(1.0))
                        .height(Auto);
                    })
                    .row_between(Pixels(tokens::SPACE_SM))
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
                }
                ModSeqSubTab::Env(env_idx) => {
                    bottom_bars::build_env_tab(cx, env_idx, p_sub, m_sub);
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
