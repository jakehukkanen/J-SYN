//! J-SYN adapter for `j_syn_ui::scopes` FX visualizers plus J-SYN-specific
//! `SubClipperScope`, `InsertTransferScope`, and `MasterScope` views.

use std::sync::Arc;

use j_syn_dsp::evaluate_insert_curve;
use j_syn_dsp::mod_matrix::{FxScopeRing, ModDestination, ModUiTelemetry, FX_SCOPE_LEN};
use j_syn_dsp::{MB_XOVER_HIGH_DEFAULT_NORM, MB_XOVER_LOW_DEFAULT_NORM};
use nih_plug::prelude::{GuiContext, ParamSetter};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::RawParamEvent;

pub(crate) use j_syn_ui::scopes::{
    fx_accent_rgb, FxSlotScopeSnapshot, FxSlotVisualizer, MbBandScopeSnapshot,
    MultibandXoverDragOverlay, MultibandXoverHost,
};

use super::mod_bar::ModUiState;
use super::scope::{paint_scope_bg, paint_scope_border};
use super::theme;
use crate::JSynPluginParams;

/// Builds a snapshot of a J-SYN FX rack slot for `j_syn_ui::scopes::FxSlotVisualizer`.
pub(crate) fn slot_scope_snapshot(
    bus_idx: usize,
    slot_idx: usize,
    params: &JSynPluginParams,
    mod_ui: &ModUiState,
) -> FxSlotScopeSnapshot {
    let slot = &params.fx_bus_slots(bus_idx)[slot_idx];
    let tel = &mod_ui.telemetry;
    let mut pre = [0.0_f32; FX_SCOPE_LEN];
    let mut post = [0.0_f32; FX_SCOPE_LEN];
    let activity = tel.fx_scope(bus_idx, slot_idx).read_snapshot(&mut pre, &mut post);

    let eff_p = [
        (slot.p1.value()
            + tel.dest_offset(ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 0)))
        .clamp(0.0, 1.0),
        (slot.p2.value()
            + tel.dest_offset(ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 1)))
        .clamp(0.0, 1.0),
        (slot.p3.value()
            + tel.dest_offset(ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 2)))
        .clamp(0.0, 1.0),
        (slot.p4.value()
            + tel.dest_offset(ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 3)))
        .clamp(0.0, 1.0),
    ];
    let eff_eq = [
        (slot.p5.value()
            + tel.dest_offset(ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 4)))
        .clamp(0.0, 1.0),
        (slot.p6.value()
            + tel.dest_offset(ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 5)))
        .clamp(0.0, 1.0),
        (slot.p7.value()
            + tel.dest_offset(ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 6)))
        .clamp(0.0, 1.0),
        (slot.p8.value()
            + tel.dest_offset(ModDestination::fx_bus_slot_param(bus_idx, slot_idx, 7)))
        .clamp(0.0, 1.0),
    ];

    FxSlotScopeSnapshot {
        enabled: slot.enabled.value(),
        fx_type: slot.fx_type.value().into(),
        stack: slot.stack.value().clamp(1, 4) as u8,
        eff_p,
        eff_eq,
        filter_mode: slot.filter_mode.value().into(),
        filter_slope: slot.filter_slope.value().into(),
        diopser_stages_idx: slot.diopser_stages.value().to_index() as usize,
        tube_res_filter_enabled: slot.tube_res_filter_enabled.value(),
        utility_inv_l: slot.utility_inv_l.value(),
        utility_inv_r: slot.utility_inv_r.value(),
        distortion_mode: slot.distortion_mode.value().into(),
        dirty_lfo: !params.hq_mode.value(),
        mb_xover_p1: slot.p1.value(),
        mb_xover_p2: slot.p2.value(),
        mb_tab: mod_ui.fx_mb_tab(bus_idx, slot_idx),
        mb_bands: std::array::from_fn(|b| {
            let mb = &slot.mb_bands[b];
            MbBandScopeSnapshot {
                enabled: mb.enabled.value(),
                solo: mb.solo.value(),
                drive: mb.drive.value().clamp(0.0, 1.0),
                level: mb.level.value(),
            }
        }),
        pre,
        post,
        activity,
    }
}

pub(crate) struct JSynMbXoverHost {
    pub bus_idx: usize,
    pub slot_idx: usize,
    pub params: Arc<JSynPluginParams>,
    pub gui_context: Arc<dyn GuiContext>,
    pub mod_ui: Arc<ModUiState>,
}

impl MultibandXoverHost for JSynMbXoverHost {
    fn xover_norms(&self) -> (f32, f32) {
        let slot = &self.params.fx_bus_slots(self.bus_idx)[self.slot_idx];
        (slot.p1.value(), slot.p2.value())
    }

    fn begin_xover(&self, handle: usize) {
        let slot = &self.params.fx_bus_slots(self.bus_idx)[self.slot_idx];
        let param = if handle == 0 { &slot.p1 } else { &slot.p2 };
        ParamSetter::new(self.gui_context.as_ref()).begin_set_parameter(param);
    }

    fn set_xover(&self, handle: usize, norm: f32) {
        let slot = &self.params.fx_bus_slots(self.bus_idx)[self.slot_idx];
        let param = if handle == 0 { &slot.p1 } else { &slot.p2 };
        ParamSetter::new(self.gui_context.as_ref()).set_parameter(param, norm);
    }

    fn end_xover(&self, handle: usize) {
        let slot = &self.params.fx_bus_slots(self.bus_idx)[self.slot_idx];
        let param = if handle == 0 { &slot.p1 } else { &slot.p2 };
        ParamSetter::new(self.gui_context.as_ref()).end_set_parameter(param);
        let _ = (MB_XOVER_LOW_DEFAULT_NORM, MB_XOVER_HIGH_DEFAULT_NORM);
    }

    fn select_band_tab(&self, tab: u8) {
        self.mod_ui.set_fx_mb_tab(self.bus_idx, self.slot_idx, tab);
    }
}

#[inline]
fn read_ring_snapshot(ring: &FxScopeRing) -> ([f32; FX_SCOPE_LEN], [f32; FX_SCOPE_LEN], f32) {
    let mut pre = [0.0_f32; FX_SCOPE_LEN];
    let mut post = [0.0_f32; FX_SCOPE_LEN];
    let act = ring.read_snapshot(&mut pre, &mut post);
    (pre, post, act)
}

/// Compact clipper scope for the `SUB FX` banner showing pre-clip vs post-clip waveform.
#[allow(dead_code)]
pub(super) struct SubClipperScope {
    params: Arc<JSynPluginParams>,
    telemetry: Arc<ModUiTelemetry>,
}

#[allow(dead_code)]
impl SubClipperScope {
    pub(super) fn new<'a>(
        cx: &'a mut Context,
        params: Arc<JSynPluginParams>,
        telemetry: Arc<ModUiTelemetry>,
    ) -> Handle<'a, Self> {
        Self { params, telemetry }.build(cx, |_| {})
    }
}

impl View for SubClipperScope {
    fn element(&self) -> Option<&'static str> {
        Some("sub-clipper-scope")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|raw_param_event, _| {
            if let RawParamEvent::ParametersChanged = raw_param_event {
                cx.needs_redraw();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 4.0 || bounds.h <= 4.0 {
            return;
        }
        let dpx = cx.scale_factor();
        let pal = theme::current_palette(cx);
        let enabled = self.params.sub_bus.enabled.value();
        let (pre, post, gr_act) = read_ring_snapshot(&self.telemetry.sub_scope);
        j_syn_ui::scopes::paint_clipper_waveform(
            canvas, bounds, dpx, &pre, &post, gr_act, 1.0, 1.25, enabled, &pal,
        );
    }
}

/// Compact transfer-curve visualizer for each per-voice Insert Shaper card (`INS 1` / `INS 2`).
pub(super) struct InsertTransferScope {
    slot_idx: usize,
    params: Arc<JSynPluginParams>,
    accent_rgb: (u8, u8, u8),
}

impl InsertTransferScope {
    pub(super) fn new<'a>(
        cx: &'a mut Context,
        slot_idx: usize,
        params: Arc<JSynPluginParams>,
        accent_rgb: (u8, u8, u8),
    ) -> Handle<'a, Self> {
        Self {
            slot_idx,
            params,
            accent_rgb,
        }
        .build(cx, |_| {})
    }
}

impl View for InsertTransferScope {
    fn element(&self) -> Option<&'static str> {
        Some("insert-transfer-scope")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|raw_param_event, _| {
            if let RawParamEvent::ParametersChanged = raw_param_event {
                cx.needs_redraw();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 4.0 || bounds.h <= 4.0 {
            return;
        }
        let dpx = cx.scale_factor();
        let pal = theme::current_palette(cx);
        let ins = &self.params.routing.inserts[self.slot_idx];
        let enabled = ins.enabled.value();
        let radius = 3.5 * dpx;
        paint_scope_bg(canvas, bounds, radius, &pal, enabled);

        let pad_x = 5.0 * dpx;
        let pad_y = 4.0 * dpx;
        let ix = bounds.x + pad_x;
        let iy = bounds.y + pad_y;
        let iw = (bounds.w - 2.0 * pad_x).max(2.0);
        let ih = (bounds.h - 2.0 * pad_y).max(2.0);

        let mut ref_path = vg::Path::new();
        ref_path.move_to(ix, iy + ih * 0.5);
        ref_path.line_to(ix + iw, iy + ih * 0.5);
        ref_path.move_to(ix + iw * 0.5, iy);
        ref_path.line_to(ix + iw * 0.5, iy + ih);
        ref_path.move_to(ix, iy + ih);
        ref_path.line_to(ix + iw, iy);
        let mut ref_paint = vg::Paint::color(theme::vg_color(
            &pal.scope_grid,
            if enabled { 130 } else { 55 },
        ));
        ref_paint.set_line_width(1.0 * dpx);
        canvas.stroke_path(&ref_path, &ref_paint);

        let mode: j_syn_dsp::InsertMode = ins.mode.value().into();
        let drive = ins.drive.value().clamp(0.0, 1.0);
        let mix = ins.mix.value().clamp(0.0, 1.0);
        let mut xfer = vg::Path::new();
        const PTS: usize = 64;
        for i in 0..PTS {
            let t = i as f32 / (PTS - 1) as f32;
            let x_in = t * 2.0 - 1.0;
            let wet = evaluate_insert_curve(mode, x_in, drive);
            let y_out = (x_in + (wet - x_in) * mix).clamp(-1.0, 1.0);
            let px = ix + t * iw;
            let py = iy + (1.0 - (y_out * 0.5 + 0.5)) * ih;
            if i == 0 {
                xfer.move_to(px, py);
            } else {
                xfer.line_to(px, py);
            }
        }
        let (cr, cg, cb) = j_syn_ui::scopes::scope_trace_rgb(self.accent_rgb, &pal, enabled);
        let mut xfer_paint = vg::Paint::color(vg::Color::rgba(
            cr,
            cg,
            cb,
            if enabled { 235 } else { 100 },
        ));
        xfer_paint.set_line_width(1.6 * dpx);
        canvas.stroke_path(&xfer, &xfer_paint);

        paint_scope_border(canvas, bounds, dpx, radius, &pal, enabled);
    }
}

/// Compact output/limiter scope in the header bar between `MASTER` and `MasterMeter`.
pub(super) struct MasterScope {
    params: Arc<JSynPluginParams>,
    telemetry: Arc<ModUiTelemetry>,
}

impl MasterScope {
    pub(super) fn new<'a>(
        cx: &'a mut Context,
        params: Arc<JSynPluginParams>,
        telemetry: Arc<ModUiTelemetry>,
    ) -> Handle<'a, Self> {
        Self { params, telemetry }.build(cx, |_| {})
    }
}

impl View for MasterScope {
    fn element(&self) -> Option<&'static str> {
        Some("master-scope")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|raw_param_event, _| {
            if let RawParamEvent::ParametersChanged = raw_param_event {
                cx.needs_redraw();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 4.0 || bounds.h <= 4.0 {
            return;
        }
        let dpx = cx.scale_factor();
        let pal = theme::current_palette(cx);
        let ceil_lin = if self.params.master_limiter.value() {
            0.988_553_1
        } else {
            1.0
        };
        let (pre, post, gr_act) = read_ring_snapshot(&self.telemetry.master_scope);
        j_syn_ui::scopes::paint_clipper_waveform(
            canvas,
            bounds,
            dpx,
            &pre,
            &post,
            gr_act,
            ceil_lin,
            1.25,
            true,
            &pal,
        );
    }
}
