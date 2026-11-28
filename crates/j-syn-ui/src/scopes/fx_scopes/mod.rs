//! Custom `femtovg` FX scope visualizers for all 14 FX Unit types (`Utility`, `Clipper`,
//! `Limiter`, `Filter`, `OTT`, `Distortion`, `MB Dist`, `Chorus`, `Flanger`, `Phaser`,
//! `Delay`, `Reverb`, `Dispersion`, `TubeResonator`), shared by `J-SYN` and standalone FX plugins.

mod dynamics;
mod spectral;
mod time_mod;

pub use dynamics::*;
pub use spectral::*;
pub use time_mod::*;

use std::sync::{Arc, OnceLock};
use std::time::Instant;

use j_syn_dsp::mod_matrix::FX_SCOPE_LEN;
use j_syn_dsp::{
    DistortionMode, FilterMode, FxFilterSlope, FxType, MB_XOVER_HIGH_DEFAULT_NORM,
    MB_XOVER_LOW_DEFAULT_NORM, mb_ordered_xover_hz, mb_xover_high_norm, mb_xover_low_norm,
};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::RawParamEvent;

use crate::shell::{TooltipState, UiKitEvent};
use crate::theme;
use crate::widgets::{ResizeCursorAxis, set_resize_cursor};

use super::filter_freq_to_norm;

#[derive(Clone, Copy, Default)]
pub struct MbBandScopeSnapshot {
    pub enabled: bool,
    pub solo: bool,
    pub drive: f32,
    pub level: f32,
}

#[derive(Clone)]
pub struct FxSlotScopeSnapshot {
    pub enabled: bool,
    pub fx_type: FxType,
    pub stack: u8,
    pub eff_p: [f32; 4],
    pub eff_eq: [f32; 4],
    pub filter_mode: FilterMode,
    pub filter_slope: FxFilterSlope,
    pub diopser_stages_idx: usize,
    pub tube_res_filter_enabled: bool,
    pub utility_inv_l: bool,
    pub utility_inv_r: bool,
    pub distortion_mode: DistortionMode,
    pub dirty_lfo: bool,
    pub mb_xover_p1: f32,
    pub mb_xover_p2: f32,
    pub mb_tab: u8,
    pub mb_bands: [MbBandScopeSnapshot; 3],
    pub pre: [f32; FX_SCOPE_LEN],
    pub post: [f32; FX_SCOPE_LEN],
    pub activity: f32,
}

impl Default for FxSlotScopeSnapshot {
    fn default() -> Self {
        Self {
            enabled: true,
            fx_type: FxType::Off,
            stack: 1,
            eff_p: [0.0; 4],
            eff_eq: [0.0; 4],
            filter_mode: FilterMode::Lp,
            filter_slope: FxFilterSlope::Db12,
            diopser_stages_idx: 2,
            tube_res_filter_enabled: true,
            utility_inv_l: false,
            utility_inv_r: false,
            distortion_mode: DistortionMode::Tube,
            dirty_lfo: false,
            mb_xover_p1: MB_XOVER_LOW_DEFAULT_NORM,
            mb_xover_p2: MB_XOVER_HIGH_DEFAULT_NORM,
            mb_tab: 0,
            mb_bands: [MbBandScopeSnapshot::default(); 3],
            pre: [0.0; FX_SCOPE_LEN],
            post: [0.0; FX_SCOPE_LEN],
            activity: 0.0,
        }
    }
}

#[inline]
pub fn fx_accent_rgb(fx_type: FxType) -> (u8, u8, u8) {
    match fx_type {
        FxType::Off => (120, 110, 100),
        FxType::Ott => (72, 200, 185),
        FxType::Distortion => (235, 118, 62),
        FxType::Chorus => (138, 178, 245),
        FxType::Delay => (245, 179, 73),
        FxType::Reverb => (195, 145, 245),
        FxType::JSatClipper => (245, 86, 86),
        FxType::Filter => (112, 224, 154),
        FxType::Phaser => (116, 208, 255),
        FxType::Diopser => (92, 226, 210),
        FxType::TubeResonator => (255, 164, 88),
        FxType::Utility => (108, 214, 255),
        FxType::Limiter => (255, 122, 89),
        FxType::MultibandDist => (255, 108, 72),
        FxType::Flanger => (156, 168, 255),
    }
}

#[inline]
pub(super) fn scope_elapsed_secs() -> f32 {
    static START_EPOCH: OnceLock<Instant> = OnceLock::new();
    START_EPOCH
        .get_or_init(Instant::now)
        .elapsed()
        .as_secs_f32()
}

/// Multi-FX live waveform, filter curve, dispersion ripple, & activity visualizer
/// powered by any plugin's `FxSlotScopeSnapshot` provider closure.
pub struct FxSlotVisualizer {
    provider: Arc<dyn Fn() -> FxSlotScopeSnapshot + Send + Sync>,
}

impl FxSlotVisualizer {
    pub fn new<'a, F>(cx: &'a mut Context, provider: F) -> Handle<'a, Self>
    where
        F: Fn() -> FxSlotScopeSnapshot + Send + Sync + 'static,
    {
        Self {
            provider: Arc::new(provider),
        }
        .build(cx, |_| {})
    }
}

impl View for FxSlotVisualizer {
    fn element(&self) -> Option<&'static str> {
        Some("fx-slot-visualizer")
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
        let snap = (self.provider)();
        match snap.fx_type {
            FxType::JSatClipper => paint_clipper_scope(canvas, bounds, dpx, &snap, &pal),
            FxType::Limiter => paint_limiter_scope(canvas, bounds, dpx, &snap, &pal),
            FxType::MultibandDist => paint_multiband_dist_scope(canvas, bounds, dpx, &snap, &pal),
            FxType::Flanger => paint_flanger_scope(canvas, bounds, dpx, &snap, &pal),
            FxType::Filter => paint_filter_scope(canvas, bounds, dpx, &snap, &pal),
            FxType::Diopser => paint_diopser_scope(canvas, bounds, dpx, &snap, &pal),
            FxType::TubeResonator => paint_tube_res_scope(canvas, bounds, dpx, &snap, &pal),
            FxType::Utility => paint_utility_scope(canvas, bounds, dpx, &snap, &pal),
            FxType::Ott => paint_ott_scope(canvas, bounds, dpx, &snap, &pal),
            FxType::Distortion => paint_distortion_scope(canvas, bounds, dpx, &snap, &pal),
            FxType::Delay | FxType::Chorus | FxType::Reverb | FxType::Phaser => {
                paint_time_fx_scope(canvas, bounds, dpx, &snap, &pal)
            }
            FxType::Off => {}
        }
    }
}

/// Host abstraction for dragging `LOW XOV` / `HIGH XOV` markers on `MultibandDist` scopes.
pub trait MultibandXoverHost: Send + Sync + 'static {
    fn xover_norms(&self) -> (f32, f32);
    fn begin_xover(&self, handle: usize);
    fn set_xover(&self, handle: usize, norm: f32);
    fn end_xover(&self, handle: usize);
    fn select_band_tab(&self, tab: u8);
}

/// Transparent overlay enabling horizontal drag of `LOW XOV` / `HIGH XOV` markers on `MultibandDist` scopes.
pub struct MultibandXoverDragOverlay {
    host: Arc<dyn MultibandXoverHost>,
    dragging_handle: Option<usize>,
    hover_cursor_active: bool,
}

impl MultibandXoverDragOverlay {
    pub fn new<'a>(cx: &'a mut Context, host: Arc<dyn MultibandXoverHost>) -> Handle<'a, Self> {
        Self {
            host,
            dragging_handle: None,
            hover_cursor_active: false,
        }
        .build(cx, |_| {})
    }

    fn x_to_hz(&self, bounds: BoundingBox, dpx: f32, mouse_x: f32) -> f32 {
        let pad_x = 4.0 * dpx;
        let ix = bounds.x + pad_x;
        let iw = (bounds.w - 2.0 * pad_x).max(1.0);
        let t = ((mouse_x - ix) / iw).clamp(0.0, 1.0);
        20.0 * 1000.0_f32.powf(t)
    }

    fn nearest_handle(&self, bounds: BoundingBox, dpx: f32, mouse_x: f32) -> Option<usize> {
        let (p1, p2) = self.host.xover_norms();
        let (low_hz, high_hz) = mb_ordered_xover_hz(p1, p2);
        let pad_x = 4.0 * dpx;
        let ix = bounds.x + pad_x;
        let iw = (bounds.w - 2.0 * pad_x).max(1.0);
        let x_low = ix + filter_freq_to_norm(low_hz) * iw;
        let x_high = ix + filter_freq_to_norm(high_hz) * iw;
        let d_low = (mouse_x - x_low).abs();
        let d_high = (mouse_x - x_high).abs();
        if d_low.min(d_high) <= 10.0 * dpx {
            Some(if d_low <= d_high { 0 } else { 1 })
        } else {
            None
        }
    }

    fn emit_xover_tooltip(
        &self,
        cx: &mut EventContext,
        bounds: BoundingBox,
        dpx: f32,
        handle: usize,
    ) {
        let (p1, p2) = self.host.xover_norms();
        let (low_hz, high_hz) = mb_ordered_xover_hz(p1, p2);
        let hz = if handle == 0 { low_hz } else { high_hz };
        let text = if hz >= 10_000.0 {
            format!("{:.1} kHz", hz / 1000.0)
        } else if hz >= 1000.0 {
            format!("{:.2} kHz", hz / 1000.0)
        } else {
            format!("{:.0} Hz", hz)
        };
        let pad_x = 4.0 * dpx;
        let ix = bounds.x + pad_x;
        let iw = (bounds.w - 2.0 * pad_x).max(1.0);
        let handle_x = ix + filter_freq_to_norm(hz) * iw;
        let s = dpx.max(0.1);
        cx.emit(UiKitEvent::ShowTooltip(TooltipState {
            visible: true,
            text,
            center_x: handle_x / s,
            anchor_top: bounds.y / s,
            anchor_bottom: (bounds.y + bounds.h) / s,
        }));
    }
}

impl View for MultibandXoverDragOverlay {
    fn element(&self) -> Option<&'static str> {
        Some("mb-xover-drag")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                let bounds = cx.bounds();
                let dpx = cx.scale_factor();
                let mx = cx.mouse().cursorx;
                if let Some(handle) = self.nearest_handle(bounds, dpx, mx) {
                    self.host.begin_xover(handle);
                    self.dragging_handle = Some(handle);
                    self.hover_cursor_active = true;
                    cx.capture();
                    set_resize_cursor(cx, ResizeCursorAxis::Horizontal);
                    self.emit_xover_tooltip(cx, bounds, dpx, handle);
                    meta.consume();
                } else {
                    let hz = self.x_to_hz(bounds, dpx, mx);
                    let (p1, p2) = self.host.xover_norms();
                    let (low_hz, high_hz) = mb_ordered_xover_hz(p1, p2);
                    let target_tab = if hz < low_hz {
                        1
                    } else if hz <= high_hz {
                        2
                    } else {
                        3
                    };
                    self.host.select_band_tab(target_tab);
                    cx.emit(UiKitEvent::ModUiChanged);
                    meta.consume();
                }
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                let bounds = cx.bounds();
                let dpx = cx.scale_factor();
                let mx = cx.mouse().cursorx;
                if let Some(handle) = self.nearest_handle(bounds, dpx, mx) {
                    let def = if handle == 0 {
                        MB_XOVER_LOW_DEFAULT_NORM
                    } else {
                        MB_XOVER_HIGH_DEFAULT_NORM
                    };
                    self.host.begin_xover(handle);
                    self.host.set_xover(handle, def);
                    self.host.end_xover(handle);
                    cx.emit(UiKitEvent::HideTooltip);
                    cx.emit(UiKitEvent::ModValChanged);
                    meta.consume();
                }
            }
            WindowEvent::MouseMove(x, _) => {
                let bounds = cx.bounds();
                let dpx = cx.scale_factor();
                if let Some(handle) = self.dragging_handle {
                    let hz = self.x_to_hz(bounds, dpx, *x);
                    let norm = if handle == 0 {
                        mb_xover_low_norm(hz)
                    } else {
                        mb_xover_high_norm(hz)
                    };
                    self.host.set_xover(handle, norm);
                    set_resize_cursor(cx, ResizeCursorAxis::Horizontal);
                    self.emit_xover_tooltip(cx, bounds, dpx, handle);
                    cx.emit(UiKitEvent::ModValChanged);
                    meta.consume();
                } else if self.nearest_handle(bounds, dpx, *x).is_some() {
                    self.hover_cursor_active = true;
                    set_resize_cursor(cx, ResizeCursorAxis::Horizontal);
                } else if self.hover_cursor_active {
                    self.hover_cursor_active = false;
                    set_resize_cursor(cx, ResizeCursorAxis::Default);
                }
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if let Some(handle) = self.dragging_handle.take() {
                    self.host.end_xover(handle);
                    cx.release();
                    self.hover_cursor_active = false;
                    set_resize_cursor(cx, ResizeCursorAxis::Default);
                    cx.emit(UiKitEvent::HideTooltip);
                    meta.consume();
                }
            }
            WindowEvent::MouseOut => {
                if self.dragging_handle.is_none() && self.hover_cursor_active {
                    self.hover_cursor_active = false;
                    set_resize_cursor(cx, ResizeCursorAxis::Default);
                }
            }
            _ => {}
        });
    }
}
