use std::path::PathBuf;
use std::sync::Arc;

use j_syn::editor::{build_j_syn_ui, BASE_HEIGHT, BASE_WIDTH};
use j_syn::{FxTypeParam, JSynPluginParams};
use j_syn_dsp::mod_matrix::{ModUiTelemetry, FX_SCOPE_LEN};
use j_syn_dsp::{FxSlotParams, FxType, WavetableBank};
use j_syn_ui::preview::render_ui_to_png;
use j_syn_ui::theme;
use j_syn_ui::NoopGuiContext;
use nih_plug::prelude::{BoolParam, EnumParam, FloatParam, FloatRange};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut out_path = PathBuf::from("target/previews/j-syn-100.png");
    let mut theme_name = "Studio Amber".to_string();
    let mut scale_pct: u32 = 100;
    let mut demo_mbdist = false;
    let mut demo_batch1 = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
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
            "--demo-mbdist" => {
                demo_mbdist = true;
            }
            "--demo-batch1" => {
                demo_batch1 = true;
            }
            _ => {}
        }
    }

    let mut params = JSynPluginParams::default();
    let gui_ctx = Arc::new(NoopGuiContext);
    let bank = Arc::new(WavetableBank::new());
    let telemetry = Arc::new(ModUiTelemetry::default());
    if demo_mbdist {
        let defs = FxSlotParams::default_params_for_type(FxType::MultibandDist);
        let slot = &mut params.fx_slots[0];
        slot.enabled = BoolParam::new("Enabled", true);
        slot.fx_type = EnumParam::new("Type", FxTypeParam::MultibandDist);
        slot.p1 = FloatParam::new("Param 1", defs[0], FloatRange::Linear { min: 0.0, max: 1.0 });
        slot.p2 = FloatParam::new("Param 2", defs[1], FloatRange::Linear { min: 0.0, max: 1.0 });
        slot.p3 = FloatParam::new("Param 3", defs[2], FloatRange::Linear { min: 0.0, max: 1.0 });
        slot.p4 = FloatParam::new("Param 4", defs[3], FloatRange::Linear { min: 0.0, max: 1.0 });
        let ring = telemetry.fx_scope(0, 0);
        for i in 0..FX_SCOPE_LEN {
            let phase = (i as f32 / FX_SCOPE_LEN as f32) * std::f32::consts::TAU * 2.0;
            let env = (0.35 + 0.45 * phase.sin().abs()).clamp(0.0, 1.0);
            ring.push(env * 0.85, env, 0.55);
        }
    }
    if demo_batch1 {
        let bus_a = [
            (FxType::Utility, FxTypeParam::Utility),
            (FxType::JSatClipper, FxTypeParam::JSatClipper),
            (FxType::Ott, FxTypeParam::Ott),
        ];
        let bus_b = [
            (FxType::Phaser, FxTypeParam::Phaser),
            (FxType::Distortion, FxTypeParam::Distortion),
            (FxType::Diopser, FxTypeParam::Diopser),
        ];
        let bus_c = [
            (FxType::Chorus, FxTypeParam::Chorus),
            (FxType::Reverb, FxTypeParam::Reverb),
            (FxType::Flanger, FxTypeParam::Flanger),
        ];
        for (bus_idx, list) in [bus_a, bus_b, bus_c].iter().enumerate() {
            for (slot_idx, (ty, ty_p)) in list.iter().copied().enumerate() {
                let defs = FxSlotParams::default_params_for_type(ty);
                let eq_defs = FxSlotParams::default_eq_params_for_type(ty);
                let slot = match bus_idx {
                    0 => &mut params.fx_slots[slot_idx],
                    1 => &mut params.fx_bus_b.slots[slot_idx],
                    _ => &mut params.fx_bus_c.slots[slot_idx],
                };
                slot.enabled = BoolParam::new("Enabled", true);
                slot.fx_type = EnumParam::new("Type", ty_p);
                slot.p1 = FloatParam::new("Param 1", defs[0], FloatRange::Linear { min: 0.0, max: 1.0 });
                slot.p2 = FloatParam::new("Param 2", defs[1], FloatRange::Linear { min: 0.0, max: 1.0 });
                slot.p3 = FloatParam::new("Param 3", defs[2], FloatRange::Linear { min: 0.0, max: 1.0 });
                slot.p4 = FloatParam::new("Param 4", defs[3], FloatRange::Linear { min: 0.0, max: 1.0 });
                slot.p5 = FloatParam::new("Param 5", eq_defs[0], FloatRange::Linear { min: 0.0, max: 1.0 });
                slot.p6 = FloatParam::new("Param 6", eq_defs[1], FloatRange::Linear { min: 0.0, max: 1.0 });
                slot.p7 = FloatParam::new("Param 7", eq_defs[2], FloatRange::Linear { min: 0.0, max: 1.0 });
                slot.p8 = FloatParam::new("Param 8", eq_defs[3], FloatRange::Linear { min: 0.0, max: 1.0 });
                let ring = telemetry.fx_scope(bus_idx, slot_idx);
                for i in 0..FX_SCOPE_LEN {
                    let phase = (i as f32 / FX_SCOPE_LEN as f32) * std::f32::consts::TAU * 2.0;
                    let env = (0.35 + 0.45 * phase.sin().abs()).clamp(0.0, 1.0);
                    ring.push(env * 0.85, env, 0.55);
                }
            }
        }
    }
    let params = Arc::new(params);
    let pal = theme::find_theme_by_name(&theme_name);
    let scale = (scale_pct as f64) / 100.0;

    render_ui_to_png(BASE_WIDTH, BASE_HEIGHT, scale, &out_path, move |cx| {
        build_j_syn_ui(
            cx,
            params,
            gui_ctx,
            bank,
            telemetry,
            pal,
            scale_pct,
            false,
        );
    })?;

    println!(
        "Rendered {} ({}x{} @ {}%)",
        out_path.display(),
        BASE_WIDTH,
        BASE_HEIGHT,
        scale_pct
    );
    Ok(())
}
