//! Top header bar: brand pill, view mode switcher, preset browser, UI scale, master volume & meter.

use std::sync::Arc;

use nih_plug_vizia::vizia::prelude::*;

use super::controls::{
    self, bold_label, build_dropdown_trigger_labels, build_header_knob_cell,
    build_ui_scale_dropdown, ArrowButton, ArrowDir, GearIcon,
};
use super::fx_scopes;
use super::mod_bar::ModUiState;
use super::scope;
use super::{tokens, MainViewMode, UiData, UiEvent};

pub(super) fn build_header_bar(cx: &mut Context, mod_ui: Arc<ModUiState>) {
    HStack::new(cx, move |cx| {
        HStack::new(cx, move |cx| {
            Element::new(cx).class("brand-pill");
            bold_label(cx, "J-SYN", "header-title");
        })
        .col_between(Pixels(6.0))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .width(Auto);

        HStack::new(cx, |cx| {
            Button::new(
                cx,
                |cx| cx.emit(UiEvent::SelectViewMode(MainViewMode::Main)),
                |cx| bold_label(cx, "MAIN", "view-switch-label").hoverable(false),
            )
            .class("view-switch-btn")
            .checked(UiData::view_mode.map(|m| *m == MainViewMode::Main))
            .width(Pixels(48.0))
            .height(Pixels(24.0));

            Button::new(
                cx,
                |cx| cx.emit(UiEvent::SelectViewMode(MainViewMode::Matrix)),
                |cx| bold_label(cx, "MATRIX", "view-switch-label").hoverable(false),
            )
            .class("view-switch-btn")
            .checked(UiData::view_mode.map(|m| *m == MainViewMode::Matrix))
            .width(Pixels(56.0))
            .height(Pixels(24.0));

            Button::new(
                cx,
                |cx| cx.emit(UiEvent::SelectViewMode(MainViewMode::Settings)),
                |cx| {
                    GearIcon::with_radius(cx, UiData::gear_radius, |cx| {
                        UiData::gear_radius.get(cx).clamp(0.10, 0.60)
                    })
                    .class("view-switch-label")
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                },
            )
            .class("view-switch-btn")
            .checked(UiData::view_mode.map(|m| *m == MainViewMode::Settings))
            .width(Pixels(36.0))
            .height(Pixels(24.0))
            .child_space(Pixels(0.0));
        })
        .class("view-switch-group")
        .col_between(Pixels(3.0))
        .width(Auto)
        .height(Pixels(30.0))
        .top(Stretch(1.0))
        .bottom(Stretch(1.0));

        // Preset management bar: < [Preset v] > INIT SAVE LOAD
        HStack::new(cx, |cx| {
            ArrowButton::new(cx, |cx| cx.emit(UiEvent::StepPreset(-1)), ArrowDir::Left)
                .class("preset-btn")
                .class("preset-btn-label")
                .width(Pixels(20.0))
                .height(Pixels(22.0));

            Dropdown::new(
                cx,
                move |cx| {
                    HStack::new(cx, move |cx| {
                        build_dropdown_trigger_labels(cx, UiData::current_preset_name);
                    })
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_top(Pixels(0.0))
                    .child_bottom(Pixels(0.0))
                    .overflow(Overflow::Hidden)
                },
                move |cx| {
                    Binding::new(cx, UiData::presets_dir_rev, move |cx, _| {
                        let cfg = crate::presets::JSynConfig::load();
                        let tree =
                            crate::presets::scan_preset_tree(cfg.presets_dir.as_deref());
                        if tree.root_files.is_empty() && tree.folders.is_empty() {
                            bold_label(cx, "Set Presets Dir...", "dropdown-action-item").on_press(
                                |cx| {
                                    cx.emit(PopupEvent::Close);
                                    cx.emit(UiEvent::PickPresetsDir);
                                },
                            );
                        } else {
                            let expanded = UiData::expanded_preset_folder.get(cx);
                            controls::build_directory_tree_items(
                                cx,
                                tree,
                                expanded,
                                |cx, folder_path| {
                                    cx.emit(UiEvent::OpenPresetFolder(folder_path.to_string()));
                                },
                                |cx, folder_path| {
                                    cx.emit(UiEvent::TogglePresetFolder(folder_path.to_string()));
                                },
                                |cx, entry| {
                                    cx.emit(UiEvent::LoadPresetPath(
                                        entry.path.clone(),
                                        entry.name.clone(),
                                    ));
                                    cx.emit(PopupEvent::Close);
                                },
                            );
                        }
                    });
                },
            )
            .width(Pixels(160.0))
            .height(Pixels(22.0));

            ArrowButton::new(cx, |cx| cx.emit(UiEvent::StepPreset(1)), ArrowDir::Right)
                .class("preset-btn")
                .class("preset-btn-label")
                .width(Pixels(20.0))
                .height(Pixels(22.0));

            Button::new(
                cx,
                |cx| cx.emit(UiEvent::InitPatch),
                |cx| bold_label(cx, "INIT", "preset-btn-label").hoverable(false),
            )
            .class("preset-btn")
            .width(Pixels(36.0))
            .height(Pixels(22.0));

            Button::new(
                cx,
                |cx| cx.emit(UiEvent::SavePatch),
                |cx| bold_label(cx, "SAVE", "preset-btn-label").hoverable(false),
            )
            .class("preset-btn")
            .width(Pixels(40.0))
            .height(Pixels(22.0));

            Button::new(
                cx,
                |cx| cx.emit(UiEvent::LoadPatchDialog),
                |cx| bold_label(cx, "LOAD", "preset-btn-label").hoverable(false),
            )
            .class("preset-btn")
            .width(Pixels(40.0))
            .height(Pixels(22.0));
        })
        .col_between(Pixels(3.0))
        .width(Auto)
        .height(Pixels(22.0))
        .top(Stretch(1.0))
        .bottom(Stretch(1.0));

        Element::new(cx).width(Stretch(1.0));

        let mod_ui_hdr = mod_ui.clone();
        HStack::new(cx, move |cx| {
            build_ui_scale_dropdown(cx);

            build_header_knob_cell(
                cx,
                UiData::params,
                |p| &p.master_volume_db,
                "MASTER",
                false,
            );

            fx_scopes::MasterScope::new(
                cx,
                mod_ui_hdr.params.clone(),
                mod_ui_hdr.telemetry.clone(),
            )
                .width(Pixels(64.0))
                .height(Pixels(34.0))
                .top(Stretch(1.0))
                .bottom(Stretch(1.0));

            scope::MasterMeter::new(cx, mod_ui_hdr.telemetry.clone())
                .width(Pixels(20.0))
                .height(Pixels(34.0))
                .top(Stretch(1.0))
                .bottom(Stretch(1.0));
        })
        .col_between(Pixels(8.0))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .width(Auto)
        .height(Stretch(1.0));
    })
    .class("header-bar")
    .col_between(Pixels(10.0))
    .width(Stretch(1.0))
    .height(Pixels(tokens::HEADER_H))
    .child_left(Pixels(12.0))
    .child_right(Pixels(12.0))
    .child_top(Stretch(1.0))
    .child_bottom(Stretch(1.0));
}

