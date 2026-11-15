//! `MACROS` bottom drawer (`M1`–`M8` performance macros with draggable mod badges & editable labels).

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModMatrixSource;
use nih_plug_vizia::vizia::prelude::*;

use super::controls::{
    build_module_header, build_param_readout_box, build_rotary_knob_with_opt_mod,
};
use super::mod_bar::{ModSourcePill, ModUiState};
use super::{UiData, UiEvent, AMBER_RGB};

pub(super) fn build_macros_tab(cx: &mut Context, mod_ui: Arc<ModUiState>) {
    VStack::new(cx, move |cx| {
        build_module_header(cx, "MACROS (DRAG M1–M8 BADGES TO MODULATE)", 22.0);

        let mod_ui_cards = mod_ui.clone();
        HStack::new(cx, move |cx| {
            for i in 0..8 {
                build_macro_card(cx, i, mod_ui_cards.clone());
            }
        })
        .col_between(Pixels(6.0))
        .width(Stretch(1.0))
        .height(Stretch(1.0))
        .top(Pixels(4.0));
    })
    .class("hardware-module")
    .class("drawer-module")
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}

fn build_macro_card(cx: &mut Context, idx: usize, mod_ui: Arc<ModUiState>) {
    let source = ModMatrixSource::MACROS[idx];
    VStack::new(cx, move |cx| {
        HStack::new(cx, move |cx| {
            ModSourcePill::new(cx, source, mod_ui.clone())
                .width(Pixels(36.0))
                .height(Pixels(18.0));

            build_param_readout_box(cx, UiData::params, move |p| &p.macros[idx].value)
                .width(Stretch(1.0))
                .height(Pixels(18.0));
        })
        .col_between(Pixels(4.0))
        .width(Stretch(1.0))
        .height(Pixels(18.0));

        build_rotary_knob_with_opt_mod(
            cx,
            UiData::params,
            move |p| &p.macros[idx].value,
            false,
            AMBER_RGB,
            None,
        )
        .width(Pixels(52.0))
        .height(Pixels(52.0))
        .left(Stretch(1.0))
        .right(Stretch(1.0))
        .top(Stretch(1.0))
        .bottom(Stretch(1.0));

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
        .height(Pixels(20.0));
    })
    .class("macro-card")
    .width(Stretch(1.0))
    .height(Stretch(1.0))
    .row_between(Pixels(4.0));
}
