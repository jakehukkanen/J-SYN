//! Shared standalone FX plugin shell (`header-bar` with cogwheel button, in-card Settings view,
//! shared `standalone-fx.json` config, and `StandaloneFxUiData`) so each
//! `crates/plugins/j-<fx>/` stays concise and visually consistent.

use std::path::PathBuf;
use std::sync::Arc;

use j_syn_dsp::FxType;
use nih_plug::prelude::{BoolParam, Editor, GuiContext, IntParam};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};
use serde::{Deserialize, Serialize};

use crate::controls::{
    bold_label, build_dropdown_trigger_labels, build_inline_pill_cell, build_ui_scale_picker,
    GearIcon, ScrollDropdown,
};
use crate::shell::{
    build_tooltip_overlay, notify_baseview_resize_macos, UiKitEvent, UiShellState,
};
use crate::theme::{self, ThemePalette};
use crate::tokens;
use crate::widgets::LedToggleSwitch;

pub const STANDALONE_FX_CONFIG_FILE: &str = "standalone-fx.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StandaloneFxConfig {
    pub theme: String,
    pub ui_scale: u32,
}

impl Default for StandaloneFxConfig {
    fn default() -> Self {
        Self {
            theme: "Studio Amber".to_string(),
            ui_scale: 100,
        }
    }
}

impl StandaloneFxConfig {
    fn config_file_path() -> Option<PathBuf> {
        theme::ecosystem_config_dir().map(|d| d.join(STANDALONE_FX_CONFIG_FILE))
    }

    pub fn load() -> Self {
        let Some(path) = Self::config_file_path() else {
            return Self::default();
        };
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&raw).unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let Some(path) = Self::config_file_path() else {
            return Err("Could not determine config directory".to_string());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config dir: {e}"))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {e}"))?;
        std::fs::write(path, json).map_err(|e| format!("Failed to write config: {e}"))
    }
}

pub fn standalone_default_state(fx_type: FxType) -> Arc<ViziaState> {
    let (w, h) = tokens::standalone_fx_window_size(fx_type);
    let cfg = StandaloneFxConfig::load();
    let scale = (cfg.ui_scale.clamp(85, 200) as f64) / 100.0;
    ViziaState::new_with_default_scale_factor(move || (w, h), scale)
}

#[derive(Debug, Clone, Copy)]
pub enum StandaloneShellEvent {
    ToggleSettings,
}

#[derive(Lens)]
pub struct StandaloneFxUiData<Params: Send + Sync + 'static> {
    pub params: Arc<Params>,
    pub ui_rev: u64,
    pub val_rev: u64,
    pub show_settings: bool,
}

impl<Params: Send + Sync + 'static> Model for StandaloneFxUiData<Params> {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|kit_event, meta| match kit_event {
            UiKitEvent::ModUiChanged => {
                self.ui_rev = self.ui_rev.wrapping_add(1);
                meta.consume();
            }
            UiKitEvent::ModValChanged => {
                self.val_rev = self.val_rev.wrapping_add(1);
                meta.consume();
            }
            _ => {}
        });
        event.map(|shell_event, meta| match shell_event {
            StandaloneShellEvent::ToggleSettings => {
                self.show_settings = !self.show_settings;
                meta.consume();
            }
        });
    }
}

#[derive(Clone, Copy)]
pub struct StandaloneFxHeaderSpec {
    pub header_title: &'static str,
    pub card_title: &'static str,
    pub fx_type: FxType,
}

fn build_theme_picker(cx: &mut Context) -> Handle<'_, ScrollDropdown> {
    ScrollDropdown::new(
        cx,
        move |cx, delta| {
            let themes = theme::load_all_themes();
            if themes.is_empty() {
                return;
            }
            let cur = UiShellState::theme_name.get(cx);
            let pos = themes
                .iter()
                .position(|t| t.name.eq_ignore_ascii_case(&cur))
                .unwrap_or(0);
            let next = (pos as isize + delta).rem_euclid(themes.len() as isize) as usize;
            cx.emit(UiKitEvent::SelectTheme(themes[next].name.clone()));
        },
        |cx| {
            Dropdown::new(
                cx,
                |cx| {
                    HStack::new(cx, move |cx| {
                        build_dropdown_trigger_labels(cx, UiShellState::theme_name);
                    })
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_top(Pixels(0.0))
                    .child_bottom(Pixels(0.0))
                    .overflow(Overflow::Hidden)
                },
                |cx| {
                    Binding::new(cx, UiShellState::themes_rev, |cx, _| {
                        let themes = theme::load_all_themes();
                        ScrollView::new(cx, 0.0, 0.0, false, true, move |cx| {
                            VStack::new(cx, move |cx| {
                                for pal in &themes {
                                    let name = pal.name.clone();
                                    bold_label(cx, &pal.name, "dropdown-item").on_press(
                                        move |cx| {
                                            cx.emit(UiKitEvent::SelectTheme(name.clone()));
                                            cx.emit(PopupEvent::Close);
                                        },
                                    );
                                }
                            })
                            .class("dropdown-scroll-content");
                        })
                        .class("dropdown-scroll-compact");
                    });
                },
            )
        },
    )
    .width(Stretch(1.0))
    .height(Pixels(tokens::CONTROL_H))
}

fn build_settings_card_body(cx: &mut Context) {
    VStack::new(cx, |cx| {
        // Card header strip
        HStack::new(cx, |cx| {
            bold_label(cx, "SETTINGS", "module-title")
                .width(Stretch(1.0))
                .height(Stretch(1.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0));
        })
        .col_between(Pixels(tokens::SPACE_MD))
        .height(Pixels(tokens::FX_UNIT_HEADER_H))
        .width(Stretch(1.0));

        // Theme & Scale controls
        VStack::new(cx, |cx| {
            VStack::new(cx, |cx| {
                bold_label(cx, "THEME", "mini-label")
                    .width(Stretch(1.0))
                    .height(Pixels(14.0));
                build_theme_picker(cx);
            })
            .row_between(Pixels(tokens::SPACE_XS))
            .width(Stretch(1.0))
            .height(Auto);

            VStack::new(cx, |cx| {
                bold_label(cx, "UI SCALE", "mini-label")
                    .width(Stretch(1.0))
                    .height(Pixels(14.0));
                build_ui_scale_picker(cx)
                    .width(Stretch(1.0))
                    .height(Pixels(tokens::CONTROL_H));
            })
            .row_between(Pixels(tokens::SPACE_XS))
            .width(Stretch(1.0))
            .height(Auto);
        })
        .row_between(Pixels(tokens::SPACE_LG))
        .top(Pixels(tokens::SPACE_MD))
        .bottom(Pixels(tokens::SPACE_MD))
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })
    .row_between(Pixels(tokens::SPACE_SM))
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}

#[allow(clippy::too_many_arguments)]
pub fn build_standalone_fx_shell<Params, FEnabled, FBody>(
    cx: &mut Context,
    spec: StandaloneFxHeaderSpec,
    params: Arc<Params>,
    pick_enabled: FEnabled,
    pick_stack: Option<fn(&Arc<Params>) -> &IntParam>,
    initial_theme: ThemePalette,
    initial_scale_pct: u32,
    persist_settings: bool,
    build_body: FBody,
) where
    Params: Send + Sync + 'static,
    FEnabled: Fn(&Arc<Params>) -> &BoolParam + Copy + Send + Sync + 'static,
    FBody: Fn(&mut Context) + Send + Sync + 'static,
{
    let (win_w, win_h) = tokens::standalone_fx_window_size(spec.fx_type);

    let mut shell = UiShellState::new(initial_theme, initial_scale_pct);
    if persist_settings {
        shell = shell
            .with_theme_callback(|name| {
                let mut cfg = StandaloneFxConfig::load();
                cfg.theme = name.to_string();
                let _ = cfg.save();
            })
            .with_scale_callback(|pct| {
                let mut cfg = StandaloneFxConfig::load();
                cfg.ui_scale = pct;
                let _ = cfg.save();
            });
    }
    shell.install(cx);

    StandaloneFxUiData {
        params: params.clone(),
        ui_rev: 0,
        val_rev: 0,
        show_settings: false,
    }
    .build(cx);

    let p_root = params;
    let body_fn = Arc::new(build_body);

    VStack::new(cx, move |cx| {
        // Sync shared UI scale immediately on open if another standalone FX changed it.
        // (`vizia_baseview` drives 60 Hz redraws directly via `on_frame` and never ticks `cx.add_timer`.)
        let target_scale = (initial_scale_pct as f64) / 100.0;
        if persist_settings && (cx.user_scale_factor() - target_scale).abs() > 0.005 {
            EventContext::new(cx).set_user_scale_factor(target_scale);
            notify_baseview_resize_macos();
        }

        // Compact standalone header bar: [Brand Pill] [J-<FX>] ... [⚙]
        HStack::new(cx, move |cx| {
            HStack::new(cx, |cx| {
                Element::new(cx)
                    .class("brand-pill")
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));
                bold_label(cx, spec.header_title, "header-title")
                    .font_size(11.5)
                    .width(Auto)
                    .height(Auto)
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));
            })
            .col_between(Pixels(tokens::SPACE_SM))
            .width(Auto)
            .height(Stretch(1.0));

            Element::new(cx).width(Stretch(1.0));

            Button::new(
                cx,
                |cx| cx.emit(StandaloneShellEvent::ToggleSettings),
                |cx| {
                    GearIcon::new(cx)
                        .width(Pixels(14.0))
                        .height(Pixels(14.0))
                        .child_space(Stretch(1.0))
                },
            )
            .class("fx-mb-side-btn")
            .toggle_class(
                "fx-subtab-active",
                StandaloneFxUiData::<Params>::show_settings,
            )
            .width(Pixels(22.0))
            .height(Pixels(22.0))
            .top(Stretch(1.0))
            .bottom(Stretch(1.0));
        })
        .class("header-bar")
        .col_between(Pixels(tokens::SPACE_SM))
        .child_left(Pixels(tokens::SPACE_MD))
        .child_right(Pixels(tokens::SPACE_MD))
        .height(Pixels(tokens::STANDALONE_FX_HEADER_H))
        .width(Stretch(1.0));

        // Main FX Unit Card (or in-card Settings view when cogwheel is active)
        let body_arc = body_fn.clone();
        VStack::new(cx, move |cx| {
            let body_inner = body_arc.clone();
            Binding::new(
                cx,
                StandaloneFxUiData::<Params>::show_settings,
                move |cx, settings_lens| {
                    let show_settings = settings_lens.get(cx);
                    if show_settings {
                        VStack::new(cx, |cx| {
                            build_settings_card_body(cx);
                        })
                        .class("hardware-module")
                        .class("fx-full-card")
                        .width(Stretch(1.0))
                        .height(Stretch(1.0));
                    } else {
                        let body_card = body_inner.clone();
                        VStack::new(cx, move |cx| {
                            HStack::new(cx, move |cx| {
                                LedToggleSwitch::new(
                                    cx,
                                    StandaloneFxUiData::<Params>::params,
                                    pick_enabled,
                                    "",
                                    "",
                                )
                                .width(Pixels(tokens::FX_UNIT_LED_W))
                                .height(Pixels(tokens::FX_UNIT_ARROW_H))
                                .top(Stretch(1.0))
                                .bottom(Stretch(1.0));

                                bold_label(cx, spec.card_title, "module-title")
                                    .width(Stretch(1.0))
                                    .height(Stretch(1.0))
                                    .child_top(Stretch(1.0))
                                    .child_bottom(Stretch(1.0));

                                if let Some(pick_stk) = pick_stack {
                                    HStack::new(cx, move |cx| {
                                        build_inline_pill_cell(
                                            cx,
                                            StandaloneFxUiData::<Params>::params,
                                            pick_stk,
                                            "STK",
                                            4,
                                            1.0,
                                        )
                                        .class("stk-pill");
                                    })
                                    .width(Pixels(tokens::FX_UNIT_STACK_W))
                                    .height(Pixels(tokens::FX_UNIT_HEADER_H));
                                }
                            })
                            .col_between(Pixels(tokens::SPACE_MD))
                            .height(Pixels(tokens::FX_UNIT_HEADER_H))
                            .width(Stretch(1.0));

                            body_card(cx);
                        })
                        .class("hardware-module")
                        .class("fx-full-card")
                        .toggle_class(
                            "module-disabled",
                            StandaloneFxUiData::<Params>::params
                                .map(move |p| !pick_enabled(p).value()),
                        )
                        .width(Stretch(1.0))
                        .height(Auto)
                        .row_between(Pixels(tokens::SPACE_SM));
                    }
                },
            );
        })
        .class("main-chassis")
        .child_space(Pixels(tokens::STANDALONE_FX_PAD))
        .width(Stretch(1.0))
        .height(Stretch(1.0));

        build_tooltip_overlay(cx, win_w as f32, win_h as f32);
    })
    .class("plugin-root")
    .toggle_class(
        "plugin-root-unused",
        StandaloneFxUiData::<Params>::val_rev.map(move |_| {
            let _ = &p_root;
            false
        }),
    )
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}

/// Creates a standalone FX `Vizia` editor that always syncs to the shared `standalone-fx.json`
/// theme & UI scale when the plugin window is opened.
pub fn create_standalone_fx_editor<F>(
    editor_state: Arc<ViziaState>,
    build_ui: F,
) -> Option<Box<dyn Editor>>
where
    F: Fn(&mut Context, Arc<dyn GuiContext>, ThemePalette, u32) + Send + Sync + 'static,
{
    create_vizia_editor(editor_state, ViziaTheming::Builtin, move |cx, gui_context| {
        let cfg = StandaloneFxConfig::load();
        let initial_scale_pct = cfg.ui_scale.clamp(85, 200);
        let initial_theme = theme::find_theme_by_name(&cfg.theme);
        build_ui(cx, gui_context, initial_theme, initial_scale_pct);
    })
}
