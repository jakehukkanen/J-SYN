//! Custom skeuomorphic `femtovg` and `ParamWidgetBase` controls (`RotaryKnob`, `LedToggleSwitch`, `ParamStepPill`, `PitchDragBox`, `RatioDragBox`).

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use j_syn_dsp::mod_matrix::ModDestination;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::util::ModifiersExt;

use crate::shell::{ModUiHost, TooltipState, UiKitEvent};

pub type ValueFormatter = Arc<dyn Fn(f32, Option<f32>) -> String + Send + Sync>;
pub(super) type EnabledCheck = Arc<dyn Fn() -> bool + Send + Sync>;
pub(super) type DisabledTooltipFn = Arc<dyn Fn() -> String + Send + Sync>;

pub(super) const KNOB_DRAG_PIXELS: f32 = 150.0;
pub(super) const KNOB_FINE_MULTIPLIER: f32 = 0.12;
pub(super) const MOD_DEPTH_DRAG_PIXELS: f32 = 120.0;

#[derive(Debug, Clone, Copy)]
pub(super) struct DragState {
    pub start_y: f32,
    pub start_val: f32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ModDepthDrag {
    pub slot_idx: usize,
    pub start_y: f32,
    pub start_depth: f32,
}

#[derive(Clone)]
pub struct ModTargetBinding {
    pub dest: ModDestination,
    pub mod_ui: Arc<dyn ModUiHost>,
    pub tab_tag: u8,
}

pub(super) fn mod_slot_offsets(mb: &ModTargetBinding, md: ModDepthDrag) -> (f32, f32, f32) {
    let (slots, indices, count) = mb.mod_ui.slots_for_dest(mb.dest);
    let slot = (0..count)
        .find(|&i| indices[i] == md.slot_idx)
        .map(|i| slots[i])
        .or_else(|| mb.mod_ui.primary_slot_for_dest(mb.dest).map(|(_, s)| s));
    let (depth, bipolar) = match slot {
        Some(s) => (s.depth, s.bipolar),
        None => (md.start_depth, false),
    };
    let (lo_offset, hi_offset) = if bipolar {
        let span = depth.abs();
        (-span, span)
    } else if depth >= 0.0 {
        (0.0, depth)
    } else {
        (depth, 0.0)
    };
    (lo_offset, hi_offset, depth)
}

pub(super) fn format_mod_range_tooltip(
    base_norm: f32,
    mb: &ModTargetBinding,
    md: ModDepthDrag,
    fmt: impl Fn(f32, Option<f32>) -> String,
) -> String {
    let (lo_offset, hi_offset, depth) = mod_slot_offsets(mb, md);
    let lo_norm = (base_norm + lo_offset).clamp(0.0, 1.0);
    let hi_norm = (base_norm + hi_offset).clamp(0.0, 1.0);
    let min_str = fmt(lo_norm, Some(lo_offset));
    let max_str = fmt(hi_norm, Some(hi_offset));
    format!("{min_str} → {max_str} ({:+.0}%)", depth * 100.0)
}

pub(super) fn emit_widget_tooltip(cx: &mut EventContext, text: String) {
    let bounds = cx.bounds();
    if bounds.w <= 0.0 || bounds.h <= 0.0 {
        return;
    }
    let dpx = cx.scale_factor().max(0.1);
    cx.emit(UiKitEvent::ShowTooltip(TooltipState {
        visible: true,
        text,
        center_x: (bounds.x + bounds.w * 0.5) / dpx,
        anchor_top: bounds.y / dpx,
        anchor_bottom: (bounds.y + bounds.h) / dpx,
    }));
}

/// Handles Alt+drag modulation depth changes when active, returning true if handled.
pub(super) fn handle_mod_depth_move(
    mod_depth_drag: &mut Option<ModDepthDrag>,
    mod_binding: &Option<ModTargetBinding>,
    cx: &mut EventContext,
    y: f32,
) -> bool {
    let (Some(md), Some(mb)) = (mod_depth_drag.as_mut(), mod_binding.as_ref()) else {
        return false;
    };
    let sensitivity = if cx.modifiers().shift() { 0.2 } else { 1.0 };
    let dy = (md.start_y - y) / (MOD_DEPTH_DRAG_PIXELS * cx.scale_factor());
    let new_depth = (md.start_depth + dy * sensitivity).clamp(-1.0, 1.0);
    let prev_pct = (md.start_depth * 100.0).round() as i32;
    let next_pct = (new_depth * 100.0).round() as i32;
    mb.mod_ui.adjust_slot_depth(md.slot_idx, new_depth);
    md.start_y = y;
    md.start_depth = new_depth;
    if prev_pct != next_pct {
        cx.emit(UiKitEvent::ModUiChanged);
    }
    cx.needs_redraw();
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeCursorAxis {
    Default,
    Vertical,
    Horizontal,
}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
fn set_macos_cursor(axis: ResizeCursorAxis) {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let cursor: *mut Object = match axis {
            ResizeCursorAxis::Vertical => msg_send![class!(NSCursor), resizeUpDownCursor],
            ResizeCursorAxis::Horizontal => msg_send![class!(NSCursor), resizeLeftRightCursor],
            ResizeCursorAxis::Default => msg_send![class!(NSCursor), arrowCursor],
        };
        if !cursor.is_null() {
            let _: () = msg_send![cursor, set];
        }
    }
}

pub fn set_resize_cursor(cx: &mut EventContext, axis: ResizeCursorAxis) {
    let icon = match axis {
        ResizeCursorAxis::Vertical => CursorIcon::RowResize,
        ResizeCursorAxis::Horizontal => CursorIcon::ColResize,
        ResizeCursorAxis::Default => CursorIcon::Default,
    };
    cx.emit(WindowEvent::SetCursor(icon));
    #[cfg(target_os = "macos")]
    set_macos_cursor(axis);
}

pub(super) fn update_mod_collar_cursor(
    cx: &mut EventContext,
    active: bool,
    cursor_active: &mut bool,
) {
    if active {
        set_resize_cursor(cx, ResizeCursorAxis::Vertical);
        *cursor_active = true;
    } else if *cursor_active {
        set_resize_cursor(cx, ResizeCursorAxis::Default);
        *cursor_active = false;
    }
}

const SCROLL_WHEEL_COOLDOWN_MS: u64 = 250;
const SCROLL_POINTER_MOVE_PX: f32 = 2.0;

static SCROLL_EPOCH: OnceLock<Instant> = OnceLock::new();
static LAST_LANE_SCROLL_MS: AtomicU64 = AtomicU64::new(0);
static LAST_LANE_SCROLL_X: AtomicU32 = AtomicU32::new(0);
static LAST_LANE_SCROLL_Y: AtomicU32 = AtomicU32::new(0);

#[inline]
fn scroll_now_ms() -> u64 {
    let start = SCROLL_EPOCH.get_or_init(Instant::now);
    // Offset by +1 so 0 means "never scrolled".
    start.elapsed().as_millis() as u64 + 1
}

/// Records that a scrollable FX lane handled a mouse-wheel event at `(cursor_x, cursor_y)`.
pub fn notify_scroll_lane_wheeled(cursor_x: f32, cursor_y: f32) {
    LAST_LANE_SCROLL_X.store(cursor_x.to_bits(), Ordering::Relaxed);
    LAST_LANE_SCROLL_Y.store(cursor_y.to_bits(), Ordering::Relaxed);
    LAST_LANE_SCROLL_MS.store(scroll_now_ms(), Ordering::Release);
}

/// Returns `true` while an FX lane scroll cooldown (`250 ms`) is active and the pointer has not
/// deliberately moved (`>= 2 px`) onto a new control, so scrolling passes through to `ScrollView`.
pub fn is_scroll_wheel_guarded(cursor_x: f32, cursor_y: f32) -> bool {
    let last_ms = LAST_LANE_SCROLL_MS.load(Ordering::Acquire);
    if last_ms == 0 {
        return false;
    }
    let now_ms = scroll_now_ms();
    if now_ms.saturating_sub(last_ms) >= SCROLL_WHEEL_COOLDOWN_MS {
        return false;
    }
    let lx = f32::from_bits(LAST_LANE_SCROLL_X.load(Ordering::Relaxed));
    let ly = f32::from_bits(LAST_LANE_SCROLL_Y.load(Ordering::Relaxed));
    (cursor_x - lx).hypot(cursor_y - ly) < SCROLL_POINTER_MOVE_PX
}

mod knob;
mod pill;
mod pitch_box;
mod ratio;
mod switch;

pub use knob::{KnobDimExt, RotaryKnob};
pub use pill::ParamStepPill;
pub use pitch_box::{PitchDragBox, PitchGroupModOverlay, PitchPart, decompose_pitch_st};
pub use ratio::RatioDragBox;
pub use switch::{LedToggleSwitch, LedToggleSwitchExt};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_wheel_guard_blocks_stationary_cursor_and_releases_on_move_or_expiry() {
        notify_scroll_lane_wheeled(100.0, 200.0);
        // Stationary or sub-pixel jitter stays guarded.
        assert!(is_scroll_wheel_guarded(100.0, 200.0));
        assert!(is_scroll_wheel_guarded(101.0, 200.5));
        // Deliberate pointer movement (>= 2 px) releases the guard immediately.
        assert!(!is_scroll_wheel_guarded(103.0, 200.0));
        assert!(!is_scroll_wheel_guarded(100.0, 203.0));
        // Cooldown expiry (> 250 ms) releases the guard even with stationary cursor.
        std::thread::sleep(std::time::Duration::from_millis(SCROLL_WHEEL_COOLDOWN_MS + 15));
        assert!(!is_scroll_wheel_guarded(100.0, 200.0));
    }
}
