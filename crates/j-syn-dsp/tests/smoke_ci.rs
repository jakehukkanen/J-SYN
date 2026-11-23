//! Exhaustive CI DSP torture matrix and deterministic cross-platform scenario regression suite.

use j_syn_dsp::{
    DirectDest, EngineMode, FactoryWavetable, FilterBusDest, FilterMode, FilterParams,
    FilterRouting, FilterSlope, FilterState, FxBusParams, FxBusTopology, FxRackParams, FxRackState,
    FxSlotParams, FxType, InsertMode, MAX_FX_STACK, ModDestination, ModMatrixSource, ModSeqMode,
    ModSeqRetrig, ModSlot, ModSource, NUM_MOD_DESTS, OscPitchMode, SynthEngine, SynthParams,
    WavetableBank,
};

fn db_from_linear(lin: f32) -> f32 {
    20.0 * lin.max(1e-12).log10()
}

#[test]
fn torture_all_fx_types_stacks_sample_rates_and_subnormals() {
    let sample_rates = [44_100.0_f32, 48_000.0, 96_000.0, 192_000.0];
    let mod_zeros = [0.0_f32; NUM_MOD_DESTS];

    for &sr in &sample_rates {
        let mut rack = FxRackState::new(sr);
        for &fx_ty in FxType::ALL.iter().filter(|&&t| t != FxType::Off) {
            for stack in 1..=(MAX_FX_STACK as u8) {
                rack.reset();
                let mut rp = FxRackParams::default();
                rp.topology = FxBusTopology::Parallel;
                let mut slot = FxSlotParams::init_patch_slot(0);
                slot.fx_type = fx_ty;
                slot.enabled = true;
                slot.stack = stack;
                slot.params = FxSlotParams::default_params_for_type(fx_ty);
                slot.eq_params = FxSlotParams::default_eq_params_for_type(fx_ty);
                rp.buses[0].slots[0] = slot;
                rack.prepare_for_params(&rp);

                // 1) Drive with impulse + stereo sine burst
                let burst_len = ((sr * 0.025) as usize).max(256);
                let mut peak = 0.0_f32;
                for i in 0..burst_len {
                    let t = i as f32 / sr;
                    let sig_l = if i == 0 {
                        0.95
                    } else {
                        0.45 * (std::f32::consts::TAU * 220.0 * t).sin()
                    };
                    let sig_r = 0.45 * (std::f32::consts::TAU * 330.0 * t).sin();
                    let (ol, or) = rack.process_buses_sample(
                        (sig_l, sig_r),
                        (0.0, 0.0),
                        (0.0, 0.0),
                        &rp,
                        &mod_zeros,
                        120.0,
                        None,
                    );
                    assert!(
                        ol.is_finite() && or.is_finite(),
                        "Non-finite output for {fx_ty:?} stack={stack} sr={sr} sample={i}"
                    );
                    peak = peak.max(ol.abs()).max(or.abs());
                }
                assert!(
                    peak < 32.0,
                    "Runaway peak {peak} for {fx_ty:?} stack={stack} sr={sr}"
                );

                // 2) Feed subnormals (1e-35) then silence to verify denormal flushing / finite stability
                for i in 0..256 {
                    let sub = if i < 64 { 1e-35_f32 } else { 0.0 };
                    let (ol, or) = rack.process_buses_sample(
                        (sub, -sub),
                        (0.0, 0.0),
                        (0.0, 0.0),
                        &rp,
                        &mod_zeros,
                        120.0,
                        None,
                    );
                    assert!(
                        ol.is_finite() && or.is_finite(),
                        "Non-finite subnormal tail for {fx_ty:?} stack={stack} sr={sr}"
                    );
                }
            }
        }
    }
}

#[test]
fn torture_all_filter_modes_and_slopes_across_sample_rates() {
    let sample_rates = [44_100.0_f32, 48_000.0, 96_000.0, 192_000.0];
    let slopes = [
        FilterSlope::Db6,
        FilterSlope::Db12,
        FilterSlope::Db24,
        FilterSlope::Db36,
        FilterSlope::Db48,
    ];

    for mode_idx in 0..=14 {
        let mode = FilterMode::from_index(mode_idx);
        for &slope in &slopes {
            let mut rms_44k = 0.0_f32;
            let mut rms_96k = 0.0_f32;

            for &sr in &sample_rates {
                let mut flt = FilterState::new(1_200.0);
                let mut fp = FilterParams::default_filter_1();
                fp.enabled = true;
                fp.mode = mode;
                fp.slope = slope;
                fp.cutoff_hz = 1_200.0;
                fp.resonance = 0.82;
                fp.drive = 0.65;
                fp.aux = 0.55;

                let n = ((sr * 0.03) as usize).max(256);
                let mut sum_sq = 0.0_f64;
                for i in 0..n {
                    let t = i as f32 / sr;
                    let in_l = 0.35 * (std::f32::consts::TAU * 300.0 * t).sin()
                        + 0.20 * (std::f32::consts::TAU * 4_500.0 * t).sin();
                    let in_r = 0.35 * (std::f32::consts::TAU * 300.0 * t + 0.5).sin();
                    let (ol, or) = flt.process_stereo(in_l, in_r, &fp, 60.0, sr, EngineMode::Hq);
                    assert!(
                        ol.is_finite() && or.is_finite(),
                        "Non-finite filter output mode={mode:?} slope={slope:?} sr={sr}"
                    );
                    assert!(
                        ol.abs() < 64.0 && or.abs() < 64.0,
                        "Filter blow-up ({ol}, {or}) mode={mode:?} slope={slope:?} sr={sr}"
                    );
                    if i >= n / 2 {
                        sum_sq += (ol as f64) * (ol as f64);
                    }
                }
                let rms = (sum_sq / (n - n / 2) as f64).sqrt() as f32;
                if (sr - 44_100.0).abs() < 1.0 {
                    rms_44k = rms;
                } else if (sr - 96_000.0).abs() < 1.0 {
                    rms_96k = rms;
                }
            }

            // Classic ZDF LP/HP/BP cutoffs should track consistently between 44.1 kHz and 96 kHz
            if matches!(mode, FilterMode::Lp | FilterMode::Hp | FilterMode::Bp) && rms_44k > 1e-3 {
                let diff_db = (db_from_linear(rms_44k) - db_from_linear(rms_96k)).abs();
                assert!(
                    diff_db < 2.5,
                    "Filter {mode:?} {slope:?} RMS drifted {diff_db:.2} dB between 44.1k and 96k"
                );
            }
        }
    }
}

#[test]
fn torture_inserts_osc_pitch_modes_mod_slams_and_odd_block_splits() {
    let bank = WavetableBank::new();
    let wt = bank.factory_table(FactoryWavetable::BasicShapes);
    let tables = [wt, wt, wt, wt];

    // 1) Sweep every InsertMode & OscPitchMode with sudden ModMatrix min/max slams
    let mut engine = Box::new(SynthEngine::new(48_000.0));
    for mode_idx in 0..=18 {
        let mut params = SynthParams::default();
        params.amp_attack_s = 0.001;
        params.master_limiter = true;
        params.oscs[0].enabled = true;
        params.oscs[0].pitch_mode = if mode_idx % 2 == 0 {
            OscPitchMode::Semitones
        } else {
            OscPitchMode::Ratio
        };
        params.oscs[0].ratio = 2.0;
        params.oscs[0].hz_offset = 45.0;
        params.routing.inserts[0].enabled = true;
        params.routing.inserts[0].mode = InsertMode::from_index(mode_idx);
        params.routing.inserts[0].drive = 0.9;
        params.routing.inserts[0].mix = 1.0;
        params.mod_matrix.slots[0] = ModSlot {
            source: ModMatrixSource::Macro1,
            destination: ModDestination::Filter1Cutoff,
            depth: 1.0,
            bipolar: true,
            enabled: true,
            bypassed: false,
            aux_source: ModMatrixSource::None,
            curve: 0.0,
        };
        params.mod_matrix.slots[1] = ModSlot {
            source: ModMatrixSource::Macro2,
            destination: ModDestination::Osc1Pitch,
            depth: -1.0,
            bipolar: true,
            enabled: true,
            bypassed: false,
            aux_source: ModMatrixSource::None,
            curve: 0.0,
        };

        engine.reset();
        engine.note_on(0, 60, 1.0, None, &params);
        for i in 0..256 {
            params.macros[0] = if i % 32 < 16 { 1.0 } else { 0.0 };
            params.macros[1] = if i % 16 < 8 { 0.0 } else { 1.0 };
            let (l, r) = engine.process_sample_with_tables(&params, tables);
            assert!(
                l.is_finite() && r.is_finite() && l.abs() <= 1.0 && r.abs() <= 1.0,
                "Insert {mode_idx} / slam failed at sample {i}: ({l}, {r})"
            );
        }
    }

    // 2) Verify odd/tiny block splits (1, 7, 64, 513) produce identical deterministic output
    let total_samples = 1_200usize;
    let mut base_params = SynthParams::default();
    base_params.amp_attack_s = 0.005;
    base_params.amp_decay_s = 0.1;
    base_params.amp_sustain = 0.8;
    base_params.amp_release_s = 0.05;
    base_params.fx_bus_a.slots[0] = FxSlotParams {
        fx_type: FxType::Chorus,
        enabled: true,
        params: FxSlotParams::default_params_for_type(FxType::Chorus),
        eq_params: FxSlotParams::default_eq_params_for_type(FxType::Chorus),
        ..FxSlotParams::init_patch_slot(0)
    };

    let render_chunked = |chunk_size: usize| -> Vec<(f32, f32)> {
        let mut eng = Box::new(SynthEngine::new(48_000.0));
        eng.prepare_fx_for_params(&base_params);
        eng.note_on(0, 60, 0.9, None, &base_params);
        let mut out = Vec::with_capacity(total_samples);
        let mut rendered = 0;
        while rendered < total_samples {
            let n = chunk_size.min(total_samples - rendered);
            for _ in 0..n {
                out.push(eng.process_sample_with_tables(&base_params, tables));
            }
            rendered += n;
        }
        out
    };

    let ref_out = render_chunked(total_samples);
    for &chunk in &[1usize, 7, 64, 513] {
        let chunked = render_chunked(chunk);
        let max_diff = ref_out
            .iter()
            .zip(chunked.iter())
            .map(|(&(al, ar), &(bl, br))| (al - bl).abs().max((ar - br).abs()))
            .fold(0.0_f32, f32::max);
        assert!(
            max_diff < 1e-6,
            "Block split chunk={chunk} diverged from contiguous render by {max_diff}"
        );
    }
}

struct ScenarioStats {
    active_rms_db: f32,
    peak: f32,
    dc_mean: f32,
    tail_peak_db: f32,
}

fn run_scenario(params: &SynthParams, sr: f32, note_len_s: f32, tail_len_s: f32) -> ScenarioStats {
    let bank = WavetableBank::new();
    let wt = bank.factory_table(FactoryWavetable::BasicShapes);
    let tables = [wt, wt, wt, wt];

    let mut engine = Box::new(SynthEngine::new(sr));
    engine.prepare_fx_for_params(params);
    let active_samples = (sr * note_len_s) as usize;
    let tail_samples = (sr * tail_len_s) as usize;

    engine.note_on(0, 48, 0.9, Some(1), params);

    let mut active_sq = 0.0_f64;
    let mut peak = 0.0_f32;
    let mut dc_sum = 0.0_f64;
    let mut dc_count = 0usize;
    let dc_window_start = active_samples / 2;

    for i in 0..active_samples {
        let (l, r) = engine.process_sample_with_tables(params, tables);
        assert!(
            l.is_finite() && r.is_finite(),
            "Non-finite active sample at {i}"
        );
        let m = 0.5 * (l + r);
        active_sq += (l as f64) * (l as f64) + (r as f64) * (r as f64);
        peak = peak.max(l.abs()).max(r.abs());
        if i >= dc_window_start {
            dc_sum += m as f64;
            dc_count += 1;
        }
    }

    engine.note_off(0, 48, Some(1), params);

    let mut tail_end_peak = 0.0_f32;
    let tail_check_start = (tail_samples * 9) / 10;
    for i in 0..tail_samples {
        let (l, r) = engine.process_sample_with_tables(params, tables);
        assert!(
            l.is_finite() && r.is_finite(),
            "Non-finite tail sample at {i}"
        );
        let m = 0.5 * (l + r);
        dc_sum += m as f64;
        dc_count += 1;
        peak = peak.max(l.abs()).max(r.abs());
        if i >= tail_check_start {
            tail_end_peak = tail_end_peak.max(l.abs()).max(r.abs());
        }
    }

    let active_rms = (active_sq / (2.0 * active_samples as f64)).sqrt() as f32;
    let dc_mean = (dc_sum / dc_count.max(1) as f64).abs() as f32;

    ScenarioStats {
        active_rms_db: db_from_linear(active_rms),
        peak,
        dc_mean,
        tail_peak_db: db_from_linear(tail_end_peak),
    }
}

#[test]
fn deterministic_scenarios_cross_platform_invariants() {
    let sr = 48_000.0_f32;

    // 1. Init Patch (default wavetable + ENV 1 -> Bus A)
    let mut init_p = SynthParams::default();
    init_p.dc_blocker = true;
    init_p.master_limiter = true;
    init_p.amp_attack_s = 0.005;
    init_p.amp_release_s = 0.08;
    let init_stats = run_scenario(&init_p, sr, 0.35, 0.45);
    assert!(
        (-28.0..=-4.0).contains(&init_stats.active_rms_db),
        "Init RMS out of bounds: {:.2} dBFS",
        init_stats.active_rms_db
    );
    assert!(
        init_stats.peak <= 0.995,
        "Init safety limiter exceeded: {}",
        init_stats.peak
    );
    assert!(
        init_stats.dc_mean < 1e-3,
        "Init DC drift: {}",
        init_stats.dc_mean
    );
    assert!(
        init_stats.tail_peak_db < -96.0,
        "Init tail did not decay: {:.1} dBFS",
        init_stats.tail_peak_db
    );

    // 2. Cross-FM / Mod Matrix Patch (OSC 2 -> OSC 1 PM + MODSEQ 1 + ENV 2)
    let mut fm_p = init_p;
    fm_p.oscs[1].enabled = true;
    fm_p.oscs[1].level = 0.0;
    fm_p.oscs[1].pitch_mode = OscPitchMode::Ratio;
    fm_p.oscs[1].ratio = 2.0;
    fm_p.oscs[0].mod_source = ModSource::Osc2;
    fm_p.oscs[0].mod_depth = 0.45;
    fm_p.modseqs[0].mode = ModSeqMode::Performer;
    fm_p.modseqs[0].retrig = ModSeqRetrig::PolyVoice;
    fm_p.mod_matrix.slots[0] = ModSlot {
        source: ModMatrixSource::ModSeq1,
        destination: ModDestination::Osc1ModDepth,
        depth: 0.35,
        bipolar: false,
        enabled: true,
        bypassed: false,
        aux_source: ModMatrixSource::None,
        curve: 0.0,
    };
    fm_p.mod_matrix.slots[1] = ModSlot {
        source: ModMatrixSource::Env2,
        destination: ModDestination::Filter1Cutoff,
        depth: 0.5,
        bipolar: false,
        enabled: true,
        bypassed: false,
        aux_source: ModMatrixSource::None,
        curve: 0.0,
    };
    let fm_stats = run_scenario(&fm_p, sr, 0.35, 0.45);
    assert!(
        (-30.0..=-3.0).contains(&fm_stats.active_rms_db),
        "Cross-FM RMS out of bounds: {:.2} dBFS",
        fm_stats.active_rms_db
    );
    assert!(fm_stats.peak <= 0.995);
    assert!(
        fm_stats.dc_mean < 1e-3,
        "Cross-FM DC drift: {}",
        fm_stats.dc_mean
    );
    assert!(fm_stats.tail_peak_db < -96.0);

    // 3. Serial Bus A -> Bus B + Sub Bus Split
    let mut serial_p = init_p;
    serial_p.fx_bus_topology = FxBusTopology::Serial;
    serial_p.routing.filter_routing = FilterRouting::Parallel;
    serial_p.routing.filter_out_bus = [FilterBusDest::BusA, FilterBusDest::BusB];
    serial_p.oscs[1].enabled = true;
    serial_p.oscs[1].level = 0.6;
    serial_p.routing.osc_direct[1] = DirectDest::Sub;
    serial_p.sub_bus.enabled = true;
    serial_p.sub_bus.lp_cutoff_hz = 140.0;
    serial_p.sub_bus.clip_drive_db = 4.0;
    serial_p.fx_bus_a = FxBusParams::default();
    serial_p.fx_bus_a.slots[0] = FxSlotParams {
        fx_type: FxType::Distortion,
        enabled: true,
        params: FxSlotParams::default_params_for_type(FxType::Distortion),
        ..FxSlotParams::init_patch_slot(0)
    };
    serial_p.fx_bus_b = FxBusParams::default();
    serial_p.fx_bus_b.slots[0] = FxSlotParams {
        fx_type: FxType::Filter,
        enabled: true,
        params: FxSlotParams::default_params_for_type(FxType::Filter),
        ..FxSlotParams::init_patch_slot(0)
    };
    let serial_stats = run_scenario(&serial_p, sr, 0.35, 0.50);
    assert!(
        (-28.0..=-2.0).contains(&serial_stats.active_rms_db),
        "Serial/Sub RMS out of bounds: {:.2} dBFS",
        serial_stats.active_rms_db
    );
    assert!(serial_stats.peak <= 0.995);
    assert!(
        serial_stats.dc_mean < 1e-3,
        "Serial/Sub DC drift: {}",
        serial_stats.dc_mean
    );
    assert!(serial_stats.tail_peak_db < -96.0);

    // 4. Tube Res. high feedback + LFO warble + filter enabled + dirty LFO
    let mut tube_p = init_p;
    tube_p.fx_bus_a.slots[0] = FxSlotParams {
        fx_type: FxType::TubeResonator,
        enabled: true,
        params: FxSlotParams::default_params_for_type(FxType::TubeResonator),
        eq_params: [0.65, 0.45, 0.35, 1.0],
        tube_res_filter_enabled: true,
        tube_res_dirty_lfo: true,
        ..FxSlotParams::init_patch_slot(0)
    };
    let tube_stats = run_scenario(&tube_p, sr, 0.35, 0.75);
    assert!(
        (-30.0..=-1.0).contains(&tube_stats.active_rms_db),
        "Tube Res. RMS out of bounds: {:.2} dBFS",
        tube_stats.active_rms_db
    );
    assert!(tube_stats.peak <= 0.995);
    assert!(
        tube_stats.dc_mean < 2e-3,
        "Tube Res. DC drift: {}",
        tube_stats.dc_mean
    );
    assert!(
        tube_stats.tail_peak_db < -72.0,
        "Tube Res. tail hung at {:.1} dBFS",
        tube_stats.tail_peak_db
    );
}
