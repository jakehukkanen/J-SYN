//! Draggable modulation source pill (`E1..E8`, `MS1..MS8`, `VEL`, `KEY`, `MW`, `AT`, `SLD`, `M1..M8`).

use std::sync::Arc;

use j_syn_dsp::mod_matrix::ModMatrixSource;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use super::controls::bold_label;
use super::mod_bar::ModUiState;
use super::modseq::ModSeqSubTab;
use super::{BottomTab, UiEvent, theme};

/// Draggable modulation source pill (`E1..E4`, `MS1..MS4`, `VEL`, `KEY`, `MW`, `AT`, `M1..M8`).
pub(crate) struct ModSourcePill {
    source: ModMatrixSource,
    mod_ui: Arc<ModUiState>,
    drag_active: bool,
    drag_moved: bool,
    hovered: bool,
    start_x: f32,
    start_y: f32,
}

impl ModSourcePill {
    pub(crate) fn new<'a>(
        cx: &'a mut Context,
        source: ModMatrixSource,
        mod_ui: Arc<ModUiState>,
    ) -> Handle<'a, Self> {
        Self {
            source,
            mod_ui,
            drag_active: false,
            drag_moved: false,
            hovered: false,
            start_x: 0.0,
            start_y: 0.0,
        }
        .build(cx, |cx| {
            bold_label(cx, source.short_label(), "mod-src-label")
                .width(Stretch(1.0))
                .height(Auto)
                .top(Stretch(1.0))
                .bottom(Stretch(1.0))
                .child_left(Stretch(1.0))
                .child_right(Stretch(1.0))
                .hoverable(false);
        })
    }

    fn subtab_for_source(src: ModMatrixSource) -> Option<ModSeqSubTab> {
        match src {
            ModMatrixSource::ModSeq1 => Some(ModSeqSubTab::MSeq(0)),
            ModMatrixSource::ModSeq2 => Some(ModSeqSubTab::MSeq(1)),
            ModMatrixSource::ModSeq3 => Some(ModSeqSubTab::MSeq(2)),
            ModMatrixSource::ModSeq4 => Some(ModSeqSubTab::MSeq(3)),
            ModMatrixSource::ModSeq5 => Some(ModSeqSubTab::MSeq(4)),
            ModMatrixSource::ModSeq6 => Some(ModSeqSubTab::MSeq(5)),
            ModMatrixSource::ModSeq7 => Some(ModSeqSubTab::MSeq(6)),
            ModMatrixSource::ModSeq8 => Some(ModSeqSubTab::MSeq(7)),
            ModMatrixSource::Env1 => Some(ModSeqSubTab::Env(0)),
            ModMatrixSource::Env2 => Some(ModSeqSubTab::Env(1)),
            ModMatrixSource::Env3 => Some(ModSeqSubTab::Env(2)),
            ModMatrixSource::Env4 => Some(ModSeqSubTab::Env(3)),
            ModMatrixSource::Env5 => Some(ModSeqSubTab::Env(4)),
            ModMatrixSource::Env6 => Some(ModSeqSubTab::Env(5)),
            ModMatrixSource::Env7 => Some(ModSeqSubTab::Env(6)),
            ModMatrixSource::Env8 => Some(ModSeqSubTab::Env(7)),
            _ => None,
        }
    }
}

impl View for ModSourcePill {
    fn element(&self) -> Option<&'static str> {
        Some("mod-source-pill")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseOver => {
                self.hovered = true;
                cx.needs_redraw();
            }
            WindowEvent::MouseOut => {
                self.hovered = false;
                cx.needs_redraw();
            }
            WindowEvent::MouseDown(MouseButton::Left)
            | WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                self.drag_active = true;
                self.drag_moved = false;
                self.start_x = cx.mouse().cursorx;
                self.start_y = cx.mouse().cursory;
                self.mod_ui.start_drag(self.source);
                cx.capture();
                cx.emit(UiEvent::ModUiChanged);
                cx.needs_redraw();
                meta.consume();
            }
            WindowEvent::MouseMove(x, y) => {
                if self.drag_active {
                    let dist = (*x - self.start_x).hypot(*y - self.start_y);
                    if !self.drag_moved && dist >= 4.0 * cx.scale_factor() {
                        self.drag_moved = true;
                    }
                    if self.drag_moved && self.mod_ui.update_hover_from_cursor(*x, *y) {
                        cx.emit(UiEvent::ModUiChanged);
                    }
                    cx.needs_redraw();
                    meta.consume();
                }
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.drag_active {
                    self.drag_active = false;
                    let moved = self.drag_moved;
                    self.drag_moved = false;
                    cx.release();
                    if moved {
                        self.mod_ui
                            .finish_drop_at_cursor(cx.mouse().cursorx, cx.mouse().cursory);
                    } else {
                        self.mod_ui.clear_drag();
                        if let Some(sub) = Self::subtab_for_source(self.source) {
                            cx.emit(UiEvent::SelectTab(BottomTab::ModSeq));
                            cx.emit(UiEvent::SelectModSeqSubTab(sub));
                        }
                    }
                    cx.emit(UiEvent::ModUiChanged);
                    cx.needs_redraw();
                    meta.consume();
                }
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 1.0 || bounds.h <= 1.0 {
            return;
        }
        let dpx = cx.scale_factor();
        let pal = theme::current_palette(cx);
        let (r, g, b) = theme::mod_source_rgb(&pal, self.source);
        let is_dragging = self.mod_ui.dragging_source() == self.source;
        let is_arc_active = self.mod_ui.active_mod_source() == self.source;
        let is_active_tab = self.mod_ui.is_source_tab_active(self.source);
        let is_hovered = self.hovered;
        let level = self
            .mod_ui
            .telemetry
            .source_level(self.source)
            .clamp(0.0, 1.0);

        let mut bg = vg::Path::new();
        bg.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 3.0 * dpx);
        let fill = if is_dragging {
            vg::Color::rgba(r, g, b, 105)
        } else if is_arc_active {
            let (mr, mg, mb) = theme::hex_to_rgb(&pal.surface_hover);
            let (tr, tg, tb) = theme::lerp_rgb((mr, mg, mb), (r, g, b), 0.26);
            vg::Color::rgb(tr, tg, tb)
        } else if is_active_tab {
            let (mr, mg, mb) = theme::hex_to_rgb(&pal.surface_mid);
            let (tr, tg, tb) = theme::lerp_rgb((mr, mg, mb), (r, g, b), 0.12);
            vg::Color::rgb(tr, tg, tb)
        } else if is_hovered {
            theme::vg_color(&pal.surface_hover, 255)
        } else {
            theme::vg_color(&pal.surface_mid, 255)
        };
        canvas.fill_path(&bg, &vg::Paint::color(fill));

        // Subtle inner glow frame on the currently active modulation source pill
        if is_arc_active || is_dragging {
            let mut glow = vg::Path::new();
            glow.rounded_rect(
                bounds.x + 0.8 * dpx,
                bounds.y + 0.8 * dpx,
                (bounds.w - 1.6 * dpx).max(1.0),
                (bounds.h - 1.6 * dpx).max(1.0),
                2.5 * dpx,
            );
            let mut glow_paint = vg::Paint::color(vg::Color::rgba(r, g, b, 68));
            glow_paint.set_line_width(2.2 * dpx);
            canvas.stroke_path(&glow, &glow_paint);
        }

        // Live telemetry bottom activity bar
        if level > 0.01 {
            let bar_w = ((bounds.w - 4.0 * dpx) * level).max(2.0 * dpx);
            let mut bar = vg::Path::new();
            bar.rounded_rect(
                bounds.x + 2.0 * dpx,
                bounds.y + bounds.h - 3.0 * dpx,
                bar_w,
                2.0 * dpx,
                1.0 * dpx,
            );
            canvas.fill_path(&bar, &vg::Paint::color(vg::Color::rgba(r, g, b, 235)));
        }

        // Left colored badge accent + subtle outline
        let stripe_w = if is_arc_active {
            3.2 * dpx
        } else if is_active_tab {
            2.6 * dpx
        } else {
            2.0 * dpx
        };
        let mut stripe = vg::Path::new();
        stripe.rounded_rect(
            bounds.x + 1.5 * dpx,
            bounds.y + 3.0 * dpx,
            stripe_w,
            bounds.h - 6.0 * dpx,
            1.0 * dpx,
        );
        canvas.fill_path(&stripe, &vg::Paint::color(vg::Color::rgb(r, g, b)));

        let mut border = vg::Paint::color(if is_dragging || is_arc_active {
            vg::Color::rgb(r, g, b)
        } else if is_hovered {
            vg::Color::rgba(r, g, b, 195)
        } else if is_active_tab {
            vg::Color::rgba(r, g, b, 135)
        } else {
            vg::Color::rgba(r, g, b, 85)
        });
        border.set_line_width(
            (if is_arc_active || is_dragging {
                1.65
            } else if is_active_tab {
                1.15
            } else {
                1.0
            }) * dpx,
        );
        canvas.stroke_path(&bg, &border);
    }
}
