//! Shared FX card drag-and-drop state and top-level translucent ghost / insertion-bar overlay.

use std::cell::Cell;
use std::sync::{Arc, Mutex};

use j_syn_dsp::{FxType, NUM_FX_BUSES, NUM_FX_SLOTS};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use super::{theme, tokens};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FxDragDropTarget {
    None,
    Insert { bus: usize, pos: usize },
    Remove,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ActiveFxDrag {
    pub from_bus: usize,
    pub from_pos: usize,
    pub from_slot: usize,
    pub fx_type: FxType,
    pub cursor_x: f32,
    pub cursor_y: f32,
    pub alt_duplicate: bool,
    pub drop_target: FxDragDropTarget,
}

struct FxDragInner {
    active: Option<ActiveFxDrag>,
    wing_bounds: BoundingBox,
    lane_bounds: [BoundingBox; NUM_FX_BUSES],
    lane_scroll_entities: [Option<Entity>; NUM_FX_BUSES],
    lane_card_counts: [usize; NUM_FX_BUSES],
    card_rects: [[Option<(usize, BoundingBox)>; NUM_FX_SLOTS]; NUM_FX_BUSES],
}

impl Default for FxDragInner {
    fn default() -> Self {
        Self {
            active: None,
            wing_bounds: BoundingBox::default(),
            lane_bounds: [BoundingBox::default(); NUM_FX_BUSES],
            lane_scroll_entities: [None; NUM_FX_BUSES],
            lane_card_counts: [0; NUM_FX_BUSES],
            card_rects: [[None; NUM_FX_SLOTS]; NUM_FX_BUSES],
        }
    }
}

/// Shared UI drag-and-drop coordinator for all 3 FX Rack lanes (`Bus A`, `Bus B`, `Bus C`).
pub(crate) struct FxDragState {
    inner: Mutex<FxDragInner>,
}

impl Default for FxDragState {
    fn default() -> Self {
        Self::new()
    }
}

impl FxDragState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(FxDragInner::default()),
        }
    }

    pub fn register_wing_bounds(&self, bounds: BoundingBox) {
        if let Ok(mut g) = self.inner.lock() {
            g.wing_bounds = bounds;
        }
    }

    pub fn register_lane_bounds(&self, bus: usize, bounds: BoundingBox, scroll_entity: Entity) {
        if bus < NUM_FX_BUSES {
            if let Ok(mut g) = self.inner.lock() {
                g.lane_bounds[bus] = bounds;
                g.lane_scroll_entities[bus] = Some(scroll_entity);
            }
        }
    }

    pub fn set_lane_card_count(&self, bus: usize, count: usize) {
        if bus < NUM_FX_BUSES {
            if let Ok(mut g) = self.inner.lock() {
                let c = count.min(NUM_FX_SLOTS);
                g.lane_card_counts[bus] = c;
                for slot in &mut g.card_rects[bus][c..] {
                    *slot = None;
                }
            }
        }
    }

    pub fn register_card_bounds(
        &self,
        bus: usize,
        visual_pos: usize,
        backing_slot: usize,
        bounds: BoundingBox,
    ) {
        if bus < NUM_FX_BUSES && visual_pos < NUM_FX_SLOTS {
            if let Ok(mut g) = self.inner.lock() {
                g.card_rects[bus][visual_pos] = Some((backing_slot, bounds));
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start_drag(
        &self,
        from_bus: usize,
        from_pos: usize,
        from_slot: usize,
        fx_type: FxType,
        cursor_x: f32,
        cursor_y: f32,
        alt_duplicate: bool,
        dpx: f32,
    ) {
        if let Ok(mut g) = self.inner.lock() {
            let drop_target =
                compute_drop_target_locked(&g, cursor_x, cursor_y, alt_duplicate, dpx);
            g.active = Some(ActiveFxDrag {
                from_bus,
                from_pos,
                from_slot,
                fx_type,
                cursor_x,
                cursor_y,
                alt_duplicate,
                drop_target,
            });
        }
    }

    pub fn update_drag(&self, cursor_x: f32, cursor_y: f32, alt_duplicate: bool, dpx: f32) {
        if let Ok(mut g) = self.inner.lock() {
            let drop_target =
                compute_drop_target_locked(&g, cursor_x, cursor_y, alt_duplicate, dpx);
            if let Some(ref mut drag) = g.active {
                drag.cursor_x = cursor_x;
                drag.cursor_y = cursor_y;
                drag.alt_duplicate = alt_duplicate;
                drag.drop_target = drop_target;
            }
        }
    }

    pub fn finish_drag(&self) -> Option<ActiveFxDrag> {
        self.inner.lock().ok().and_then(|mut g| g.active.take())
    }

    pub fn cancel_drag(&self) -> bool {
        self.inner
            .lock()
            .ok()
            .and_then(|mut g| g.active.take())
            .is_some()
    }

    #[inline]
    pub fn is_dragging_source_move(&self, bus: usize, slot: usize) -> bool {
        self.inner.lock().ok().is_some_and(|g| {
            g.active
                .is_some_and(|d| !d.alt_duplicate && d.from_bus == bus && d.from_slot == slot)
        })
    }

    /// Returns `(scroll_entity, scroll_dy)` when an active drag hovers within `24 px` of a
    /// lane viewport's top or bottom edge, allowing smooth 60 Hz auto-scrolling.
    pub fn edge_scroll_target(&self, dpx: f32) -> Option<(Entity, f32)> {
        let g = self.inner.lock().ok()?;
        let drag = g.active?;
        let zone = 24.0 * dpx.max(1.0);
        for b in 0..NUM_FX_BUSES {
            let lb = g.lane_bounds[b];
            if lb.w <= 1.0 || lb.h <= 1.0 {
                continue;
            }
            if drag.cursor_x >= lb.x && drag.cursor_x <= lb.x + lb.w {
                let entity = g.lane_scroll_entities[b]?;
                if drag.cursor_y >= lb.y && drag.cursor_y < lb.y + zone {
                    return Some((entity, 1.4));
                }
                if drag.cursor_y <= lb.y + lb.h && drag.cursor_y > lb.y + lb.h - zone {
                    return Some((entity, -1.4));
                }
            }
        }
        None
    }
}

fn compute_drop_target_locked(
    g: &FxDragInner,
    x: f32,
    y: f32,
    alt_duplicate: bool,
    dpx: f32,
) -> FxDragDropTarget {
    let wing = g.wing_bounds;
    let margin = tokens::FX_UNIT_DRAG_REMOVE_MARGIN * dpx.max(1.0);
    if wing.w > 1.0 && wing.h > 1.0 {
        let outside_remove = x < wing.x - margin
            || x > wing.x + wing.w + margin
            || y < wing.y - margin
            || y > wing.y + wing.h + margin;
        if outside_remove {
            return if alt_duplicate {
                FxDragDropTarget::None
            } else {
                FxDragDropTarget::Remove
            };
        }
    }

    // Find lane whose horizontal bounds contain `x` (or nearest lane if inside wing bounds).
    let mut target_bus = None;
    let mut best_dist = f32::MAX;
    for (b, &lb) in g.lane_bounds.iter().enumerate() {
        if lb.w <= 1.0 || lb.h <= 1.0 {
            continue;
        }
        if x >= lb.x && x <= lb.x + lb.w {
            target_bus = Some(b);
            break;
        }
        let cx = lb.x + lb.w * 0.5;
        let dist = (x - cx).abs();
        if dist < best_dist {
            best_dist = dist;
            target_bus = Some(b);
        }
    }

    let Some(bus) = target_bus else {
        return FxDragDropTarget::None;
    };

    let count = g.lane_card_counts[bus].min(NUM_FX_SLOTS);
    let mut pos = count;
    for (i, &entry) in g.card_rects[bus].iter().enumerate().take(count) {
        if let Some((_, rect)) = entry {
            if rect.h > 1.0 && y < rect.y + rect.h * 0.5 {
                pos = i;
                break;
            }
        }
    }
    FxDragDropTarget::Insert { bus, pos }
}

/// Top-level non-hoverable `SelfDirected` overlay drawing the live horizontal insertion indicator
/// and floating translucent FX card header ghost during drag-and-drop.
pub(crate) struct FxDragOverlay {
    drag_state: Arc<FxDragState>,
    bold_font: Cell<Option<vg::FontId>>,
}

impl FxDragOverlay {
    pub(crate) fn new(cx: &mut Context, drag_state: Arc<FxDragState>) -> Handle<'_, Self> {
        Self {
            drag_state,
            bold_font: Cell::new(None),
        }
        .build(cx, |_| {})
        .position_type(PositionType::SelfDirected)
        .left(Pixels(0.0))
        .top(Pixels(0.0))
        .width(Stretch(1.0))
        .height(Stretch(1.0))
        .z_index(380)
        .hoverable(false)
    }
}

impl View for FxDragOverlay {
    fn element(&self) -> Option<&'static str> {
        Some("fx-drag-overlay")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let Ok(g) = self.drag_state.inner.lock() else {
            return;
        };
        let Some(drag) = g.active else {
            return;
        };
        let dpx = cx.scale_factor();
        let pal = theme::current_palette(cx);
        let (ar, ag, ab) = pal.accent_rgb();

        // 1. Draw crisp horizontal insertion bar snapped to target slot gap
        if let FxDragDropTarget::Insert { bus, pos } = drag.drop_target {
            if bus < NUM_FX_BUSES {
                let lb = g.lane_bounds[bus];
                if lb.w > 1.0 && lb.h > 1.0 {
                    let count = g.lane_card_counts[bus].min(NUM_FX_SLOTS);
                    let line_y = if count == 0 {
                        lb.y + 4.0 * dpx
                    } else if pos == 0 {
                        g.card_rects[bus][0]
                            .map(|(_, r)| r.y - 2.0 * dpx)
                            .unwrap_or(lb.y + 4.0 * dpx)
                    } else if pos >= count {
                        g.card_rects[bus][count - 1]
                            .map(|(_, r)| r.y + r.h + 2.0 * dpx)
                            .unwrap_or(lb.y + 4.0 * dpx)
                    } else {
                        match (g.card_rects[bus][pos - 1], g.card_rects[bus][pos]) {
                            (Some((_, prev)), Some((_, next))) => {
                                0.5 * ((prev.y + prev.h) + next.y)
                            }
                            (_, Some((_, next))) => next.y - 2.0 * dpx,
                            (Some((_, prev)), _) => prev.y + prev.h + 2.0 * dpx,
                            _ => lb.y + 4.0 * dpx,
                        }
                    };
                    let clamped_y = line_y.clamp(lb.y + 2.0 * dpx, lb.y + lb.h - 2.0 * dpx);
                    let x0 = lb.x + 2.0 * dpx;
                    let x1 = lb.x + (tokens::FX_UNIT_W * dpx).min(lb.w - 4.0 * dpx);

                    let mut glow = vg::Path::new();
                    glow.move_to(x0, clamped_y);
                    glow.line_to(x1, clamped_y);
                    let mut glow_paint = vg::Paint::color(vg::Color::rgba(ar, ag, ab, 90));
                    glow_paint.set_line_width(5.0 * dpx);
                    glow_paint.set_line_cap(vg::LineCap::Round);
                    canvas.stroke_path(&glow, &glow_paint);

                    let mut bar = vg::Path::new();
                    bar.move_to(x0, clamped_y);
                    bar.line_to(x1, clamped_y);
                    let mut bar_paint = vg::Paint::color(vg::Color::rgba(ar, ag, ab, 245));
                    bar_paint.set_line_width(2.2 * dpx);
                    bar_paint.set_line_cap(vg::LineCap::Round);
                    canvas.stroke_path(&bar, &bar_paint);

                    for cx_end in [x0, x1] {
                        let mut cap = vg::Path::new();
                        cap.circle(cx_end, clamped_y, 3.0 * dpx);
                        canvas.fill_path(&cap, &vg::Paint::color(vg::Color::rgb(ar, ag, ab)));
                    }
                }
            }
        }

        // 2. Draw floating translucent FX card header ghost following cursor
        let is_remove = drag.drop_target == FxDragDropTarget::Remove;
        let gw = tokens::FX_UNIT_W * dpx;
        let gh = 22.0 * dpx;
        let gx = drag.cursor_x - gw * 0.5;
        let gy = drag.cursor_y - gh * 0.5 - 6.0 * dpx;
        let r = 4.0 * dpx;

        let mut card = vg::Path::new();
        card.rounded_rect(gx, gy, gw, gh, r);

        let (fill_col, border_col, text_col) = if is_remove {
            let (sr, sg, sb) = theme::hex_to_rgb(&pal.surface_dark);
            let (dr, dg, db) = theme::hex_to_rgb(&pal.mod_midi);
            let (mr, mg, mb) = theme::lerp_rgb((sr, sg, sb), (dr, dg, db), 0.35);
            (
                vg::Color::rgba(mr, mg, mb, 232),
                vg::Color::rgba(dr, dg, db, 250),
                theme::vg_color(&pal.text_bright, 255),
            )
        } else if drag.alt_duplicate {
            let (sr, sg, sb) = theme::hex_to_rgb(&pal.surface_hover);
            let (mr, mg, mb) = theme::lerp_rgb((sr, sg, sb), (ar, ag, ab), 0.28);
            (
                vg::Color::rgba(mr, mg, mb, 228),
                vg::Color::rgba(ar, ag, ab, 250),
                theme::vg_color(&pal.text_bright, 255),
            )
        } else {
            (
                theme::vg_color(&pal.surface_mid, 224),
                vg::Color::rgba(ar, ag, ab, 225),
                theme::vg_color(&pal.text_bright, 255),
            )
        };

        canvas.fill_path(&card, &vg::Paint::color(fill_col));
        let mut border = vg::Paint::color(border_col);
        border.set_line_width(1.25 * dpx);
        canvas.stroke_path(&card, &border);

        let label = if is_remove {
            format!("REMOVE  {}", drag.fx_type.label().to_uppercase())
        } else if drag.alt_duplicate {
            format!("+  {}", drag.fx_type.label())
        } else {
            drag.fx_type.label().to_string()
        };

        let mut text_paint = vg::Paint::color(text_col);
        if let Some(font_id) = j_syn_ui::shell::ensure_canvas_bold_font(canvas, &self.bold_font) {
            text_paint.set_font(&[font_id]);
        }
        text_paint.set_font_size(10.0 * dpx);
        text_paint.set_text_align(vg::Align::Center);
        text_paint.set_text_baseline(vg::Baseline::Middle);
        let _ = canvas.fill_text(gx + gw * 0.5, gy + gh * 0.5, &label, &text_paint);
    }
}
