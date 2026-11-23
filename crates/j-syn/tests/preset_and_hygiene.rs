//! Frozen preset snapshot locks, parameter ID stability checks, and anti-slop structural tests.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use j_syn::JSynPluginParams;
use j_syn::presets::JSynPatch;
use j_syn_dsp::WavetableBank;
use j_syn_dsp::mod_matrix::{ModDestination, ModMatrixSource};
use nih_plug::params::internals::ParamPtr;
use nih_plug::prelude::{GuiContext, Params, PluginApi, PluginState};

#[derive(Default)]
struct RecordingGuiContext {
    written: Mutex<HashMap<ParamPtr, f32>>,
}

impl RecordingGuiContext {
    fn written_label(&self, params: &JSynPluginParams, target_id: &str) -> String {
        let (_, ptr, _) = params
            .param_map()
            .into_iter()
            .find(|(id, _, _)| id == target_id)
            .unwrap_or_else(|| panic!("Missing parameter ID '{target_id}'"));
        let norm = *self
            .written
            .lock()
            .unwrap()
            .get(&ptr)
            .unwrap_or_else(|| panic!("Parameter '{target_id}' was never written by apply()"));
        // SAFETY: `ptr` comes from `params.param_map()` on a live `JSynPluginParams`.
        unsafe { ptr.normalized_value_to_string(norm, false) }
    }

    fn written_norm(&self, params: &JSynPluginParams, target_id: &str) -> f32 {
        let (_, ptr, _) = params
            .param_map()
            .into_iter()
            .find(|(id, _, _)| id == target_id)
            .unwrap_or_else(|| panic!("Missing parameter ID '{target_id}'"));
        *self
            .written
            .lock()
            .unwrap()
            .get(&ptr)
            .unwrap_or_else(|| panic!("Parameter '{target_id}' was never written by apply()"))
    }
}

impl GuiContext for RecordingGuiContext {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Standalone
    }

    fn request_resize(&self) -> bool {
        false
    }

    unsafe fn raw_begin_set_parameter(&self, _param: ParamPtr) {}

    unsafe fn raw_set_parameter_normalized(&self, param: ParamPtr, normalized: f32) {
        self.written.lock().unwrap().insert(param, normalized);
    }

    unsafe fn raw_end_set_parameter(&self, _param: ParamPtr) {}

    fn get_state(&self) -> PluginState {
        PluginState {
            version: String::new(),
            params: Default::default(),
            fields: Default::default(),
        }
    }

    fn set_state(&self, _state: PluginState) {}
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/j-syn has workspace root")
        .to_path_buf()
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.is_dir() {
            collect_rs_files(&p, out);
        } else if p.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(p);
        }
    }
}

#[test]
fn frozen_legacy_v1_fixture_migrates_cleanly() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/legacy_v1.json");
    let patch = JSynPatch::load_from_file(&fixture).expect("load frozen v1 fixture");

    let params = Arc::new(JSynPluginParams::default());
    let rec = Arc::new(RecordingGuiContext::default());
    let gui_ctx: Arc<dyn GuiContext> = rec.clone();
    let bank = Arc::new(WavetableBank::new());
    patch.apply(&params, &gui_ctx, &bank);

    // v0.1 10-variant normalized FX types migrate cleanly
    assert_eq!(rec.written_label(&params, "fx_type_1"), "Reverb");
    assert_eq!(rec.written_label(&params, "fx_type_2"), "Distortion");
    assert_eq!(rec.written_label(&params, "fx_type_3"), "Dispersion");
    assert_eq!(rec.written_label(&params, "bus_b_fx_type_1"), "Clipper");

    // Legacy enum name aliases migrate cleanly
    assert_eq!(
        rec.written_label(&params, "fx_bus_topo"),
        "Serial (A -> B | C)"
    );
    assert_eq!(rec.written_label(&params, "f1_out_bus"), "Bus C");
    assert_eq!(rec.written_label(&params, "ins_mode_1"), "T -> M");
    assert_eq!(rec.written_label(&params, "ins_pos_1"), "| F1");

    // ModMatrix slot preserved
    let mat = params.mod_matrix.snapshot();
    assert!(mat.slots[0].enabled);
    assert_eq!(mat.slots[0].source, ModMatrixSource::Env2);
    assert_eq!(mat.slots[0].destination, ModDestination::Filter1Cutoff);
    assert!((mat.slots[0].depth - 0.45).abs() < 1e-4);
}

#[test]
fn frozen_current_v2_fixture_resolves_and_persists_cleanly() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/current_v2.json");
    let patch = JSynPatch::load_from_file(&fixture).expect("load frozen v2 fixture");

    let params = Arc::new(JSynPluginParams::default());
    let rec = Arc::new(RecordingGuiContext::default());
    let gui_ctx: Arc<dyn GuiContext> = rec.clone();
    let bank = Arc::new(WavetableBank::new());
    let macro_names = patch.apply(&params, &gui_ctx, &bank);

    assert!((rec.written_norm(&params, "mpe_en") - 1.0).abs() < 1e-4);
    assert_eq!(rec.written_label(&params, "mpe_pb"), "48");
    assert_eq!(rec.written_label(&params, "fx_bus_topo"), "A+B -> C");
    assert_eq!(rec.written_label(&params, "fx_type_1"), "OTT");
    assert_eq!(rec.written_label(&params, "fx_stack_1"), "2");
    assert!((rec.written_norm(&params, "fx_p1_1") - 0.75).abs() < 1e-4);
    assert_eq!(rec.written_label(&params, "bus_b_fx_type_1"), "Dispersion");
    assert_eq!(rec.written_label(&params, "bus_c_fx_type_1"), "Limiter");
    assert_eq!(rec.written_label(&params, "flt_mode_1"), "Scream");
    assert_eq!(rec.written_label(&params, "flt_slope_1"), "24 dB");
    assert_eq!(macro_names[0], "OTT AMT");

    // Capture -> JSON -> reload preserves mod_matrix, modseq_steps, macro_names, and param count
    let captured = JSynPatch::capture("roundtrip", &params);
    let json = serde_json::to_string(&captured).expect("serialize captured patch");
    let reloaded: JSynPatch = serde_json::from_str(&json).expect("deserialize captured patch");
    assert_eq!(reloaded.mod_matrix, params.mod_matrix.snapshot());
    assert_eq!(reloaded.modseq_steps, params.modseq_steps.snapshot());
    assert_eq!(reloaded.macro_names[0], "OTT AMT");
    assert_eq!(reloaded.params.len(), params.param_map().len());
}

#[test]
fn canonical_parameter_ids_are_unique_and_stable() {
    let params = JSynPluginParams::default();
    let map = params.param_map();
    let mut seen = HashSet::with_capacity(map.len());
    for (id, _, _) in &map {
        assert!(
            seen.insert(id.as_str()),
            "Duplicate nih-plug parameter ID: {id}"
        );
    }

    // Core frozen IDs that must never be renamed without an explicit migration
    let required_ids = [
        "master_vol",
        "dc_blocker",
        "master_limiter",
        "modseq_swing",
        "hq_mode",
        "mono_legato",
        "glide_ms",
        "pb_range",
        "mpe_en",
        "mpe_pb",
        "amp_atk",
        "amp_dec",
        "amp_sus",
        "amp_rel",
        "bus_a_lvl",
        "bus_a_pan",
        "bus_b_lvl",
        "bus_b_pan",
        "bus_c_lvl",
        "bus_c_pan",
        "fx_bus_topo",
        "sub_en",
        "sub_cutoff",
        "sub_drive",
        "sub_lvl",
        "fx_type_1",
        "fx_en_1",
        "fx_stack_1",
        "fx_p1_1",
        "bus_b_fx_type_1",
        "bus_c_fx_type_1",
        "flt_mode_1",
        "flt_slope_1",
        "ins_mode_1",
        "ins_pos_1",
        "ms_mode_1",
        "ms_sync_1",
        "ms_rate_1",
    ];
    for id in required_ids {
        assert!(
            seen.contains(id),
            "Canonical parameter ID '{id}' is missing (accidental rename breaks presets!)"
        );
    }
}

#[test]
fn anti_slop_no_raw_hex_colors_in_editor_or_ui_rust_outside_theme() {
    let root = workspace_root();
    let mut files = Vec::new();
    collect_rs_files(&root.join("crates/j-syn/src/editor"), &mut files);
    collect_rs_files(&root.join("crates/j-syn-ui/src"), &mut files);

    for path in files {
        let fname = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if fname == "theme.rs" {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("read rs file");
        let prod_src = src.split("#[cfg(test)]").next().unwrap_or(&src);
        for (line_idx, line) in prod_src.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            let bytes = code.as_bytes();
            let mut i = 0;
            while i + 7 < bytes.len() {
                if bytes[i] == b'"' && bytes[i + 1] == b'#' {
                    let hex_len = bytes[i + 2..]
                        .iter()
                        .take_while(|b| b.is_ascii_hexdigit())
                        .count();
                    if (hex_len == 6 || hex_len == 8) && bytes.get(i + 2 + hex_len) == Some(&b'"') {
                        panic!(
                            "Raw hardcoded hex color in {}:{} -> {}",
                            path.display(),
                            line_idx + 1,
                            line.trim()
                        );
                    }
                }
                i += 1;
            }
        }
    }
}

#[test]
fn anti_slop_single_timer_no_zindex_in_fx_scrollview_and_no_focus_within() {
    let root = workspace_root();

    // 1. Exactly one cx.add_timer call across editor/ (in mod_bar.rs)
    let mut editor_files = Vec::new();
    collect_rs_files(&root.join("crates/j-syn/src/editor"), &mut editor_files);
    let mut timer_hits = Vec::new();
    for path in &editor_files {
        let src = std::fs::read_to_string(path).expect("read editor file");
        let prod_src = src.split("#[cfg(test)]").next().unwrap_or(&src);
        for (idx, line) in prod_src.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if code.contains("add_timer(") {
                timer_hits.push(format!("{}:{}", path.display(), idx + 1));
            }
        }
    }
    assert_eq!(
        timer_hits.len(),
        1,
        "Expected exactly 1 add_timer() call in editor (mod_bar.rs), found: {timer_hits:?}"
    );
    assert!(
        timer_hits[0].contains("mod_bar.rs"),
        "Single 60 Hz timer must live in mod_bar.rs, found: {}",
        timer_hits[0]
    );

    // 2. Zero .z_index( inside fx_rack/ or j-syn-ui/src/fx_units/ (ScrollView clip bug)
    let mut fx_ui_files = Vec::new();
    collect_rs_files(
        &root.join("crates/j-syn/src/editor/fx_rack"),
        &mut fx_ui_files,
    );
    collect_rs_files(&root.join("crates/j-syn-ui/src/fx_units"), &mut fx_ui_files);
    for path in &fx_ui_files {
        let src = std::fs::read_to_string(path).expect("read fx ui file");
        let prod_src = src.split("#[cfg(test)]").next().unwrap_or(&src);
        for (idx, line) in prod_src.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            assert!(
                !code.contains(".z_index("),
                "Forbidden .z_index() inside ScrollView FX UI at {}:{}",
                path.display(),
                idx + 1
            );
            assert!(
                !code.to_ascii_lowercase().contains("snapin"),
                "Use 'FX Unit' (fx_unit), never 'snapin', at {}:{}",
                path.display(),
                idx + 1
            );
        }
    }

    // 3. Zero :focus-within in theme.css
    let css = std::fs::read_to_string(root.join("crates/j-syn-ui/src/theme.css"))
        .expect("read theme.css");
    assert!(
        !css.contains(":focus-within"),
        "Unsupported Vizia CSS selector :focus-within crashes Ableton Live"
    );
}
