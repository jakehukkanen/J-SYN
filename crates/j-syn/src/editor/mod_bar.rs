//! Drag-and-drop Modulation Source Pill bar, live telemetry 60 Hz redraw ticker,
//! and inline modulation slot inspector (`UNI/BIP`, depth readout, slot cycle, remove).

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use j_syn_dsp::mod_matrix::{
    ModDestination, ModMatrixSource, ModSlot, ModUiTelemetry, NUM_MOD_DESTS, NUM_MOD_SLOTS,
};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use super::controls::bold_label;
use super::modseq_tab::ModSeqSubTab;
use super::{BottomTab, UiData, UiEvent};
use crate::JSynPluginParams;

const UNSET_U8: u8 = 255;

/// Converts `BottomTab` to the visibility tag used for drop-target hit testing (`1..=4`, `0` = always visible).
#[inline]
pub(crate) fn bottom_tab_tag(tab: BottomTab) -> u8 {
    match tab {
        BottomTab::VoiceEnv => 1,
        BottomTab::ModSeq => 2,
        BottomTab::Routing => 3,
        BottomTab::MacrosGlobal => 4,
    }
}

/// Shared lock-free UI coordination state for drag-and-drop assignment, drop rects, and inspector focus.
pub(crate) struct ModUiState {
    pub params: Arc<JSynPluginParams>,
    pub telemetry: Arc<ModUiTelemetry>,
    dragging_source: AtomicU8,
    hover_dest: AtomicU8,
    active_bottom_tab: AtomicU8,
    inspected_dest: AtomicU8,
    inspected_slot: AtomicU8,
    drop_rects: Mutex<[(f32, f32, f32, f32, u8); NUM_MOD_DESTS]>,
}

impl ModUiState {
    pub fn new(params: Arc<JSynPluginParams>, telemetry: Arc<ModUiTelemetry>) -> Self {
        Self {
            params,
            telemetry,
            dragging_source: AtomicU8::new(0),
            hover_dest: AtomicU8::new(UNSET_U8),
            active_bottom_tab: AtomicU8::new(bottom_tab_tag(BottomTab::VoiceEnv)),
            inspected_dest: AtomicU8::new(UNSET_U8),
            inspected_slot: AtomicU8::new(UNSET_U8),
            drop_rects: Mutex::new([(0.0, 0.0, 0.0, 0.0, UNSET_U8); NUM_MOD_DESTS]),
        }
    }

    pub fn set_active_tab(&self, tab: BottomTab) {
        self.active_bottom_tab
            .store(bottom_tab_tag(tab), Ordering::Relaxed);
    }

    #[inline]
    pub fn dragging_source(&self) -> ModMatrixSource {
        ModMatrixSource::from_u8(self.dragging_source.load(Ordering::Relaxed))
    }

    pub fn start_drag(&self, src: ModMatrixSource) {
        self.dragging_source.store(src as u8, Ordering::Relaxed);
        self.hover_dest.store(UNSET_U8, Ordering::Relaxed);
    }

    pub fn clear_drag(&self) {
        self.dragging_source.store(0, Ordering::Relaxed);
        self.hover_dest.store(UNSET_U8, Ordering::Relaxed);
    }

    #[inline]
    pub fn hover_destination(&self) -> Option<ModDestination> {
        let idx = self.hover_dest.load(Ordering::Relaxed);
        if idx == UNSET_U8 {
            None
        } else {
            ModDestination::from_index(idx as usize)
        }
    }

    #[inline]
    pub fn is_drop_target(&self, dest: ModDestination) -> bool {
        self.dragging_source() != ModMatrixSource::None
            && self.hover_dest.load(Ordering::Relaxed) == (dest.index() as u8)
    }

    pub fn register_drop_rect(&self, dest: ModDestination, tab_tag: u8, bounds: BoundingBox) {
        if let Ok(mut rects) = self.drop_rects.lock() {
            rects[dest.index()] = (bounds.x, bounds.y, bounds.w, bounds.h, tab_tag);
        }
    }

    fn hit_test_cursor(&self, x: f32, y: f32) -> Option<usize> {
        let active_tab = self.active_bottom_tab.load(Ordering::Relaxed);
        let rects = self.drop_rects.lock().ok()?;
        for (idx, &(rx, ry, rw, rh, tag)) in rects.iter().enumerate() {
            if rw <= 1.0 || rh <= 1.0 {
                continue;
            }
            if tag != 0 && tag != active_tab {
                continue;
            }
            if x >= rx && x <= rx + rw && y >= ry && y <= ry + rh {
                return Some(idx);
            }
        }
        None
    }

    /// Updates hovered destination during drag; returns `true` when hover changed.
    pub fn update_hover_from_cursor(&self, x: f32, y: f32) -> bool {
        let next = self
            .hit_test_cursor(x, y)
            .map(|idx| idx as u8)
            .unwrap_or(UNSET_U8);
        let prev = self.hover_dest.swap(next, Ordering::Relaxed);
        prev != next
    }

    /// Completes a drag-drop onto any hovered destination; returns `true` if matrix state changed.
    pub fn finish_drop_at_cursor(&self, x: f32, y: f32) -> bool {
        let src = self.dragging_source();
        let target_idx = self.hit_test_cursor(x, y);
        self.clear_drag();
        if let (Some(dest), false) = (
            target_idx.and_then(ModDestination::from_index),
            src == ModMatrixSource::None,
        ) {
            return self.assign_drop(src, dest).is_some();
        }
        false
    }

    pub fn assign_drop(&self, src: ModMatrixSource, dest: ModDestination) -> Option<usize> {
        let mut mat = self.params.mod_matrix.snapshot();
        let slot_idx = mat.assign_or_update(src, dest, 0.25)?;
        self.params.mod_matrix.store_value(mat);
        self.inspected_dest
            .store(dest.index() as u8, Ordering::Relaxed);
        self.inspected_slot.store(slot_idx as u8, Ordering::Relaxed);
        Some(slot_idx)
    }

    /// Collects up to 4 active modulation slots targeting `dest` for ring/bar rendering.
    pub fn slots_for_dest(&self, dest: ModDestination) -> ([ModSlot; 4], [usize; 4], usize) {
        let mat = self.params.mod_matrix.snapshot();
        let mut out_slots = [ModSlot::default(); 4];
        let mut out_indices = [0usize; 4];
        let mut count = 0usize;
        for (idx, slot) in mat.slots.iter().enumerate() {
            if slot.enabled && slot.source != ModMatrixSource::None && slot.destination == dest {
                if count < 4 {
                    out_slots[count] = *slot;
                    out_indices[count] = idx;
                    count += 1;
                } else {
                    break;
                }
            }
        }
        (out_slots, out_indices, count)
    }

    #[inline]
    pub fn has_slots_for_dest(&self, dest: ModDestination) -> bool {
        self.slots_for_dest(dest).2 > 0
    }

    pub fn primary_slot_for_dest(&self, dest: ModDestination) -> Option<(usize, ModSlot)> {
        let mat = self.params.mod_matrix.snapshot();
        let inspected = self.inspected_slot.load(Ordering::Relaxed) as usize;
        if inspected < NUM_MOD_SLOTS {
            let s = mat.slots[inspected];
            if s.enabled && s.source != ModMatrixSource::None && s.destination == dest {
                return Some((inspected, s));
            }
        }
        mat.slots
            .iter()
            .enumerate()
            .find(|(_, s)| s.enabled && s.source != ModMatrixSource::None && s.destination == dest)
            .map(|(idx, s)| (idx, *s))
    }

    pub fn focus_slot(&self, dest: ModDestination, slot_idx: usize) {
        self.inspected_dest
            .store(dest.index() as u8, Ordering::Relaxed);
        self.inspected_slot.store(slot_idx as u8, Ordering::Relaxed);
    }

    pub fn cycle_inspected_slot_for_dest(&self, dest: ModDestination) {
        let mat = self.params.mod_matrix.snapshot();
        let mut matches = [0usize; NUM_MOD_SLOTS];
        let mut count = 0usize;
        for (idx, slot) in mat.slots.iter().enumerate() {
            if slot.enabled && slot.source != ModMatrixSource::None && slot.destination == dest {
                matches[count] = idx;
                count += 1;
            }
        }
        if count == 0 {
            return;
        }
        let cur_slot = self.inspected_slot.load(Ordering::Relaxed) as usize;
        let cur_dest = self.inspected_dest.load(Ordering::Relaxed) as usize;
        let next_idx = if cur_dest == dest.index() {
            matches[..count]
                .iter()
                .position(|&idx| idx == cur_slot)
                .map(|pos| matches[(pos + 1) % count])
                .unwrap_or(matches[0])
        } else {
            matches[0]
        };
        self.focus_slot(dest, next_idx);
    }

    pub fn cycle_any_active_slot(&self) {
        let mat = self.params.mod_matrix.snapshot();
        let mut active = [0usize; NUM_MOD_SLOTS];
        let mut count = 0usize;
        for (idx, slot) in mat.slots.iter().enumerate() {
            if slot.enabled && slot.source != ModMatrixSource::None {
                active[count] = idx;
                count += 1;
            }
        }
        if count == 0 {
            self.inspected_slot.store(UNSET_U8, Ordering::Relaxed);
            self.inspected_dest.store(UNSET_U8, Ordering::Relaxed);
            return;
        }
        let cur = self.inspected_slot.load(Ordering::Relaxed) as usize;
        let next_idx = active[..count]
            .iter()
            .position(|&idx| idx == cur)
            .map(|pos| active[(pos + 1) % count])
            .unwrap_or(active[0]);
        self.focus_slot(mat.slots[next_idx].destination, next_idx);
    }

    pub fn adjust_slot_depth(&self, slot_idx: usize, new_depth: f32) {
        if slot_idx >= NUM_MOD_SLOTS {
            return;
        }
        let mut mat = self.params.mod_matrix.snapshot();
        if mat.slots[slot_idx].enabled {
            mat.slots[slot_idx].depth = new_depth.clamp(-1.0, 1.0);
            self.params.mod_matrix.store_value(mat);
            self.focus_slot(mat.slots[slot_idx].destination, slot_idx);
        }
    }

    pub fn remove_primary_slot_for_dest(&self, dest: ModDestination) -> bool {
        let Some((idx, _)) = self.primary_slot_for_dest(dest) else {
            return false;
        };
        let mut mat = self.params.mod_matrix.snapshot();
        mat.clear_slot(idx);
        self.params.mod_matrix.store_value(mat);

        if let Some((next_idx, _)) = mat
            .slots
            .iter()
            .enumerate()
            .find(|(_, s)| s.enabled && s.source != ModMatrixSource::None && s.destination == dest)
        {
            self.focus_slot(dest, next_idx);
        } else {
            self.inspected_slot.store(UNSET_U8, Ordering::Relaxed);
            self.inspected_dest.store(UNSET_U8, Ordering::Relaxed);
        }
        true
    }

    pub fn inspected_entry(&self) -> Option<(usize, ModSlot)> {
        let slot_idx = self.inspected_slot.load(Ordering::Relaxed) as usize;
        if slot_idx >= NUM_MOD_SLOTS {
            return None;
        }
        let mat = self.params.mod_matrix.snapshot();
        let slot = mat.slots[slot_idx];
        if slot.enabled && slot.source != ModMatrixSource::None {
            Some((slot_idx, slot))
        } else {
            None
        }
    }

    pub fn active_slot_count(&self) -> usize {
        let mat = self.params.mod_matrix.snapshot();
        mat.slots
            .iter()
            .filter(|s| s.enabled && s.source != ModMatrixSource::None)
            .count()
    }

    pub fn toggle_inspected_bipolar(&self) {
        let slot_idx = self.inspected_slot.load(Ordering::Relaxed) as usize;
        if slot_idx >= NUM_MOD_SLOTS {
            return;
        }
        let mut mat = self.params.mod_matrix.snapshot();
        if mat.slots[slot_idx].enabled {
            mat.slots[slot_idx].bipolar = !mat.slots[slot_idx].bipolar;
            self.params.mod_matrix.store_value(mat);
        }
    }

    pub fn remove_inspected_slot(&self) {
        let slot_idx = self.inspected_slot.load(Ordering::Relaxed) as usize;
        if slot_idx >= NUM_MOD_SLOTS {
            return;
        }
        let mut mat = self.params.mod_matrix.snapshot();
        let old_dest = mat.slots[slot_idx].destination;
        mat.clear_slot(slot_idx);
        self.params.mod_matrix.store_value(mat);
        if let Some((next_idx, _)) = mat.slots.iter().enumerate().find(|(_, s)| {
            s.enabled && s.source != ModMatrixSource::None && s.destination == old_dest
        }) {
            self.focus_slot(old_dest, next_idx);
        } else {
            self.inspected_slot.store(UNSET_U8, Ordering::Relaxed);
            self.inspected_dest.store(UNSET_U8, Ordering::Relaxed);
        }
    }
}

/// Draggable modulation source pill (`E1..E4`, `MS1..MS4`, `VEL`, `KEY`, `MW`, `AT`).
struct ModSourcePill {
    source: ModMatrixSource,
    mod_ui: Arc<ModUiState>,
    drag_active: bool,
    drag_moved: bool,
    hovered: bool,
    start_x: f32,
    start_y: f32,
}

impl ModSourcePill {
    fn new<'a>(
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
                .height(Stretch(1.0))
                .child_space(Stretch(1.0))
                .hoverable(false);
        })
    }

    fn subtab_for_source(src: ModMatrixSource) -> Option<ModSeqSubTab> {
        match src {
            ModMatrixSource::ModSeq1 => Some(ModSeqSubTab::MSeq(0)),
            ModMatrixSource::ModSeq2 => Some(ModSeqSubTab::MSeq(1)),
            ModMatrixSource::ModSeq3 => Some(ModSeqSubTab::MSeq(2)),
            ModMatrixSource::ModSeq4 => Some(ModSeqSubTab::MSeq(3)),
            ModMatrixSource::Env2 => Some(ModSeqSubTab::Env(0)),
            ModMatrixSource::Env3 => Some(ModSeqSubTab::Env(1)),
            ModMatrixSource::Env4 => Some(ModSeqSubTab::Env(2)),
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
                        self.mod_ui.finish_drop_at_cursor(cx.mouse().cursorx, cx.mouse().cursory);
                    } else {
                        self.mod_ui.clear_drag();
                        if self.source == ModMatrixSource::Env1 {
                            cx.emit(UiEvent::SelectTab(BottomTab::VoiceEnv));
                        } else if let Some(sub) = Self::subtab_for_source(self.source) {
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
        let (r, g, b) = self.source.color_rgb();
        let is_dragging = self.mod_ui.dragging_source() == self.source;
        let is_hovered = self.hovered;
        let level = self.mod_ui.telemetry.source_level(self.source).clamp(0.0, 1.0);

        let mut bg = vg::Path::new();
        bg.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, 3.0 * dpx);
        let fill = if is_dragging {
            vg::Color::rgba(r, g, b, 90)
        } else if is_hovered {
            vg::Color::rgb(68, 56, 46)
        } else {
            vg::Color::rgb(46, 39, 34)
        };
        canvas.fill_path(&bg, &vg::Paint::color(fill));

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
        let mut stripe = vg::Path::new();
        stripe.rounded_rect(
            bounds.x + 1.5 * dpx,
            bounds.y + 3.0 * dpx,
            2.2 * dpx,
            bounds.h - 6.0 * dpx,
            1.0 * dpx,
        );
        canvas.fill_path(&stripe, &vg::Paint::color(vg::Color::rgb(r, g, b)));

        let mut border = vg::Paint::color(if is_dragging {
            vg::Color::rgb(r, g, b)
        } else if is_hovered {
            vg::Color::rgba(r, g, b, 195)
        } else {
            vg::Color::rgba(r, g, b, 90)
        });
        border.set_line_width(1.0 * dpx);
        canvas.stroke_path(&bg, &border);
    }
}

/// Builds the 12 modulation source pills + inline slot inspector inside the bottom `.tab-bar`
/// and starts a 60 Hz redraw timer (`16 ms`) for live mod-ring and playhead animations.
pub(crate) fn build_mod_source_bar(cx: &mut Context, mod_ui: Arc<ModUiState>) {
    let timer = cx.add_timer(Duration::from_millis(16), None, |cx, action| {
        if matches!(action, TimerAction::Tick(_)) {
            cx.needs_redraw();
        }
    });
    cx.start_timer(timer);

    let ui_for_pills = mod_ui.clone();
    HStack::new(cx, move |cx| {
        for &src in &ModMatrixSource::ALL_ACTIVE {
            ModSourcePill::new(cx, src, ui_for_pills.clone())
                .width(Pixels(40.0))
                .height(Pixels(18.0))
                .top(Stretch(1.0))
                .bottom(Stretch(1.0));
        }
    })
    .col_between(Pixels(3.0))
    .width(Auto)
    .height(Stretch(1.0));

    // Inline modulation inspector (drag status or selected slot UNI/BIP + remove)
    HStack::new(cx, move |cx| {
        let ui_insp = mod_ui.clone();
        Binding::new(cx, UiData::mod_ui_rev, move |cx, _| {
            let drag_src = ui_insp.dragging_source();
            if drag_src != ModMatrixSource::None {
                let text = match ui_insp.hover_destination() {
                    Some(dest) => format!("DROP {} > {}", drag_src.short_label(), dest.name()),
                    None => format!("DRAG {} -> KNOB", drag_src.short_label()),
                };
                bold_label(cx, &text, "mod-insp-drag")
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_space(Stretch(1.0));
            } else if let Some((_idx, slot)) = ui_insp.inspected_entry() {
                let ui_cycle = ui_insp.clone();
                let ui_bip = ui_insp.clone();
                let ui_rem = ui_insp.clone();
                let dest = slot.destination;
                let summary = format!(
                    "{}>{} {:+.0}%",
                    slot.source.short_label(),
                    dest.name(),
                    slot.depth * 100.0
                );
                let bip_txt = if slot.bipolar { "BIP" } else { "UNI" };

                Button::new(
                    cx,
                    move |cx| {
                        ui_cycle.cycle_inspected_slot_for_dest(dest);
                        cx.emit(UiEvent::ModUiChanged);
                    },
                    move |cx| {
                        bold_label(cx, &summary, "mod-insp-text")
                            .width(Stretch(1.0))
                            .height(Stretch(1.0))
                            .child_top(Stretch(1.0))
                            .child_bottom(Stretch(1.0))
                            .hoverable(false)
                    },
                )
                .class("mod-insp-slot")
                .width(Stretch(1.0))
                .height(Pixels(18.0));

                Button::new(
                    cx,
                    move |cx| {
                        ui_bip.toggle_inspected_bipolar();
                        cx.emit(UiEvent::ModUiChanged);
                    },
                    move |cx| {
                        bold_label(cx, bip_txt, "mod-insp-btn-text")
                            .child_space(Stretch(1.0))
                            .hoverable(false)
                    },
                )
                .class("mod-insp-btn")
                .checked(slot.bipolar)
                .width(Pixels(30.0))
                .height(Pixels(18.0));

                Button::new(
                    cx,
                    move |cx| {
                        ui_rem.remove_inspected_slot();
                        cx.emit(UiEvent::ModUiChanged);
                    },
                    |cx| {
                        bold_label(cx, "x", "mod-insp-btn-text")
                            .child_space(Stretch(1.0))
                            .hoverable(false)
                    },
                )
                .class("mod-insp-del")
                .width(Pixels(18.0))
                .height(Pixels(18.0));
            } else {
                let ui_all = ui_insp.clone();
                let active_n = ui_insp.active_slot_count();
                let hint = if active_n > 0 {
                    format!("MODS: {active_n}/32  [CYCLE]")
                } else {
                    String::from("DRAG SRC -> KNOB  |  R-CLICK MOD")
                };
                Button::new(
                    cx,
                    move |cx| {
                        ui_all.cycle_any_active_slot();
                        cx.emit(UiEvent::ModUiChanged);
                    },
                    move |cx| {
                        bold_label(cx, &hint, "mod-insp-hint")
                            .width(Stretch(1.0))
                            .height(Stretch(1.0))
                            .child_space(Stretch(1.0))
                            .hoverable(false)
                    },
                )
                .class("mod-insp-slot")
                .width(Stretch(1.0))
                .height(Pixels(18.0));
            }
        });
    })
    .class("mod-insp-bar")
    .col_between(Pixels(3.0))
    .width(Stretch(1.0))
    .height(Pixels(18.0))
    .top(Stretch(1.0))
    .bottom(Stretch(1.0));
}
