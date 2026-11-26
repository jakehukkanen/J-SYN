//! Permanent bottom-center Macro (`2×4` `M1`–`M8`) & Performance Source (`VEL..SLD`) dock.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModMatrixSource;
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{build_param_readout_box, build_rotary_knob_with_opt_mod};
use super::mod_bar::{ModSourcePill, ModUiState};
use super::tokens;
use super::{UiData, UiEvent};

pub(super) fn build_macros_tab(cx: &mut Context, mod_ui: Arc<ModUiState>) {
    HStack::new(cx, move |cx| {
        // Left vertical Performance Source column: VEL, KEY, MW, AT, SLD
        let perf_ui = mod_ui.clone();
        VStack::new(cx, move |cx| {
            for &src in &ModMatrixSource::PERF {
                ModSourcePill::new(cx, src, perf_ui.clone())
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
            }
        })
        .row_between(Pixels(tokens::SPACE_XS))
        .width(Pixels(tokens::PERF_PILL_COL_W))
        .height(Stretch(1.0));

        // Right: Shrunk 2 × 4 Macro Grid (M1–M4 top row, M5–M8 bottom row)
        VStack::new(cx, move |cx| {
            for row in 0..2 {
                let row_ui = mod_ui.clone();
                HStack::new(cx, move |cx| {
                    for col in 0..4 {
                        build_macro_card(cx, row * 4 + col, row_ui.clone());
                    }
                })
                .col_between(Pixels(tokens::SPACE_SM))
                .width(Stretch(1.0))
                .height(Stretch(1.0));
            }
        })
        .row_between(Pixels(tokens::SPACE_XS))
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })
    .class("hardware-module")
    .class("drawer-module")
    .col_between(Pixels(tokens::SPACE_SM))
    .width(Stretch(1.0))
    .height(Pixels(tokens::MACROS_DOCK_H));
}

fn build_macro_card(cx: &mut Context, idx: usize, mod_ui: Arc<ModUiState>) {
    let source = ModMatrixSource::MACROS[idx];
    VStack::new(cx, move |cx| {
        HStack::new(cx, move |cx| {
            ModSourcePill::new(cx, source, mod_ui.clone())
                .width(Pixels(tokens::MACRO_PILL_W))
                .height(Pixels(tokens::MACRO_HDR_H));

            Textbox::new(
                cx,
                UiData::macro_names.map(move |names| names[idx].clone()),
            )
            .on_edit(move |cx, text| {
                cx.emit(UiEvent::SetMacroName(idx, text));
            })
            .on_submit(move |cx, text, _| {
                cx.emit(UiEvent::SetMacroName(idx, text));
            })
            .class("macro-name-input")
            .width(Stretch(1.0))
            .height(Pixels(tokens::MACRO_HDR_H));
        })
        .col_between(Pixels(tokens::SPACE_XS))
        .width(Stretch(1.0))
        .height(Pixels(tokens::MACRO_HDR_H));

        HStack::new(cx, move |cx| {
            build_rotary_knob_with_opt_mod(
                cx,
                UiData::params,
                move |p| &p.macros[idx].value,
                false,
                None,
            )
            .width(Pixels(tokens::KNOB_COMPACT))
            .height(Pixels(tokens::KNOB_COMPACT))
            .top(Stretch(1.0))
            .bottom(Stretch(1.0));

            build_param_readout_box(cx, UiData::params, move |p| &p.macros[idx].value)
                .width(Stretch(1.0))
                .height(Pixels(tokens::READOUT_H))
                .top(Stretch(1.0))
                .bottom(Stretch(1.0));
        })
        .col_between(Pixels(tokens::SPACE_XS))
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })
    .class("macro-card")
    .width(Stretch(1.0))
    .height(Stretch(1.0))
    .row_between(Pixels(tokens::SPACE_XS));
}
