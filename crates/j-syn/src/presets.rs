//! Local configuration (`presets_dir` / `wavetables_dir`), directory scanning, and JSON patch
//! serialization/deserialization for `J-SYN`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use j_syn_dsp::{ModMatrixState, ModSeqStepBank, Wavetable, WavetableBank};
use nih_plug::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{JSynPluginParams, WavetableChoice};

fn default_theme_name() -> String {
    "Studio Amber".to_string()
}

fn default_ui_scale() -> u32 {
    100
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JSynConfig {
    #[serde(default)]
    pub presets_dir: Option<String>,
    #[serde(default)]
    pub wavetables_dir: Option<String>,
    #[serde(default = "default_theme_name")]
    pub theme: String,
    #[serde(default = "default_ui_scale")]
    pub ui_scale: u32,
}

impl Default for JSynConfig {
    fn default() -> Self {
        Self {
            presets_dir: None,
            wavetables_dir: None,
            theme: default_theme_name(),
            ui_scale: default_ui_scale(),
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
        std::fs::write(&path, json).map_err(|e| format!("Failed to write config: {e}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryFileEntry {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryFolder {
    /// Relative folder path from the configured root (e.g. `"Analog"` or `"Bass/Reese"`).
    pub name: String,
    pub files: Vec<DirectoryFileEntry>,
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

fn collect_tree_recursive(
    root: &std::path::Path,
    dir: &std::path::Path,
    ext_lower: &str,
    depth: usize,
    out: &mut DirectoryTree,
) {
    if depth > 8 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
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

    if !files.is_empty() {
        if depth == 0 {
            out.root_files = files;
        } else if let Ok(rel) = dir.strip_prefix(root) {
            let folder_name = rel.to_string_lossy().replace('\\', "/");
            out.folders.push(DirectoryFolder {
                name: folder_name,
                files,
            });
        }
    }

    for sub in subdirs {
        collect_tree_recursive(root, &sub, ext_lower, depth + 1, out);
    }
}

pub fn scan_directory_tree(dir: Option<&str>, ext_lower: &str) -> DirectoryTree {
    let Some(dir_str) = dir.filter(|s| !s.trim().is_empty()) else {
        return DirectoryTree::default();
    };
    let root = std::path::Path::new(dir_str);
    let mut tree = DirectoryTree::default();
    collect_tree_recursive(root, root, ext_lower, 0, &mut tree);
    tree
}

pub fn scan_preset_tree(dir: Option<&str>) -> DirectoryTree {
    scan_directory_tree(dir, "json")
}

pub fn scan_wavetable_tree(dir: Option<&str>) -> DirectoryTree {
    scan_directory_tree(dir, "wav")
}

fn scan_extension(dir: Option<&str>, ext_lower: &str) -> Vec<DirectoryFileEntry> {
    let tree = scan_directory_tree(dir, ext_lower);
    let mut entries = tree.root_files;
    for folder in tree.folders {
        entries.extend(folder.files);
    }
    entries
}

pub fn scan_presets(dir: Option<&str>) -> Vec<DirectoryFileEntry> {
    scan_extension(dir, "json")
}

#[cfg(test)]
pub fn scan_wavetables(dir: Option<&str>) -> Vec<DirectoryFileEntry> {
    scan_extension(dir, "wav")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JSynPatch {
    #[serde(default = "default_patch_version")]
    pub version: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub params: BTreeMap<String, f32>,
    #[serde(default)]
    pub mod_matrix: ModMatrixState,
    #[serde(default)]
    pub modseq_steps: ModSeqStepBank,
    #[serde(default = "default_macro_names")]
    pub macro_names: [String; 8],
    #[serde(default)]
    pub wavetable_paths: [String; 4],
}

fn default_patch_version() -> u32 {
    1
}

fn default_macro_names() -> [String; 8] {
    std::array::from_fn(|i| format!("MACRO {}", i + 1))
}

impl JSynPatch {
    pub fn capture(name: impl Into<String>, params: &Arc<JSynPluginParams>) -> Self {
        let mut param_values = BTreeMap::new();
        for (id, ptr, _) in params.param_map() {
            // SAFETY: `ptr` comes from `params.param_map()` on a live `JSynPluginParams`.
            let norm = unsafe { ptr.unmodulated_normalized_value() };
            param_values.insert(id, norm);
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
            version: 1,
            name: name.into(),
            params: param_values,
            mod_matrix: params.mod_matrix.snapshot(),
            modseq_steps: params.modseq_steps.snapshot(),
            macro_names,
            wavetable_paths,
        }
    }

    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize patch: {e}"))?;
        std::fs::write(path, json).map_err(|e| format!("Failed to write patch: {e}"))
    }

    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read patch '{}': {e}", path.display()))?;
        serde_json::from_str(&raw)
            .map_err(|e| format!("Failed to parse patch '{}': {e}", path.display()))
    }

    pub fn apply(
        &self,
        params: &Arc<JSynPluginParams>,
        gui_context: &Arc<dyn GuiContext>,
        bank: &Arc<WavetableBank>,
    ) -> [String; 8] {
        for (id, ptr, _) in params.param_map() {
            if let Some(&norm) = self.params.get(&id) {
                let norm = norm.clamp(0.0, 1.0);
                // SAFETY: `ptr` belongs to `params` and is modified via `GuiContext`.
                unsafe {
                    gui_context.raw_begin_set_parameter(ptr);
                    gui_context.raw_set_parameter_normalized(ptr, norm);
                    gui_context.raw_end_set_parameter(ptr);
                }
            }
        }
        params.mod_matrix.store_value(self.mod_matrix);
        params.modseq_steps.store_value(self.modseq_steps);

        if let Ok(mut guard) = params.macro_names.lock() {
            *guard = self.macro_names.clone();
        }
        if let Ok(mut guard) = params.wavetable_paths.lock() {
            *guard = self.wavetable_paths.clone();
        }
        for (osc_idx, path_str) in self.wavetable_paths.iter().enumerate() {
            if !path_str.is_empty() {
                if let Ok(wt) = Wavetable::load_from_wav(Path::new(path_str)) {
                    bank.swap_custom(osc_idx, Arc::new(wt));
                }
            } else {
                let factory = params.oscs[osc_idx]
                    .wt_select
                    .value()
                    .factory_preset()
                    .unwrap_or_else(|| j_syn_dsp::FactoryWavetable::from_index(0));
                bank.select_factory(osc_idx, factory);
            }
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
        assert_eq!(tree.folders[1].name, "Bass/Reese");
        assert_eq!(tree.folders[1].files[0].name, "Sub");

        let flat = scan_wavetables(base.to_str());
        assert_eq!(flat.len(), 4);

        let _ = std::fs::remove_dir_all(&base);
    }
}
