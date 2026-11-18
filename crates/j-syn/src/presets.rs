//! Local configuration (`presets_dir` / `wavetables_dir`), directory scanning, and JSON patch
//! serialization/deserialization for `J-SYN`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use j_syn_dsp::{ModMatrixState, ModSeqStepBank, Wavetable, WavetableBank};
use nih_plug::params::internals::ParamPtr;
use nih_plug::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{JSynPluginParams, WavetableChoice};

fn default_theme_name() -> String {
    "Studio Amber".to_string()
}

fn default_ui_scale() -> u32 {
    100
}

fn default_gear_radius() -> f32 {
    0.32
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JSynConfig {
    #[serde(default)]
    pub presets_dir: Option<String>,
    #[serde(default)]
    pub wavetables_dir: Option<String>,
    #[serde(default = "default_theme_name")]
    pub theme: String,
    #[serde(default = "default_ui_scale")]
    pub ui_scale: u32,
    #[serde(default = "default_gear_radius")]
    pub gear_radius: f32,
}

impl Default for JSynConfig {
    fn default() -> Self {
        Self {
            presets_dir: None,
            wavetables_dir: None,
            theme: default_theme_name(),
            ui_scale: default_ui_scale(),
            gear_radius: default_gear_radius(),
        }
    }
}

impl JSynConfig {
    pub fn themes_dir() -> Option<PathBuf> {
        Self::config_file_path().and_then(|p| p.parent().map(|d| d.join("themes")))
    }
    pub fn config_file_path() -> Option<PathBuf> {
        #[cfg(target_os = "macos")]
        {
            std::env::var_os("HOME").map(|h| {
                PathBuf::from(h)
                    .join("Library")
                    .join("Application Support")
                    .join("j-syn")
                    .join("config.json")
            })
        }
        #[cfg(target_os = "windows")]
        {
            std::env::var_os("APPDATA")
                .map(|a| PathBuf::from(a).join("j-syn").join("config.json"))
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            std::env::var_os("HOME").map(|h| {
                PathBuf::from(h)
                    .join(".config")
                    .join("j-syn")
                    .join("config.json")
            })
        }
    }

    pub fn load() -> Self {
        let Some(path) = Self::config_file_path() else {
            return Self::default();
        };
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&raw).unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let Some(path) = Self::config_file_path() else {
            return Err("Could not determine config directory".to_string());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config dir: {e}"))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {e}"))?;
        write_text_atomic(&path, &json).map_err(|e| format!("Failed to write config: {e}"))
    }
}

fn write_text_atomic(path: &Path, content: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, content).is_ok() {
        if std::fs::rename(&tmp, path).is_ok() {
            return Ok(());
        }
        let _ = std::fs::remove_file(&tmp);
    }
    std::fs::write(path, content)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryFileEntry {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirectoryFolder {
    /// Immediate folder name (e.g. `"Analog"` or `"Bass"`).
    pub name: String,
    /// Relative folder path from the configured root (e.g. `"Bass/Reese"`).
    pub rel_path: String,
    pub files: Vec<DirectoryFileEntry>,
    pub subfolders: Vec<DirectoryFolder>,
}

impl DirectoryFolder {
    pub fn total_file_count(&self) -> usize {
        self.files.len()
            + self
                .subfolders
                .iter()
                .map(Self::total_file_count)
                .sum::<usize>()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirectoryTree {
    pub root_files: Vec<DirectoryFileEntry>,
    pub folders: Vec<DirectoryFolder>,
}

fn is_hidden_name(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.starts_with('.'))
}

fn collect_dir_entries(
    root: &std::path::Path,
    dir: &std::path::Path,
    ext_lower: &str,
    depth: usize,
) -> (Vec<DirectoryFileEntry>, Vec<DirectoryFolder>) {
    if depth > 8 {
        return (Vec::new(), Vec::new());
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return (Vec::new(), Vec::new());
    };
    let mut files = Vec::new();
    let mut subdirs = Vec::new();
    for entry in rd.filter_map(|res| res.ok()) {
        let path = entry.path();
        if is_hidden_name(&path) {
            continue;
        }
        if path.is_dir() {
            subdirs.push(path);
        } else if path.is_file() {
            let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
                continue;
            };
            if ext.to_ascii_lowercase() != ext_lower {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            files.push(DirectoryFileEntry {
                name: name.to_string(),
                path,
            });
        }
    }
    files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    subdirs.sort_by(|a, b| {
        a.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase()
            .cmp(
                &b.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase(),
            )
    });

    let mut folders = Vec::new();
    for sub in subdirs {
        let (sub_files, sub_subfolders) = collect_dir_entries(root, &sub, ext_lower, depth + 1);
        if sub_files.is_empty() && sub_subfolders.is_empty() {
            continue;
        }
        let name = sub
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let rel_path = sub
            .strip_prefix(root)
            .map(|rel| rel.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| name.clone());
        folders.push(DirectoryFolder {
            name,
            rel_path,
            files: sub_files,
            subfolders: sub_subfolders,
        });
    }

    (files, folders)
}

pub fn scan_directory_tree(dir: Option<&str>, ext_lower: &str) -> DirectoryTree {
    let Some(dir_str) = dir.filter(|s| !s.trim().is_empty()) else {
        return DirectoryTree::default();
    };
    let root = std::path::Path::new(dir_str);
    let (root_files, folders) = collect_dir_entries(root, root, ext_lower, 0);
    DirectoryTree {
        root_files,
        folders,
    }
}

pub fn scan_preset_tree(dir: Option<&str>) -> DirectoryTree {
    scan_directory_tree(dir, "json")
}

pub fn scan_wavetable_tree(dir: Option<&str>) -> DirectoryTree {
    scan_directory_tree(dir, "wav")
}

fn collect_folder_files(folders: &[DirectoryFolder], out: &mut Vec<DirectoryFileEntry>) {
    for folder in folders {
        out.extend(folder.files.iter().cloned());
        collect_folder_files(&folder.subfolders, out);
    }
}

fn scan_extension(dir: Option<&str>, ext_lower: &str) -> Vec<DirectoryFileEntry> {
    let tree = scan_directory_tree(dir, ext_lower);
    let mut entries = tree.root_files;
    collect_folder_files(&tree.folders, &mut entries);
    entries
}

pub fn scan_presets(dir: Option<&str>) -> Vec<DirectoryFileEntry> {
    scan_extension(dir, "json")
}

#[cfg(test)]
pub fn scan_wavetables(dir: Option<&str>) -> Vec<DirectoryFileEntry> {
    scan_extension(dir, "wav")
}

pub const CURRENT_PATCH_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct JSynPatch {
    #[serde(default = "default_patch_version")]
    pub version: u32,
    #[serde(deserialize_with = "j_syn_dsp::serde_lenient::default_on_null")]
    pub name: String,
    #[serde(
        default,
        deserialize_with = "j_syn_dsp::serde_lenient::default_on_null",
        skip_serializing_if = "String::is_empty"
    )]
    pub plugin_version: String,
    /// Normalized `[0.0, 1.0]` values for backward compatibility and fallback recall.
    #[serde(deserialize_with = "j_syn_dsp::serde_lenient::default_on_null")]
    pub params: BTreeMap<String, f32>,
    /// Unnormalized plain values (`Hz`, `dB`, `%`, integer counts, enum indices) so future
    /// parameter range or step-count expansions preserve existing presets.
    #[serde(
        default,
        deserialize_with = "j_syn_dsp::serde_lenient::default_on_null",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub plain_params: BTreeMap<String, f32>,
    /// Canonical `#[name]` variant strings for `EnumParam`s so adding or reordering variants
    /// never shifts saved presets.
    #[serde(
        default,
        deserialize_with = "j_syn_dsp::serde_lenient::default_on_null",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub enum_names: BTreeMap<String, String>,
    #[serde(deserialize_with = "j_syn_dsp::serde_lenient::default_on_null")]
    pub mod_matrix: ModMatrixState,
    #[serde(deserialize_with = "j_syn_dsp::serde_lenient::default_on_null")]
    pub modseq_steps: ModSeqStepBank,
    #[serde(
        default = "default_macro_names",
        deserialize_with = "deserialize_macro_names"
    )]
    pub macro_names: [String; 8],
    #[serde(deserialize_with = "j_syn_dsp::serde_lenient::fixed_array")]
    pub wavetable_paths: [String; 4],
}

impl Default for JSynPatch {
    fn default() -> Self {
        Self {
            version: default_patch_version(),
            name: String::new(),
            plugin_version: String::new(),
            params: BTreeMap::new(),
            plain_params: BTreeMap::new(),
            enum_names: BTreeMap::new(),
            mod_matrix: ModMatrixState::default(),
            modseq_steps: ModSeqStepBank::default(),
            macro_names: default_macro_names(),
            wavetable_paths: Default::default(),
        }
    }
}

fn default_patch_version() -> u32 {
    // Missing `"version"` implies legacy v1 so old normalized-enum migrations still run.
    1
}

fn default_macro_names() -> [String; 8] {
    std::array::from_fn(|i| format!("MACRO {}", i + 1))
}

fn deserialize_macro_names<'de, D>(deserializer: D) -> Result<[String; 8], D::Error>
where
    D: serde::Deserializer<'de>,
{
    let mut names: [String; 8] = j_syn_dsp::serde_lenient::fixed_array(deserializer)?;
    for (i, slot) in names.iter_mut().enumerate() {
        if slot.trim().is_empty() {
            *slot = format!("MACRO {}", i + 1);
        }
    }
    Ok(names)
}

#[inline]
fn clamp_finite_norm(norm: f32, ptr: ParamPtr) -> f32 {
    if norm.is_finite() {
        norm.clamp(0.0, 1.0)
    } else {
        // SAFETY: Caller passes valid `ParamPtr` from a live `JSynPluginParams`.
        unsafe { ptr.default_normalized_value() }
    }
}

#[inline]
fn enum_name_alias<'a>(id: &str, name: &'a str) -> &'a str {
    if name == "Sub"
        && (id.ends_with("_direct") || id == "f1_out_bus" || id == "f2_out_bus")
    {
        "Bus C"
    } else if id == "fx_bus_topo" {
        match name {
            "Parallel (A | B)" => "Parallel (A | B | C)",
            "Serial (A -> B)" => "Serial (A -> B | C)",
            _ => name,
        }
    } else if id.starts_with("ins_mode_") {
        match name {
            "Tube -> Mech" => "T -> M",
            "Mech -> Tube" => "M -> T",
            "Sine Shaper" => "Sine",
            "Bitcrush" => "Crush",
            "Linear Fold" => "Lin Fold",
            "Sine Fold" => "Sin Fold",
            _ => name,
        }
    } else if id.starts_with("ins_pos_") {
        match name {
            "Pre-F1" => "| F1",
            "Between F1->F2" => "F1 | F2",
            "Post-F2" => "F2 |",
            _ => name,
        }
    } else {
        name
    }
}

/// Maps `version < 2` normalized enum floats whose variant count expanded in newer builds.
fn migrate_legacy_v1_enum_norm(
    id: &str,
    raw_norm: f32,
    params: &BTreeMap<String, f32>,
    ptr: ParamPtr,
) -> Option<f32> {
    let old_steps = if id.starts_with("fx_type_") || id.starts_with("bus_b_fx_type_") {
        if params.contains_key("fx_tr_flt_1") {
            Some(10usize)
        } else if params.contains_key("fx_diop_s_1") {
            // `v0.1` / `v0.1.1`: 10 FxType variants (Empty..=Dispersion).
            Some(9)
        } else if params.contains_key("flt_slope_1") {
            Some(8)
        } else {
            Some(7)
        }
    } else if (id == "flt_mode_1" || id == "flt_mode_2") && !params.contains_key("flt_slope_1") {
        // Earliest builds before `flt_slope` / Bandreject / Peak+Peak / Notch+Notch had 12 filter modes.
        Some(11)
    } else {
        None
    }?;

    let old_idx = (raw_norm * old_steps as f32).round().clamp(0.0, old_steps as f32);
    // SAFETY: Caller passes valid `ParamPtr` from a live `JSynPluginParams`.
    Some(clamp_finite_norm(unsafe { ptr.preview_normalized(old_idx) }, ptr))
}

fn legacy_distortion_slot_keys(id: &str) -> Option<(String, String, String)> {
    if let Some(suffix) = id
        .strip_prefix("bus_b_fx_dist_m_")
        .or_else(|| id.strip_prefix("bus_b_fx_p1_"))
    {
        Some((
            format!("bus_b_fx_type_{suffix}"),
            format!("bus_b_fx_dist_m_{suffix}"),
            format!("bus_b_fx_p1_{suffix}"),
        ))
    } else if let Some(suffix) = id
        .strip_prefix("fx_dist_m_")
        .or_else(|| id.strip_prefix("fx_p1_"))
    {
        Some((
            format!("fx_type_{suffix}"),
            format!("fx_dist_m_{suffix}"),
            format!("fx_p1_{suffix}"),
        ))
    } else {
        None
    }
}

fn is_legacy_distortion_slot(patch: &JSynPatch, type_id: &str, dist_m_id: &str) -> bool {
    if patch.enum_names.contains_key(dist_m_id)
        || patch.plain_params.contains_key(dist_m_id)
        || patch.params.contains_key(dist_m_id)
    {
        return false;
    }
    if patch
        .enum_names
        .get(type_id)
        .is_some_and(|n| n.eq_ignore_ascii_case("Distortion"))
    {
        return true;
    }
    if patch
        .plain_params
        .get(type_id)
        .is_some_and(|v| v.is_finite() && v.round() as i32 == 2)
    {
        return true;
    }
    if let Some(&raw_norm) = patch.params.get(type_id) {
        if raw_norm.is_finite() {
            let n = raw_norm.clamp(0.0, 1.0);
            let old_steps = if patch.params.contains_key("fx_tr_flt_1") {
                10.0
            } else if patch.params.contains_key("fx_diop_s_1") {
                9.0
            } else if patch.params.contains_key("flt_slope_1") {
                8.0
            } else if patch.version < 2 {
                7.0
            } else {
                12.0
            };
            return (n * old_steps).round() as i32 == 2;
        }
    }
    false
}

fn resolve_patch_param_norm(patch: &JSynPatch, id: &str, ptr: ParamPtr) -> f32 {
    // SAFETY: Caller passes valid `ParamPtr` from a live `JSynPluginParams`.
    unsafe {
        match ptr {
            ParamPtr::EnumParam(_) => {
                let steps = ptr.step_count().unwrap_or(0);
                if let Some(raw_name) = patch.enum_names.get(id) {
                    let aliased = enum_name_alias(id, raw_name);
                    if let Some(n) = ptr.string_to_normalized_value(aliased) {
                        return clamp_finite_norm(n, ptr);
                    }
                    if let Some(&plain) = patch.plain_params.get(id) {
                        if plain.is_finite() {
                            let idx = plain.round();
                            if idx >= 0.0 && idx <= steps as f32 {
                                return clamp_finite_norm(ptr.preview_normalized(idx), ptr);
                            }
                        }
                    }
                    return ptr.default_normalized_value();
                }

                if let Some(&plain) = patch.plain_params.get(id) {
                    if plain.is_finite() && plain >= 0.0 {
                        let idx = plain.round();
                        if idx <= steps as f32 {
                            return clamp_finite_norm(ptr.preview_normalized(idx), ptr);
                        }
                        return ptr.default_normalized_value();
                    }
                }

                if let Some(&raw_norm) = patch.params.get(id) {
                    if raw_norm.is_finite() {
                        let n = raw_norm.clamp(0.0, 1.0);
                        if patch.version < 2 {
                            if let Some(migrated) =
                                migrate_legacy_v1_enum_norm(id, n, &patch.params, ptr)
                            {
                                return migrated;
                            }
                        }
                        return n;
                    }
                }

                if id.starts_with("fx_dist_m_") || id.starts_with("bus_b_fx_dist_m_") {
                    if let Some((type_id, dist_m_id, p1_id)) = legacy_distortion_slot_keys(id) {
                        if is_legacy_distortion_slot(patch, &type_id, &dist_m_id) {
                            let old_p1 = patch
                                .plain_params
                                .get(&p1_id)
                                .or_else(|| patch.params.get(&p1_id))
                                .copied()
                                .filter(|v| v.is_finite())
                                .unwrap_or(0.0)
                                .clamp(0.0, 1.0);
                            let idx = j_syn_dsp::waveshaper::legacy_distortion_submode_index(old_p1);
                            return clamp_finite_norm(ptr.preview_normalized(idx as f32), ptr);
                        }
                    }
                }

                ptr.default_normalized_value()
            }
            ParamPtr::IntParam(_) => {
                if let Some(&plain) = patch.plain_params.get(id) {
                    if plain.is_finite() {
                        return clamp_finite_norm(ptr.preview_normalized(plain.round()), ptr);
                    }
                }
                if let Some(&norm) = patch.params.get(id) {
                    if norm.is_finite() {
                        return norm.clamp(0.0, 1.0);
                    }
                }
                ptr.default_normalized_value()
            }
            ParamPtr::FloatParam(_) => {
                if id.starts_with("fx_p1_") || id.starts_with("bus_b_fx_p1_") {
                    if let Some((type_id, dist_m_id, _)) = legacy_distortion_slot_keys(id) {
                        if is_legacy_distortion_slot(patch, &type_id, &dist_m_id) {
                            return 0.50;
                        }
                    }
                }
                if let Some(&plain) = patch.plain_params.get(id) {
                    if plain.is_finite() {
                        return clamp_finite_norm(ptr.preview_normalized(plain), ptr);
                    }
                }
                if let Some(&norm) = patch.params.get(id) {
                    if norm.is_finite() {
                        return norm.clamp(0.0, 1.0);
                    }
                }
                ptr.default_normalized_value()
            }
            ParamPtr::BoolParam(_) => {
                if let Some(&plain) = patch.plain_params.get(id) {
                    if plain.is_finite() {
                        return if plain >= 0.5 { 1.0 } else { 0.0 };
                    }
                }
                if let Some(&norm) = patch.params.get(id) {
                    if norm.is_finite() {
                        return if norm >= 0.5 { 1.0 } else { 0.0 };
                    }
                }
                ptr.default_normalized_value()
            }
        }
    }
}

/// Resolves a stored custom wavetable path, falling back to `wavetables_dir` filename lookup
/// when sharing presets across computers or moved libraries.
pub fn resolve_wavetable_path(path_str: &str) -> Option<PathBuf> {
    let cfg = JSynConfig::load();
    resolve_wavetable_path_in_dir(path_str, cfg.wavetables_dir.as_deref())
}

fn resolve_wavetable_path_in_dir(path_str: &str, wavetables_dir: Option<&str>) -> Option<PathBuf> {
    let trimmed = path_str.trim();
    if trimmed.is_empty() {
        return None;
    }
    let direct = Path::new(trimmed);
    if direct.is_file() {
        return Some(direct.to_path_buf());
    }
    let fname = trimmed
        .rsplit(['/', '\\'])
        .next()
        .filter(|s| !s.is_empty())?;
    scan_extension(wavetables_dir, "wav")
        .into_iter()
        .find(|entry| {
            entry
                .path
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case(fname))
        })
        .map(|entry| entry.path)
}

impl JSynPatch {
    pub fn capture(name: impl Into<String>, params: &Arc<JSynPluginParams>) -> Self {
        let mut param_values = BTreeMap::new();
        let mut plain_values = BTreeMap::new();
        let mut enum_names = BTreeMap::new();
        for (id, ptr, _) in params.param_map() {
            // SAFETY: `ptr` comes from `params.param_map()` on a live `JSynPluginParams`.
            let (norm, plain, enum_name) = unsafe {
                let n = ptr.unmodulated_normalized_value();
                let p = ptr.unmodulated_plain_value();
                let en = matches!(ptr, ParamPtr::EnumParam(_))
                    .then(|| ptr.normalized_value_to_string(n, false));
                (n, p, en)
            };
            if norm.is_finite() {
                param_values.insert(id.clone(), norm);
            }
            if plain.is_finite() {
                plain_values.insert(id.clone(), plain);
            }
            if let Some(en) = enum_name {
                enum_names.insert(id, en);
            }
        }
        let macro_names = params
            .macro_names
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| default_macro_names());
        let wavetable_paths = params
            .wavetable_paths
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();

        Self {
            version: CURRENT_PATCH_VERSION,
            name: name.into(),
            plugin_version: env!("CARGO_PKG_VERSION").to_string(),
            params: param_values,
            plain_params: plain_values,
            enum_names,
            mod_matrix: params.mod_matrix.snapshot(),
            modseq_steps: params.modseq_steps.snapshot(),
            macro_names,
            wavetable_paths,
        }
    }

    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize patch: {e}"))?;
        write_text_atomic(path, &json).map_err(|e| format!("Failed to write patch: {e}"))
    }

    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read patch '{}': {e}", path.display()))?;
        let trimmed = raw.trim_start_matches('\u{feff}');
        serde_json::from_str(trimmed)
            .map_err(|e| format!("Failed to parse patch '{}': {e}", path.display()))
    }

    pub fn apply(
        &self,
        params: &Arc<JSynPluginParams>,
        gui_context: &Arc<dyn GuiContext>,
        bank: &Arc<WavetableBank>,
    ) -> [String; 8] {
        for (id, ptr, _) in params.param_map() {
            let norm = resolve_patch_param_norm(self, &id, ptr);
            // SAFETY: `ptr` belongs to `params` and is modified via `GuiContext`.
            unsafe {
                gui_context.raw_begin_set_parameter(ptr);
                gui_context.raw_set_parameter_normalized(ptr, norm);
                gui_context.raw_end_set_parameter(ptr);
            }
        }
        params.mod_matrix.store_value(self.mod_matrix);
        params.modseq_steps.store_value(self.modseq_steps);

        if let Ok(mut guard) = params.macro_names.lock() {
            *guard = self.macro_names.clone();
        }
        let mut resolved_paths: [String; 4] = Default::default();
        for osc_idx in 0..4 {
            let wt_choice = params.oscs[osc_idx].wt_select.value();
            if wt_choice == WavetableChoice::Custom {
                let raw_path = &self.wavetable_paths[osc_idx];
                if let Some(found) = resolve_wavetable_path(raw_path) {
                    if let Ok(wt) = Wavetable::load_from_wav(&found) {
                        bank.swap_custom(osc_idx, Arc::new(wt));
                        resolved_paths[osc_idx] = found.to_string_lossy().into_owned();
                        continue;
                    }
                }
            }
            let factory = wt_choice
                .factory_preset()
                .unwrap_or_else(|| j_syn_dsp::FactoryWavetable::from_index(0));
            bank.select_factory(osc_idx, factory);
        }
        if let Ok(mut guard) = params.wavetable_paths.lock() {
            *guard = resolved_paths;
        }
        self.macro_names.clone()
    }
}

pub fn init_patch(
    params: &Arc<JSynPluginParams>,
    gui_context: &Arc<dyn GuiContext>,
    bank: &Arc<WavetableBank>,
) -> [String; 8] {
    for (_, ptr, _) in params.param_map() {
        // SAFETY: `ptr` belongs to `params`.
        unsafe {
            let def = ptr.default_normalized_value();
            gui_context.raw_begin_set_parameter(ptr);
            gui_context.raw_set_parameter_normalized(ptr, def);
            gui_context.raw_end_set_parameter(ptr);
        }
    }
    params.mod_matrix.store_value(ModMatrixState::default());
    params.modseq_steps.store_value(ModSeqStepBank::default());

    let names = default_macro_names();
    if let Ok(mut guard) = params.macro_names.lock() {
        *guard = names.clone();
    }
    if let Ok(mut guard) = params.wavetable_paths.lock() {
        *guard = [String::new(), String::new(), String::new(), String::new()];
    }
    for osc_idx in 0..4 {
        bank.select_factory(osc_idx, j_syn_dsp::FactoryWavetable::from_index(0));
    }
    names
}

pub fn load_wavetable_file_for_osc(
    osc_idx: usize,
    path: &Path,
    params: &Arc<JSynPluginParams>,
    gui_context: &Arc<dyn GuiContext>,
    bank: &Arc<WavetableBank>,
) -> Result<(), String> {
    let wt = Wavetable::load_from_wav(path)?;
    bank.swap_custom(osc_idx, Arc::new(wt));
    if let Ok(mut guard) = params.wavetable_paths.lock() {
        guard[osc_idx] = path.to_string_lossy().into_owned();
    }
    let setter = ParamSetter::new(gui_context.as_ref());
    setter.begin_set_parameter(&params.oscs[osc_idx].wt_select);
    setter.set_parameter(&params.oscs[osc_idx].wt_select, WavetableChoice::Custom);
    setter.end_set_parameter(&params.oscs[osc_idx].wt_select);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_directory_tree_handles_subfolders_and_ignores_hidden_or_empty() {
        let base = std::env::temp_dir().join(format!(
            "jsyn_scan_tree_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("Analog")).unwrap();
        std::fs::create_dir_all(base.join("Bass/Reese")).unwrap();
        std::fs::create_dir_all(base.join("EmptyDir")).unwrap();
        std::fs::create_dir_all(base.join(".hidden_dir")).unwrap();

        std::fs::write(base.join("RootSaw.wav"), b"").unwrap();
        std::fs::write(base.join(".Hidden.wav"), b"").unwrap();
        std::fs::write(base.join("Analog/Juno.WAV"), b"").unwrap();
        std::fs::write(base.join("Analog/Arp.wav"), b"").unwrap();
        std::fs::write(base.join("Bass/Reese/Sub.wav"), b"").unwrap();
        std::fs::write(base.join(".hidden_dir/Ignore.wav"), b"").unwrap();
        std::fs::write(base.join("Analog/readme.txt"), b"").unwrap();

        let tree = scan_wavetable_tree(base.to_str());
        assert_eq!(tree.root_files.len(), 1);
        assert_eq!(tree.root_files[0].name, "RootSaw");

        assert_eq!(tree.folders.len(), 2);
        assert_eq!(tree.folders[0].name, "Analog");
        assert_eq!(
            tree.folders[0]
                .files
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Arp", "Juno"]
        );
        assert_eq!(tree.folders[1].name, "Bass");
        assert_eq!(tree.folders[1].rel_path, "Bass");
        assert_eq!(tree.folders[1].total_file_count(), 1);
        assert_eq!(tree.folders[1].subfolders.len(), 1);
        assert_eq!(tree.folders[1].subfolders[0].name, "Reese");
        assert_eq!(tree.folders[1].subfolders[0].rel_path, "Bass/Reese");
        assert_eq!(tree.folders[1].subfolders[0].files[0].name, "Sub");

        let flat = scan_wavetables(base.to_str());
        assert_eq!(flat.len(), 4);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn all_enum_and_plain_params_roundtrip_via_preset_primitives() {
        let params = Arc::new(JSynPluginParams::default());
        for (id, ptr, _) in params.param_map() {
            // SAFETY: `ptr` lives as long as `params`.
            unsafe {
                match ptr {
                    ParamPtr::EnumParam(_) => {
                        let steps = ptr.step_count().expect("enum has discrete steps");
                        for step in 0..=steps {
                            let expected = ptr.preview_normalized(step as f32);
                            let name = ptr.normalized_value_to_string(expected, false);
                            let by_name = ptr.string_to_normalized_value(&name).unwrap_or_else(|| {
                                panic!("Enum {id} failed name roundtrip for {name:?} (step {step})")
                            });
                            assert!(
                                (by_name - expected).abs() < 1e-4,
                                "Enum {id} name {name:?} mapped to {by_name}, expected {expected}"
                            );
                        }
                    }
                    ParamPtr::FloatParam(_) | ParamPtr::IntParam(_) | ParamPtr::BoolParam(_) => {
                        let norm = ptr.unmodulated_normalized_value();
                        let plain = ptr.unmodulated_plain_value();
                        let recovered = ptr.preview_normalized(plain);
                        assert!(
                            (recovered - norm).abs() < 1e-4,
                            "Param {id} plain={plain} roundtrip drifted: {norm} -> {recovered}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn legacy_v1_patches_migrate_fx_types_and_filter_modes_accurately() {
        let params = Arc::new(JSynPluginParams::default());
        let ptr_for = |target: &str| {
            params
                .param_map()
                .into_iter()
                .find(|(id, _, _)| id == target)
                .map(|(_, ptr, _)| ptr)
                .unwrap_or_else(|| panic!("Missing param {target}"))
        };

        // Simulate a v0.1 / v0.1.1 v1 patch: has `fx_diop_s_1`, lacks `fx_tr_flt_1`, 10 FxType variants (/9.0)
        let mut v01_patch = JSynPatch::default();
        v01_patch.version = 1;
        v01_patch.params.insert("fx_diop_s_1".into(), 0.5);
        v01_patch.params.insert("flt_slope_1".into(), 0.25);
        v01_patch.params.insert("fx_type_1".into(), 5.0 / 9.0); // Reverb
        v01_patch.params.insert("fx_type_2".into(), 6.0 / 9.0); // Clipper
        v01_patch.params.insert("fx_type_3".into(), 7.0 / 9.0); // Filter
        v01_patch.params.insert("fx_type_4".into(), 8.0 / 9.0); // Phaser
        v01_patch.params.insert("fx_type_5".into(), 9.0 / 9.0); // Dispersion
        v01_patch.params.insert("bus_b_fx_type_1".into(), 5.0 / 9.0); // Reverb on Bus B

        for (id, expected_label) in [
            ("fx_type_1", "Reverb"),
            ("fx_type_2", "Clipper"),
            ("fx_type_3", "Filter"),
            ("fx_type_4", "Phaser"),
            ("fx_type_5", "Dispersion"),
            ("bus_b_fx_type_1", "Reverb"),
        ] {
            let ptr = ptr_for(id);
            let n = resolve_patch_param_norm(&v01_patch, id, ptr);
            let label = unsafe { ptr.normalized_value_to_string(n, false) };
            assert_eq!(label, expected_label, "v0.1 migration mismatch on {id}");
        }

        // Simulate earliest pre-081fbb0 v1 patch: no `fx_diop_s_1`, no `flt_slope_1`
        let mut early_patch = JSynPatch::default();
        early_patch.version = 1;
        early_patch.params.insert("fx_type_1".into(), 7.0 / 7.0); // Filter (was variant 7 of 8)
        early_patch.params.insert("flt_mode_1".into(), 11.0 / 11.0); // Peak (was variant 11 of 12)
        let fx1 = resolve_patch_param_norm(&early_patch, "fx_type_1", ptr_for("fx_type_1"));
        let flt1 = resolve_patch_param_norm(&early_patch, "flt_mode_1", ptr_for("flt_mode_1"));
        assert_eq!(
            unsafe { ptr_for("fx_type_1").normalized_value_to_string(fx1, false) },
            "Filter"
        );
        assert_eq!(
            unsafe { ptr_for("flt_mode_1").normalized_value_to_string(flt1, false) },
            "Peak"
        );
    }

    #[test]
    fn future_and_malformed_patch_fields_deserialize_and_resolve_safely() {
        let params = Arc::new(JSynPluginParams::default());
        let ptr_for = |target: &str| {
            params
                .param_map()
                .into_iter()
                .find(|(id, _, _)| id == target)
                .map(|(_, ptr, _)| ptr)
                .unwrap()
        };

        let raw_json = "\u{feff}{
            \"version\": 99,
            \"name\": \"Future Patch\",
            \"plugin_version\": null,
            \"params\": {
                \"fx_type_1\": 0.99,
                \"fx_type_2\": 0.1,
                \"glide_ms\": 0.9,
                \"fx_stack_1\": 0.0,
                \"future_unknown_param\": 0.5
            },
            \"plain_params\": {
                \"fx_type_1\": 42.0,
                \"fx_type_2\": 99.0,
                \"glide_ms\": 125.0,
                \"fx_stack_1\": 3.0
            },
            \"enum_names\": {
                \"fx_type_1\": \"FutureSpectralGrain\",
                \"fx_type_2\": \"Reverb\"
            },
            \"macro_names\": [\"SUB\", \"\", \"GRIT\"],
            \"wavetable_paths\": [\"a.wav\", \"b.wav\", \"c.wav\", \"d.wav\", \"e.wav\"],
            \"mod_matrix\": {
                \"slots\": [
                    { \"source\": \"Macro1\", \"destination\": \"FutureDest\", \"depth\": 4.0, \"enabled\": true, \"curve\": 2.0 },
                    { \"source\": \"Env1\", \"destination\": \"Filter1Cutoff\", \"depth\": 0.5, \"enabled\": true, \"aux_source\": \"FutureAux\" }
                ]
            },
            \"modseq_steps\": {
                \"seqs\": [
                    [{ \"start_level\": 2.0, \"end_level\": -1.0, \"tension\": 9.0, \"shape\": \"FutureShape\" }]
                ],
                \"msegs\": [
                    { \"nodes\": [{ \"time_x\": 0.0, \"level_y\": 1.0, \"tension\": 0.0 }], \"num_nodes\": 1 }
                ]
            }
        }";

        let tmp = std::env::temp_dir().join(format!("jsyn_future_patch_{}.json", std::process::id()));
        std::fs::write(&tmp, raw_json).unwrap();
        let patch = JSynPatch::load_from_file(&tmp).expect("lenient patch load must succeed");
        let _ = std::fs::remove_file(&tmp);

        // Unknown future FX variant falls back to default (Empty), NOT clamped onto Tube Res.
        let fx1 = resolve_patch_param_norm(&patch, "fx_type_1", ptr_for("fx_type_1"));
        assert_eq!(
            unsafe { ptr_for("fx_type_1").normalized_value_to_string(fx1, false) },
            "Empty"
        );
        // Semantic `enum_names` beats stale index/normalized values
        let fx2 = resolve_patch_param_norm(&patch, "fx_type_2", ptr_for("fx_type_2"));
        assert_eq!(
            unsafe { ptr_for("fx_type_2").normalized_value_to_string(fx2, false) },
            "Reverb"
        );
        // `plain_params` beats normalized for Float/Int range changes
        let glide = resolve_patch_param_norm(&patch, "glide_ms", ptr_for("glide_ms"));
        assert!((unsafe { ptr_for("glide_ms").preview_plain(glide) } - 125.0).abs() < 0.01);
        let stack = resolve_patch_param_norm(&patch, "fx_stack_1", ptr_for("fx_stack_1"));
        assert_eq!(unsafe { ptr_for("fx_stack_1").preview_plain(stack) }.round(), 3.0);

        // ModMatrix: unknown dest slot disabled & clamped; known slot preserves routing + drops unknown aux
        assert!(!patch.mod_matrix.slots[0].enabled);
        assert!((patch.mod_matrix.slots[0].depth - 1.0).abs() < 1e-6);
        assert!(patch.mod_matrix.slots[1].enabled);
        assert_eq!(patch.mod_matrix.slots[1].aux_source, j_syn_dsp::ModMatrixSource::None);

        // ModSeq & Mseg: clamped safely, unknown shape falls back to Linear, degenerate MSEG resets cleanly
        let s0 = patch.modseq_steps.seqs[0][0];
        assert!((s0.start_level - 1.0).abs() < 1e-6 && (s0.end_level - 0.0).abs() < 1e-6);
        assert_eq!(s0.shape, j_syn_dsp::ModSeqStepShape::Linear);
        assert_eq!(patch.modseq_steps.msegs[0].num_nodes, 4);

        // Missing parameter (`fx_tr_flt_1` omitted) resets cleanly to its default (true = 1.0)
        let tr_flt = resolve_patch_param_norm(&patch, "fx_tr_flt_1", ptr_for("fx_tr_flt_1"));
        assert!((tr_flt - 1.0).abs() < 1e-6);

        // Padded macro names & truncated wavetable paths
        assert_eq!(patch.macro_names[0], "SUB");
        assert_eq!(patch.macro_names[1], "MACRO 2");
        assert_eq!(patch.macro_names[7], "MACRO 8");
        assert_eq!(patch.wavetable_paths[3], "d.wav");

        // Cross-platform wavetable filename fallback (Windows path resolved inside configured dir)
        let wt_root = std::env::temp_dir().join(format!("jsyn_wt_resolve_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&wt_root);
        std::fs::create_dir_all(wt_root.join("Bass")).unwrap();
        let target_wt = wt_root.join("Bass/Reese.wav");
        std::fs::write(&target_wt, b"RIFF").unwrap();
        let resolved = resolve_wavetable_path_in_dir(
            r"C:\Users\Artist\Desktop\Tables\reese.wav",
            wt_root.to_str(),
        );
        assert_eq!(resolved.as_deref(), Some(target_wt.as_path()));
        let _ = std::fs::remove_dir_all(&wt_root);
    }

    #[test]
    fn legacy_distortion_quartile_patches_migrate_to_distortion_mode_and_neutral_bias() {
        let params_obj = JSynPluginParams::default();
        let param_map = params_obj.param_map();
        let ptr_for = |id: &str| {
            *param_map
                .iter()
                .find(|(pid, _, _)| pid == id)
                .map(|(_, ptr, _)| ptr)
                .unwrap_or_else(|| panic!("missing param id {id}"))
        };

        let mut legacy = JSynPatch::default();
        legacy.version = 2;
        for (slot, &(p1_val, expected_mode)) in [
            (0.10_f32, "Tube"),
            (0.35_f32, "Mech"),
            (0.60_f32, "Sine"),
            (0.85_f32, "Crush"),
        ]
        .iter()
        .enumerate()
        {
            let slot_num = slot + 1;
            let ty_id = format!("fx_type_{slot_num}");
            let p1_id = format!("fx_p1_{slot_num}");
            let dm_id = format!("fx_dist_m_{slot_num}");
            legacy.enum_names.insert(ty_id.clone(), "Distortion".to_string());
            legacy.params.insert(p1_id.clone(), p1_val);

            let resolved_mode = resolve_patch_param_norm(&legacy, &dm_id, ptr_for(&dm_id));
            assert_eq!(
                unsafe { ptr_for(&dm_id).normalized_value_to_string(resolved_mode, false) },
                *expected_mode
            );
            let resolved_bias = resolve_patch_param_norm(&legacy, &p1_id, ptr_for(&p1_id));
            assert!(
                (resolved_bias - 0.50).abs() < 1e-6,
                "legacy Distortion p1 ({p1_id}) should reset to 0.50 neutral Bias, got {resolved_bias}"
            );
        }

        // Modern patch with fx_dist_m_1 present preserves custom Bias
        let mut modern = JSynPatch::default();
        modern.version = 2;
        modern.enum_names.insert("fx_type_1".into(), "Distortion".into());
        modern.enum_names.insert("fx_dist_m_1".into(), "Classic B".into());
        modern.params.insert("fx_p1_1".into(), 0.72);
        let dm = resolve_patch_param_norm(&modern, "fx_dist_m_1", ptr_for("fx_dist_m_1"));
        assert_eq!(
            unsafe { ptr_for("fx_dist_m_1").normalized_value_to_string(dm, false) },
            "Classic B"
        );
        let bias = resolve_patch_param_norm(&modern, "fx_p1_1", ptr_for("fx_p1_1"));
        assert!((bias - 0.72).abs() < 1e-6);
    }

    #[test]
    fn legacy_insert_enum_names_alias_to_compact_labels() {
        let params_obj = JSynPluginParams::default();
        let param_map = params_obj.param_map();
        let ptr_for = |id: &str| {
            *param_map
                .iter()
                .find(|(pid, _, _)| pid == id)
                .map(|(_, ptr, _)| ptr)
                .unwrap_or_else(|| panic!("missing param id {id}"))
        };

        let mut legacy = JSynPatch::default();
        legacy.enum_names.insert("ins_mode_1".into(), "Tube -> Mech".into());
        legacy.enum_names.insert("ins_mode_2".into(), "Linear Fold".into());
        legacy.enum_names.insert("ins_pos_1".into(), "Pre-F1".into());
        legacy.enum_names.insert("ins_pos_2".into(), "Post-F2".into());

        for (id, expected) in [
            ("ins_mode_1", "T -> M"),
            ("ins_mode_2", "Lin Fold"),
            ("ins_pos_1", "| F1"),
            ("ins_pos_2", "F2 |"),
        ] {
            let ptr = ptr_for(id);
            let norm = resolve_patch_param_norm(&legacy, id, ptr);
            assert_eq!(
                unsafe { ptr.normalized_value_to_string(norm, false) },
                expected
            );
        }
    }
}
