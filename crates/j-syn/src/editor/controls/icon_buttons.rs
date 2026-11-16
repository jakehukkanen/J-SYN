//! Self-contained vector icon buttons and carets (`ArrowButton`, `CrossButton`, `VectorArrow`).

use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use super::super::theme;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArrowDir {
    Left,
    Right,
    Up,
    Down,
}

#[inline]
fn resolve_glyph_color(cx: &DrawContext) -> vg::Color {
    use nih_plug_vizia::vizia::vg;
    let fc = cx.font_color();
    if fc.a() == 0 {
        let pal = theme::current_palette(cx);
        theme::vg_color(&pal.text_primary, 255)
    } else {
        vg::Color::rgba(fc.r(), fc.g(), fc.b(), fc.a())
    }
}

fn paint_button_box(cx: &DrawContext, canvas: &mut Canvas) {
    use nih_plug_vizia::vizia::vg;

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
    use nih_plug_vizia::vizia::vg;

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
pub(crate) struct ArrowButton {
    action: Option<Box<dyn Fn(&mut EventContext)>>,
    dir_fn: Box<dyn Fn(&mut DrawContext) -> ArrowDir>,
}

impl ArrowButton {
    pub(crate) fn new<'a, A>(
        cx: &'a mut Context,
        action: A,
        dir: ArrowDir,
    ) -> Handle<'a, Self>
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

    pub(crate) fn reactive<'a, A, L, F>(
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
pub(crate) struct CrossButton {
    action: Option<Box<dyn Fn(&mut EventContext)>>,
}

impl CrossButton {
    pub(crate) fn new<'a, A>(cx: &'a mut Context, action: A) -> Handle<'a, Self>
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
        use nih_plug_vizia::vizia::vg;

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
pub(crate) struct VectorArrow {
    dir: ArrowDir,
}

impl VectorArrow {
    pub(crate) fn caret<'a>(
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
