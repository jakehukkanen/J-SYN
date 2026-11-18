use std::path::PathBuf;
use std::sync::Arc;

use j_syn::editor::{build_j_syn_ui, BASE_HEIGHT, BASE_WIDTH};
use j_syn::JSynPluginParams;
use j_syn_dsp::mod_matrix::ModUiTelemetry;
use j_syn_dsp::WavetableBank;
use j_syn_ui::preview::render_ui_to_png;
use j_syn_ui::theme;
use j_syn_ui::NoopGuiContext;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut out_path = PathBuf::from("target/previews/j-syn-100.png");
    let mut theme_name = "Studio Amber".to_string();
    let mut scale_pct: u32 = 100;

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
            _ => {}
        }
    }

    let params = Arc::new(JSynPluginParams::default());
    let bank = Arc::new(WavetableBank::new());
    let telemetry = Arc::new(ModUiTelemetry::default());
    let pal = theme::find_theme_by_name(&theme_name);
    let scale = (scale_pct as f64) / 100.0;
    let gui_ctx = Arc::new(NoopGuiContext);

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
