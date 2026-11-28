//! Local configuration (`presets_dir` / `wavetables_dir`) and recursive directory tree scanning.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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
    #[allow(dead_code)]
    pub fn themes_dir() -> Option<PathBuf> {
        j_syn_ui::theme::themes_dir()
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
            std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("j-syn").join("config.json"))
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

pub(super) fn write_text_atomic(path: &Path, content: &str) -> std::io::Result<()> {
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

pub use j_syn_ui::controls::{DirectoryFileEntry, DirectoryFolder, DirectoryTree};

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
    files.sort_by_key(|a| a.name.to_lowercase());
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

pub(super) fn scan_extension(dir: Option<&str>, ext_lower: &str) -> Vec<DirectoryFileEntry> {
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
