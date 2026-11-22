//! Shared standalone FX plugin shell (`header-bar`, theme/scale pickers, `hardware-module` card,
//! `StandaloneFxConfig`, `StandaloneFxUiData`, and 60 Hz redraw timer) so each `crates/plugins/j-<fx>/`
//! stays concise and visually consistent.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use j_syn_dsp::FxType;
use nih_plug::prelude::BoolParam;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::ViziaState;
use serde::{Deserialize, Serialize};

use crate::controls::{
    bold_label, build_dropdown_trigger_labels, build_ui_scale_picker, ScrollDropdown,
};
use crate::scopes::fx_accent_rgb;
use crate::shell::{build_tooltip_overlay, UiKitEvent, UiShellState};
use crate::theme::{self, ThemePalette};
use crate::tokens;
use crate::widgets::LedToggleSwitch;

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
    fn config_file_path(file_name: &str) -> Option<PathBuf> {
        theme::ecosystem_config_dir().map(|d| d.join(file_name))
    }

    pub fn load(file_name: &str) -> Self {
        let Some(path) = Self::config_file_path(file_name) else {
            return Self::default();
        };
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&raw).unwrap_or_default()
    }

    pub fn save(&self, file_name: &str) -> Result<(), String> {
        let Some(path) = Self::config_file_path(file_name) else {
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

pub fn standalone_default_state(fx_type: FxType, config_file: &str) -> Arc<ViziaState> {
    let (w, h) = tokens::standalone_fx_window_size(fx_type);
    let cfg = StandaloneFxConfig::load(config_file);
    let scale = (cfg.ui_scale.clamp(85, 200) as f64) / 100.0;
    ViziaState::new_with_default_scale_factor(move || (w, h), scale)
}

#[derive(Lens)]
pub struct StandaloneFxUiData<Params: Send + Sync + 'static> {
    pub params: Arc<Params>,
    pub ui_rev: u64,
    pub val_rev: u64,
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
    }
}

#[derive(Clone, Copy)]
pub struct StandaloneFxHeaderSpec {
    pub header_title: &'static str,
    pub card_title: &'static str,
    pub card_subtitle: &'static str,
    pub fx_type: FxType,
    pub config_file: &'static str,
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
                        .class("dropdown-scroll");
                    });
                },
            )
        },
    )
    .width(Pixels(88.0))
    .height(Pixels(tokens::PILL_H))
}

#[allow(clippy::too_many_arguments)]
pub fn build_standalone_fx_shell<Params, FEnabled, FBody>(
    cx: &mut Context,
    spec: StandaloneFxHeaderSpec,
    params: Arc<Params>,
    pick_enabled: FEnabled,
    initial_theme: ThemePalette,
    initial_scale_pct: u32,
    persist_settings: bool,
    build_body: FBody,
) where
    Params: Send + Sync + 'static,
    FEnabled: Fn(&Arc<Params>) -> &BoolParam + Copy + Send + Sync + 'static,
    FBody: FnOnce(&mut Context) + 'static,
{
    let (win_w, win_h) = tokens::standalone_fx_window_size(spec.fx_type);
    let accent = fx_accent_rgb(spec.fx_type);
    let cfg_file_theme = spec.config_file;
    let cfg_file_scale = spec.config_file;

    let mut shell = UiShellState::new(initial_theme, initial_scale_pct);
    if persist_settings {
        shell = shell
            .with_theme_callback(move |name| {
                let mut cfg = StandaloneFxConfig::load(cfg_file_theme);
                cfg.theme = name.to_string();
                let _ = cfg.save(cfg_file_theme);
            })
            .with_scale_callback(move |pct| {
                let mut cfg = StandaloneFxConfig::load(cfg_file_scale);
                cfg.ui_scale = pct;
                let _ = cfg.save(cfg_file_scale);
            });
    }
    shell.install(cx);

    StandaloneFxUiData {
        params: params.clone(),
        ui_rev: 0,
        val_rev: 0,
    }
    .build(cx);

    let p_root = params;

    VStack::new(cx, move |cx| {
        // Single 60 Hz scope redraw timer
        let timer = cx.add_timer(Duration::from_millis(16), None, move |cx, action| {
            if matches!(action, TimerAction::Tick(_)) {
                cx.needs_redraw();
            }
        });
        cx.start_timer(timer);

        // Compact standalone header bar (224 px window, 6 px horizontal padding)
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

            build_theme_picker(cx)
                .top(Stretch(1.0))
                .bottom(Stretch(1.0));

            build_ui_scale_picker(cx)
                .width(Pixels(48.0))
                .height(Pixels(tokens::PILL_H))
                .top(Stretch(1.0))
                .bottom(Stretch(1.0));
        })
        .class("header-bar")
        .col_between(Pixels(tokens::SPACE_SM))
        .child_left(Pixels(tokens::SPACE_MD))
        .child_right(Pixels(tokens::SPACE_MD))
        .height(Pixels(tokens::STANDALONE_FX_HEADER_H))
        .width(Stretch(1.0));

        // Main FX Unit Card reusing j-syn-ui's exact FX Unit body & scope
        let mut body_opt = Some(build_body);
        VStack::new(cx, move |cx| {
            VStack::new(cx, move |cx| {
                // Card header strip
                HStack::new(cx, move |cx| {
                    LedToggleSwitch::new(
                        cx,
                        StandaloneFxUiData::<Params>::params,
                        pick_enabled,
                        "OFF",
                        "ON",
                        accent,
                    )
                    .width(Pixels(tokens::FX_UNIT_LED_W + 4.0))
                    .height(Pixels(tokens::FX_UNIT_ARROW_H))
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));

                    bold_label(cx, spec.card_title, "module-title")
                        .width(Stretch(1.0))
                        .height(Stretch(1.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0));

                    bold_label(cx, spec.card_subtitle, "mini-label")
                        .width(Auto)
                        .height(Stretch(1.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0));
                })
                .col_between(Pixels(tokens::SPACE_MD))
                .height(Pixels(tokens::FX_UNIT_HEADER_H))
                .width(Stretch(1.0));

                if let Some(build_body) = body_opt.take() {
                    build_body(cx);
                }
            })
            .class("hardware-module")
            .class("fx-full-card")
            .toggle_class(
                "module-disabled",
                StandaloneFxUiData::<Params>::params.map(move |p| !pick_enabled(p).value()),
            )
            .width(Stretch(1.0))
            .height(Auto)
            .row_between(Pixels(tokens::SPACE_SM));
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
