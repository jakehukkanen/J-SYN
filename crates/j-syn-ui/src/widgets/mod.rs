//! Custom skeuomorphic `femtovg` and `ParamWidgetBase` controls (`RotaryKnob`, `LedToggleSwitch`, `ParamStepPill`, `PitchDragBox`, `RatioDragBox`).

use std::sync::Arc;

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

mod knob;
mod pill;
mod ratio;
mod switch;

pub use knob::{KnobDimExt, RotaryKnob};
pub use pill::{ParamStepPill, PitchDragBox, PitchGroupModOverlay, PitchPart, decompose_pitch_st};
pub use ratio::RatioDragBox;
pub use switch::LedToggleSwitch;
