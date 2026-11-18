//! Standalone `J-MBDist` Vizia editor built directly on `j-syn-ui` primitives and shared `MB Dist` FX Unit view.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use j_syn_dsp::mod_matrix::{FxScopeRing, FX_SCOPE_LEN};
use j_syn_dsp::FxType;
use j_syn_ui::controls::{
    bold_label, build_dropdown_trigger_labels, build_ui_scale_picker, ScrollDropdown,
};
use j_syn_ui::fx_units::{
    build_multiband_dist_fx_unit_body, DistortionModeParam, MultibandDistUnitParams,
    MultibandTabHost,
};
use j_syn_ui::scopes::{fx_accent_rgb, FxSlotScopeSnapshot, MbBandScopeSnapshot};
use j_syn_ui::theme::{self, ThemePalette};
use j_syn_ui::widgets::LedToggleSwitch;
use j_syn_ui::{build_tooltip_overlay, tokens, UiKitEvent, UiShellState};
use nih_plug::prelude::{BoolParam, Editor, EnumParam, FloatParam, GuiContext};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{create_vizia_editor, ViziaState, ViziaTheming};
use serde::{Deserialize, Serialize};

use crate::JMbDistParams;

pub fn mbdist_window_size() -> (u32, u32) {
    tokens::standalone_fx_window_size(FxType::MultibandDist)
}

pub fn default_state() -> Arc<ViziaState> {
    let (w, h) = mbdist_window_size();
    let cfg = JMbDistConfig::load();
    let scale = (cfg.ui_scale.clamp(85, 200) as f64) / 100.0;
    ViziaState::new_with_default_scale_factor(move || (w, h), scale)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JMbDistConfig {
    pub theme: String,
    pub ui_scale: u32,
}

impl Default for JMbDistConfig {
    fn default() -> Self {
        Self {
            theme: "Studio Amber".to_string(),
            ui_scale: 100,
        }
    }
}

impl JMbDistConfig {
    fn config_file_path() -> Option<PathBuf> {
        theme::ecosystem_config_dir().map(|d| d.join("j-mbdist.json"))
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
pub struct MbDistUiData {
    pub params: Arc<JMbDistParams>,
    pub ui_rev: u64,
    pub val_rev: u64,
}

impl Model for MbDistUiData {
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

struct StandaloneMbTabHost {
    tab: AtomicU8,
}

impl StandaloneMbTabHost {
    fn new(initial_tab: u8) -> Self {
        Self {
            tab: AtomicU8::new(initial_tab.min(3)),
        }
    }
}

impl MultibandTabHost for StandaloneMbTabHost {
    fn mb_tab(&self) -> u8 {
        self.tab.load(Ordering::Relaxed)
    }

    fn set_mb_tab(&self, tab: u8) {
        self.tab.store(tab.min(3), Ordering::Relaxed);
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

/// Builds the full `J-MBDist` editor view tree on `cx` (shared between live DAW window and PNG preview harness).
#[allow(clippy::too_many_arguments)]
pub fn build_mbdist_ui(
    cx: &mut Context,
    params: Arc<JMbDistParams>,
    gui_context: Arc<dyn GuiContext>,
    scope_ring: Arc<FxScopeRing>,
    initial_theme: ThemePalette,
    initial_scale_pct: u32,
    initial_tab: u8,
    persist_settings: bool,
) {
    let (win_w, win_h) = mbdist_window_size();

    let mut shell = UiShellState::new(initial_theme, initial_scale_pct);
    if persist_settings {
        shell = shell
            .with_theme_callback(|name| {
                let mut cfg = JMbDistConfig::load();
                cfg.theme = name.to_string();
                let _ = cfg.save();
            })
            .with_scale_callback(|pct| {
                let mut cfg = JMbDistConfig::load();
                cfg.ui_scale = pct;
                let _ = cfg.save();
            });
    }
    shell.install(cx);

    MbDistUiData {
        params: params.clone(),
        ui_rev: 0,
        val_rev: 0,
    }
    .build(cx);

    let tab_host = Arc::new(StandaloneMbTabHost::new(initial_tab));
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
                bold_label(cx, "J-MBDIST", "header-title")
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

        // Main FX Unit Card reusing j-syn-ui's exact MB Dist FX Unit layout & scope
        VStack::new(cx, move |cx| {
            VStack::new(cx, move |cx| {
                // Card header strip
                HStack::new(cx, move |cx| {
                    LedToggleSwitch::new(
                        cx,
                        MbDistUiData::params,
                        |p| &p.enabled,
                        "OFF",
                        "ON",
                        fx_accent_rgb(FxType::MultibandDist),
                    )
                    .width(Pixels(tokens::FX_UNIT_LED_W + 4.0))
                    .height(Pixels(tokens::FX_UNIT_ARROW_H))
                    .top(Stretch(1.0))
                    .bottom(Stretch(1.0));

                    bold_label(cx, "MB DIST", "module-title")
                        .width(Stretch(1.0))
                        .height(Stretch(1.0))
                        .child_top(Stretch(1.0))
                        .child_bottom(Stretch(1.0));

                    bold_label(cx, "3-BAND LR4", "mini-label")
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
                let tab_scope = tab_host.clone();
                #[derive(Clone, Copy)]
                struct StandaloneMbDistParams;
                impl MultibandDistUnitParams<JMbDistParams> for StandaloneMbDistParams {
                    fn enabled<'a>(&self, p: &'a JMbDistParams) -> &'a BoolParam {
                        &p.enabled
                    }
                    fn low_xover<'a>(&self, p: &'a JMbDistParams) -> &'a FloatParam {
                        &p.low_xover
                    }
                    fn high_xover<'a>(&self, p: &'a JMbDistParams) -> &'a FloatParam {
                        &p.high_xover
                    }
                    fn out_gain<'a>(&self, p: &'a JMbDistParams) -> &'a FloatParam {
                        &p.out_gain
                    }
                    fn mix<'a>(&self, p: &'a JMbDistParams) -> &'a FloatParam {
                        &p.mix
                    }
                    fn band_enabled<'a>(&self, p: &'a JMbDistParams, band: usize) -> &'a BoolParam {
                        &p.band(band).enabled
                    }
                    fn band_solo<'a>(&self, p: &'a JMbDistParams, band: usize) -> &'a BoolParam {
                        &p.band(band).solo
                    }
                    fn band_mode<'a>(
                        &self,
                        p: &'a JMbDistParams,
                        band: usize,
                    ) -> &'a EnumParam<DistortionModeParam> {
                        &p.band(band).mode
                    }
                    fn band_drive<'a>(&self, p: &'a JMbDistParams, band: usize) -> &'a FloatParam {
                        &p.band(band).drive
                    }
                    fn band_bias<'a>(&self, p: &'a JMbDistParams, band: usize) -> &'a FloatParam {
                        &p.band(band).bias
                    }
                    fn band_level<'a>(&self, p: &'a JMbDistParams, band: usize) -> &'a FloatParam {
                        &p.band(band).level
                    }
                    fn band_mix<'a>(&self, p: &'a JMbDistParams, band: usize) -> &'a FloatParam {
                        &p.band(band).mix
                    }
                }

                build_multiband_dist_fx_unit_body(
                    cx,
                    MbDistUiData::params,
                    MbDistUiData::ui_rev,
                    p_card,
                    g_card,
                    StandaloneMbDistParams,
                    tab_host,
                    None,
                    move || {
                        let mut pre = [0.0_f32; FX_SCOPE_LEN];
                        let mut post = [0.0_f32; FX_SCOPE_LEN];
                        let act = ring.read_snapshot(&mut pre, &mut post);
                        FxSlotScopeSnapshot {
                            enabled: p_scope.enabled.value(),
                            fx_type: FxType::MultibandDist,
                            eff_p: [
                                p_scope.low_xover.value(),
                                p_scope.high_xover.value(),
                                p_scope.out_gain.value(),
                                p_scope.mix.value(),
                            ],
                            mb_xover_p1: p_scope.low_xover.value(),
                            mb_xover_p2: p_scope.high_xover.value(),
                            mb_tab: tab_scope.mb_tab(),
                            mb_bands: std::array::from_fn(|b| {
                                let mb = p_scope.band(b);
                                MbBandScopeSnapshot {
                                    enabled: mb.enabled.value(),
                                    solo: mb.solo.value(),
                                    drive: mb.drive.value().clamp(0.0, 1.0),
                                    level: mb.level.value(),
                                }
                            }),
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
                MbDistUiData::params.map(move |p| !p.enabled.value()),
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
        MbDistUiData::val_rev.map(move |_| {
            let _ = &p_root;
            false
        }),
    )
    .width(Stretch(1.0))
    .height(Stretch(1.0));
}

pub fn create(
    params: Arc<JMbDistParams>,
    editor_state: Arc<ViziaState>,
    scope_ring: Arc<FxScopeRing>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Builtin, move |cx, gui_context| {
        let cfg = JMbDistConfig::load();
        let initial_scale_pct = cfg.ui_scale.clamp(85, 200);
        let initial_theme = theme::find_theme_by_name(&cfg.theme);
        build_mbdist_ui(
            cx,
            params.clone(),
            gui_context,
            scope_ring.clone(),
            initial_theme,
            initial_scale_pct,
            0,
            true,
        );
    })
}
