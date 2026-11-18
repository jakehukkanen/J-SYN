use std::path::PathBuf;
use std::sync::Arc;

use j_mbdist::editor::{build_mbdist_ui, mbdist_window_size};
use j_mbdist::JMbDistPlugin;
use j_syn_dsp::mod_matrix::FX_SCOPE_LEN;
use j_syn_ui::preview::render_ui_to_png;
use j_syn_ui::theme;
use j_syn_ui::NoopGuiContext;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut out_path = PathBuf::from("target/previews/j-mbdist-100.png");
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

    let plugin = JMbDistPlugin::default();
    for i in 0..FX_SCOPE_LEN {
        let phase = (i as f32 / FX_SCOPE_LEN as f32) * std::f32::consts::TAU * 2.0;
        let env = (0.35 + 0.45 * phase.sin().abs()).clamp(0.0, 1.0);
        plugin.scope_ring.push(env * 0.85, env, 0.55);
    }

    let (w, h) = mbdist_window_size();
    let scale = (scale_pct as f64) / 100.0;
    let pal = theme::find_theme_by_name(&theme_name);
    let params = plugin.params.clone();
    let ring = plugin.scope_ring.clone();
    let gui_ctx = Arc::new(NoopGuiContext);

    render_ui_to_png(w, h, scale, &out_path, move |cx| {
        build_mbdist_ui(cx, params, gui_ctx, ring, pal, scale_pct, initial_tab, false);
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
