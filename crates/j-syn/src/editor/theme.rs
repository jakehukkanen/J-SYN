//! Multi-theme palette definitions, custom `<config_dir>/themes/*.json` loading,
//! and dynamic CSS + `femtovg` color resolution for `J-SYN`.

use std::sync::{Arc, RwLock};

use j_syn_dsp::mod_matrix::ModMatrixSource;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use serde::{Deserialize, Serialize};

use crate::presets::JSynConfig;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemePalette {
    pub name: String,
    pub window_bg: String,
    pub header_bg: String,
    pub panel_bg: String,
    pub panel_header_bg: String,
    pub surface_dark: String,
    pub surface_mid: String,
    pub surface_hover: String,
    pub surface_active: String,
    pub border_dark: String,
    pub border_mid: String,
    pub border_light: String,
    pub text_primary: String,
    pub text_bright: String,
    pub text_muted: String,
    pub text_dim: String,
    pub accent_primary: String,
    pub accent_secondary: String,
    pub accent_gold: String,
    pub mod_env: String,
    pub mod_seq: String,
    pub mod_macro: String,
    pub mod_midi: String,
    pub scope_bg: String,
    pub scope_grid: String,
    pub scope_trace: String,
    pub knob_body: String,
    pub knob_track: String,
}

impl Default for ThemePalette {
    fn default() -> Self {
        Self::studio_amber()
    }
}

impl ThemePalette {
    pub fn studio_amber() -> Self {
        Self {
            name: "Studio Amber".to_string(),
            window_bg: "#28231e".to_string(),
            header_bg: "#362f29".to_string(),
            panel_bg: "#322c26".to_string(),
            panel_header_bg: "#3b332b".to_string(),
            surface_dark: "#211c17".to_string(),
            surface_mid: "#2c241e".to_string(),
            surface_hover: "#4d3e2f".to_string(),
            surface_active: "#594631".to_string(),
            border_dark: "#1c1814".to_string(),
            border_mid: "#463b32".to_string(),
            border_light: "#5e4f42".to_string(),
            text_primary: "#f0e4d6".to_string(),
            text_bright: "#fff8ee".to_string(),
            text_muted: "#cbb9a6".to_string(),
            text_dim: "#84786c".to_string(),
            accent_primary: "#f5b349".to_string(),
            accent_secondary: "#eb8c55".to_string(),
            accent_gold: "#ffd382".to_string(),
            mod_env: "#00e676".to_string(),
            mod_seq: "#00e5ff".to_string(),
            mod_macro: "#f5b349".to_string(),
            mod_midi: "#ff6b6b".to_string(),
            scope_bg: "#161310".to_string(),
            scope_grid: "#2e2720".to_string(),
            scope_trace: "#f5b349".to_string(),
            knob_body: "#241f1a".to_string(),
            knob_track: "#181411".to_string(),
        }
    }

    pub fn arctic_slate() -> Self {
        Self {
            name: "Arctic Slate".to_string(),
            window_bg: "#171c22".to_string(),
            header_bg: "#202730".to_string(),
            panel_bg: "#1e242d".to_string(),
            panel_header_bg: "#262e39".to_string(),
            surface_dark: "#12161b".to_string(),
            surface_mid: "#1b2129".to_string(),
            surface_hover: "#2a3848".to_string(),
            surface_active: "#1e4554".to_string(),
            border_dark: "#0e1115".to_string(),
            border_mid: "#2f3a47".to_string(),
            border_light: "#415061".to_string(),
            text_primary: "#e2ecf4".to_string(),
            text_bright: "#f5fbff".to_string(),
            text_muted: "#9bb0c4".to_string(),
            text_dim: "#627486".to_string(),
            accent_primary: "#22d3ee".to_string(),
            accent_secondary: "#38bdf8".to_string(),
            accent_gold: "#67e8f9".to_string(),
            mod_env: "#34d399".to_string(),
            mod_seq: "#22d3ee".to_string(),
            mod_macro: "#fbbf24".to_string(),
            mod_midi: "#fb7185".to_string(),
            scope_bg: "#0e1217".to_string(),
            scope_grid: "#1f2a36".to_string(),
            scope_trace: "#22d3ee".to_string(),
            knob_body: "#161b22".to_string(),
            knob_track: "#0d1116".to_string(),
        }
    }

    pub fn midnight_violet() -> Self {
        Self {
            name: "Midnight Violet".to_string(),
            window_bg: "#151521".to_string(),
            header_bg: "#1e1e30".to_string(),
            panel_bg: "#1c1c2c".to_string(),
            panel_header_bg: "#25253a".to_string(),
            surface_dark: "#10101a".to_string(),
            surface_mid: "#181828".to_string(),
            surface_hover: "#2f2a4a".to_string(),
            surface_active: "#412d66".to_string(),
            border_dark: "#0b0b12".to_string(),
            border_mid: "#2e2e47".to_string(),
            border_light: "#444266".to_string(),
            text_primary: "#ebe6f7".to_string(),
            text_bright: "#faf8ff".to_string(),
            text_muted: "#a9a3c4".to_string(),
            text_dim: "#6b6685".to_string(),
            accent_primary: "#a855f7".to_string(),
            accent_secondary: "#2dd4bf".to_string(),
            accent_gold: "#c084fc".to_string(),
            mod_env: "#2dd4bf".to_string(),
            mod_seq: "#38bdf8".to_string(),
            mod_macro: "#c084fc".to_string(),
            mod_midi: "#f472b6".to_string(),
            scope_bg: "#0c0c14".to_string(),
            scope_grid: "#222238".to_string(),
            scope_trace: "#a855f7".to_string(),
            knob_body: "#141420".to_string(),
            knob_track: "#0a0a12".to_string(),
        }
    }

    pub fn phosphor() -> Self {
        Self {
            name: "Phosphor".to_string(),
            window_bg: "#111613".to_string(),
            header_bg: "#18201b".to_string(),
            panel_bg: "#161d19".to_string(),
            panel_header_bg: "#1e2822".to_string(),
            surface_dark: "#0c100e".to_string(),
            surface_mid: "#131a16".to_string(),
            surface_hover: "#22362a".to_string(),
            surface_active: "#1d4a31".to_string(),
            border_dark: "#080b09".to_string(),
            border_mid: "#25332b".to_string(),
            border_light: "#364a3e".to_string(),
            text_primary: "#dff2e6".to_string(),
            text_bright: "#f2fff7".to_string(),
            text_muted: "#95b8a2".to_string(),
            text_dim: "#587563".to_string(),
            accent_primary: "#22c55e".to_string(),
            accent_secondary: "#84cc16".to_string(),
            accent_gold: "#4ade80".to_string(),
            mod_env: "#4ade80".to_string(),
            mod_seq: "#2dd4bf".to_string(),
            mod_macro: "#a3e635".to_string(),
            mod_midi: "#f87171".to_string(),
            scope_bg: "#090d0b".to_string(),
            scope_grid: "#1a2920".to_string(),
            scope_trace: "#22c55e".to_string(),
            knob_body: "#101613".to_string(),
            knob_track: "#070a08".to_string(),
        }
    }

    pub fn daylight_studio() -> Self {
        Self {
            name: "Daylight Studio".to_string(),
            window_bg: "#d6dbe2".to_string(),
            header_bg: "#c7ced8".to_string(),
            panel_bg: "#e7eaef".to_string(),
            panel_header_bg: "#dce1e8".to_string(),
            surface_dark: "#cfd5de".to_string(),
            surface_mid: "#dfe3ea".to_string(),
            surface_hover: "#bcc5d2".to_string(),
            surface_active: "#a8c7e0".to_string(),
            border_dark: "#9aa4b2".to_string(),
            border_mid: "#adb6c4".to_string(),
            border_light: "#8c96a6".to_string(),
            text_primary: "#1b2028".to_string(),
            text_bright: "#0c1016".to_string(),
            text_muted: "#485262".to_string(),
            text_dim: "#687385".to_string(),
            accent_primary: "#0284c7".to_string(),
            accent_secondary: "#d97706".to_string(),
            accent_gold: "#0369a1".to_string(),
            mod_env: "#059669".to_string(),
            mod_seq: "#0284c7".to_string(),
            mod_macro: "#d97706".to_string(),
            mod_midi: "#dc2626".to_string(),
            scope_bg: "#c5ccd6".to_string(),
            scope_grid: "#aeb7c4".to_string(),
            scope_trace: "#0284c7".to_string(),
            knob_body: "#d2d8e0".to_string(),
            knob_track: "#aeb6c2".to_string(),
        }
    }

    pub fn built_in_themes() -> Vec<Self> {
        vec![
            Self::studio_amber(),
            Self::arctic_slate(),
            Self::midnight_violet(),
            Self::phosphor(),
            Self::daylight_studio(),
        ]
    }

    #[inline]
    pub fn accent_rgb(&self) -> (u8, u8, u8) {
        hex_to_rgb(&self.accent_primary)
    }

    #[inline]
    pub fn secondary_rgb(&self) -> (u8, u8, u8) {
        hex_to_rgb(&self.accent_secondary)
    }

    #[inline]
    #[allow(dead_code)]
    pub fn gold_rgb(&self) -> (u8, u8, u8) {
        hex_to_rgb(&self.accent_gold)
    }

    #[inline]
    pub fn mod_env_rgb(&self) -> (u8, u8, u8) {
        hex_to_rgb(&self.mod_env)
    }

    #[inline]
    pub fn mod_seq_rgb(&self) -> (u8, u8, u8) {
        hex_to_rgb(&self.mod_seq)
    }

    #[inline]
    pub fn mod_macro_rgb(&self) -> (u8, u8, u8) {
        hex_to_rgb(&self.mod_macro)
    }

    #[inline]
    pub fn mod_midi_rgb(&self) -> (u8, u8, u8) {
        hex_to_rgb(&self.mod_midi)
    }

    /// Maps the canonical `Studio Amber` CSS hex tokens in `theme.css` in a single pass
    /// so custom and built-in themes apply cleanly without double-replacement.
    pub fn render_css(&self) -> String {
        let base = include_str!("../theme.css");
        let mut out = String::with_capacity(base.len());
        let bytes = base.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'#' && i + 7 <= bytes.len() {
                let slice = &base[i..i + 7];
                if slice[1..].bytes().all(|b| b.is_ascii_hexdigit()) {
                    let mapped = self.map_hex_token(slice);
                    out.push_str(mapped);
                    i += 7;
                    continue;
                }
            }
            let ch = base[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
        out
    }

    fn map_hex_token<'a>(&'a self, token: &'a str) -> &'a str {
        let lower = token.to_ascii_lowercase();
        match lower.as_str() {
            // Window & deepest backgrounds
            "#181411" | "#1c1713" | "#1c1714" | "#1c1814" | "#1e1915" | "#1f1a16" | "#201b16" => {
                &self.border_dark
            }
            // Dark recessed surfaces, inputs, popups, readouts
            "#211b17" | "#211c17" | "#211c18" | "#241f1a" | "#261f1a" | "#26201b" | "#28221d"
            | "#29221c" | "#29231e" => &self.surface_dark,
            // Main window / panel backgrounds
            "#28231e" => &self.window_bg,
            "#2b231d" | "#2c241e" | "#2c251f" | "#2d2620" | "#2d2722" | "#302822" | "#312923"
            | "#312a24" | "#322a23" => &self.surface_mid,
            "#322c26" | "#352d27" | "#352e28" | "#362c24" | "#362d26" | "#362e27" | "#362f28" => {
                &self.panel_bg
            }
            "#362f29" => &self.header_bg,
            // Panel headers & mid borders
            "#372e27" | "#382f28" | "#39312a" | "#3c3229" | "#3d332a" | "#3e362f" | "#3f352e" => {
                &self.panel_header_bg
            }
            "#44392f" | "#443930" | "#463b32" | "#463b33" | "#463c34" | "#483d34" | "#493e36"
            | "#4a3f35" | "#4a3f37" | "#4b4037" | "#4c4036" => &self.border_mid,
            // Hover & active states
            "#473828" | "#48392a" | "#493b2f" | "#4a3c30" | "#4d3e2f" | "#4d4034" | "#4e4238"
            | "#4f3d28" => &self.surface_hover,
            "#584533" | "#59442e" | "#594631" | "#5a4835" | "#5e4933" => &self.surface_active,
            "#4f443b" | "#524336" | "#52463d" | "#594d43" | "#5a4a3c" | "#5b4d42" | "#5b5046"
            | "#5c4e43" | "#5f554c" | "#6b5d52" | "#6b6157" | "#6d5c4e" => &self.border_light,
            // Dim & muted text
            "#7c7064" | "#84786c" | "#9a8675" | "#9e8a77" => &self.text_dim,
            "#a8998a" | "#a99582" | "#bfa994" | "#cbb9a6" | "#d6c3b0" => &self.text_muted,
            // Primary & bright text
            "#e6d1ba" | "#e6d7c4" | "#e8dacb" | "#eee1d3" | "#f0e4d6" => &self.text_primary,
            "#f3e6d3" | "#f6ede2" | "#f6efe6" | "#f7eee4" | "#fbf5ee" | "#fff3dc" | "#fff6e6"
            | "#fff8ee" => &self.text_bright,
            // Accents
            "#f5b349" | "#f2cb61" | "#4a3f28" | "#4d4228" | "#8a6a42" => &self.accent_primary,
            "#ffd382" => &self.accent_gold,
            "#eb8c55" => &self.accent_secondary,
            // Mod accents
            "#00e676" => &self.mod_env,
            "#00e5ff" | "#9be8ff" => &self.mod_seq,
            "#ff6b6b" | "#7a4540" | "#8a4b44" => &self.mod_midi,
            "#3f2826" | "#472826" => &self.surface_mid,
            "#6b3530" | "#6b3632" => &self.surface_hover,
            "#ffe3de" => &self.text_bright,
            _ => token,
        }
    }
}

#[inline]
pub fn hex_to_rgb(hex: &str) -> (u8, u8, u8) {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&clean[0..2], 16),
            u8::from_str_radix(&clean[2..4], 16),
            u8::from_str_radix(&clean[4..6], 16),
        ) {
            return (r, g, b);
        }
    }
    (245, 179, 73)
}

#[inline]
pub fn vg_color(hex: &str, alpha: u8) -> vg::Color {
    let (r, g, b) = hex_to_rgb(hex);
    vg::Color::rgba(r, g, b, alpha)
}

#[inline]
pub fn lerp_rgb(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    (
        (a.0 as f32 + (b.0 as f32 - a.0 as f32) * t).round() as u8,
        (a.1 as f32 + (b.1 as f32 - a.1 as f32) * t).round() as u8,
        (a.2 as f32 + (b.2 as f32 - a.2 as f32) * t).round() as u8,
    )
}

#[inline]
pub fn vg_lerp(hex_a: &str, hex_b: &str, t: f32, alpha: u8) -> vg::Color {
    let (r, g, b) = lerp_rgb(hex_to_rgb(hex_a), hex_to_rgb(hex_b), t);
    vg::Color::rgba(r, g, b, alpha)
}

pub fn load_all_themes() -> Vec<ThemePalette> {
    let mut themes = ThemePalette::built_in_themes();

    if let Some(dir) = JSynConfig::themes_dir() {
        if !dir.exists() {
            if std::fs::create_dir_all(&dir).is_ok() {
                let template_path = dir.join("Studio Amber.json");
                if let Ok(json) = serde_json::to_string_pretty(&ThemePalette::studio_amber()) {
                    let _ = std::fs::write(template_path, json);
                }
            }
        }
        if let Ok(rd) = std::fs::read_dir(&dir) {
            let mut custom = Vec::new();
            for entry in rd.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.starts_with('.'))
                {
                    continue;
                }
                if path
                    .extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("json"))
                {
                    if let Ok(raw) = std::fs::read_to_string(&path) {
                        if let Ok(pal) = serde_json::from_str::<ThemePalette>(&raw) {
                            if !pal.name.trim().is_empty() {
                                custom.push(pal);
                            }
                        }
                    }
                }
            }
            custom.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            for pal in custom {
                if let Some(existing) = themes.iter_mut().find(|t| t.name.eq_ignore_ascii_case(&pal.name)) {
                    *existing = pal;
                } else {
                    themes.push(pal);
                }
            }
        }
    }

    themes
}

pub fn find_theme_by_name(name: &str) -> ThemePalette {
    let themes = load_all_themes();
    themes
        .into_iter()
        .find(|t| t.name.eq_ignore_ascii_case(name.trim()))
        .unwrap_or_else(ThemePalette::studio_amber)
}

#[inline]
pub(super) fn current_palette(cx: &DrawContext) -> ThemePalette {
    cx.data::<super::UiData>()
        .map(|d| d.active_theme.clone())
        .unwrap_or_else(ThemePalette::studio_amber)
}

#[inline]
pub(super) fn mod_source_rgb(pal: &ThemePalette, src: ModMatrixSource) -> (u8, u8, u8) {
    match src {
        ModMatrixSource::Env1
        | ModMatrixSource::Env2
        | ModMatrixSource::Env3
        | ModMatrixSource::Env4 => pal.mod_env_rgb(),
        ModMatrixSource::ModSeq1
        | ModMatrixSource::ModSeq2
        | ModMatrixSource::ModSeq3
        | ModMatrixSource::ModSeq4 => pal.mod_seq_rgb(),
        ModMatrixSource::Macro1
        | ModMatrixSource::Macro2
        | ModMatrixSource::Macro3
        | ModMatrixSource::Macro4
        | ModMatrixSource::Macro5
        | ModMatrixSource::Macro6
        | ModMatrixSource::Macro7
        | ModMatrixSource::Macro8 => pal.mod_macro_rgb(),
        ModMatrixSource::Velocity
        | ModMatrixSource::Keytrack
        | ModMatrixSource::ModWheel
        | ModMatrixSource::Aftertouch => pal.mod_midi_rgb(),
        ModMatrixSource::None => hex_to_rgb(&pal.text_dim),
    }
}

pub(crate) struct DynamicCssSource(pub Arc<RwLock<String>>);

impl nih_plug_vizia::vizia::util::IntoCssStr for DynamicCssSource {
    fn get_style(&self) -> Result<String, std::io::Error> {
        Ok(self.0.read().map(|g| g.clone()).unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_themes_render_valid_css() {
        let themes = ThemePalette::built_in_themes();
        assert_eq!(themes.len(), 5);
        for theme in &themes {
            let css = theme.render_css();
            assert!(css.contains(".plugin-root"));
            assert!(css.contains(&theme.accent_primary));
        }
    }
}
