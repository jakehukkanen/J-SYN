//! Permanent bottom-center Macro (`2×4` `M1`–`M8`) & Performance Source dock.

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModMatrixSource;
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{build_param_readout_box, build_rotary_knob_with_opt_mod};
use super::mod_bar::{build_mod_source_bar, ModSourcePill, ModUiState};
use super::tokens;
use super::{UiData, UiEvent, AMBER_RGB};

pub(super) fn build_macros_tab(cx: &mut Context, mod_ui: Arc<ModUiState>) {
    VStack::new(cx, move |cx| {
        HStack::new(cx, {
            let mod_bar = mod_ui.clone();
            move |cx| {
                build_mod_source_bar(cx, mod_bar);
            }
        })
        .class("tab-bar")
        .col_between(Pixels(tokens::SPACE_SM))
        .height(Pixels(24.0))
        .width(Stretch(1.0));

        // 2 × 4 Macro Grid: M1–M4 top row, M5–M8 bottom row
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
        .row_between(Pixels(tokens::SPACE_SM))
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })
    .class("hardware-module")
    .class("drawer-module")
    .row_between(Pixels(tokens::SPACE_SM))
    .width(Stretch(1.0))
    .height(Pixels(166.0));
}

fn build_macro_card(cx: &mut Context, idx: usize, mod_ui: Arc<ModUiState>) {
    let source = ModMatrixSource::MACROS[idx];
    VStack::new(cx, move |cx| {
        HStack::new(cx, move |cx| {
            ModSourcePill::new(cx, source, mod_ui.clone())
                .width(Pixels(30.0))
                .height(Pixels(16.0));

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
            .height(Pixels(16.0));
        })
        .col_between(Pixels(tokens::SPACE_XS))
        .width(Stretch(1.0))
        .height(Pixels(16.0));

        HStack::new(cx, move |cx| {
            build_rotary_knob_with_opt_mod(
                cx,
                UiData::params,
                move |p| &p.macros[idx].value,
                false,
                AMBER_RGB,
                None,
            )
            .width(Pixels(tokens::KNOB_STD))
            .height(Pixels(tokens::KNOB_STD))
            .top(Stretch(1.0))
            .bottom(Stretch(1.0));

            build_param_readout_box(cx, UiData::params, move |p| &p.macros[idx].value)
                .width(Stretch(1.0))
                .height(Pixels(18.0))
                .top(Stretch(1.0))
                .bottom(Stretch(1.0));
        })
        .col_between(Pixels(tokens::SPACE_SM))
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })
    .class("macro-card")
    .width(Stretch(1.0))
    .height(Stretch(1.0))
    .row_between(Pixels(tokens::SPACE_XS));
}
