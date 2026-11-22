use std::path::PathBuf;
use std::sync::Arc;

use j_delay::editor::{build_delay_ui, delay_window_size};
use j_delay::JDelayPlugin;
use j_syn_dsp::mod_matrix::FX_SCOPE_LEN;
use j_syn_ui::preview::render_ui_to_png;
use j_syn_ui::theme;
use j_syn_ui::NoopGuiContext;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut out_path = PathBuf::from("target/previews/j-delay-main-100.png");
    let mut theme_name = "Studio Amber".to_string();
    let mut scale_pct: u32 = 100;
    let mut initial_tab: u8 = 0;

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
            "--tab" => {
                if let Some(t) = args.next() {
                    initial_tab = t.parse().unwrap_or(0);
                }
            }
            _ => {}
        }
    }

    let plugin = JDelayPlugin::default();
    for i in 0..FX_SCOPE_LEN {
        let phase = (i as f32 / FX_SCOPE_LEN as f32) * std::f32::consts::TAU * 3.0;
        let tap = (0.7 * (-0.04 * i as f32).exp() * phase.sin().abs()).clamp(0.0, 1.0);
        plugin.scope_ring.push(tap * 0.85, tap, 0.50);
    }

    let (w, h) = delay_window_size();
    let scale = (scale_pct as f64) / 100.0;
    let pal = theme::find_theme_by_name(&theme_name);
    let params = plugin.params.clone();
    let ring = plugin.scope_ring.clone();
    let gui_ctx = Arc::new(NoopGuiContext);

    render_ui_to_png(w, h, scale, &out_path, move |cx| {
        build_delay_ui(cx, params, gui_ctx, ring, pal, scale_pct, initial_tab, false);
    })?;

    println!(
        "Rendered {} ({}x{} @ {}%, tab={})",
        out_path.display(),
        w,
        h,
        scale_pct,
        initial_tab
    );
    Ok(())
}
