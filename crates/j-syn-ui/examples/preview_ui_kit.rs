use std::path::PathBuf;
use std::sync::Arc;

use j_syn_dsp::mod_matrix::{FxScopeRing, FX_SCOPE_LEN};
use j_syn_dsp::{utility_width_from_norm, FxType, UTILITY_WIDTH_DEFAULT_NORM};
use j_syn_ui::controls::bold_label;
use j_syn_ui::fx_units::{build_utility_fx_unit_body, UtilityUnitParams};
use j_syn_ui::preview::render_ui_to_png;
use j_syn_ui::scopes::FxSlotScopeSnapshot;
use j_syn_ui::theme;
use j_syn_ui::widgets::{KnobDimExt, LedToggleSwitch, RotaryKnob};
use j_syn_ui::{tokens, NoopGuiContext, UiShellState};
use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;

#[derive(Params)]
struct DemoParams {
    #[id = "en"]
    enabled: BoolParam,
    #[id = "w"]
    width: FloatParam,
    #[id = "p"]
    pan: FloatParam,
    #[id = "g"]
    gain: FloatParam,
    #[id = "m"]
    mix: FloatParam,
    #[id = "il"]
    inv_l: BoolParam,
    #[id = "ir"]
    inv_r: BoolParam,
}

impl Default for DemoParams {
    fn default() -> Self {
        Self {
            enabled: BoolParam::new("On", true),
            width: FloatParam::new(
                "Width",
                UTILITY_WIDTH_DEFAULT_NORM,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(Arc::new(|v| format!("{:.0}%", utility_width_from_norm(v) * 100.0))),
            pan: FloatParam::new("Pan", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            gain: FloatParam::new("Gain", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
            mix: FloatParam::new("Mix", 1.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            inv_l: BoolParam::new("Inv L", false),
            inv_r: BoolParam::new("Inv R", false),
        }
    }
}

#[derive(Lens)]
struct DemoData {
    params: Arc<DemoParams>,
}

impl Model for DemoData {}

#[derive(Clone, Copy)]
struct DemoUtilityParams;
impl UtilityUnitParams<DemoParams> for DemoUtilityParams {
    fn enabled<'a>(&self, p: &'a DemoParams) -> &'a BoolParam {
        &p.enabled
    }
    fn width<'a>(&self, p: &'a DemoParams) -> &'a FloatParam {
        &p.width
    }
    fn pan<'a>(&self, p: &'a DemoParams) -> &'a FloatParam {
        &p.pan
    }
    fn gain<'a>(&self, p: &'a DemoParams) -> &'a FloatParam {
        &p.gain
    }
    fn mix<'a>(&self, p: &'a DemoParams) -> &'a FloatParam {
        &p.mix
    }
    fn inv_l<'a>(&self, p: &'a DemoParams) -> &'a BoolParam {
        &p.inv_l
    }
    fn inv_r<'a>(&self, p: &'a DemoParams) -> &'a BoolParam {
        &p.inv_r
    }
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut element = "fx-unit-utility".to_string();
    let mut out_path = PathBuf::from("target/previews/ui-element.png");
    let mut theme_name = "Studio Amber".to_string();
    let mut scale_pct: u32 = 100;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--element" => {
                if let Some(el) = args.next() {
                    element = el;
                }
            }
            "--out" | "--render-preview" => {
                if let Some(p) = args.next() {
                    out_path = PathBuf::from(p);
                }
            }
            "--theme" => {
                if let Some(t) = args.next() {
                    theme_name = t;
                }
            }
            "--scale" => {
                if let Some(s) = args.next() {
                    scale_pct = s.parse().unwrap_or(100);
                }
            }
            _ => {}
        }
    }

    let pal = theme::find_theme_by_name(&theme_name);
    let scale = (scale_pct as f64) / 100.0;
    let params = Arc::new(DemoParams::default());
    let ring = Arc::new(FxScopeRing::default());
    for i in 0..FX_SCOPE_LEN {
        let phase = (i as f32 / FX_SCOPE_LEN as f32) * std::f32::consts::TAU * 2.0;
        let env = (0.35 + 0.45 * phase.sin().abs()).clamp(0.0, 1.0);
        ring.push(env * 0.85, env, 0.55);
    }

    let (w, h) = match element.as_str() {
        "knob-states" => (220, 96),
        _ => (244, 140),
    };

    render_ui_to_png(w, h, scale, &out_path, move |cx| {
        UiShellState::new(pal, scale_pct).install(cx);
        DemoData {
            params: params.clone(),
        }
        .build(cx);

        VStack::new(cx, move |cx| match element.as_str() {
            "knob-states" => {
                HStack::new(cx, move |cx| {
                    VStack::new(cx, |cx| {
                        bold_label(cx, "UNIPOLAR", "knob-title")
                            .height(Pixels(14.0))
                            .width(Stretch(1.0))
                            .child_space(Stretch(1.0));
                        RotaryKnob::new(cx, DemoData::params, |p| &p.mix, false, (245, 179, 73))
                            .width(Pixels(tokens::KNOB_OSC))
                            .height(Pixels(tokens::KNOB_OSC))
                            .left(Stretch(1.0))
                            .right(Stretch(1.0));
                    })
                    .width(Stretch(1.0))
                    .height(Auto);

                    VStack::new(cx, |cx| {
                        bold_label(cx, "BIPOLAR", "knob-title")
                            .height(Pixels(14.0))
                            .width(Stretch(1.0))
                            .child_space(Stretch(1.0));
                        RotaryKnob::new(cx, DemoData::params, |p| &p.pan, true, (245, 179, 73))
                            .width(Pixels(tokens::KNOB_OSC))
                            .height(Pixels(tokens::KNOB_OSC))
                            .left(Stretch(1.0))
                            .right(Stretch(1.0));
                    })
                    .width(Stretch(1.0))
                    .height(Auto);

                    let p_dim = params.clone();
                    VStack::new(cx, move |cx| {
                        bold_label(cx, "DIMMED", "knob-title")
                            .height(Pixels(14.0))
                            .width(Stretch(1.0))
                            .child_space(Stretch(1.0));
                        RotaryKnob::new(cx, DemoData::params, |p| &p.width, false, (245, 179, 73))
                            .enabled_by(p_dim, |_| false)
                            .width(Pixels(tokens::KNOB_OSC))
                            .height(Pixels(tokens::KNOB_OSC))
                            .left(Stretch(1.0))
                            .right(Stretch(1.0));
                    })
                    .width(Stretch(1.0))
                    .height(Auto);
                })
                .col_between(Pixels(8.0))
                .child_space(Pixels(12.0))
                .width(Stretch(1.0))
                .height(Stretch(1.0));
            }
            _ => {
                let p_card = params.clone();
                let p_scope = params.clone();
                let g_card = Arc::new(NoopGuiContext);
                let r_scope = ring.clone();
                VStack::new(cx, move |cx| {
                    HStack::new(cx, |cx| {
                        LedToggleSwitch::new(
                            cx,
                            DemoData::params,
                            |p| &p.enabled,
                            "",
                            "",
                            (108, 214, 255),
                        )
                        .width(Pixels(tokens::FX_UNIT_LED_W))
                        .height(Pixels(tokens::FX_UNIT_ARROW_H));
                        bold_label(cx, "Utility", "module-title")
                            .width(Stretch(1.0))
                            .height(Stretch(1.0))
                            .child_top(Stretch(1.0))
                            .child_bottom(Stretch(1.0));
                    })
                    .col_between(Pixels(tokens::SPACE_SM))
                    .height(Pixels(tokens::FX_UNIT_HEADER_H))
                    .width(Stretch(1.0));

                    build_utility_fx_unit_body(
                        cx,
                        DemoData::params,
                        p_card,
                        g_card,
                        DemoUtilityParams,
                        None,
                        move || {
                            let mut pre = [0.0_f32; FX_SCOPE_LEN];
                            let mut post = [0.0_f32; FX_SCOPE_LEN];
                            let act = r_scope.read_snapshot(&mut pre, &mut post);
                            FxSlotScopeSnapshot {
                                enabled: p_scope.enabled.value(),
                                fx_type: FxType::Utility,
                                eff_p: [
                                    p_scope.width.value(),
                                    p_scope.pan.value(),
                                    p_scope.gain.value(),
                                    p_scope.mix.value(),
                                ],
                                utility_inv_l: p_scope.inv_l.value(),
                                utility_inv_r: p_scope.inv_r.value(),
                                pre,
                                post,
                                activity: act,
                                ..FxSlotScopeSnapshot::default()
                            }
                        },
                        tokens::KNOB_FX_UNIT_2X2,
                    );
                })
                .class("hardware-module")
                .class("fx-full-card")
                .width(Stretch(1.0))
                .height(Auto)
                .row_between(Pixels(tokens::SPACE_SM));
            }
        })
        .class("plugin-root")
        .child_space(Pixels(10.0))
        .width(Stretch(1.0))
        .height(Stretch(1.0));
    })?;

    println!("Rendered {} ({}x{} @ {}%)", out_path.display(), w, h, scale_pct);
    Ok(())
}
