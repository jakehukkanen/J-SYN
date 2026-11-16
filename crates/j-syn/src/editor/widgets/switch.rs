use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::util::ModifiersExt;
use nih_plug_vizia::widgets::RawParamEvent;

use crate::editor::controls::bold_label;

/// Skeuomorphic illuminated hardware push-button / LED switch.
pub struct LedToggleSwitch {
    param_base: ParamWidgetBase,
    #[allow(dead_code)]
    accent_rgb: (u8, u8, u8),
    hovered: bool,
}

impl LedToggleSwitch {
    pub fn new<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        off_label: &'static str,
        on_label: &'static str,
        #[allow(dead_code)]
    accent_rgb: (u8, u8, u8),
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        let text_params = params.clone();
        Self {
            param_base: ParamWidgetBase::new(cx, params.clone(), params_to_param),
            accent_rgb,
            hovered: false,
        }
        .build(cx, move |cx| {
            let text_lens = ParamWidgetBase::make_lens(
                text_params,
                params_to_param,
                move |param| {
                    if param.modulated_normalized_value() >= 0.5 {
                        on_label
                    } else {
                        off_label
                    }
                },
            );
            HStack::new(cx, move |cx| {
                Element::new(cx)
                    .width(Pixels(11.0))
                    .height(Stretch(1.0))
                    .hoverable(false);
                bold_label(cx, text_lens, "switch-label")
                    .width(Auto)
                    .height(Stretch(1.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0))
                    .hoverable(false);
            })
            .width(Stretch(1.0))
            .height(Stretch(1.0))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0))
            .child_left(Pixels(4.0))
            .child_right(Pixels(4.0))
            .col_between(Pixels(3.0))
            .hoverable(false);
        })
        .checked(ParamWidgetBase::make_lens(
            params,
            params_to_param,
            |param| param.modulated_normalized_value() >= 0.5,
        ))
    }

    fn toggle(&self, cx: &mut EventContext) {
        let current = self.param_base.unmodulated_normalized_value();
        let next = if current >= 0.5 { 0.0 } else { 1.0 };
        self.param_base.begin_set_parameter(cx);
        self.param_base.set_normalized_value(cx, next);
        self.param_base.end_set_parameter(cx);
        cx.needs_redraw();
    }
}

impl View for LedToggleSwitch {
    fn element(&self) -> Option<&'static str> {
        Some("led-toggle")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|raw_param_event, _| {
            if let RawParamEvent::ParametersChanged = raw_param_event {
                cx.set_checked(self.param_base.modulated_normalized_value() >= 0.5);
                cx.needs_redraw();
            }
        });

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
            | WindowEvent::MouseDoubleClick(MouseButton::Left)
            | WindowEvent::MouseTripleClick(MouseButton::Left) => {
                if cx.modifiers().command() || cx.modifiers().contains(Modifiers::CTRL) {
                    let def = self.param_base.default_normalized_value();
                    self.param_base.begin_set_parameter(cx);
                    self.param_base.set_normalized_value(cx, def);
                    self.param_base.end_set_parameter(cx);
                    cx.emit(crate::editor::UiEvent::ModUiChanged);
                    cx.needs_redraw();
                } else {
                    self.toggle(cx);
                    cx.emit(crate::editor::UiEvent::ModUiChanged);
                }
                meta.consume();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 0.0 || bounds.h <= 0.0 {
            return;
        }

        let dpx = cx.scale_factor();
        let pal = crate::editor::theme::current_palette(cx);
        let is_on = self.param_base.modulated_normalized_value() >= 0.5;
        let (ar, ag, ab) = pal.accent_rgb();

        let r = 3.5 * dpx;
        let mut bg = vg::Path::new();
        bg.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, r);

        let housing_paint = if is_on && self.hovered {
            vg::Paint::linear_gradient(
                bounds.x,
                bounds.y,
                bounds.x,
                bounds.y + bounds.h,
                crate::editor::theme::vg_color(&pal.border_light, 255),
                crate::editor::theme::vg_color(&pal.surface_hover, 255),
            )
        } else if is_on {
            vg::Paint::linear_gradient(
                bounds.x,
                bounds.y,
                bounds.x,
                bounds.y + bounds.h,
                crate::editor::theme::vg_color(&pal.surface_hover, 255),
                crate::editor::theme::vg_color(&pal.panel_header_bg, 255),
            )
        } else if self.hovered {
            vg::Paint::linear_gradient(
                bounds.x,
                bounds.y,
                bounds.x,
                bounds.y + bounds.h,
                crate::editor::theme::vg_color(&pal.panel_header_bg, 255),
                crate::editor::theme::vg_color(&pal.surface_mid, 255),
            )
        } else {
            vg::Paint::linear_gradient(
                bounds.x,
                bounds.y,
                bounds.x,
                bounds.y + bounds.h,
                crate::editor::theme::vg_color(&pal.surface_mid, 255),
                crate::editor::theme::vg_color(&pal.surface_dark, 255),
            )
        };
        canvas.fill_path(&bg, &housing_paint);

        let mut border = vg::Paint::color(if is_on {
            vg::Color::rgba(ar, ag, ab, if self.hovered { 235 } else { 175 })
        } else if self.hovered {
            vg::Color::rgba(ar, ag, ab, 120)
        } else {
            crate::editor::theme::vg_color(&pal.border_dark, 235)
        });
        border.set_line_width(1.0 * dpx);
        canvas.stroke_path(&bg, &border);

        let led_x = bounds.x + 10.0 * dpx;
        let led_y = bounds.y + bounds.h * 0.5;
        let led_r = 3.2 * dpx;

        if is_on {
            let mut glow = vg::Path::new();
            glow.circle(led_x, led_y, led_r * 2.0);
            let glow_paint = vg::Paint::radial_gradient(
                led_x,
                led_y,
                led_r * 0.2,
                led_r * 2.0,
                vg::Color::rgba(ar, ag, ab, 165),
                vg::Color::rgba(ar, ag, ab, 0),
            );
            canvas.fill_path(&glow, &glow_paint);
        }

        let mut lamp = vg::Path::new();
        lamp.circle(led_x, led_y, led_r);
        let lamp_paint = if is_on {
            vg::Paint::radial_gradient(
                led_x - led_r * 0.25,
                led_y - led_r * 0.25,
                led_r * 0.1,
                led_r,
                vg::Color::rgb(255, 250, 240),
                vg::Color::rgb(ar, ag, ab),
            )
        } else {
            vg::Paint::radial_gradient(
                led_x - led_r * 0.2,
                led_y - led_r * 0.2,
                led_r * 0.1,
                led_r,
                vg::Color::rgb(92, 80, 70),
                vg::Color::rgb(44, 38, 33),
            )
        };
        canvas.fill_path(&lamp, &lamp_paint);
    }
}

