//! Standalone `J-Utility` Vizia editor built directly on `j-syn-ui` primitives and shared `Utility` FX Unit view.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use j_syn_dsp::mod_matrix::{FxScopeRing, FX_SCOPE_LEN};
use j_syn_dsp::FxType;
use j_syn_ui::controls::{
    bold_label, build_dropdown_trigger_labels, build_ui_scale_picker, ScrollDropdown,
};
use j_syn_ui::fx_units::{build_utility_fx_unit_body, UtilityUnitParams};
use j_syn_ui::scopes::FxSlotScopeSnapshot;
use j_syn_ui::theme::{self, ThemePalette};
use j_syn_ui::widgets::LedToggleSwitch;
use j_syn_ui::{build_tooltip_overlay, tokens, UiKitEvent, UiShellState};
use nih_plug::prelude::{Editor, GuiContext};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};
use serde::{Deserialize, Serialize};

use crate::JUtilityParams;

pub fn utility_window_size() -> (u32, u32) {
    tokens::standalone_fx_window_size(FxType::Utility)
}

pub fn default_state() -> Arc<ViziaState> {
    let (w, h) = utility_window_size();
    let cfg = JUtilityConfig::load();
    let scale = (cfg.ui_scale.clamp(85, 200) as f64) / 100.0;
    ViziaState::new_with_default_scale_factor(move || (w, h), scale)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JUtilityConfig {
    pub theme: String,
    pub ui_scale: u32,
}

impl Default for JUtilityConfig {
    fn default() -> Self {
        Self {
            theme: "Studio Amber".to_string(),
            ui_scale: 100,
        }
    }
}

impl JUtilityConfig {
    fn config_file_path() -> Option<PathBuf> {
        theme::ecosystem_config_dir().map(|d| d.join("j-utility.json"))
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

#[derive(Lens)]
pub struct UtilityUiData {
    pub params: Arc<JUtilityParams>,
    pub ui_rev: u64,
}

impl Model for UtilityUiData {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|kit_event, meta| match kit_event {
            UiKitEvent::ModUiChanged | UiKitEvent::ModValChanged => {
                self.ui_rev = self.ui_rev.wrapping_add(1);
                meta.consume();
            }
            _ => {}
        });
    }
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
    .width(Pixels(112.0))
    .height(Pixels(tokens::PILL_H))
}

/// Builds the full `J-Utility` editor view tree on `cx` (shared between live DAW window and PNG preview harness).
pub fn build_utility_ui(
    cx: &mut Context,
    params: Arc<JUtilityParams>,
    gui_context: Arc<dyn GuiContext>,
    scope_ring: Arc<FxScopeRing>,
    initial_theme: ThemePalette,
    initial_scale_pct: u32,
    persist_settings: bool,
) {
    let (win_w, win_h) = utility_window_size();

    let mut shell = UiShellState::new(initial_theme, initial_scale_pct);
    if persist_settings {
        shell = shell
            .with_theme_callback(|name| {
                let mut cfg = JUtilityConfig::load();
                cfg.theme = name.to_string();
                let _ = cfg.save();
            })
            .with_scale_callback(|pct| {
                let mut cfg = JUtilityConfig::load();
                cfg.ui_scale = pct;
                let _ = cfg.save();
            });
    }
    shell.install(cx);

    UtilityUiData {
        params: params.clone(),
        ui_rev: 0,
    }
    .build(cx);

    let p_root = params.clone();
    let p_card = params;
    let g_card = gui_context;
    let ring_scope = scope_ring;

    VStack::new(cx, move |cx| {
        // Single 60 Hz scope redraw timer
        let timer = cx.add_timer(Duration::from_millis(16), None, move |cx, action| {
            if matches!(action, TimerAction::Tick(_)) {
                cx.needs_redraw();
            }
        });
        cx.start_timer(timer);

        // Compact standalone header bar
        HStack::new(cx, move |cx| {
            HStack::new(cx, |cx| {
                Element::new(cx)
                    .class("brand-pill")
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));
                bold_label(cx, "J-UTILITY", "header-title")
                    .font_size(13.0)
                    .width(Auto)
                    .height(Auto)
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));
            })
            .col_between(Pixels(tokens::SPACE_MD))
            .width(Auto)
            .height(Stretch(1.0));

            Element::new(cx).width(Stretch(1.0));

            build_theme_picker(cx)
                .top(Stretch(1.0))
                .bottom(Stretch(1.0));

            build_ui_scale_picker(cx)
                .width(Pixels(54.0))
                .height(Pixels(tokens::PILL_H))
                .top(Stretch(1.0))
                .bottom(Stretch(1.0));
        })
        .class("header-bar")
        .col_between(Pixels(tokens::SPACE_SM))
        .child_left(Pixels(tokens::SPACE_LG))
        .child_right(Pixels(tokens::SPACE_LG))
        .height(Pixels(tokens::STANDALONE_FX_HEADER_H))
        .width(Stretch(1.0));

        // Main FX Unit Card reusing j-syn-ui's exact Utility FX Unit layout & scope
        VStack::new(cx, move |cx| {
            VStack::new(cx, move |cx| {
                // Card header strip
                HStack::new(cx, move |cx| {
                    LedToggleSwitch::new(
                        cx,
                        UtilityUiData::params,
                        |p| &p.enabled,
                        "OFF",
                        "ON",
                        (108, 214, 255),
                    )
                    .width(Pixels(tokens::FX_UNIT_LED_W + 4.0))
                    .height(Pixels(tokens::FX_UNIT_ARROW_H))
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));

                    bold_label(cx, "UTILITY", "module-title")
                        .width(Stretch(1.0))
                        .height(Stretch(1.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0));

                    bold_label(cx, "M/S • GAIN • Ø", "mini-label")
                        .width(Auto)
                        .height(Stretch(1.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0));
                })
                .col_between(Pixels(tokens::SPACE_MD))
                .height(Pixels(tokens::FX_UNIT_HEADER_H))
                .width(Stretch(1.0));

                let p_scope = p_card.clone();
                let ring = ring_scope.clone();
                #[derive(Clone, Copy)]
                struct StandaloneUtilityParams;
                impl UtilityUnitParams<JUtilityParams> for StandaloneUtilityParams {
                    fn enabled<'a>(&self, p: &'a JUtilityParams) -> &'a nih_plug::prelude::BoolParam {
                        &p.enabled
                    }
                    fn width<'a>(&self, p: &'a JUtilityParams) -> &'a nih_plug::prelude::FloatParam {
                        &p.width
                    }
                    fn pan<'a>(&self, p: &'a JUtilityParams) -> &'a nih_plug::prelude::FloatParam {
                        &p.pan
                    }
                    fn gain<'a>(&self, p: &'a JUtilityParams) -> &'a nih_plug::prelude::FloatParam {
                        &p.gain
                    }
                    fn mix<'a>(&self, p: &'a JUtilityParams) -> &'a nih_plug::prelude::FloatParam {
                        &p.mix
                    }
                    fn inv_l<'a>(&self, p: &'a JUtilityParams) -> &'a nih_plug::prelude::BoolParam {
                        &p.inv_l
                    }
                    fn inv_r<'a>(&self, p: &'a JUtilityParams) -> &'a nih_plug::prelude::BoolParam {
                        &p.inv_r
                    }
                }
                build_utility_fx_unit_body(
                    cx,
                    UtilityUiData::params,
                    p_card,
                    g_card,
                    StandaloneUtilityParams,
                    None,
                    move || {
                        let mut pre = [0.0_f32; FX_SCOPE_LEN];
                        let mut post = [0.0_f32; FX_SCOPE_LEN];
                        let act = ring.read_snapshot(&mut pre, &mut post);
                        FxSlotScopeSnapshot {
                            enabled: p_scope.enabled.value(),
                            fx_type: FxType::Utility,
                            eff_p: [
                                p_scope.width.value(),
                                p_scope.pan.value(),
                                p_scope.gain.value(),
                                p_scope.mix.value(),
                            ],
                            utility_inv_l: p_scope.inv_l.value(),
                            utility_inv_r: p_scope.inv_r.value(),
                            pre,
                            post,
                            activity: act,
                            ..FxSlotScopeSnapshot::default()
                        }
                    },
                    tokens::KNOB_FX_UNIT,
                );
            })
            .class("hardware-module")
            .class("fx-full-card")
            .toggle_class(
                "module-disabled",
                UtilityUiData::params.map(move |p| !p.enabled.value()),
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
        UtilityUiData::ui_rev.map(move |_| {
            let _ = &p_root;
            false
        }),
    )
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}

pub fn create(
    params: Arc<JUtilityParams>,
    editor_state: Arc<ViziaState>,
    scope_ring: Arc<FxScopeRing>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Builtin, move |cx, gui_context| {
        let cfg = JUtilityConfig::load();
        let initial_scale_pct = cfg.ui_scale.clamp(85, 200);
        let initial_theme = theme::find_theme_by_name(&cfg.theme);
        build_utility_ui(
            cx,
            params.clone(),
            gui_context,
            scope_ring.clone(),
            initial_theme,
            initial_scale_pct,
            true,
        );
    })
}
