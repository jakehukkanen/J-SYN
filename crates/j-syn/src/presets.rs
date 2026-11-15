//! Local configuration (`presets_dir` / `wavetables_dir`), directory scanning, and JSON patch
//! serialization/deserialization for `J-SYN`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use j_syn_dsp::{ModMatrixState, ModSeqStepBank, Wavetable, WavetableBank};
use nih_plug::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{JSynPluginParams, WavetableChoice};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JSynConfig {
    #[serde(default)]
    pub presets_dir: Option<String>,
    #[serde(default)]
    pub wavetables_dir: Option<String>,
}

impl JSynConfig {
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

fn scan_extension(dir: Option<&str>, ext_lower: &str) -> Vec<DirectoryFileEntry> {
    let Some(dir_str) = dir.filter(|s| !s.trim().is_empty()) else {
        return Vec::new();
    };
    let Ok(rd) = std::fs::read_dir(dir_str) else {
        return Vec::new();
    };
    let mut entries: Vec<DirectoryFileEntry> = rd
        .filter_map(|res| res.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let ext = path.extension()?.to_str()?.to_ascii_lowercase();
            if ext != ext_lower {
                return None;
            }
            let name = path.file_stem()?.to_str()?.to_string();
            Some(DirectoryFileEntry { name, path })
        })
        .collect();
    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    entries
}

pub fn scan_presets(dir: Option<&str>) -> Vec<DirectoryFileEntry> {
    scan_extension(dir, "json")
}

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
