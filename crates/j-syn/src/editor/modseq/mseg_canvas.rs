//! Adaptive screen-space rendering, hit-testing, and breakpoint editing helpers for `MSEG` / `LFO` mode.

use j_syn_dsp::{
    MAX_MSEG_NODES, MSEG_WALL_EPS, MsegCurve, MsegNode, evaluate_mseg_curve, evaluate_mseg_segment,
    mseg_tension_for_midpoint,
};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use super::super::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MsegHover {
    Node(usize),
    Tension(usize),
}

#[inline]
pub(super) fn mseg_segment_has_handle(a: MsegNode, b: MsegNode) -> bool {
    (b.time_x - a.time_x) > 1e-4 && (b.level_y - a.level_y).abs() >= 1e-3
}

#[allow(clippy::too_many_arguments)]
fn subdivide_mseg_segment(
    a: MsegNode,
    b: MsegNode,
    t0: f32,
    y0_val: f32,
    t1: f32,
    y1_val: f32,
    x0: f32,
    y0: f32,
    w: f32,
    h: f32,
    tol_px: f32,
    depth: u8,
    emit: &mut impl FnMut(f32, f32),
) {
    if depth >= 8 {
        let bx = x0 + (a.time_x + (b.time_x - a.time_x) * t1) * w;
        let by = y0 + (1.0 - y1_val) * h;
        emit(bx, by);
        return;
    }

    let tm = 0.5 * (t0 + t1);
    let ym_val = evaluate_mseg_segment(a, b, tm);

    let ax = x0 + (a.time_x + (b.time_x - a.time_x) * t0) * w;
    let ay = y0 + (1.0 - y0_val) * h;
    let bx = x0 + (a.time_x + (b.time_x - a.time_x) * t1) * w;
    let by = y0 + (1.0 - y1_val) * h;
    let mx = x0 + (a.time_x + (b.time_x - a.time_x) * tm) * w;
    let my = y0 + (1.0 - ym_val) * h;

    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    let dist = if len_sq <= 1e-6 {
        (mx - ax).hypot(my - ay)
    } else {
        ((dy * mx - dx * my + bx * ay - by * ax).abs()) / len_sq.sqrt()
    };

    if dist <= tol_px && depth >= 1 {
        emit(bx, by);
    } else {
        subdivide_mseg_segment(
            a,
            b,
            t0,
            y0_val,
            tm,
            ym_val,
            x0,
            y0,
            w,
            h,
            tol_px,
            depth + 1,
            emit,
        );
        subdivide_mseg_segment(
            a,
            b,
            tm,
            ym_val,
            t1,
            y1_val,
            x0,
            y0,
            w,
            h,
            tol_px,
            depth + 1,
            emit,
        );
    }
}

/// Emits adaptive screen-space polyline vertices `(px, py)` covering every segment of `mseg`.
pub(super) fn emit_mseg_curve_points(
    mseg: &MsegCurve,
    x0: f32,
    y0: f32,
    w: f32,
    h: f32,
    tol_px: f32,
    mut emit: impl FnMut(f32, f32),
) {
    let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
    let first = mseg.nodes[0];
    emit(
        x0 + first.time_x * w,
        y0 + (1.0 - first.level_y.clamp(0.0, 1.0)) * h,
    );

    for i in 0..(n - 1) {
        let a = mseg.nodes[i];
        let b = mseg.nodes[i + 1];
        let ay = a.level_y.clamp(0.0, 1.0);
        let by = b.level_y.clamp(0.0, 1.0);
        let bx = x0 + b.time_x * w;
        let py_b = y0 + (1.0 - by) * h;

        if (b.time_x - a.time_x) <= MSEG_WALL_EPS
            || (by - ay).abs() < 1e-5
            || a.tension.abs() < 1e-4
        {
            emit(bx, py_b);
        } else {
            subdivide_mseg_segment(a, b, 0.0, ay, 1.0, by, x0, y0, w, h, tol_px, 0, &mut emit);
        }
    }
}

pub(super) fn hit_mseg_node(
    mseg: &MsegCurve,
    rect: (f32, f32, f32, f32),
    x: f32,
    y: f32,
    scale: f32,
) -> Option<usize> {
    let (x0, y0, w, h) = rect;
    let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
    let s = scale.max(1.0);
    let radius = 9.5 * s;
    let tie_eps = 0.5 * s;
    let mut best: Option<(usize, f32)> = None;
    for (i, node) in mseg.nodes.iter().enumerate().take(n) {
        let px = x0 + node.time_x * w;
        let py = y0 + (1.0 - node.level_y) * h;
        let d = (x - px).hypot(y - py);
        if d <= radius {
            let replace = match best {
                None => true,
                Some((bi, bd)) => {
                    let i_interior = i > 0 && i + 1 < n;
                    let b_interior = bi > 0 && bi + 1 < n;
                    d + tie_eps < bd || ((d - bd).abs() <= tie_eps && i_interior && !b_interior)
                }
            };
            if replace {
                best = Some((i, d));
            }
        }
    }
    best.map(|(idx, _)| idx)
}

pub(super) fn hit_mseg_tension_handle(
    mseg: &MsegCurve,
    rect: (f32, f32, f32, f32),
    x: f32,
    y: f32,
    scale: f32,
) -> Option<usize> {
    let (x0, y0, w, h) = rect;
    let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
    let radius = 8.5 * scale.max(1.0);
    let mut best: Option<(usize, f32)> = None;
    for i in 0..(n - 1) {
        let a = mseg.nodes[i];
        let b = mseg.nodes[i + 1];
        if !mseg_segment_has_handle(a, b) {
            continue;
        }
        let mid_x = 0.5 * (a.time_x + b.time_x);
        let mid_y = evaluate_mseg_segment(a, b, 0.5);
        let px = x0 + mid_x * w;
        let py = y0 + (1.0 - mid_y) * h;
        let d = (x - px).hypot(y - py);
        if d <= radius && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((i, d));
        }
    }
    best.map(|(idx, _)| idx)
}

pub(super) fn segment_index_for_x(mseg: &MsegCurve, nx: f32) -> Option<usize> {
    let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
    let mut fallback: Option<usize> = None;
    for i in 0..(n - 1) {
        let a = mseg.nodes[i];
        let b = mseg.nodes[i + 1];
        if (b.time_x - a.time_x) <= MSEG_WALL_EPS {
            continue;
        }
        fallback = Some(i);
        if nx <= b.time_x {
            return Some(i);
        }
    }
    fallback
}

pub(super) fn apply_mseg_node_drag(
    mseg: &mut MsegCurve,
    node_idx: usize,
    mut nx: f32,
    mut ny: f32,
    snap_active: bool,
) {
    let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
    let idx = node_idx.min(n - 1);
    if snap_active {
        ny = (ny * 8.0).round() / 8.0;
        if idx > 0 && idx + 1 < n {
            nx = (nx * 16.0).round() / 16.0;
        }
    }
    mseg.nodes[idx].level_y = ny.clamp(0.0, 1.0);
    if idx == 0 {
        mseg.nodes[idx].time_x = 0.0;
    } else if idx + 1 == n {
        mseg.nodes[idx].time_x = 1.0;
    } else {
        let min_x = mseg.nodes[idx - 1].time_x;
        let max_x = mseg.nodes[idx + 1].time_x;
        mseg.nodes[idx].time_x = nx.clamp(min_x, max_x);
    }
}

pub(super) fn apply_mseg_tension_direct(mseg: &mut MsegCurve, seg_idx: usize, ny: f32) {
    let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
    if seg_idx + 1 >= n {
        return;
    }
    let ay = mseg.nodes[seg_idx].level_y;
    let by = mseg.nodes[seg_idx + 1].level_y;
    let dy = by - ay;
    if dy.abs() < 1e-3 {
        return;
    }
    let m = ((ny - ay) / dy).clamp(1e-4, 1.0 - 1e-4);
    mseg.nodes[seg_idx].tension = mseg_tension_for_midpoint(m);
}

pub(super) fn apply_mseg_tension_relative(
    mseg: &mut MsegCurve,
    seg_idx: usize,
    start_y: f32,
    y: f32,
    scale: f32,
    start_tension: f32,
) {
    let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
    if seg_idx + 1 >= n {
        return;
    }
    let sign = if mseg.nodes[seg_idx + 1].level_y >= mseg.nodes[seg_idx].level_y {
        -1.0
    } else {
        1.0
    };
    let dy = sign * (start_y - y) / (75.0 * scale.max(1.0));
    mseg.nodes[seg_idx].tension = (start_tension + dy).clamp(-1.0, 1.0);
}

pub(super) fn insert_mseg_node(
    mseg: &mut MsegCurve,
    raw_x: f32,
    raw_y: f32,
    snap_active: bool,
) -> bool {
    let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
    if n >= MAX_MSEG_NODES {
        return false;
    }
    let nx = if snap_active {
        (raw_x * 16.0).round() / 16.0
    } else {
        raw_x
    };
    let ny = if snap_active {
        (raw_y * 8.0).round() / 8.0
    } else {
        raw_y
    };
    let tx = nx.clamp(0.0, 1.0);
    let mut insert_at = n - 1;
    for (i, node) in mseg.nodes.iter().enumerate().take(n).skip(1) {
        if tx < node.time_x {
            insert_at = i;
            break;
        }
    }
    let left_x = mseg.nodes[insert_at - 1].time_x;
    let right_x = mseg.nodes[insert_at].time_x;
    for i in (insert_at..n).rev() {
        mseg.nodes[i + 1] = mseg.nodes[i];
    }
    mseg.nodes[insert_at] = MsegNode {
        time_x: tx.clamp(left_x, right_x),
        level_y: ny.clamp(0.0, 1.0),
        tension: 0.0,
    };
    mseg.num_nodes = n + 1;
    true
}

pub(super) fn delete_mseg_node(mseg: &mut MsegCurve, idx: usize) -> bool {
    let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
    if idx == 0 || idx + 1 >= n || n <= 2 {
        return false;
    }
    for i in idx..(n - 1) {
        mseg.nodes[i] = mseg.nodes[i + 1];
    }
    mseg.num_nodes = n - 1;
    true
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_mseg(
    canvas: &mut Canvas,
    rect: (f32, f32, f32, f32),
    dpx: f32,
    pal: &j_syn_ui::ThemePalette,
    accent: (u8, u8, u8),
    mseg: &MsegCurve,
    snap_on: bool,
    phase: f32,
    active_hover: Option<MsegHover>,
) {
    let (x0, y0, w, h) = rect;
    let (ar, ag, ab) = accent;

    let mut grid_paint = vg::Paint::color(theme::vg_color(&pal.scope_grid, 175));
    grid_paint.set_line_width(0.8 * dpx);
    let mut snap_grid_paint = vg::Paint::color(theme::vg_color(&pal.scope_grid, 90));
    snap_grid_paint.set_line_width(0.65 * dpx);

    let h_divs = if snap_on { 8 } else { 4 };
    for g in 1..h_divs {
        let gy = y0 + (g as f32 / h_divs as f32) * h;
        let mut line = vg::Path::new();
        line.move_to(x0, gy);
        line.line_to(x0 + w, gy);
        let paint = if snap_on && g % 2 != 0 {
            &snap_grid_paint
        } else {
            &grid_paint
        };
        canvas.stroke_path(&line, paint);
    }
    let v_divs = if snap_on { 16 } else { 4 };
    for s in 1..v_divs {
        let gx = x0 + (s as f32 / v_divs as f32) * w;
        let mut line = vg::Path::new();
        line.move_to(gx, y0);
        line.line_to(gx, y0 + h);
        let paint = if snap_on && s % 4 != 0 {
            &snap_grid_paint
        } else {
            &grid_paint
        };
        canvas.stroke_path(&line, paint);
    }

    let mut fill_path = vg::Path::new();
    let mut stroke_path = vg::Path::new();
    fill_path.move_to(x0, y0 + h);
    let mut first = true;
    emit_mseg_curve_points(mseg, x0, y0, w, h, 0.35 * dpx, |px, py| {
        fill_path.line_to(px, py);
        if first {
            stroke_path.move_to(px, py);
            first = false;
        } else {
            stroke_path.line_to(px, py);
        }
    });
    fill_path.line_to(x0 + w, y0 + h);
    fill_path.close();

    let fill_paint = vg::Paint::linear_gradient(
        x0,
        y0,
        x0,
        y0 + h,
        vg::Color::rgba(ar, ag, ab, 95),
        vg::Color::rgba(ar, ag, ab, 14),
    );
    canvas.fill_path(&fill_path, &fill_paint);

    let mut stroke_paint = vg::Paint::color(vg::Color::rgb(ar, ag, ab));
    stroke_paint.set_line_width(1.8 * dpx);
    stroke_paint.set_line_cap(vg::LineCap::Round);
    stroke_paint.set_line_join(vg::LineJoin::Round);
    canvas.stroke_path(&stroke_path, &stroke_paint);

    let n = mseg.num_nodes.clamp(2, MAX_MSEG_NODES);
    for i in 0..(n - 1) {
        let a = mseg.nodes[i];
        let b = mseg.nodes[i + 1];
        if !mseg_segment_has_handle(a, b) {
            continue;
        }
        let mid_x = 0.5 * (a.time_x + b.time_x);
        let mid_y = evaluate_mseg_segment(a, b, 0.5);
        let mx = x0 + mid_x * w;
        let my = y0 + (1.0 - mid_y) * h;
        let is_hot = active_hover == Some(MsegHover::Tension(i));
        if is_hot {
            let mut halo = vg::Path::new();
            halo.circle(mx, my, 6.2 * dpx);
            canvas.fill_path(
                &halo,
                &vg::Paint::color(theme::vg_color(&pal.accent_primary, 75)),
            );
        }
        let mut hdl = vg::Path::new();
        hdl.circle(mx, my, (if is_hot { 3.8 } else { 2.6 }) * dpx);
        canvas.fill_path(
            &hdl,
            &vg::Paint::color(theme::vg_color(
                if is_hot {
                    &pal.text_primary
                } else {
                    &pal.accent_primary
                },
                235,
            )),
        );
        let mut hdl_ring = vg::Paint::color(theme::vg_color(&pal.scope_bg, 220));
        hdl_ring.set_line_width(1.0 * dpx);
        canvas.stroke_path(&hdl, &hdl_ring);
    }

    for (i, node) in mseg.nodes.iter().enumerate().take(n) {
        let nx = x0 + node.time_x * w;
        let ny = y0 + (1.0 - node.level_y) * h;
        let is_hot = active_hover == Some(MsegHover::Node(i));
        if is_hot {
            let mut halo = vg::Path::new();
            halo.circle(nx, ny, 7.0 * dpx);
            canvas.fill_path(&halo, &vg::Paint::color(vg::Color::rgba(ar, ag, ab, 75)));
        }
        let mut ring = vg::Path::new();
        ring.circle(nx, ny, (if is_hot { 4.8 } else { 3.6 }) * dpx);
        canvas.fill_path(&ring, &vg::Paint::color(vg::Color::rgb(ar, ag, ab)));
        let mut inner = vg::Path::new();
        inner.circle(nx, ny, (if is_hot { 2.6 } else { 2.0 }) * dpx);
        canvas.fill_path(
            &inner,
            &vg::Paint::color(theme::vg_color(&pal.text_primary, 255)),
        );
    }

    let ph = phase.clamp(0.0, 1.0);
    let ph_x = x0 + ph * w;
    let mut ph_line = vg::Path::new();
    ph_line.move_to(ph_x, y0);
    ph_line.line_to(ph_x, y0 + h);
    let mut ph_paint = vg::Paint::color(theme::vg_color(&pal.text_primary, 210));
    ph_paint.set_line_width(1.4 * dpx);
    canvas.stroke_path(&ph_line, &ph_paint);

    let cur_val = evaluate_mseg_curve(mseg, ph.min(0.999_999));
    let mut ph_dot = vg::Path::new();
    ph_dot.circle(ph_x, y0 + (1.0 - cur_val) * h, 3.0 * dpx);
    canvas.fill_path(&ph_dot, &vg::Paint::color(vg::Color::rgb(ar, ag, ab)));
    let mut ph_ring = vg::Paint::color(theme::vg_color(&pal.text_primary, 255));
    ph_ring.set_line_width(1.1 * dpx);
    canvas.stroke_path(&ph_dot, &ph_ring);
}
