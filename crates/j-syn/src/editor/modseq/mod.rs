//! `MODSEQ` bottom drawer: 8 hybrid 32-step (`MODSEQ` 2×16) / freeform breakpoint (`LFO`) modulators
//! (`MSEQ 1`–`MSEQ 8`) and 8 envelopes (`ENV 1`–`ENV 8`).

mod curve_canvas;
mod palette;

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModDestination;
use j_syn_dsp::{MODSEQ_ROW_STEPS, ModSeqStep, fill_modseq_row_with_tile};
use nih_plug::prelude::{FloatParam, GuiContext, Param};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

use self::curve_canvas::ModSeqCurveCanvas;
use self::palette::{AdsrPreviewScope, LfoPresetBar, ModSeqPaletteGrid};
use super::controls::{
    bold_label, build_enum_dropdown, build_flex_pill_cell, build_knob_cell,
    build_knob_cell_mod_fmt, build_knob_cell_with_readout,
};
use super::mod_bar::{ModUiState, bottom_tab_tag};
use super::tokens;
use super::{BottomTab, UiData, UiEvent};
use crate::{
    JSynPluginParams, ModSeqModeParam, ModSeqPlayParam, ModSeqRetrigParam, ModSeqRowModeParam,
    ModSeqRowStartParam, ModSeqSyncParam,
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
    ("Step Ratio", ModSeqSyncParam::StepRatio),
    ("Step ms", ModSeqSyncParam::StepMs),
];

const LFO_SYNC_ITEMS: &[(&str, ModSeqSyncParam)] = &[
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

const ROW_START_ITEMS: &[(&str, ModSeqRowStartParam)] = &[
    ("Row 1", ModSeqRowStartParam::Row1),
    ("Row 2", ModSeqRowStartParam::Row2),
];

const ROW_MODE_ITEMS: &[(&str, ModSeqRowModeParam)] = &[
    ("Continue", ModSeqRowModeParam::Continue),
    ("Loop", ModSeqRowModeParam::LoopRow),
];

/// Wraps a labeled dropdown so it also acts as a Mod Matrix drop target and right-click target.
struct ModDestDropWrapper {
    dest: ModDestination,
    mod_ui: Arc<ModUiState>,
    tab_tag: u8,
}

impl ModDestDropWrapper {
    fn new(
        cx: &mut Context,
        dest: ModDestination,
        mod_ui: Arc<ModUiState>,
        tab_tag: u8,
        content: impl FnOnce(&mut Context),
    ) -> Handle<'_, Self> {
        Self {
            dest,
            mod_ui,
            tab_tag,
        }
        .build(cx, content)
    }
}

impl Drop for ModDestDropWrapper {
    fn drop(&mut self) {
        self.mod_ui.unregister_drop_rect(self.dest);
    }
}

impl View for ModDestDropWrapper {
    fn element(&self) -> Option<&'static str> {
        Some("mod-dest-drop-wrapper")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Right) = window_event {
                cx.emit(j_syn_ui::UiKitEvent::ShowModContextMenu(
                    self.dest,
                    cx.mouse().cursorx,
                    cx.mouse().cursory,
                ));
                meta.consume();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        self.mod_ui
            .register_drop_rect(self.dest, self.tab_tag, bounds);
        let dpx = cx.scale_factor();
        if self.mod_ui.is_drop_target(self.dest) {
            let mut ring = vg::Path::new();
            ring.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 3.0 * dpx);
            let pal = super::theme::current_palette(cx);
            let mut p = vg::Paint::color(super::theme::vg_color(&pal.accent_primary, 220));
            p.set_line_width(1.2 * dpx);
            canvas.stroke_path(&ring, &p);
        }
        if let Some((_, slot)) = self.mod_ui.primary_slot_for_dest(self.dest) {
            let pal = super::theme::current_palette(cx);
            let (r, g, b) = j_syn_ui::theme::mod_source_rgb(&pal, slot.source);
            let mut dot = vg::Path::new();
            dot.circle(
                bounds.x + bounds.w - 4.0 * dpx,
                bounds.y + 4.5 * dpx,
                2.5 * dpx,
            );
            canvas.fill_path(&dot, &vg::Paint::color(vg::Color::rgb(r, g, b)));
        }
    }
}

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
            .height(Pixels(tokens::KNOB_TITLE_COMPACT_H))
            .child_space(Stretch(1.0));
        build_enum_dropdown(cx, params.clone(), gui_context.clone(), map_fn, items)
            .width(Stretch(1.0))
            .height(Pixels(tokens::CONTROL_H));
    })
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(1.0));
}

fn build_modseq_dropdown_cols(
    cx: &mut Context,
    seq_idx: usize,
    params: Arc<JSynPluginParams>,
    gui_context: Arc<dyn GuiContext>,
    mod_ui: Arc<ModUiState>,
    include_row_controls: bool,
) {
    let row_gap = if include_row_controls {
        tokens::SPACE_SM
    } else {
        tokens::SPACE_XS
    };

    let col1 = VStack::new(cx, {
        let p = params.clone();
        let g = gui_context.clone();
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
                p.clone(),
                g.clone(),
                move |pr| &pr.modseqs[seq_idx].play_mode,
                PLAY_ITEMS,
            );
            if include_row_controls {
                build_labeled_dropdown(
                    cx,
                    "ROW MODE",
                    p,
                    g,
                    move |pr| &pr.modseqs[seq_idx].row_mode,
                    ROW_MODE_ITEMS,
                );
            }
        }
    })
    .row_between(Pixels(row_gap))
    .width(Pixels(tokens::MODSEQ_DROPDOWN_W))
    .height(Auto);
    if !include_row_controls {
        col1.top(Stretch(1.0)).bottom(Stretch(1.0));
    }

    let col2 = VStack::new(cx, move |cx| {
        if include_row_controls {
            build_labeled_dropdown(
                cx,
                "SYNC",
                params.clone(),
                gui_context.clone(),
                move |pr| &pr.modseqs[seq_idx].sync_mode,
                SYNC_ITEMS,
            );
        } else {
            let p_sync = params.clone();
            let g_sync = gui_context.clone();
            VStack::new(cx, move |cx| {
                bold_label(cx, "SYNC", "mini-label")
                    .width(Stretch(1.0))
                    .height(Pixels(tokens::KNOB_TITLE_COMPACT_H))
                    .child_space(Stretch(1.0));
                j_syn_ui::controls::build_enum_dropdown_with_override(
                    cx,
                    UiData::params,
                    p_sync,
                    g_sync,
                    move |pr| &pr.modseqs[seq_idx].sync_mode,
                    LFO_SYNC_ITEMS,
                    move |pr| match pr.modseqs[seq_idx].sync_mode.value() {
                        ModSeqSyncParam::StepRatio | ModSeqSyncParam::StepMs => Some("Free"),
                        _ => None,
                    },
                )
                .width(Stretch(1.0))
                .height(Pixels(tokens::CONTROL_H));
            })
            .width(Stretch(1.0))
            .height(Auto)
            .row_between(Pixels(1.0));
        }
        build_labeled_dropdown(
            cx,
            "RETRIG",
            params.clone(),
            gui_context.clone(),
            move |pr| &pr.modseqs[seq_idx].retrig,
            RETRIG_ITEMS,
        );
        if include_row_controls {
            ModDestDropWrapper::new(
                cx,
                ModDestination::modseq_row(seq_idx),
                mod_ui,
                bottom_tab_tag(BottomTab::ModSeq),
                move |cx| {
                    build_labeled_dropdown(
                        cx,
                        "START ROW",
                        params,
                        gui_context,
                        move |pr| &pr.modseqs[seq_idx].start_row,
                        ROW_START_ITEMS,
                    );
                },
            )
            .width(Stretch(1.0))
            .height(Auto);
        }
    })
    .row_between(Pixels(row_gap))
    .width(Pixels(tokens::MODSEQ_DROPDOWN_W))
    .height(Auto);
    if !include_row_controls {
        col2.top(Stretch(1.0)).bottom(Stretch(1.0));
    }
}

fn build_modseq_rate_smooth_knobs(
    cx: &mut Context,
    seq_idx: usize,
    params: Arc<JSynPluginParams>,
    mod_ui: Arc<ModUiState>,
) {
    let p_fmt = params;
    build_knob_cell_mod_fmt(
        cx,
        UiData::params,
        move |pr| &pr.modseqs[seq_idx].rate,
        "RATE",
        false,
        tokens::KNOB_STD,
        46.0,
        ModDestination::modseq_rate(seq_idx),
        mod_ui,
        bottom_tab_tag(BottomTab::ModSeq),
        Arc::new(move |norm, _opt_mod_offset| {
            let mode: j_syn_dsp::ModSeqSyncMode = p_fmt.modseqs[seq_idx].sync_mode.value().into();
            if matches!(
                mode,
                j_syn_dsp::ModSeqSyncMode::Free
                    | j_syn_dsp::ModSeqSyncMode::StepRatio
                    | j_syn_dsp::ModSeqSyncMode::StepMs
            ) {
                format!("{:.2} Hz", j_syn_dsp::modseq_free_norm_to_hz(norm))
            } else {
                j_syn_dsp::modseq_division_label(mode, norm).to_string()
            }
        }),
    );

    build_knob_cell(
        cx,
        UiData::params,
        move |pr| &pr.modseqs[seq_idx].smooth,
        "SMOOTH",
        false,
        tokens::KNOB_STD,
        44.0,
    );
}

fn format_step_ms(ms: f32) -> String {
    if ms < 10.0 {
        format!("{ms:.2} ms")
    } else if ms < 100.0 {
        format!("{ms:.1} ms")
    } else {
        format!("{ms:.0} ms")
    }
}

fn build_modseq_timing_controls(
    cx: &mut Context,
    seq_idx: usize,
    params: Arc<JSynPluginParams>,
    mod_ui: Arc<ModUiState>,
) {
    HStack::new(
        cx,
        ParamWidgetBase::build_view(
            UiData::params,
            move |pr| &pr.modseqs[seq_idx].sync_mode,
            move |cx, param_data| {
                let sync_lens = param_data.make_lens(|pr| match pr.unmodulated_plain_value() {
                    ModSeqSyncParam::StepRatio => 1_u8,
                    ModSeqSyncParam::StepMs => 2_u8,
                    _ => 0_u8,
                });
                let p_t = params.clone();
                let u_t = mod_ui.clone();
                Binding::new(cx, sync_lens, move |cx, sync_m| {
                    let p_cur = p_t.clone();
                    let u_cur = u_t.clone();
                    match sync_m.get(cx) {
                        1 => {
                            HStack::new(cx, move |cx| {
                                VStack::new(cx, move |cx| {
                                    bold_label(cx, "RATIO", "mini-label")
                                        .width(Stretch(1.0))
                                        .height(Pixels(tokens::KNOB_TITLE_COMPACT_H))
                                        .child_space(Stretch(1.0));
                                    j_syn_ui::widgets::ParamStepPill::new(
                                        cx,
                                        UiData::params,
                                        move |pr| &pr.modseqs[seq_idx].step_ratio_num,
                                        16,
                                    )
                                    .width(Stretch(1.0))
                                    .height(Pixels(tokens::READOUT_H));
                                    j_syn_ui::widgets::ParamStepPill::new(
                                        cx,
                                        UiData::params,
                                        move |pr| &pr.modseqs[seq_idx].step_ratio_den,
                                        64,
                                    )
                                    .width(Stretch(1.0))
                                    .height(Pixels(tokens::READOUT_H));
                                })
                                .row_between(Pixels(2.0))
                                .width(Pixels(46.0))
                                .height(Auto);

                                build_knob_cell(
                                    cx,
                                    UiData::params,
                                    move |pr| &pr.modseqs[seq_idx].smooth,
                                    "SMOOTH",
                                    false,
                                    tokens::KNOB_STD,
                                    44.0,
                                );
                            })
                            .col_between(Pixels(tokens::SPACE_SM))
                            .child_left(Stretch(1.0))
                            .child_right(Stretch(1.0))
                            .width(Stretch(1.0))
                            .height(Auto);
                        }
                        2 => {
                            HStack::new(cx, move |cx| {
                                build_knob_cell_mod_fmt(
                                    cx,
                                    UiData::params,
                                    move |pr| &pr.modseqs[seq_idx].rate,
                                    "STEP MS",
                                    false,
                                    tokens::KNOB_STD,
                                    46.0,
                                    ModDestination::modseq_rate(seq_idx),
                                    u_cur,
                                    bottom_tab_tag(BottomTab::ModSeq),
                                    Arc::new(move |norm, _opt_mod_offset| {
                                        format_step_ms(j_syn_dsp::modseq_step_ms_from_norm(norm))
                                    }),
                                );
                                build_knob_cell(
                                    cx,
                                    UiData::params,
                                    move |pr| &pr.modseqs[seq_idx].smooth,
                                    "SMOOTH",
                                    false,
                                    tokens::KNOB_STD,
                                    44.0,
                                );
                            })
                            .col_between(Pixels(tokens::SPACE_SM))
                            .child_left(Stretch(1.0))
                            .child_right(Stretch(1.0))
                            .width(Stretch(1.0))
                            .height(Auto);
                        }
                        _ => {
                            HStack::new(cx, move |cx| {
                                build_modseq_rate_smooth_knobs(cx, seq_idx, p_cur, u_cur);
                            })
                            .col_between(Pixels(tokens::SPACE_SM))
                            .child_left(Stretch(1.0))
                            .child_right(Stretch(1.0))
                            .width(Stretch(1.0))
                            .height(Auto);
                        }
                    }
                });
            },
        ),
    )
    .width(Stretch(1.0))
    .height(Auto);
}

fn build_preset_action_btn<'a>(
    cx: &'a mut Context,
    label: &'static str,
    on_press: impl Fn(&mut EventContext) + Send + Sync + 'static,
) -> Handle<'a, Button> {
    Button::new(cx, on_press, move |cx| {
        bold_label(cx, label, "preset-btn-label")
            .child_space(Stretch(1.0))
            .hoverable(false)
    })
    .class("preset-btn")
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
        3 => (
            |p| &p.mod_envs[2].attack,
            |p| &p.mod_envs[2].decay,
            |p| &p.mod_envs[2].sustain,
            |p| &p.mod_envs[2].release,
        ),
        4 => (
            |p| &p.mod_envs[3].attack,
            |p| &p.mod_envs[3].decay,
            |p| &p.mod_envs[3].sustain,
            |p| &p.mod_envs[3].release,
        ),
        5 => (
            |p| &p.mod_envs[4].attack,
            |p| &p.mod_envs[4].decay,
            |p| &p.mod_envs[4].sustain,
            |p| &p.mod_envs[4].release,
        ),
        6 => (
            |p| &p.mod_envs[5].attack,
            |p| &p.mod_envs[5].decay,
            |p| &p.mod_envs[5].sustain,
            |p| &p.mod_envs[5].release,
        ),
        _ => (
            |p| &p.mod_envs[6].attack,
            |p| &p.mod_envs[6].decay,
            |p| &p.mod_envs[6].sustain,
            |p| &p.mod_envs[6].release,
        ),
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
                                        let p_mode = p_m.clone();
                                        let g_mode = g_m.clone();
                                        let u_mode = u_m.clone();
                                        if !m.get(cx) {
                                            HStack::new(cx, move |cx| {
                                                // Left: 4x9 palette + active-row FILL / CLEAR
                                                let p_pal = p_mode.clone();
                                                let u_pal = u_mode.clone();
                                                VStack::new(cx, move |cx| {
                                                    ModSeqPaletteGrid::new(
                                                        cx,
                                                        seq_idx,
                                                        p_pal.clone(),
                                                        u_pal.clone(),
                                                    )
                                                    .width(Stretch(1.0))
                                                    .height(Stretch(1.0));

                                                    HStack::new(cx, move |cx| {
                                                        let p_fill = p_pal.clone();
                                                        let u_fill = u_pal.clone();
                                                        build_preset_action_btn(
                                                            cx,
                                                            "FILL",
                                                            move |cx| {
                                                                let tile = u_fill.active_tile();
                                                                let row = u_fill
                                                                    .active_modseq_row(seq_idx);
                                                                let mut bank =
                                                                    p_fill.modseq_steps.snapshot();
                                                                fill_modseq_row_with_tile(
                                                                    &mut bank.seqs[seq_idx],
                                                                    row,
                                                                    tile,
                                                                );
                                                                p_fill
                                                                    .modseq_steps
                                                                    .store_value(bank);
                                                                cx.needs_redraw();
                                                            },
                                                        )
                                                        .width(Stretch(1.0))
                                                        .height(Pixels(18.0));

                                                        let p_clr = p_pal;
                                                        let u_clr = u_pal;
                                                        build_preset_action_btn(
                                                            cx,
                                                            "CLEAR",
                                                            move |cx| {
                                                                let row_start = u_clr
                                                                    .active_modseq_row(seq_idx)
                                                                    * MODSEQ_ROW_STEPS;
                                                                let mut bank =
                                                                    p_clr.modseq_steps.snapshot();
                                                                for step in bank.seqs[seq_idx]
                                                                    [row_start
                                                                        ..(row_start
                                                                            + MODSEQ_ROW_STEPS)]
                                                                    .iter_mut()
                                                                {
                                                                    *step = ModSeqStep::default();
                                                                }
                                                                p_clr
                                                                    .modseq_steps
                                                                    .store_value(bank);
                                                                cx.needs_redraw();
                                                            },
                                                        )
                                                        .width(Stretch(1.0))
                                                        .height(Pixels(18.0));
                                                    })
                                                    .col_between(Pixels(tokens::SPACE_SM))
                                                    .width(Stretch(1.0))
                                                    .height(Pixels(18.0));
                                                })
                                                .row_between(Pixels(tokens::SPACE_XS))
                                                .width(Pixels(tokens::MODSEQ_PALETTE_W))
                                                .height(Stretch(1.0));

                                                // Right: 3-row dropdowns + STEPS / CPY / SWP / RATE / SMOOTH
                                                let p_ctrl = p_mode;
                                                let g_ctrl = g_mode;
                                                let u_ctrl = u_mode;
                                                HStack::new(cx, move |cx| {
                                                    build_modseq_dropdown_cols(
                                                        cx,
                                                        seq_idx,
                                                        p_ctrl.clone(),
                                                        g_ctrl,
                                                        u_ctrl.clone(),
                                                        true,
                                                    );

                                                    let p_col3 = p_ctrl;
                                                    let u_col3 = u_ctrl;
                                                    VStack::new(cx, move |cx| {
                                                        build_flex_pill_cell(
                                                            cx,
                                                            UiData::params,
                                                            move |pr| {
                                                                &pr.modseqs[seq_idx].num_steps
                                                            },
                                                            "STEPS",
                                                            31,
                                                        );

                                                        HStack::new(cx, {
                                                            let p_cp = p_col3.clone();
                                                            let p_sw = p_col3.clone();
                                                            move |cx| {
                                                                build_preset_action_btn(
                                                                    cx,
                                                                    "CPY",
                                                                    move |cx| {
                                                                        let mut bank = p_cp
                                                                            .modseq_steps
                                                                            .snapshot();
                                                                        for c in 0..MODSEQ_ROW_STEPS
                                                                        {
                                                                            bank.seqs[seq_idx]
                                                                                [MODSEQ_ROW_STEPS
                                                                                    + c] = bank
                                                                                .seqs[seq_idx][c];
                                                                        }
                                                                        p_cp.modseq_steps
                                                                            .store_value(bank);
                                                                        cx.needs_redraw();
                                                                    },
                                                                )
                                                                .width(Stretch(1.0))
                                                                .height(Pixels(18.0));

                                                                build_preset_action_btn(
                                                                    cx,
                                                                    "SWP",
                                                                    move |cx| {
                                                                        let mut bank = p_sw
                                                                            .modseq_steps
                                                                            .snapshot();
                                                                        for c in 0..MODSEQ_ROW_STEPS
                                                                        {
                                                                            bank.seqs[seq_idx]
                                                                                .swap(
                                                                                    c,
                                                                                    MODSEQ_ROW_STEPS
                                                                                        + c,
                                                                                );
                                                                        }
                                                                        p_sw.modseq_steps
                                                                            .store_value(bank);
                                                                        cx.needs_redraw();
                                                                    },
                                                                )
                                                                .width(Stretch(1.0))
                                                                .height(Pixels(18.0));
                                                            }
                                                        })
                                                        .col_between(Pixels(tokens::SPACE_SM))
                                                        .width(Stretch(1.0))
                                                        .height(Pixels(18.0));

                                                        build_modseq_timing_controls(
                                                            cx, seq_idx, p_col3, u_col3,
                                                        );
                                                    })
                                                    .row_between(Pixels(tokens::SPACE_SM))
                                                    .width(Stretch(1.0))
                                                    .height(Stretch(1.0));
                                                })
                                                .col_between(Pixels(tokens::SPACE_SM))
                                                .child_top(Pixels(2.0))
                                                .child_bottom(Stretch(1.0))
                                                .width(Stretch(1.0))
                                                .height(Stretch(1.0));
                                            })
                                            .col_between(Pixels(tokens::SPACE_MD))
                                            .width(Stretch(1.0))
                                            .height(Pixels(tokens::MODSEQ_BOTTOM_BAR_H));
                                        } else {
                                            HStack::new(cx, move |cx| {
                                                let p_lfo = p_mode.clone();
                                                VStack::new(cx, move |cx| {
                                                    LfoPresetBar::new(cx, seq_idx, p_lfo.clone())
                                                        .width(Stretch(1.0))
                                                        .height(Pixels(28.0));

                                                    let p_rst = p_lfo;
                                                    build_preset_action_btn(
                                                        cx,
                                                        "RESET CURVE",
                                                        move |cx| {
                                                            let mut bank =
                                                                p_rst.modseq_steps.snapshot();
                                                            bank.msegs[seq_idx] =
                                                                j_syn_dsp::MsegCurve::default();
                                                            p_rst.modseq_steps.store_value(bank);
                                                            cx.emit(UiEvent::ModUiChanged);
                                                            cx.needs_redraw();
                                                        },
                                                    )
                                                    .width(Stretch(1.0))
                                                    .height(Pixels(20.0));
                                                })
                                                .row_between(Pixels(tokens::SPACE_SM))
                                                .top(Stretch(1.0))
                                                .bottom(Stretch(1.0))
                                                .width(Pixels(224.0))
                                                .height(Auto);

                                                let p_ctrl = p_mode;
                                                let g_ctrl = g_mode;
                                                let u_ctrl = u_mode;
                                                HStack::new(cx, move |cx| {
                                                    build_modseq_dropdown_cols(
                                                        cx,
                                                        seq_idx,
                                                        p_ctrl.clone(),
                                                        g_ctrl,
                                                        u_ctrl.clone(),
                                                        false,
                                                    );

                                                    VStack::new(cx, {
                                                        let u_s = u_ctrl.clone();
                                                        move |cx| {
                                                            let u_press = u_s.clone();
                                                            let u_chk = u_s;
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
                                                            .checked(
                                                                UiData::mod_ui_rev.map(move |_| {
                                                                    u_chk.mseg_snap()
                                                                }),
                                                            )
                                                            .width(Stretch(1.0))
                                                            .height(Pixels(18.0));
                                                        }
                                                    })
                                                    .row_between(Pixels(1.0))
                                                    .width(Pixels(48.0))
                                                    .height(Auto)
                                                    .top(Stretch(1.0))
                                                    .bottom(Stretch(1.0));

                                                    build_modseq_rate_smooth_knobs(
                                                        cx, seq_idx, p_ctrl, u_ctrl,
                                                    );
                                                })
                                                .col_between(Pixels(tokens::SPACE_XS))
                                                .child_top(Stretch(1.0))
                                                .child_bottom(Stretch(1.0))
                                                .width(Stretch(1.0))
                                                .height(Stretch(1.0));
                                            })
                                            .col_between(Pixels(tokens::SPACE_MD))
                                            .width(Stretch(1.0))
                                            .height(Pixels(tokens::MODSEQ_LFO_BOTTOM_BAR_H));
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
                    let (atk_fn, dec_fn, sus_fn, rel_fn) = env_param_fns(env_idx);
                    VStack::new(cx, move |cx| {
                        AdsrPreviewScope::new(cx, env_idx, p_sub.clone(), m_sub.clone())
                            .width(Stretch(1.0))
                            .height(Stretch(1.0));

                        HStack::new(cx, move |cx| {
                            build_knob_cell_with_readout(
                                cx,
                                UiData::params,
                                atk_fn,
                                "ATTACK",
                                false,
                                tokens::KNOB_FLT,
                                84.0,
                            );
                            build_knob_cell_with_readout(
                                cx,
                                UiData::params,
                                dec_fn,
                                "DECAY",
                                false,
                                tokens::KNOB_FLT,
                                84.0,
                            );
                            build_knob_cell_with_readout(
                                cx,
                                UiData::params,
                                sus_fn,
                                "SUSTAIN",
                                false,
                                tokens::KNOB_FLT,
                                84.0,
                            );
                            build_knob_cell_with_readout(
                                cx,
                                UiData::params,
                                rel_fn,
                                "RELEASE",
                                false,
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
