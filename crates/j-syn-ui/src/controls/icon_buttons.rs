//! Self-contained vector icon buttons and carets (`ArrowButton`, `CrossButton`, `VectorArrow`, `CheckIcon`).

use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use crate::theme;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ArrowDir {
    Left,
    Right,
    Up,
    Down,
}

#[inline]
fn resolve_glyph_color(cx: &DrawContext) -> vg::Color {
    let fc = cx.font_color();
    if fc.a() == 0 {
        let pal = theme::current_palette(cx);
        theme::vg_color(&pal.text_primary, 255)
    } else {
        vg::Color::rgba(fc.r(), fc.g(), fc.b(), fc.a())
    }
}

fn paint_button_box(cx: &DrawContext, canvas: &mut Canvas) {
    let b = cx.bounds();
    let dpx = cx.scale_factor();
    let rad = cx.border_top_left_radius().max(0.0);
    let lw = cx.border_width().max(1.0 * dpx);

    let bg = cx.background_color();
    if bg.a() > 0 {
        let mut path = vg::Path::new();
        path.rounded_rect(b.x, b.y, b.w, b.h, rad);
        canvas.fill_path(
            &path,
            &vg::Paint::color(vg::Color::rgba(bg.r(), bg.g(), bg.b(), bg.a())),
        );
    }

    let bc = cx.border_color();
    if bc.a() > 0 && lw > 0.0 {
        let inset = lw * 0.5;
        let mut border = vg::Path::new();
        border.rounded_rect(
            b.x + inset,
            b.y + inset,
            (b.w - lw).max(0.0),
            (b.h - lw).max(0.0),
            (rad - inset).max(0.0),
        );
        let mut stroke = vg::Paint::color(vg::Color::rgba(bc.r(), bc.g(), bc.b(), bc.a()));
        stroke.set_line_width(lw);
        canvas.stroke_path(&border, &stroke);
    }
}

fn draw_triangle_glyph(
    cx: &DrawContext,
    canvas: &mut Canvas,
    dir: ArrowDir,
    fixed_size_px: Option<f32>,
) {
    let b = cx.bounds();
    if b.w <= 1.0 || b.h <= 1.0 {
        return;
    }

    let cx_pt = (b.x + b.w * 0.5).round();
    let cy_pt = (b.y + b.h * 0.5).round();
    let size = match fixed_size_px {
        Some(px) => px * cx.scale_factor(),
        None => b.w.min(b.h) * 0.34,
    };
    let half_span = size * 0.52;
    let half_base = size * 0.56;
    let nudge = size * 0.04;

    let mut tri = vg::Path::new();
    match dir {
        ArrowDir::Left => {
            tri.move_to(cx_pt - half_span - nudge, cy_pt);
            tri.line_to(cx_pt + half_span - nudge, cy_pt - half_base);
            tri.line_to(cx_pt + half_span - nudge, cy_pt + half_base);
        }
        ArrowDir::Right => {
            tri.move_to(cx_pt + half_span + nudge, cy_pt);
            tri.line_to(cx_pt - half_span + nudge, cy_pt - half_base);
            tri.line_to(cx_pt - half_span + nudge, cy_pt + half_base);
        }
        ArrowDir::Up => {
            tri.move_to(cx_pt, cy_pt - half_span - nudge);
            tri.line_to(cx_pt - half_base, cy_pt + half_span - nudge);
            tri.line_to(cx_pt + half_base, cy_pt + half_span - nudge);
        }
        ArrowDir::Down => {
            tri.move_to(cx_pt, cy_pt + half_span + nudge);
            tri.line_to(cx_pt - half_base, cy_pt - half_span + nudge);
            tri.line_to(cx_pt + half_base, cy_pt - half_span + nudge);
        }
    }
    tri.close();
    canvas.fill_path(&tri, &vg::Paint::color(resolve_glyph_color(cx)));
}

fn handle_icon_button_event(
    action: &Option<Box<dyn Fn(&mut EventContext)>>,
    cx: &mut EventContext,
    event: &mut Event,
) {
    event.map(|window_event, meta| match window_event {
        WindowEvent::PressDown { mouse } => {
            if *mouse {
                cx.capture();
            }
            cx.focus();
        }
        WindowEvent::Press { .. } => {
            if meta.target == cx.current() {
                if let Some(cb) = action {
                    (cb)(cx);
                }
            }
        }
        WindowEvent::MouseUp(button) if *button == MouseButton::Left => {
            cx.release();
        }
        WindowEvent::ActionRequest(req) => {
            if let Action::Default = req.action {
                if let Some(cb) = action {
                    (cb)(cx);
                }
            }
        }
        _ => {}
    });
}

/// Self-contained arrow button drawing both its rounded border/background and centered triangle
/// from the exact same `cx.bounds()` rect in one pass.
pub struct ArrowButton {
    action: Option<Box<dyn Fn(&mut EventContext)>>,
    dir_fn: Box<dyn Fn(&mut DrawContext) -> ArrowDir>,
}

impl ArrowButton {
    pub fn new<'a, A>(cx: &'a mut Context, action: A, dir: ArrowDir) -> Handle<'a, Self>
    where
        A: 'static + Fn(&mut EventContext),
    {
        Self {
            action: Some(Box::new(action)),
            dir_fn: Box::new(move |_| dir),
        }
        .build(cx, |_| {})
        .role(Role::Button)
        .default_action_verb(DefaultActionVerb::Click)
        .cursor(CursorIcon::Hand)
        .navigable(true)
    }

    pub fn reactive<'a, A, L, F>(
        cx: &'a mut Context,
        action: A,
        lens: L,
        dir_fn: F,
    ) -> Handle<'a, Self>
    where
        A: 'static + Fn(&mut EventContext),
        L: Lens,
        <L as Lens>::Target: Data,
        F: 'static + Fn(&mut DrawContext) -> ArrowDir,
    {
        Self {
            action: Some(Box::new(action)),
            dir_fn: Box::new(dir_fn),
        }
        .build(cx, |_| {})
        .role(Role::Button)
        .default_action_verb(DefaultActionVerb::Click)
        .cursor(CursorIcon::Hand)
        .navigable(true)
        .bind(lens, |mut handle, _| handle.needs_redraw())
    }
}

impl View for ArrowButton {
    fn element(&self) -> Option<&'static str> {
        Some("arrow-button")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        handle_icon_button_event(&self.action, cx, event);
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        if b.w <= 1.0 || b.h <= 1.0 {
            return;
        }
        paint_button_box(cx, canvas);
        let dir = (self.dir_fn)(cx);
        draw_triangle_glyph(cx, canvas, dir, None);
    }
}

/// Self-contained close (`×`) button drawing both its rounded box and centered cross in one pass.
pub struct CrossButton {
    action: Option<Box<dyn Fn(&mut EventContext)>>,
}

impl CrossButton {
    pub fn new<'a, A>(cx: &'a mut Context, action: A) -> Handle<'a, Self>
    where
        A: 'static + Fn(&mut EventContext),
    {
        Self {
            action: Some(Box::new(action)),
        }
        .build(cx, |_| {})
        .role(Role::Button)
        .default_action_verb(DefaultActionVerb::Click)
        .cursor(CursorIcon::Hand)
        .navigable(true)
    }
}

impl View for CrossButton {
    fn element(&self) -> Option<&'static str> {
        Some("cross-button")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        handle_icon_button_event(&self.action, cx, event);
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        if b.w <= 1.0 || b.h <= 1.0 {
            return;
        }
        paint_button_box(cx, canvas);

        let cx_pt = (b.x + b.w * 0.5).round();
        let cy_pt = (b.y + b.h * 0.5).round();
        let r = b.w.min(b.h) * 0.17;
        let mut cross = vg::Path::new();
        cross.move_to(cx_pt - r, cy_pt - r);
        cross.line_to(cx_pt + r, cy_pt + r);
        cross.move_to(cx_pt + r, cy_pt - r);
        cross.line_to(cx_pt - r, cy_pt + r);
        let mut paint = vg::Paint::color(resolve_glyph_color(cx));
        paint.set_line_width((1.5 * cx.scale_factor()).max(1.3));
        paint.set_line_cap(vg::LineCap::Round);
        canvas.stroke_path(&cross, &paint);
    }
}

/// Compact triangle caret view for dropdown triggers and folder disclosure rows.
pub struct VectorArrow {
    dir: ArrowDir,
}

impl VectorArrow {
    pub fn caret<'a>(
        cx: &'a mut Context,
        dir: ArrowDir,
        class_name: &str,
    ) -> Handle<'a, Self> {
        Self { dir }
            .build(cx, |_| {})
            .class(class_name)
            .hoverable(false)
    }
}

impl View for VectorArrow {
    fn element(&self) -> Option<&'static str> {
        Some("vector-arrow")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        draw_triangle_glyph(cx, canvas, self.dir, Some(4.5));
    }
}

/// Crisp centered vector checkmark used for active menu/mode toggles across all platforms.
pub struct CheckIcon;

impl CheckIcon {
    pub fn new<'a>(cx: &'a mut Context) -> Handle<'a, Self> {
        Self.build(cx, |_| {}).hoverable(false)
    }
}

impl View for CheckIcon {
    fn element(&self) -> Option<&'static str> {
        Some("check-icon")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        if b.w <= 1.0 || b.h <= 1.0 {
            return;
        }
        let s = b.w.min(b.h) * 0.34;
        let cx_pt = b.x + b.w * 0.5;
        let cy_pt = b.y + b.h * 0.52;
        let mut path = vg::Path::new();
        path.move_to(cx_pt - s * 0.95, cy_pt - s * 0.05);
        path.line_to(cx_pt - s * 0.25, cy_pt + s * 0.70);
        path.line_to(cx_pt + s * 0.95, cy_pt - s * 0.75);
        let mut paint = vg::Paint::color(resolve_glyph_color(cx));
        paint.set_line_width((1.6 * cx.scale_factor()).max(1.4));
        paint.set_line_cap(vg::LineCap::Round);
        paint.set_line_join(vg::LineJoin::Round);
        canvas.stroke_path(&path, &paint);
    }
}

/// Crisp centered 6-tooth vector cogwheel icon for settings buttons.
pub struct GearIcon;

impl GearIcon {
    pub fn new<'a>(cx: &'a mut Context) -> Handle<'a, Self> {
        Self.build(cx, |_| {}).hoverable(false)
    }
}

impl View for GearIcon {
    fn element(&self) -> Option<&'static str> {
        Some("gear-icon")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 2.0 || bounds.h <= 2.0 {
            return;
        }
        let pal = theme::current_palette(cx);
        let color = theme::vg_color(&pal.text_muted, 255);
        let cx_p = bounds.x + bounds.w * 0.5;
        let cy_p = bounds.y + bounds.h * 0.5;
        let r_outer = bounds.w.min(bounds.h) * 0.36;
        let r_inner = r_outer * 0.68;
        let r_hole = r_outer * 0.31;
        let teeth = 6usize;

        let mut gear = vg::Path::new();
        for i in 0..teeth {
            let base = (i as f32 / teeth as f32) * std::f32::consts::TAU;
            let step = std::f32::consts::TAU / (teeth as f32 * 4.0);
            let a0 = base - step * 1.15;
            let a1 = base - step * 0.60;
            let a2 = base + step * 0.60;
            let a3 = base + step * 1.15;
            let a4 = base + step * 2.00;

            let p0 = (cx_p + r_inner * a0.cos(), cy_p + r_inner * a0.sin());
            let p1 = (cx_p + r_outer * a1.cos(), cy_p + r_outer * a1.sin());
            let p2 = (cx_p + r_outer * a2.cos(), cy_p + r_outer * a2.sin());
            let p3 = (cx_p + r_inner * a3.cos(), cy_p + r_inner * a3.sin());
            let p4 = (cx_p + r_inner * a4.cos(), cy_p + r_inner * a4.sin());
            if i == 0 {
                gear.move_to(p0.0, p0.1);
            } else {
                gear.line_to(p0.0, p0.1);
            }
            gear.line_to(p1.0, p1.1);
            gear.line_to(p2.0, p2.1);
            gear.line_to(p3.0, p3.1);
            gear.line_to(p4.0, p4.1);
        }
        gear.close();
        gear.circle(cx_p, cy_p, r_hole);
        gear.solidity(vg::Solidity::Hole);

        canvas.fill_path(&gear, &vg::Paint::color(color));
    }
}
