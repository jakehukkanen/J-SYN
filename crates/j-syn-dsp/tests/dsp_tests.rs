use std::path::PathBuf;

use j_syn_dsp::{
    AdsrStage, AmpAdsr, EngineMode, ModSource, ModType, PolyMode, SynthEngine, SynthParams,
    WarpMode, Wavetable, WavetableBank, FRAME_LEN, MAX_VOICES, NUM_MIP_LEVELS,
};

#[test]
fn factory_wavetables_have_valid_frames_and_mips() {
    let bank = WavetableBank::new();
    for wt in &bank.factory {
        assert!(!wt.raw_frames.is_empty());
        assert_eq!(wt.mip_maps.len(), NUM_MIP_LEVELS);
        for level in &wt.mip_maps {
            assert_eq!(level.len(), wt.num_frames);
        }
        // Check non-zero energy
        let energy: f32 = wt.raw_frames[0].iter().map(|s| s * s).sum();
        assert!(energy > 10.0, "Factory table {} has too little energy", wt.name);
    }
}

#[test]
fn engine_2007_and_hq_produce_clean_audio() {
    let bank = WavetableBank::new();
    for mode in [EngineMode::Vintage2007, EngineMode::Hq] {
        let mut engine = SynthEngine::new(48000.0);
        let mut params = SynthParams::default();
        params.engine_mode = mode;

        engine.note_on(0, 60, 0.9, None, &params);
        let mut peak = 0.0_f32;
        for _ in 0..2048 {
            let (l, r) = engine.process_sample(&params, &bank);
            assert!(l.is_finite() && r.is_finite());
            peak = peak.max(l.abs()).max(r.abs());
        }
        assert!(peak > 0.05, "Expected audible output in {:?}", mode);
    }
}

#[test]
fn pre_fader_cross_pm_and_feedback_stability() {
    let bank = WavetableBank::new();
    let mut engine = SynthEngine::new(44100.0);
    let mut params = SynthParams::default();

    // OSC 1 is silent (level = 0.0) but enabled, modulating OSC 2 via PM
    params.oscs[0].level = 0.0;
    params.oscs[0].mod_source = ModSource::Osc2; // backward mod 2 -> 1
    params.oscs[0].mod_type = ModType::Pm;
    params.oscs[0].mod_depth = 0.9;

    params.oscs[1].level = 0.8;
    params.oscs[1].mod_source = ModSource::Osc1; // forward mod 1 -> 2
    params.oscs[1].mod_type = ModType::Pm;
    params.oscs[1].mod_depth = 0.85;

    // Self-modulation on OSC 3
    params.oscs[2].level = 0.5;
    params.oscs[2].mod_source = ModSource::Osc3;
    params.oscs[2].mod_type = ModType::Pm;
    params.oscs[2].mod_depth = 1.0;

    engine.note_on(0, 57, 1.0, None, &params);
    let mut peak = 0.0_f32;
    for _ in 0..4096 {
        let (l, r) = engine.process_sample(&params, &bank);
        assert!(l.is_finite() && r.is_finite());
        assert!(l.abs() < 4.0 && r.abs() < 4.0, "Feedback blew up: {l}, {r}");
        peak = peak.max(l.abs()).max(r.abs());
    }
    assert!(peak > 0.05);
}

#[test]
fn polyphonic_16_voices_and_click_free_stealing() {
    let bank = WavetableBank::new();
    let mut engine = SynthEngine::new(44100.0);
    let params = SynthParams::default();

    // Trigger 72 notes on 64-voice engine
    for i in 0..72u8 {
        let n = 36 + (i % 60);
        engine.note_on(0, n, 0.8, None, &params);
        for _ in 0..32 {
            let (l, r) = engine.process_sample(&params, &bank);
            assert!(l.is_finite() && r.is_finite());
        }
    }
    assert_eq!(engine.active_voice_count(), MAX_VOICES);
    // Tail voices should be active from stealing
    assert!(engine.tail_voices.iter().any(|tv| tv.active));

    // Process 10 ms so 3 ms tail voices finish cleanly
    for _ in 0..441 {
        let _ = engine.process_sample(&params, &bank);
    }
    assert!(!engine.tail_voices.iter().any(|tv| tv.active));
}

#[test]
fn mono_legato_preserves_single_voice_and_glides() {
    let bank = WavetableBank::new();
    let mut engine = SynthEngine::new(44100.0);
    let mut params = SynthParams::default();
    params.poly_mode = PolyMode::MonoLegato;
    params.glide_ms = 50.0;

    engine.note_on(0, 60, 0.8, None, &params);
    for _ in 0..1000 {
        let _ = engine.process_sample(&params, &bank);
    }
    assert_eq!(engine.active_voice_count(), 1);

    engine.note_on(0, 72, 0.8, None, &params);
    assert_eq!(engine.active_voice_count(), 1);
    assert!((engine.voices[0].target_midi_note - 72.0).abs() < 1e-4);
    // Mid-glide should be between 60 and 72
    for _ in 0..200 {
        let _ = engine.process_sample(&params, &bank);
    }
    assert!(engine.voices[0].current_midi_note > 60.1);
    assert!(engine.voices[0].current_midi_note < 71.9);

    // Release 72 should glide back to 60 without releasing envelope
    engine.note_off(0, 72, None, &params);
    assert!(!engine.voices[0].releasing);
    assert!((engine.voices[0].target_midi_note - 60.0).abs() < 1e-4);
}

#[test]
fn adsr_envelope_lifecycle() {
    let mut env = AmpAdsr::new();
    assert_eq!(env.stage(), AdsrStage::Idle);

    env.reset_and_trigger();
    assert_eq!(env.stage(), AdsrStage::Attack);

    for _ in 0..500 {
        env.process(44100.0, 0.005, 0.02, 0.5, 0.02);
    }
    assert!(env.level() > 0.5);

    env.release();
    assert_eq!(env.stage(), AdsrStage::Release);
    for _ in 0..5000 {
        env.process(44100.0, 0.005, 0.02, 0.5, 0.02);
    }
    assert_eq!(env.stage(), AdsrStage::Idle);
    assert_eq!(env.level(), 0.0);
}

#[test]
fn warp_modes_produce_finite_audio() {
    let bank = WavetableBank::new();
    for mode in [
        WarpMode::None,
        WarpMode::BendMinus,
        WarpMode::BendPlus,
        WarpMode::Spectrum,
    ] {
        let mut engine = SynthEngine::new(48000.0);
        let mut params = SynthParams::default();
        params.oscs[0].warp_mode = mode;
        params.oscs[0].warp_amount = 0.75;
        params.oscs[0].unison = 4;
        engine.note_on(0, 64, 0.9, None, &params);
        for _ in 0..512 {
            let (l, r) = engine.process_sample(&params, &bank);
            assert!(l.is_finite() && r.is_finite());
        }
    }
}

#[test]
fn wav_file_roundtrip_loads_2048_sample_frames() {
    let tmp_path: PathBuf = std::env::temp_dir().join("j_syn_test_wt.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44100,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    {
        let mut writer = hound::WavWriter::create(&tmp_path, spec).unwrap();
        for f in 0..2 {
            for i in 0..FRAME_LEN {
                let phase = i as f32 / FRAME_LEN as f32;
                let s = if f == 0 {
                    (std::f32::consts::TAU * phase).sin()
                } else {
                    2.0 * phase - 1.0
                };
                writer.write_sample(s).unwrap();
            }
        }
        writer.finalize().unwrap();
    }

    let wt = Wavetable::load_from_wav(&tmp_path).unwrap();
    assert_eq!(wt.num_frames, 2);
    assert_eq!(wt.mip_maps.len(), NUM_MIP_LEVELS);
    let _ = std::fs::remove_file(&tmp_path);
}

#[test]
fn default_amp_sustain_is_100_percent() {
    let params = SynthParams::default();
    assert!((params.amp_sustain - 1.0).abs() < 1e-6);
}

#[test]
fn phase_180_deg_cancels_0_deg_and_updates_realtime() {
    let bank = WavetableBank::new();
    bank.select_factory(1, j_syn_dsp::FactoryWavetable::BasicShapes);
    let mut engine = SynthEngine::new(48000.0);
    let mut params = SynthParams::default();
    params.amp_attack_s = 0.001;
    params.amp_sustain = 1.0;

    // OSC 1 at 0 deg, OSC 2 at 180 deg on identical pure sine (BasicShapes wt_pos = 0.0)
    params.oscs[0].wt_pos = 0.0;
    params.oscs[0].level = 0.8;
    params.oscs[0].pan = 0.0;
    params.oscs[0].retrig = true;
    params.oscs[0].phase_offset = (0.0_f32 / 360.0).rem_euclid(1.0).fract();

    params.oscs[1].wt_pos = 0.0;
    params.oscs[1].level = 0.8;
    params.oscs[1].pan = 0.0;
    params.oscs[1].retrig = true;
    params.oscs[1].phase_offset = (180.0_f32 / 360.0).rem_euclid(1.0).fract();

    engine.note_on(0, 60, 1.0, None, &params);
    let mut cancel_peak = 0.0_f32;
    for _ in 0..512 {
        let (l, r) = engine.process_sample(&params, &bank);
        cancel_peak = cancel_peak.max(l.abs()).max(r.abs());
    }
    assert!(
        cancel_peak < 1e-4,
        "Expected 0° + 180° sine phase cancellation, got peak {cancel_peak}"
    );

    // Turning PHASE in real time without a new NoteOn should un-cancel immediately
    // (expected amplitude ~0.198 with constant-power pan * level=0.8 * headroom=0.35 * -3dB master)
    params.oscs[1].phase_offset = 0.0;
    let mut active_peak = 0.0_f32;
    for _ in 0..512 {
        let (l, r) = engine.process_sample(&params, &bank);
        active_peak = active_peak.max(l.abs()).max(r.abs());
    }
    assert!(
        active_peak > 0.15,
        "Expected real-time PHASE knob movement to un-cancel waveform, got {active_peak}"
    );
}

#[test]
fn retrig_off_preserves_and_randomizes_phases() {
    let bank = WavetableBank::new();
    let mut engine = SynthEngine::new(44100.0);
    let mut params = SynthParams::default();
    params.oscs[0].retrig = false;
    params.oscs[0].unison = 4;

    engine.note_on(0, 60, 1.0, None, &params);
    let phases_v0 = engine.voices[0].oscillators[0].phases;
    assert!(
        phases_v0[0] > 0.0 && phases_v0[0] < 1.0,
        "Expected randomized non-zero phase when retrig=false, got {:?}",
        phases_v0[0]
    );
    assert!(
        (phases_v0[0] - phases_v0[1]).abs() > 1e-4 && (phases_v0[1] - phases_v0[2]).abs() > 1e-4,
        "Expected distinct unison phases when retrig=false, got {:?}",
        phases_v0
    );

    for _ in 0..128 {
        let _ = engine.process_sample(&params, &bank);
    }

    // Triggering a second voice should receive a different randomized phase set
    engine.note_on(0, 64, 1.0, None, &params);
    let phases_v1 = engine.voices[1].oscillators[0].phases;
    assert!(
        (phases_v0[0] - phases_v1[0]).abs() > 1e-4,
        "Expected second voice to get independent randomized phases"
    );
}

#[test]
fn cross_mod_immediate_and_bidirectional() {
    let bank = WavetableBank::new();

    // Case A: Backward modulation (OSC 1 modulated by OSC 2, B >= A) while OSC 2 level == 0.0
    {
        let mut engine_unmod = SynthEngine::new(48000.0);
        let mut engine_mod = SynthEngine::new(48000.0);

        let mut params_base = SynthParams::default();
        params_base.amp_attack_s = 0.001;
        params_base.oscs[0].level = 0.8;
        params_base.oscs[1].level = 0.0; // Silent modulator
        params_base.oscs[1].pitch_semitones = 7.0;

        engine_unmod.note_on(0, 60, 1.0, None, &params_base);
        engine_mod.note_on(0, 60, 1.0, None, &params_base);

        // Advance past attack while unmodulated
        for _ in 0..256 {
            let _ = engine_unmod.process_sample(&params_base, &bank);
            let _ = engine_mod.process_sample(&params_base, &bank);
        }

        // Now enable OSC 2 -> OSC 1 PM immediately on the already-playing voice
        let mut params_mod = params_base;
        params_mod.oscs[0].mod_source = ModSource::Osc2;
        params_mod.oscs[0].mod_type = ModType::Pm;
        params_mod.oscs[0].mod_depth = 0.9;

        let mut diff_energy = 0.0_f32;
        for _ in 0..512 {
            let (lu, _) = engine_unmod.process_sample(&params_base, &bank);
            let (lm, rm) = engine_mod.process_sample(&params_mod, &bank);
            assert!(lm.is_finite() && rm.is_finite());
            diff_energy += (lm - lu).powi(2);
        }
        assert!(
            diff_energy > 0.25,
            "Expected immediate backward PM (OSC 2 -> OSC 1) even at OSC 2 level=0, diff={diff_energy}"
        );
    }

    // Case B: Forward modulation (OSC 2 modulated by OSC 1, B < A) while OSC 1 level == 0.0
    {
        let mut engine_unmod = SynthEngine::new(48000.0);
        let mut engine_mod = SynthEngine::new(48000.0);

        let mut params_base = SynthParams::default();
        params_base.amp_attack_s = 0.001;
        params_base.oscs[0].level = 0.0; // Silent modulator
        params_base.oscs[0].pitch_semitones = 7.0;
        params_base.oscs[1].level = 0.8;

        engine_unmod.note_on(0, 60, 1.0, None, &params_base);
        engine_mod.note_on(0, 60, 1.0, None, &params_base);

        for _ in 0..256 {
            let _ = engine_unmod.process_sample(&params_base, &bank);
            let _ = engine_mod.process_sample(&params_base, &bank);
        }

        let mut params_mod = params_base;
        params_mod.oscs[1].mod_source = ModSource::Osc1;
        params_mod.oscs[1].mod_type = ModType::Rm;
        params_mod.oscs[1].mod_depth = 0.9;

        let mut diff_energy = 0.0_f32;
        for _ in 0..512 {
            let (lu, _) = engine_unmod.process_sample(&params_base, &bank);
            let (lm, rm) = engine_mod.process_sample(&params_mod, &bank);
            assert!(lm.is_finite() && rm.is_finite());
            diff_energy += (lm - lu).powi(2);
        }
        assert!(
            diff_energy > 0.25,
            "Expected immediate forward RM (OSC 1 -> OSC 2) even at OSC 1 level=0, diff={diff_energy}"
        );
    }
}

#[test]
fn bend_warp_is_symmetric_around_center() {
    use j_syn_dsp::warp_phase_bend;
    for mode in [WarpMode::BendPlus, WarpMode::BendMinus] {
        let center = warp_phase_bend(0.5, mode, 0.75);
        assert!(
            (center - 0.5).abs() < 1e-6,
            "Center phase 0.5 should remain 0.5 for {mode:?}, got {center}"
        );

        for &d in &[0.05_f32, 0.15, 0.25, 0.4] {
            let left = warp_phase_bend(0.5 - d, mode, 0.75);
            let right = warp_phase_bend(0.5 + d, mode, 0.75);
            assert!(
                ((left + right) - 1.0).abs() < 1e-5,
                "Expected symmetric warp around 0.5 for {mode:?}: left={left}, right={right}"
            );
            if mode == WarpMode::BendPlus {
                // Pinches toward 0.5 (closer to 0.5 than unwarped 0.5 - d)
                assert!(left > 0.5 - d && right < 0.5 + d);
            } else {
                // Expands toward 0.0 and 1.0 (farther from 0.5 than unwarped 0.5 - d)
                assert!(left < 0.5 - d && right > 0.5 + d);
            }
        }
    }
}

#[test]
fn mod_order_pm_then_warp_differs_from_warp_then_pm() {
    use j_syn_dsp::{ModOrder, ModSource, ModType};
    let bank = WavetableBank::new();

    let mut base_params = SynthParams::default();
    base_params.oscs[0].level = 0.75;
    base_params.oscs[0].level = 0.0;
    base_params.oscs[1].level = 0.75;
    base_params.oscs[1].level = 1.0;
    base_params.oscs[1].mod_source = ModSource::Osc1;
    base_params.oscs[1].mod_type = ModType::Pm;
    base_params.oscs[1].mod_depth = 0.65;
    base_params.oscs[1].warp_mode = WarpMode::BendPlus;
    base_params.oscs[1].warp_amount = 0.75;

    let mut params_a = base_params;
    params_a.oscs[1].mod_order = ModOrder::PmThenWarp;
    let mut params_b = base_params;
    params_b.oscs[1].mod_order = ModOrder::WarpThenPm;

    let mut eng_a = SynthEngine::new(48_000.0);
    let mut eng_b = SynthEngine::new(48_000.0);
    eng_a.note_on(0, 60, 1.0, None, &params_a);
    eng_b.note_on(0, 60, 1.0, None, &params_b);

    let mut diff_energy = 0.0_f32;
    for _ in 0..512 {
        let (la, ra) = eng_a.process_sample(&params_a, &bank);
        let (lb, rb) = eng_b.process_sample(&params_b, &bank);
        assert!(la.is_finite() && ra.is_finite() && lb.is_finite() && rb.is_finite());
        diff_energy += (la - lb).powi(2);
    }
    assert!(
        diff_energy > 0.1,
        "Expected PM -> Warp and Warp -> PM to produce distinct waveforms, diff={diff_energy}"
    );
}

#[test]
fn noise_generator_white_pink_brown_and_stereo_decorrelation() {
    use j_syn_dsp::{NoiseType, SynthEngine, SynthParams, WavetableBank};

    let bank = WavetableBank::new();
    let sample_rate = 48_000.0;

    let render_noise = |noise_type: NoiseType, color: f32, stereo: f32| {
        let mut params = SynthParams::default();
        for osc in &mut params.oscs {
            osc.level = 0.0;
        }
        params.amp_attack_s = 0.001;
        params.filters[0].enabled = false;
        params.noise.enabled = true;
        params.noise.noise_type = noise_type;
        params.noise.color = color;
        params.noise.stereo = stereo;
        params.noise.level = 1.0;

        let mut eng = SynthEngine::new(sample_rate);
        eng.note_on(0, 60, 0.8, None, &params);
        let mut out = Vec::with_capacity(2048);
        for _ in 0..2048 {
            let (l, r) = eng.process_sample(&params, &bank);
            assert!(l.is_finite() && r.is_finite());
            out.push((l, r));
        }
        out
    };

    for nt in [NoiseType::White, NoiseType::Pink, NoiseType::Brown] {
        let out = render_noise(nt, 0.0, 0.5);
        let rms = (out[256..].iter().map(|(l, _)| l * l).sum::<f32>() / (out.len() - 256) as f32)
            .sqrt();
        assert!(
            rms > 0.01,
            "Expected audible RMS for noise type {nt:?}, got {rms}"
        );
    }

    let mono_out = render_noise(NoiseType::White, 0.0, 0.0);
    for &(l, r) in &mono_out {
        assert!(
            (l - r).abs() < 1e-6,
            "Expected identical L/R when stereo=0, got l={l}, r={r}"
        );
    }

    let stereo_out = render_noise(NoiseType::White, 0.0, 1.0);
    let stereo_diff: f32 = stereo_out[256..]
        .iter()
        .map(|(l, r)| (l - r).powi(2))
        .sum();
    assert!(
        stereo_diff > 0.05,
        "Expected decorrelated L/R when stereo=1.0, diff={stereo_diff}"
    );

    let dark = render_noise(NoiseType::White, -1.0, 0.0);
    let bright = render_noise(NoiseType::White, 1.0, 0.0);
    let hf_ratio = |buf: &[(f32, f32)]| -> f32 {
        let slice = &buf[256..];
        let e_tot: f32 = slice.iter().map(|(l, _)| l * l).sum::<f32>().max(1e-6);
        let e_diff: f32 = slice.windows(2).map(|w| (w[1].0 - w[0].0).powi(2)).sum();
        e_diff / e_tot
    };
    let dark_hf = hf_ratio(&dark);
    let bright_hf = hf_ratio(&bright);
    assert!(
        bright_hf > dark_hf * 2.0,
        "Expected bright color (+1) to have higher HF ratio than dark (-1): bright={bright_hf}, dark={dark_hf}"
    );
}

#[test]
fn dual_multimode_filters_all_topologies_stable_and_audible() {
    use j_syn_dsp::{
        EngineMode, FilterMode, FilterParams, FilterSlope, SynthEngine, SynthParams, WavetableBank,
    };

    let bank = WavetableBank::new();
    let sample_rate = 48_000.0;
    let modes = [
        (FilterMode::Lp, FilterSlope::Db12),
        (FilterMode::Lp, FilterSlope::Db24),
        (FilterMode::Bp, FilterSlope::Db12),
        (FilterMode::Hp, FilterSlope::Db12),
        (FilterMode::Scream, FilterSlope::Db12),
        (FilterMode::Daft, FilterSlope::Db12),
        (FilterMode::Comb, FilterSlope::Db12),
    ];

    for engine_mode in [EngineMode::Vintage2007, EngineMode::Hq] {
        for (mode, slope) in modes {
            // Musical high-resonance / driven check
            let mut params = SynthParams::default();
            params.engine_mode = engine_mode;
            params.amp_attack_s = 0.002;
            params.oscs[0].wt_pos = 0.75;
            params.filters[0] = FilterParams {
                enabled: true,
                mode,
                slope,
                cutoff_hz: 900.0,
                resonance: 0.85,
                drive: 0.75,
                keytrack: 0.5,
                aux: 0.75,
            };
            params.filters[1].enabled = false;

            let mut eng = SynthEngine::new(sample_rate);
            eng.note_on(0, 60, 0.8, None, &params);
            let mut energy = 0.0_f32;
            for _ in 0..2048 {
                let (l, r) = eng.process_sample(&params, &bank);
                assert!(
                    l.is_finite() && r.is_finite() && l.abs() < 4.0 && r.abs() < 4.0,
                    "Filter unstable for {mode:?} ({engine_mode:?}): l={l}, r={r}"
                );
                energy += l * l;
            }
            let rms = (energy / 2048.0).sqrt();
            assert!(
                rms > 0.01,
                "Expected audible signal through {mode:?} ({engine_mode:?}), rms={rms}"
            );

            // Extreme self-oscillation stability check
            params.filters[0].resonance = 1.0;
            params.filters[0].drive = 1.0;
            let mut eng_ext = SynthEngine::new(sample_rate);
            eng_ext.note_on(0, 60, 0.8, None, &params);
            for _ in 0..2048 {
                let (l, r) = eng_ext.process_sample(&params, &bank);
                assert!(
                    l.is_finite() && r.is_finite() && l.abs() < 4.0 && r.abs() < 4.0,
                    "Self-oscillation blew up for {mode:?} ({engine_mode:?}): l={l}, r={r}"
                );
            }
        }
    }
}

#[test]
fn serial_and_parallel_filter_routing_and_source_balance() {
    use j_syn_dsp::{
        FilterMode, FilterParams, FilterRouting, FilterSlope, SynthEngine, SynthParams,
        WavetableBank,
    };

    let bank = WavetableBank::new();
    let sample_rate = 48_000.0;

    let render = |routing: FilterRouting, osc1_bal: f32, flt_mix: f32| {
        let mut params = SynthParams::default();
        params.master_gain = 1.0;
        params.amp_attack_s = 0.002;
        params.oscs[0].level = 1.0;
        params.oscs[0].wt_pos = 0.75; // rich harmonic waveform
        params.filters[0] = FilterParams {
            enabled: true,
            mode: FilterMode::Lp,
            slope: FilterSlope::Db24,
            cutoff_hz: 400.0,
            resonance: 0.2,
            drive: 0.0,
            keytrack: 0.0,
            aux: 1.0,
        };
        params.filters[1] = FilterParams {
            enabled: true,
            mode: FilterMode::Hp,
            slope: FilterSlope::Db12,
            cutoff_hz: 4_000.0,
            resonance: 0.2,
            drive: 0.0,
            keytrack: 0.0,
            aux: 1.0,
        };
        params.routing.filter_routing = routing;
        params.routing.osc_filter_balance[0] = osc1_bal;
        params.routing.filter_mix = flt_mix;

        let mut eng = SynthEngine::new(sample_rate);
        eng.note_on(0, 60, 0.8, None, &params);
        let mut buf = Vec::with_capacity(2048);
        for _ in 0..2048 {
            let (l, r) = eng.process_sample(&params, &bank);
            assert!(l.is_finite() && r.is_finite());
            buf.push(l);
        }
        buf
    };

    // Serial F1 (LP400) -> F2 (HP4000) with mix=F2 leaves almost no energy
    let serial = render(FilterRouting::Serial, -1.0, 1.0);
    // Parallel F1 + F2 with centered source send and mix passes lows + highs
    let parallel = render(FilterRouting::Parallel, 0.0, 0.0);

    let rms = |b: &[f32]| (b[256..].iter().map(|s| s * s).sum::<f32>() / (b.len() - 256) as f32).sqrt();
    let serial_rms = rms(&serial);
    let parallel_rms = rms(&parallel);
    let diff: f32 = serial.iter().zip(parallel.iter()).map(|(a, b)| (a - b).powi(2)).sum();

    assert!(
        parallel_rms > serial_rms * 2.0 && diff > 0.01,
        "Expected Parallel ({parallel_rms}) and Serial ({serial_rms}) routing to differ strongly (diff={diff})"
    );

    // Verify per-source balance routes OSC1 exclusively to F1 vs F2 in Parallel
    let to_f1_only = render(FilterRouting::Parallel, -1.0, -1.0);
    let muted_by_send_to_f2 = render(FilterRouting::Parallel, 1.0, -1.0);
    let f1_rms = rms(&to_f1_only);
    let muted_rms = rms(&muted_by_send_to_f2);
    assert!(
        f1_rms > 0.005 && muted_rms < f1_rms * 0.1,
        "Expected osc_filter_balance=-1.0 routed to F1 to be audible ({f1_rms}) vs balance=+1.0 with mix=-1.0 muted ({muted_rms})"
    );
}

#[test]
fn insert_waveshaper_modes_positions_and_voice_feedback() {
    use j_syn_dsp::{
        FilterMode, FilterParams, FilterSlope, InsertMode, InsertPosition, SynthEngine,
        SynthParams, WavetableBank,
    };

    let bank = WavetableBank::new();
    let sample_rate = 48_000.0;

    let render = |mode: InsertMode, pos: InsertPosition, feedback: f32| {
        let mut params = SynthParams::default();
        params.amp_attack_s = 0.002;
        params.oscs[0].wt_pos = 0.0;
        params.filters[0] = FilterParams {
            enabled: true,
            mode: FilterMode::Lp,
            slope: FilterSlope::Db12,
            cutoff_hz: 1_800.0,
            resonance: 0.25,
            drive: 0.0,
            keytrack: 0.0,
            aux: 1.0,
        };
        params.filters[1] = FilterParams {
            enabled: true,
            mode: FilterMode::Lp,
            slope: FilterSlope::Db12,
            cutoff_hz: 4_000.0,
            resonance: 0.15,
            drive: 0.0,
            keytrack: 0.0,
            aux: 1.0,
        };
        params.routing.inserts[0].mode = mode;
        params.routing.inserts[0].position = pos;
        params.routing.inserts[0].drive = 0.8;
        params.routing.inserts[0].mix = 1.0;
        params.routing.feedback_amount = feedback;

        let mut eng = SynthEngine::new(sample_rate);
        eng.note_on(0, 60, 0.8, None, &params);
        let mut buf = Vec::with_capacity(2048);
        for _ in 0..2048 {
            let (l, r) = eng.process_sample(&params, &bank);
            assert!(
                l.is_finite() && r.is_finite() && l.abs() < 4.0 && r.abs() < 4.0,
                "Insert/feedback unstable for {mode:?} @ {pos:?}: l={l}, r={r}"
            );
            buf.push(l);
        }
        buf
    };

    let baseline = render(InsertMode::Off, InsertPosition::BetweenFilters, 0.0);

    for mode in [InsertMode::SineShaper, InsertMode::Parabolic, InsertMode::Bitcrush] {
        for pos in [
            InsertPosition::PreFilter,
            InsertPosition::BetweenFilters,
            InsertPosition::PostFilter,
        ] {
            let out = render(mode, pos, 0.85);
            let rms = (out[256..].iter().map(|s| s * s).sum::<f32>() / (out.len() - 256) as f32)
                .sqrt();
            let diff: f32 = out.iter().zip(baseline.iter()).map(|(a, b)| (a - b).powi(2)).sum();
            assert!(
                rms > 0.01 && diff > 0.01,
                "Expected audible waveshaping for {mode:?} @ {pos:?}: rms={rms}, diff={diff}"
            );
        }
    }
}

#[test]
fn direct_out_bypasses_filters_inserts_and_feedback() {
    use j_syn_dsp::{
        DirectDest, FilterMode, FilterParams, FilterSlope, InsertMode, InsertPosition, SynthEngine,
        SynthParams, WavetableBank,
    };

    let bank = WavetableBank::new();
    let sample_rate = 48_000.0;

    let render = |osc_direct: bool, filters_on: bool| {
        let mut params = SynthParams::default();
        params.master_gain = 1.0;
        params.amp_attack_s = 0.001;
        params.oscs[0].level = 1.0;
        params.oscs[0].wt_pos = 0.0;
        params.routing.osc_direct[0] = if osc_direct {
            DirectDest::BusA
        } else {
            DirectDest::Off
        };
        params.filters[0] = FilterParams {
            enabled: filters_on,
            mode: FilterMode::Lp,
            slope: FilterSlope::Db24,
            cutoff_hz: 80.0, // heavy attenuation when routed through filter
            resonance: 0.0,
            drive: 0.0,
            keytrack: 0.0,
            aux: 1.0,
        };
        params.routing.inserts[0].mode = if filters_on {
            InsertMode::SoftClip
        } else {
            InsertMode::Off
        };
        params.routing.inserts[0].position = InsertPosition::PreFilter;
        params.routing.inserts[0].drive = 1.0;

        let mut eng = SynthEngine::new(sample_rate);
        eng.note_on(0, 84, 0.8, None, &params);
        let mut buf = Vec::with_capacity(2048);
        for _ in 0..2048 {
            let (l, r) = eng.process_sample(&params, &bank);
            assert!(l.is_finite() && r.is_finite());
            buf.push(l);
        }
        buf
    };

    let direct_with_heavy_filter = render(true, true);
    let direct_without_filter = render(true, false);
    let filtered = render(false, true);

    let max_diff: f32 = direct_with_heavy_filter
        .iter()
        .zip(direct_without_filter.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    assert!(
        max_diff < 1e-6,
        "Direct out should completely bypass filters/inserts, max_diff={max_diff}"
    );

    let rms = |b: &[f32]| (b[1024..].iter().map(|s| s * s).sum::<f32>() / (b.len() - 1024) as f32).sqrt();
    let direct_rms = rms(&direct_with_heavy_filter);
    let filtered_rms = rms(&filtered);
    assert!(
        direct_rms > filtered_rms * 10.0,
        "Direct out ({direct_rms}) should remain full amplitude even when Filter 1 is at 80 Hz (filtered={filtered_rms})"
    );
}

#[test]
fn osc_keytrack_off_locks_pitch_to_c3() {
    use j_syn_dsp::{SynthEngine, SynthParams, WavetableBank};

    let bank = WavetableBank::new();
    let sample_rate = 48_000.0;

    let render_note = |note: u8, keytrack: bool| {
        let mut params = SynthParams::default();
        params.amp_attack_s = 0.001;
        params.filters[0].enabled = false;
        params.oscs[0].keytrack = keytrack;
        params.oscs[0].pitch_semitones = 3.0; // verify tuning offset still applies

        let mut eng = SynthEngine::new(sample_rate);
        eng.note_on(0, note, 0.8, None, &params);
        let mut buf = Vec::with_capacity(1024);
        for _ in 0..1024 {
            let (l, _) = eng.process_sample(&params, &bank);
            buf.push(l);
        }
        buf
    };

    let c3 = render_note(60, false);
    let c5_locked = render_note(84, false);
    let c5_tracked = render_note(84, true);

    let diff_locked: f32 = c3
        .iter()
        .zip(c5_locked.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    assert!(
        diff_locked < 1e-5,
        "Expected keytrack=false on C5 to match C3 identically, max_diff={diff_locked}"
    );

    let diff_tracked: f32 = c3
        .iter()
        .zip(c5_tracked.iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum();
    assert!(
        diff_tracked > 0.5,
        "Expected keytrack=true on C5 to differ from C3, diff={diff_tracked}"
    );
}

#[test]
fn multi_filters_and_dual_inserts_are_stable_and_audible() {
    use j_syn_dsp::{
        evaluate_filter_response_db, EngineMode, FilterMode, FilterParams, FilterSlope, InsertMode,
        InsertPosition, SynthEngine, SynthParams, WavetableBank,
    };

    let bank = WavetableBank::new();
    let sample_rate = 48_000.0;

    for mode in [
        FilterMode::HpPeak,
        FilterMode::HpNotch,
        FilterMode::LpNotch,
        FilterMode::LpPeak,
    ] {
        let fp = FilterParams {
            enabled: true,
            mode,
            slope: FilterSlope::Db12,
            cutoff_hz: 600.0,
            resonance: 0.75,
            drive: 0.5,
            keytrack: 0.0,
            aux: 0.65,
        };
        assert!(evaluate_filter_response_db(&fp, 1_000.0).is_finite());

        let mut params = SynthParams::default();
        params.engine_mode = EngineMode::Hq;
        params.amp_attack_s = 0.002;
        params.oscs[0].wt_pos = 0.75;
        params.filters[0] = fp;
        params.routing.inserts[0].mode = InsertMode::SoftClip;
        params.routing.inserts[0].position = InsertPosition::PreFilter;
        params.routing.inserts[0].drive = 0.7;
        params.routing.inserts[1].mode = InsertMode::Parabolic;
        params.routing.inserts[1].position = InsertPosition::PostFilter;
        params.routing.inserts[1].drive = 0.6;

        let mut eng = SynthEngine::new(sample_rate);
        eng.note_on(0, 60, 0.8, None, &params);
        let mut energy = 0.0_f32;
        for _ in 0..2048 {
            let (l, r) = eng.process_sample(&params, &bank);
            assert!(
                l.is_finite() && r.is_finite() && l.abs() < 4.0 && r.abs() < 4.0,
                "Multi-filter/dual-insert unstable for {mode:?}: l={l}, r={r}"
            );
            energy += l * l;
        }
        let rms = (energy / 2048.0).sqrt();
        assert!(rms > 0.01, "Expected audible output for {mode:?}, rms={rms}");
    }
}


#[test]
fn filter_slopes_6_to_48_db_per_octave_audio_and_response() {
    use j_syn_dsp::{
        evaluate_filter_response_db, FilterMode, FilterParams, FilterSlope, SynthEngine,
        SynthParams, WavetableBank,
    };

    let bank = WavetableBank::new();
    let sample_rate = 48_000.0;
    let slopes = [
        FilterSlope::Db6,
        FilterSlope::Db12,
        FilterSlope::Db24,
        FilterSlope::Db36,
        FilterSlope::Db48,
    ];

    for mode in [FilterMode::Lp, FilterMode::Bp, FilterMode::Hp] {
        let mut prev_stopband_db = f32::INFINITY;
        let stopband_freq = match mode {
            FilterMode::Lp => 8_000.0,
            FilterMode::Hp => 125.0,
            FilterMode::Bp => 8_000.0,
            _ => 8_000.0,
        };

        for slope in slopes {
            let fp = FilterParams {
                enabled: true,
                mode,
                slope,
                cutoff_hz: 1_000.0,
                resonance: 0.35,
                drive: 0.0,
                keytrack: 0.0,
                aux: 1.0,
            };
            let db = evaluate_filter_response_db(&fp, stopband_freq);
            assert!(
                db.is_finite() && db <= prev_stopband_db + 0.01,
                "Expected steeper stopband attenuation for {mode:?} {slope:?}: db={db}, prev={prev_stopband_db}"
            );
            prev_stopband_db = db;

            // Verify audio stability at high resonance (resonance = 1.0)
            let mut params = SynthParams::default();
            params.amp_attack_s = 0.002;
            params.oscs[0].wt_pos = 0.75;
            params.filters[0] = FilterParams {
                resonance: 1.0,
                ..fp
            };
            let mut eng = SynthEngine::new(sample_rate);
            eng.note_on(0, 60, 0.8, None, &params);
            for _ in 0..2048 {
                let (l, r) = eng.process_sample(&params, &bank);
                assert!(
                    l.is_finite() && r.is_finite() && l.abs() < 4.0 && r.abs() < 4.0,
                    "Filter slope unstable at max resonance for {mode:?} {slope:?}: l={l}, r={r}"
                );
            }
        }
    }
}


#[test]
fn tube_mech_and_cascaded_insert_shapers_per_voice_isolation() {
    use j_syn_dsp::{InsertMode, InsertPosition, SynthEngine, SynthParams, WavetableBank};

    let bank = WavetableBank::new();
    let sample_rate = 48_000.0;

    for mode in [
        InsertMode::Tube,
        InsertMode::Mech,
        InsertMode::TubeToMech,
        InsertMode::MechToTube,
    ] {
        let mut params = SynthParams::default();
        params.master_gain = 1.0;
        params.amp_attack_s = 0.002;
        params.oscs[0].wt_pos = 0.25;
        params.routing.inserts[0].mode = mode;
        params.routing.inserts[0].position = InsertPosition::PostFilter;
        params.routing.inserts[0].drive = 0.85;
        params.routing.inserts[0].mix = 1.0;

        // Render note A alone, note B alone, and A+B chord together
        let mut eng_a = SynthEngine::new(sample_rate);
        eng_a.note_on(0, 48, 0.8, None, &params);

        let mut eng_b = SynthEngine::new(sample_rate);
        eng_b.note_on(0, 55, 0.8, None, &params);

        let mut eng_chord = SynthEngine::new(sample_rate);
        eng_chord.note_on(0, 48, 0.8, None, &params);
        eng_chord.note_on(0, 55, 0.8, None, &params);

        let mut peak = 0.0_f32;
        let mut max_intermod_err = 0.0_f32;
        for _ in 0..1024 {
            let (la, ra) = eng_a.process_sample(&params, &bank);
            let (lb, rb) = eng_b.process_sample(&params, &bank);
            let (lc, rc) = eng_chord.process_sample(&params, &bank);

            assert!(
                lc.is_finite() && rc.is_finite() && lc.abs() < 2.5 && rc.abs() < 2.5,
                "Insert mode {mode:?} blew up: lc={lc}, rc={rc}"
            );
            peak = peak.max(la.abs()).max(ra.abs());
            max_intermod_err = max_intermod_err
                .max((lc - (la + lb)).abs())
                .max((rc - (ra + rb)).abs());
        }

        assert!(peak > 0.05 && peak <= 1.2, "Unexpected peak for {mode:?}: {peak}");
        assert!(
            max_intermod_err < 1e-4,
            "Expected zero polyphonic intermodulation for {mode:?}, err={max_intermod_err}"
        );
    }
}

#[test]
fn mod_matrix_32_slots_envelopes_and_modseq_modulate_destinations() {
    use j_syn_dsp::{
        evaluate_adsr_preview, evaluate_modseq_curve, ModDestination, ModMatrixSource,
        ModSeqPlayMode, ModSeqRetrig, ModSeqShapePreset, ModSeqSyncMode, ModSlot, SynthEngine,
        SynthParams, WavetableBank, MAX_MODSEQ_STEPS, NUM_MOD_SLOTS,
    };

    let bank = WavetableBank::new();
    let sample_rate = 48_000.0;

    // 1. Verify all 32 modulation matrix slots evaluate simultaneously + bipolar/unipolar math
    let mut params = SynthParams::default();
    params.master_gain = 1.0;
    params.amp_attack_s = 0.001;
    params.filters[0].enabled = false;
    params.oscs[0].level = 0.25;
    for i in 0..NUM_MOD_SLOTS {
        params.mod_matrix.slots[i] = ModSlot {
            source: ModMatrixSource::ModWheel,
            destination: ModDestination::Osc1Level,
            depth: 0.01,
            bipolar: false,
            enabled: true,
            ..ModSlot::default()
        };
    }
    let mut eng_lo = SynthEngine::new(sample_rate);
    eng_lo.set_mod_wheel(0.0);
    eng_lo.note_on(0, 60, 1.0, None, &params);
    let mut eng_hi = SynthEngine::new(sample_rate);
    eng_hi.set_mod_wheel(1.0);
    eng_hi.note_on(0, 60, 1.0, None, &params);

    let rms_of = |e: &mut SynthEngine, p: &SynthParams, n: usize| {
        let mut s = 0.0_f32;
        for _ in 0..n {
            let (l, r) = e.process_sample(p, &bank);
            assert!(l.is_finite() && r.is_finite());
            s += l * l + r * r;
        }
        (s / (2.0 * n as f32)).sqrt()
    };
    let rms_lo = rms_of(&mut eng_lo, &params, 512);
    let rms_hi = rms_of(&mut eng_hi, &params, 512);
    assert!(
        rms_hi > rms_lo * 2.0,
        "Expected 32 ModWheel slots (+0.32 total level) to boost RMS substantially: lo={rms_lo}, hi={rms_hi}"
    );
    // Telemetry should reflect summed dest_offset (~0.32) and MW level (~1.0)
    let tel = eng_hi.telemetry();
    assert!((tel.dest_offset(ModDestination::Osc1Level) - 0.32).abs() < 1e-3);
    assert!((tel.source_level(ModMatrixSource::ModWheel) - 1.0).abs() < 1e-3);

    // 2. Bipolar Aftertouch + Env2 + Velocity + Keytrack checks
    let mut bp_params = SynthParams::default();
    bp_params.filters[0].enabled = false;
    bp_params.oscs[0].level = 0.5;
    bp_params.mod_matrix.slots[0] = ModSlot {
        source: ModMatrixSource::Aftertouch,
        destination: ModDestination::Osc1Level,
        depth: 0.4,
        bipolar: true,
        enabled: true,
        ..ModSlot::default()
    };
    bp_params.mod_matrix.slots[1] = ModSlot {
        source: ModMatrixSource::Env2,
        destination: ModDestination::Filter1Cutoff,
        depth: 0.5,
        bipolar: false,
        enabled: true,
        ..ModSlot::default()
    };
    bp_params.mod_matrix.slots[2] = ModSlot {
        source: ModMatrixSource::Velocity,
        destination: ModDestination::Osc1WtPos,
        depth: 0.5,
        bipolar: false,
        enabled: true,
        ..ModSlot::default()
    };
    bp_params.mod_matrix.slots[3] = ModSlot {
        source: ModMatrixSource::Keytrack,
        destination: ModDestination::Osc1Pitch,
        depth: 0.25,
        bipolar: false,
        enabled: true,
        ..ModSlot::default()
    };
    bp_params.mod_envs[0].attack_s = 0.001;
    bp_params.mod_envs[0].sustain = 1.0;

    let mut eng_bp = SynthEngine::new(sample_rate);
    eng_bp.set_aftertouch(0.0); // bipolar -> -1.0 -> -0.4 offset
    eng_bp.note_on(0, 72, 0.8, None, &bp_params);
    for _ in 0..256 {
        eng_bp.process_sample(&bp_params, &bank);
    }
    let tel_bp = eng_bp.telemetry();
    assert!((tel_bp.dest_offset(ModDestination::Osc1Level) - (-0.4)).abs() < 1e-3);
    assert!(tel_bp.dest_offset(ModDestination::Filter1Cutoff) > 0.2);
    assert!((tel_bp.dest_offset(ModDestination::Osc1WtPos) - 0.4).abs() < 1e-3);
    assert!(tel_bp.dest_offset(ModDestination::Osc1Pitch) > 0.1);
    assert!(evaluate_adsr_preview(&bp_params.mod_envs[0], 0.5).is_finite());

    // 3. ModSeq curve presets, tension, Loop / OneShot / PingPong, PolyVoice & MonoGlobal
    let mut seq_params = SynthParams::default();
    ModSeqShapePreset::RampUp.apply_to_sequence(&mut seq_params.modseq_steps.seqs[0], 8);
    seq_params.modseq_steps.seqs[0][0].tension = 0.5;
    seq_params.modseqs[0].sync_mode = ModSeqSyncMode::Free;
    seq_params.modseqs[0].rate_norm = 0.767; // ~10.0 Hz
    seq_params.modseqs[0].play_mode = ModSeqPlayMode::OneShot;
    seq_params.modseqs[0].retrig = ModSeqRetrig::MonoGlobal;
    seq_params.modseqs[0].smooth = 0.0;
    let c0 = evaluate_modseq_curve(&seq_params.modseq_steps.seqs[0], 8, 0.0);
    let c1 = evaluate_modseq_curve(&seq_params.modseq_steps.seqs[0], 8, 0.99);
    assert!(c1 > c0 + 0.7, "RampUp curve expected to rise: c0={c0}, c1={c1}");

    let mut ms_steps = seq_params.modseq_steps.seqs[0];
    ModSeqShapePreset::SampleAndHold.apply_to_sequence(&mut ms_steps, MAX_MODSEQ_STEPS);
    assert!(ms_steps[0].start_level == ms_steps[0].end_level);

    seq_params.mod_matrix.slots[0] = ModSlot {
        source: ModMatrixSource::ModSeq1,
        destination: ModDestination::Osc1Warp,
        depth: 0.75,
        bipolar: false,
        enabled: true,
        ..ModSlot::default()
    };
    let mut eng_ms = SynthEngine::new(sample_rate);
    eng_ms.note_on(0, 60, 0.9, None, &seq_params);
    for _ in 0..8000 {
        let (l, r) = eng_ms.process_sample(&seq_params, &bank);
        assert!(l.is_finite() && r.is_finite());
    }
    let tel_ms = eng_ms.telemetry();
    // At 10 Hz over 0.166 s, OneShot should be completed near phase 1.0 (offset ~0.75)
    assert!(tel_ms.modseq_phase(0) > 0.95);
    assert!(tel_ms.dest_offset(ModDestination::Osc1Warp) > 0.6);

    // PingPong reverses direction cleanly without exceeding [0, 1)
    seq_params.modseqs[1].play_mode = ModSeqPlayMode::PingPong;
    seq_params.modseqs[1].retrig = ModSeqRetrig::PolyVoice;
    for _ in 0..4000 {
        eng_ms.process_sample(&seq_params, &bank);
    }
    let p1 = eng_ms.telemetry().modseq_phase(1);
    assert!((0.0..=1.0).contains(&p1));
}

#[test]
fn test_modseq_stamps_and_mseg_curve() {
    use j_syn_dsp::{
        evaluate_modseq_curve, evaluate_modseq_curve_with_swing, evaluate_mseg_curve,
        evaluate_step_shape, LfoPreset, ModDestination, ModMatrixSource, ModSeqMode, ModSeqStep,
        ModSeqStepShape, ModSeqSyncMode, ModSlot, MsegCurve, MsegNode, MAX_MODSEQ_STEPS,
        MAX_MSEG_NODES,
    };

    // 1. Verify all 18 MODSEQ palette curves (single-cycle + 2x/3x subdivisions) evaluate cleanly
    assert_eq!(ModSeqStepShape::PALETTE.len(), 18);
    for &shape in &ModSeqStepShape::PALETTE {
        let step = ModSeqStep::stamped(shape, 0.8, 0.0);
        for k in 0..=20 {
            let t = k as f32 / 20.0;
            let v = evaluate_step_shape(step, t);
            assert!(v.is_finite() && (0.0..=1.0).contains(&v));
        }
    }
    let ramp2x = ModSeqStep::stamped(ModSeqStepShape::RampDown2x, 0.8, 0.0);
    assert!(evaluate_step_shape(ramp2x, 0.1) > evaluate_step_shape(ramp2x, 0.4));
    assert!(evaluate_step_shape(ramp2x, 0.6) > evaluate_step_shape(ramp2x, 0.9));
    let sqr2x = ModSeqStep::stamped(ModSeqStepShape::Square2x, 0.8, 0.0);
    assert!((evaluate_step_shape(sqr2x, 0.1) - 0.8).abs() < 1e-4);
    assert!(evaluate_step_shape(sqr2x, 0.35).abs() < 1e-4);
    assert!((evaluate_step_shape(sqr2x, 0.6) - 0.8).abs() < 1e-4);
    assert!(evaluate_step_shape(sqr2x, 0.85).abs() < 1e-4);
    let tri3x = ModSeqStep::stamped(ModSeqStepShape::TriSine3x, 0.8, 0.0);
    assert!(evaluate_step_shape(tri3x, 1.0 / 6.0) > 0.75);
    assert!(evaluate_step_shape(tri3x, 3.0 / 6.0) > 0.75);
    assert!(evaluate_step_shape(tri3x, 5.0 / 6.0) > 0.75);

    let mut steps = [ModSeqStep::default(); MAX_MODSEQ_STEPS];
    for (idx, &shape) in ModSeqStepShape::PALETTE
        .iter()
        .take(MAX_MODSEQ_STEPS)
        .enumerate()
    {
        steps[idx] = ModSeqStep::stamped(shape, 0.8, 0.0);
    }

    // Default straight evaluation vs experimental alternating groove swing
    let arches = [ModSeqStep::stamped(ModSeqStepShape::SineArchUp, 1.0, 0.0); MAX_MODSEQ_STEPS];
    // Centered at t = 0.5 of step 0 and step 1 (1.0 - centered)
    let p0_straight = evaluate_modseq_curve(&arches, 8, (0.5) / 8.0);
    let p1_straight = evaluate_modseq_curve(&arches, 8, (1.5) / 8.0);
    assert!((p0_straight - 1.0).abs() < 1e-4 && (p1_straight - 1.0).abs() < 1e-4);
    let p0_early = evaluate_modseq_curve(&arches, 8, (0.28) / 8.0);
    let p1_early = evaluate_modseq_curve(&arches, 8, (1.28) / 8.0);
    assert!((p0_early - p1_early).abs() < 1e-4);
    // Experimental swing warps even steps early while odd steps stay centered
    let p0_swing_early = evaluate_modseq_curve_with_swing(&arches, 8, (0.28) / 8.0, true);
    let p1_swing_early = evaluate_modseq_curve_with_swing(&arches, 8, (1.28) / 8.0, true);
    assert!(
        p0_swing_early > p1_swing_early + 0.15,
        "Experimental groove swing should shift step 0 peak early relative to step 1"
    );
    // Step 0 = Flat (0.8), Step 7 = SineArchUp (~0 at start, ~0.8 at mid), Step 10 = Pulse (0.8 first half, 0.0 second half)
    assert!((evaluate_modseq_curve(&steps, 16, 0.5 / 16.0) - 0.8).abs() < 1e-4);
    assert!(evaluate_modseq_curve(&steps, 16, 7.01 / 16.0) < 0.06);
    assert!((evaluate_modseq_curve(&steps, 16, 7.5 / 16.0) - 0.8).abs() < 1e-3);
    assert!((evaluate_modseq_curve(&steps, 16, 10.25 / 16.0) - 0.8).abs() < 1e-4);
    assert!(evaluate_modseq_curve(&steps, 16, 10.75 / 16.0).abs() < 1e-4);

    // 2. Verify all 9 LFO presets (including Sine) & multi-node MSEG curve + zero-allocation evaluation
    assert_eq!(LfoPreset::ALL.len(), 9);
    for &preset in &LfoPreset::ALL {
        let curve = preset.to_mseg();
        for k in 0..=20 {
            let v = evaluate_mseg_curve(&curve, k as f32 / 20.0);
            assert!(v.is_finite() && (0.0..=1.0).contains(&v));
        }
    }
    let sine_curve = LfoPreset::Sine.to_mseg();
    assert!((evaluate_mseg_curve(&sine_curve, 0.0) - 0.5).abs() < 1e-4);
    assert!((evaluate_mseg_curve(&sine_curve, 0.25) - 1.0).abs() < 1e-4);
    assert!((evaluate_mseg_curve(&sine_curve, 0.5) - 0.5).abs() < 1e-4);
    assert!((evaluate_mseg_curve(&sine_curve, 0.75) - 0.0).abs() < 1e-4);
    assert!((evaluate_mseg_curve(&sine_curve, 1.0) - 0.5).abs() < 1e-4);
    let mid_rise = evaluate_mseg_curve(&sine_curve, 0.125);
    assert!(mid_rise > 0.75 && mid_rise < 0.95, "Sine 1/8th phase expected ~0.85, got {mid_rise}");

    let mut mseg = MsegCurve {
        nodes: [MsegNode::default(); MAX_MSEG_NODES],
        num_nodes: 3,
    };
    mseg.nodes[0] = MsegNode {
        time_x: 0.0,
        level_y: 0.1,
        tension: 0.0,
    };
    mseg.nodes[1] = MsegNode {
        time_x: 0.25,
        level_y: 1.0,
        tension: 0.0,
    };
    mseg.nodes[2] = MsegNode {
        time_x: 1.0,
        level_y: 0.0,
        tension: 0.0,
    };
    assert!((evaluate_mseg_curve(&mseg, 0.0) - 0.1).abs() < 1e-5);
    assert!((evaluate_mseg_curve(&mseg, 0.25) - 1.0).abs() < 1e-5);
    assert!((evaluate_mseg_curve(&mseg, 0.625) - 0.5).abs() < 1e-5);

    let bank = WavetableBank::new();
    let mut params = SynthParams::default();
    params.modseqs[0].mode = ModSeqMode::Mseg;
    params.modseqs[0].sync_mode = ModSeqSyncMode::Free;
    params.modseqs[0].rate_norm = 0.634; // ~4.0 Hz
    params.modseqs[0].smooth = 0.0;
    params.modseq_steps.msegs[0] = mseg;
    params.mod_matrix.slots[0] = ModSlot {
        source: ModMatrixSource::ModSeq1,
        destination: ModDestination::Filter1Cutoff,
        depth: 0.5,
        bipolar: false,
        enabled: true,
        ..ModSlot::default()
    };

    let mut engine = SynthEngine::new(48_000.0);
    engine.note_on(0, 60, 1.0, None, &params);
    for _ in 0..3000 {
        let (l, r) = engine.process_sample(&params, &bank);
        assert!(l.is_finite() && r.is_finite());
    }
    let tel = engine.telemetry();
    assert!(tel.modseq_phase(0) > 0.2 && tel.modseq_phase(0) < 0.35);
    assert!(tel.dest_offset(ModDestination::Filter1Cutoff) > 0.35);
}

#[test]
fn osc_pitch_range_minus_48_to_plus_48_and_auto_sleep() {
    let bank = WavetableBank::new();
    let sample_rate = 48_000.0;

    let render = |note: u8, pitch_semitones: f32| {
        let mut params = SynthParams::default();
        params.amp_attack_s = 0.001;
        params.filters[0].enabled = false;
        params.filters[1].enabled = false;
        params.oscs[0].keytrack = true;
        params.oscs[0].pitch_semitones = pitch_semitones;

        let mut eng = SynthEngine::new(sample_rate);
        eng.note_on(0, note, 0.9, None, &params);
        // Skip attack transient so steady-state periodic waveform is measured cleanly
        for _ in 0..512 {
            let (l, r) = eng.process_sample(&params, &bank);
            assert!(l.is_finite() && r.is_finite());
        }
        let mut buf = Vec::with_capacity(32_768);
        for _ in 0..32_768 {
            let (l, r) = eng.process_sample(&params, &bank);
            assert!(l.is_finite() && r.is_finite());
            buf.push(l);
        }
        buf
    };

    let dft_mag = |buf: &[f32], freq_hz: f32| -> f32 {
        let step = core::f32::consts::TAU * freq_hz / sample_rate;
        let mut re = 0.0_f32;
        let mut im = 0.0_f32;
        for (i, &s) in buf.iter().enumerate() {
            let angle = step * i as f32;
            re += s * angle.cos();
            im -= s * angle.sin();
        }
        (re * re + im * im).sqrt() / buf.len() as f32
    };

    // Note 57 = A3 (220 Hz): -48 st = 13.75 Hz, 0 st = 220 Hz, +48 st = 3520 Hz
    let buf_m4 = render(57, -48.0);
    let buf_0 = render(57, 0.0);
    let buf_p4 = render(57, 48.0);

    let peak_m4 = buf_m4.iter().fold(0.0_f32, |m, s| m.max(s.abs()));
    let peak_0 = buf_0.iter().fold(0.0_f32, |m, s| m.max(s.abs()));
    let peak_p4 = buf_p4.iter().fold(0.0_f32, |m, s| m.max(s.abs()));
    assert!(peak_m4 > 0.05 && peak_0 > 0.05 && peak_p4 > 0.05);

    let m4_fund = dft_mag(&buf_m4, 13.75);
    let m4_oct = dft_mag(&buf_m4, 27.5);
    let m4_base = dft_mag(&buf_m4, 220.0);
    assert!(
        m4_fund > m4_oct * 1.5 && m4_fund > m4_base * 3.0,
        "Expected -48 st fundamental at 13.75 Hz: fund={m4_fund}, 27.5Hz={m4_oct}, 220Hz={m4_base}"
    );

    let z_fund = dft_mag(&buf_0, 220.0);
    let z_sub = dft_mag(&buf_0, 110.0);
    let z_low = dft_mag(&buf_0, 13.75);
    assert!(
        z_fund > z_sub * 3.0 && z_fund > z_low * 3.0,
        "Expected 0 st fundamental at 220 Hz: fund={z_fund}, 110Hz={z_sub}, 13.75Hz={z_low}"
    );

    let p4_fund = dft_mag(&buf_p4, 3520.0);
    let p4_sub = dft_mag(&buf_p4, 1760.0);
    let p4_base = dft_mag(&buf_p4, 220.0);
    assert!(
        p4_fund > p4_sub * 3.0 && p4_fund > p4_base * 3.0,
        "Expected +48 st fundamental at 3520 Hz: fund={p4_fund}, 1760Hz={p4_sub}, 220Hz={p4_base}"
    );

    // Fractional cents (+0.50 st = +50 cents -> 220 * 2^(0.5/12) Hz)
    let buf_cents = render(57, 0.50);
    let cents_target_hz = 220.0 * 2.0_f32.powf(0.5 / 12.0);
    let cents_fund = dft_mag(&buf_cents, cents_target_hz);
    let cents_orig = dft_mag(&buf_cents, 220.0);
    assert!(
        cents_fund > cents_orig * 3.0,
        "Expected +0.50 st fundamental at {cents_target_hz} Hz: target={cents_fund}, 220Hz={cents_orig}"
    );

    // Exact pitch equivalence with +/- 48 MIDI semitones
    let equiv_m4 = render(12, 0.0);
    let ref_m4 = render(60, -48.0);
    let diff_m4: f32 = equiv_m4
        .iter()
        .zip(ref_m4.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    assert!(diff_m4 < 1e-4, "note 60 @ -48 st must match note 12 @ 0 st, diff={diff_m4}");

    let equiv_p4 = render(96, 0.0);
    let ref_p4 = render(48, 48.0);
    let diff_p4: f32 = equiv_p4
        .iter()
        .zip(ref_p4.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    assert!(diff_p4 < 1e-4, "note 48 @ +48 st must match note 96 @ 0 st, diff={diff_p4}");

    // Auto-sleep: level == 0.0 on all oscs yields exact silence
    let mut sleep_params = SynthParams::default();
    sleep_params.amp_attack_s = 0.001;
    sleep_params.oscs[0].level = 0.0;
    let mut sleep_eng = SynthEngine::new(sample_rate);
    sleep_eng.note_on(0, 60, 0.9, None, &sleep_params);
    for _ in 0..256 {
        let (l, r) = sleep_eng.process_sample(&sleep_params, &bank);
        assert_eq!(l, 0.0);
        assert_eq!(r, 0.0);
    }

    // Auto-sleep wakes cross-modulator chain (OSC 1 <- OSC 2 <- OSC 3) even with OSC 2 & 3 at level == 0.0
    let mut chain_base = SynthParams::default();
    chain_base.amp_attack_s = 0.001;
    chain_base.filters[0].enabled = false;
    chain_base.filters[1].enabled = false;
    chain_base.oscs[0].level = 0.8;
    chain_base.oscs[0].mod_source = ModSource::Osc2;
    chain_base.oscs[0].mod_depth = 0.7;
    chain_base.oscs[1].level = 0.0;
    chain_base.oscs[1].mod_source = ModSource::Osc3;
    chain_base.oscs[1].mod_depth = 0.8;
    chain_base.oscs[2].level = 0.0;
    chain_base.oscs[2].pitch_semitones = 7.0;

    let mut chain_alt = chain_base;
    chain_alt.oscs[2].pitch_semitones = 19.0;

    let mut e1 = SynthEngine::new(sample_rate);
    let mut e2 = SynthEngine::new(sample_rate);
    e1.note_on(0, 60, 0.9, None, &chain_base);
    e2.note_on(0, 60, 0.9, None, &chain_alt);
    let mut max_chain_diff = 0.0_f32;
    for _ in 0..1024 {
        let (l1, _) = e1.process_sample(&chain_base, &bank);
        let (l2, _) = e2.process_sample(&chain_alt, &bank);
        max_chain_diff = max_chain_diff.max((l1 - l2).abs());
    }
    assert!(
        max_chain_diff > 1e-2,
        "Silent cross-modulator OSC 3 must wake through OSC 2 when OSC 1 uses OSC 2, diff={max_chain_diff}"
    );
}

#[test]
fn mod_matrix_modseq_rate_modulation_free_and_sync() {
    use j_syn_dsp::{
        modseq_division_label, modseq_rate_hz, ModDestination, ModMatrixSource, ModSeqPlayMode,
        ModSeqRetrig, ModSeqSyncMode, ModSlot,
    };

    let bank = WavetableBank::new();
    let sample_rate = 48_000.0;

    assert_eq!(ModDestination::modseq_rate(0), ModDestination::ModSeq1Rate);
    assert_eq!(ModDestination::modseq_rate(3), ModDestination::ModSeq4Rate);
    assert_eq!(ModDestination::ModSeq1Rate.name(), "MS1 RATE");
    assert_eq!(modseq_division_label(ModSeqSyncMode::Straight, 0.35), "1 Bar");
    assert_eq!(modseq_division_label(ModSeqSyncMode::Straight, 0.56), "1/4");
    assert_eq!(modseq_division_label(ModSeqSyncMode::Triplet, 0.5), "1/8T");
    assert_eq!(modseq_division_label(ModSeqSyncMode::Dotted, 0.5), "1/8D");

    // 1. Free mode (PolyVoice): modulating rate_norm +0.25 increases exponential Hz cleanly
    let mut free_params = SynthParams::default();
    free_params.modseqs[0].sync_mode = ModSeqSyncMode::Free;
    free_params.modseqs[0].rate_norm = 0.45;
    free_params.modseqs[0].play_mode = ModSeqPlayMode::Loop;
    free_params.modseqs[0].retrig = ModSeqRetrig::PolyVoice;
    free_params.mod_matrix.slots[0] = ModSlot {
        source: ModMatrixSource::ModWheel,
        destination: ModDestination::ModSeq1Rate,
        depth: 0.25,
        bipolar: false,
        enabled: true,
        ..ModSlot::default()
    };

    let run_phase = |params: &SynthParams, mod_wheel: f32, samples: usize| -> f32 {
        let mut eng = SynthEngine::new(sample_rate);
        eng.set_mod_wheel(mod_wheel);
        eng.note_on(0, 60, 0.9, None, params);
        for _ in 0..samples {
            let (l, r) = eng.process_sample(params, &bank);
            assert!(l.is_finite() && r.is_finite());
        }
        eng.telemetry().modseq_phase(0)
    };

    let base_hz = modseq_rate_hz(ModSeqSyncMode::Free, 0.45, 120.0);
    let mod_hz = modseq_rate_hz(ModSeqSyncMode::Free, 0.70, 120.0);
    let free_base = run_phase(&free_params, 0.0, 2400);
    let free_fast = run_phase(&free_params, 1.0, 2400);
    assert!(
        mod_hz > base_hz * 5.0 && free_fast > free_base * 4.5,
        "Expected Free rate_norm modulation speedup: base={free_base}, fast={free_fast}"
    );

    // 2. Straight sync mode (PolyVoice): modulating rate_norm steps to a faster rhythmic division
    let mut sync_poly = SynthParams::default();
    sync_poly.tempo_bpm = 120.0;
    sync_poly.modseqs[0].sync_mode = ModSeqSyncMode::Straight;
    sync_poly.modseqs[0].rate_norm = 0.15; // 2 Bars
    sync_poly.modseqs[0].play_mode = ModSeqPlayMode::Loop;
    sync_poly.modseqs[0].retrig = ModSeqRetrig::PolyVoice;
    sync_poly.mod_matrix.slots[0] = ModSlot {
        source: ModMatrixSource::ModWheel,
        destination: ModDestination::ModSeq1Rate,
        depth: 0.20, // -> 0.35 (1/4)
        bipolar: false,
        enabled: true,
        ..ModSlot::default()
    };

    let sync_poly_base = run_phase(&sync_poly, 0.0, 4800);
    let sync_poly_fast = run_phase(&sync_poly, 1.0, 4800);
    assert!(
        sync_poly_fast > sync_poly_base * 3.9,
        "Expected Straight sync PolyVoice speedup: base={sync_poly_base}, fast={sync_poly_fast}"
    );

    // 3. Straight sync mode (MonoGlobal): global ModWheel drives global_modseqs[0]
    let mut sync_mono = sync_poly;
    sync_mono.modseqs[0].retrig = ModSeqRetrig::MonoGlobal;
    let sync_mono_base = run_phase(&sync_mono, 0.0, 4800);
    let sync_mono_fast = run_phase(&sync_mono, 1.0, 4800);
    assert!(
        sync_mono_fast > sync_mono_base * 3.9,
        "Expected Straight sync MonoGlobal speedup: base={sync_mono_base}, fast={sync_mono_fast}"
    );
}

#[test]
fn master_fx_rack_8_slots_stacking_and_modulation() {
    use j_syn_dsp::{
        FxSlotParams, FxType, ModDestination, ModMatrixSource, ModSlot, FX_SCOPE_LEN, NUM_FX_SLOTS,
    };

    for slot_idx in 0..NUM_FX_SLOTS {
        let init_slot = FxSlotParams::init_patch_slot(slot_idx);
        assert_eq!(init_slot.fx_type, FxType::Off);
        assert!(init_slot.enabled);
    }
    for fx_ty in [
        FxType::Distortion,
        FxType::Chorus,
        FxType::Delay,
        FxType::Reverb,
        FxType::JSatClipper,
        FxType::Filter,
    ] {
        assert_eq!(FxSlotParams::default_params_for_type(fx_ty)[3], 1.0);
    }
    assert_eq!(ModDestination::fx_param(3, 1), ModDestination::Fx4P2);
    assert!(ModDestination::Fx4P2.is_fx_slot(3));
    assert!(!ModDestination::Fx4P2.is_fx_slot(2));

    let bank = WavetableBank::new();
    let sample_rate = 48_000.0;

    // 1. Full 8-slot chain runs cleanly & records live telemetry scopes
    let mut init_params = SynthParams::default();
    let test_types = [
        FxType::Ott,
        FxType::Distortion,
        FxType::Delay,
        FxType::JSatClipper,
        FxType::Reverb,
        FxType::Chorus,
    ];
    for (idx, &fx_type) in test_types.iter().enumerate() {
        init_params.fx_bus_a.slots[idx] = FxSlotParams {
            fx_type,
            enabled: true,
            stack: 1,
            params: FxSlotParams::default_params_for_type(fx_type),
            eq_params: FxSlotParams::default_eq_params_for_type(fx_type),
            ..FxSlotParams::default()
        };
    }

    let mut eng = SynthEngine::new(sample_rate);
    eng.note_on(0, 57, 1.0, None, &init_params);
    let mut max_abs = 0.0_f32;
    for _ in 0..8_192 {
        let (l, r) = eng.process_sample(&init_params, &bank);
        assert!(l.is_finite() && r.is_finite());
        max_abs = max_abs.max(l.abs()).max(r.abs());
    }
    assert!(max_abs > 0.01 && max_abs <= 2.0, "Unexpected init FX output peak {max_abs}");

    let mut pre_buf = [0.0_f32; FX_SCOPE_LEN];
    let mut post_buf = [0.0_f32; FX_SCOPE_LEN];
    let _ = eng.telemetry().fx_scope(0, 3).read_snapshot(&mut pre_buf, &mut post_buf);
    assert!(pre_buf.iter().any(|&v| v > 0.0), "Expected live FX scope telemetry");

    // 2. Series STACK 4 vs STACK 1 changes output energy and J-SAT ceiling clipping limits post peak
    let run_peak = |stack: u8, ceil_norm: f32, mod_wheel: f32| -> f32 {
        let mut p = SynthParams::default();
        p.fx_bus_a.slots = [FxSlotParams::default(); NUM_FX_SLOTS];
        p.fx_bus_a.slots[0] = FxSlotParams {
            fx_type: FxType::Distortion,
            enabled: true,
            stack,
            params: [0.0, 0.75, 0.5, 1.0],
            ..FxSlotParams::default()
        };
        p.fx_bus_a.slots[1] = FxSlotParams {
            fx_type: FxType::JSatClipper,
            enabled: true,
            stack: 1,
            // Drive +12 dB (0.50), Ceiling configurable, Knee 0% (hard clip), Mix 100%
            params: [0.50, ceil_norm, 0.0, 1.0],
            ..FxSlotParams::default()
        };
        p.mod_matrix.slots[0] = ModSlot {
            source: ModMatrixSource::ModWheel,
            destination: ModDestination::Fx2P2,
            depth: 0.7,
            bipolar: false,
            enabled: true,
            ..ModSlot::default()
        };
        let mut e = SynthEngine::new(sample_rate);
        e.set_mod_wheel(mod_wheel);
        e.note_on(0, 48, 1.0, None, &p);
        let mut pk = 0.0_f32;
        for i in 0..4_096 {
            let (l, r) = e.process_sample(&p, &bank);
            assert!(l.is_finite() && r.is_finite());
            if i > 256 {
                pk = pk.max(l.abs()).max(r.abs());
            }
        }
        pk
    };

    let low_ceil_peak = run_peak(1, 0.15, 0.0);
    let mod_raised_peak = run_peak(1, 0.15, 1.0);
    assert!(
        mod_raised_peak > low_ceil_peak * 1.5,
        "ModWheel -> Fx2P2 (Ceiling) must raise clipped output peak: low={low_ceil_peak}, mod={mod_raised_peak}"
    );

    let stack1_pk = run_peak(1, 1.0, 0.0);
    let stack4_pk = run_peak(4, 1.0, 0.0);
    assert!(
        (stack4_pk - stack1_pk).abs() > 1e-4,
        "Series STACK 4 should alter FX output compared to STACK 1 ({stack1_pk} vs {stack4_pk})"
    );
}

#[test]
fn filter_notch_peak_and_normalized_cutoff_modulation() {
    use j_syn_dsp::filter::{
        cutoff_hz_to_norm, cutoff_norm_to_hz, evaluate_filter_response_db, FilterMode,
        FilterParams,
    };

    // Normalized cutoff curve round-trip check
    for &hz in &[20.0_f32, 120.0, 1000.0, 5000.0, 20_000.0] {
        let norm = cutoff_hz_to_norm(hz);
        let back = cutoff_norm_to_hz(norm);
        assert!(
            (back - hz).abs() < hz * 1e-4 + 1e-3,
            "Cutoff norm round-trip mismatch for {hz}: norm={norm}, back={back}"
        );
    }

    // Notch cuts at cutoff, Peak boosts at cutoff
    let notch_params = FilterParams {
        enabled: true,
        mode: FilterMode::Notch,
        cutoff_hz: 1000.0,
        resonance: 0.6,
        ..FilterParams::default()
    };
    let notch_at_fc = evaluate_filter_response_db(&notch_params, 1000.0);
    let notch_low = evaluate_filter_response_db(&notch_params, 100.0);
    assert!(notch_at_fc < notch_low - 6.0, "Notch response expected deep dip at fc");

    let peak_params = FilterParams {
        enabled: true,
        mode: FilterMode::Peak,
        cutoff_hz: 1000.0,
        resonance: 0.6,
        ..FilterParams::default()
    };
    let peak_at_fc = evaluate_filter_response_db(&peak_params, 1000.0);
    let peak_low = evaluate_filter_response_db(&peak_params, 100.0);
    assert!(peak_at_fc > peak_low + 3.0, "Peak response expected boost at fc");

    let bank = WavetableBank::new();
    for mode in [FilterMode::Notch, FilterMode::Peak] {
        let mut p = SynthParams::default();
        p.filters[0].enabled = true;
        p.filters[0].mode = mode;
        p.filters[0].cutoff_hz = 1200.0;
        p.filters[0].resonance = 0.55;
        let mut eng = SynthEngine::new(48_000.0);
        eng.note_on(0, 60, 1.0, None, &p);
        for _ in 0..2048 {
            let (l, r) = eng.process_sample(&p, &bank);
            assert!(l.is_finite() && r.is_finite());
        }
    }
}

#[test]
fn fx_filter_all_modes_and_macro_mod_sources() {
    use j_syn_dsp::{
        FilterMode, FxFilterSlope, FxSlotParams, FxType, ModDestination, ModMatrixSource, ModSlot,
    };

    let bank = WavetableBank::new();
    // All 15 FxType::Filter modes process cleanly with finite audible output
    for mode_idx in 0..15 {
        let mut p = SynthParams::default();
        p.oscs[0].wt_pos = 0.75; // rich harmonics so HP/BP/Notch/Comb modes are audible
        p.fx_bus_a.slots[0] = FxSlotParams {
            fx_type: FxType::Filter,
            enabled: true,
            stack: 1,
            params: [0.45, 0.4, 0.25, 1.0],
            filter_mode: FilterMode::from_index(mode_idx),
            filter_slope: FxFilterSlope::Db24,
            ..FxSlotParams::default()
        };
        let mut eng = SynthEngine::new(48_000.0);
        eng.note_on(0, 57, 1.0, None, &p);
        let mut peak = 0.0_f32;
        for _ in 0..2048 {
            let (l, r) = eng.process_sample(&p, &bank);
            assert!(l.is_finite() && r.is_finite());
            peak = peak.max(l.abs()).max(r.abs());
        }
        assert!(peak > 0.005, "FxType::Filter mode {mode_idx} produced silence");
    }

    // Macro1..Macro8 modulate destinations cleanly
    assert_eq!(ModMatrixSource::MACROS.len(), 8);
    assert_eq!(ModMatrixSource::Macro1.short_label(), "M1");
    assert_eq!(ModMatrixSource::Macro8.short_label(), "M8");

    let mut mp = SynthParams::default();
    mp.filters[0].enabled = true;
    mp.filters[0].cutoff_hz = 200.0;
    mp.macros[0] = 0.0;
    mp.mod_matrix.slots[0] = ModSlot {
        source: ModMatrixSource::Macro1,
        destination: ModDestination::Filter1Cutoff,
        depth: 0.75,
        bipolar: false,
        enabled: true,
        ..ModSlot::default()
    };

    let run_rms_and_fc = |params: &SynthParams| -> (f32, f32) {
        let mut eng = SynthEngine::new(48_000.0);
        eng.note_on(0, 60, 1.0, None, params);
        let mut sum_sq = 0.0_f32;
        for _ in 0..2048 {
            let (l, r) = eng.process_sample(params, &bank);
            sum_sq += 0.5 * (l * l + r * r);
        }
        ((sum_sq / 2048.0).sqrt(), eng.telemetry().filter_live(0)[0])
    };

    let (rms_low, fc_low) = run_rms_and_fc(&mp);
    mp.macros[0] = 1.0;
    let (rms_high, fc_high) = run_rms_and_fc(&mp);
    assert!(
        rms_high > rms_low * 1.15 && fc_high > fc_low * 10.0,
        "Expected Macro1 -> Filter1Cutoff to brighten output and raise live cutoff telemetry: low=({rms_low}, {fc_low} Hz), high=({rms_high}, {fc_high} Hz)"
    );
}

#[test]
fn mod_matrix_aux_source_curve_and_master_safety_stages() {
    use j_syn_dsp::mod_matrix::{
        ModDestination, ModMatrixSource, ModSlot, NUM_ACTIVE_SOURCES, NUM_MOD_DESTS,
    };

    // Aux modulator + curve shaping + swap_slots
    let mut matrix = j_syn_dsp::ModMatrixState::default();
    matrix.slots[0] = ModSlot {
        source: ModMatrixSource::ModWheel,
        destination: ModDestination::Filter1Cutoff,
        depth: 1.0,
        bipolar: false,
        enabled: true,
        bypassed: false,
        aux_source: ModMatrixSource::Macro1,
        curve: 0.5,
    };
    let mut sources = [0.0_f32; NUM_ACTIVE_SOURCES];
    sources[ModMatrixSource::ModWheel.active_index().unwrap()] = 0.5;
    let mut deltas = [0.0_f32; NUM_MOD_DESTS];
    matrix.evaluate_destinations(&sources, &mut deltas);
    assert!(
        deltas[ModDestination::Filter1Cutoff.index()].abs() < 1e-6,
        "Aux source at 0 should scale modulation to 0"
    );
    sources[ModMatrixSource::Macro1.active_index().unwrap()] = 1.0;
    matrix.evaluate_destinations(&sources, &mut deltas);
    let curved = deltas[ModDestination::Filter1Cutoff.index()];
    assert!(
        curved > 0.0 && curved < 0.5,
        "Positive curve should attenuate mid-range value below linear 0.5, got {curved}"
    );
    matrix.swap_slots(0, 1);
    assert!(matrix.slots[1].enabled && !matrix.slots[0].enabled);

    // Master limiter clamps peaks to <= -0.1 dBFS (~0.9886) and DC blocker removes DC offset
    let bank = WavetableBank::new();
    let mut params = SynthParams::default();
    params.master_gain = 16.0;
    params.dc_blocker = true;
    params.master_limiter = true;
    params.oscs[0].level = 1.0;
    let mut eng = SynthEngine::new(48_000.0);
    eng.note_on(0, 48, 1.0, None, &params);
    let mut max_abs = 0.0_f32;
    for _ in 0..4096 {
        let (l, r) = eng.process_sample(&params, &bank);
        max_abs = max_abs.max(l.abs()).max(r.abs());
    }
    assert!(
        max_abs <= 0.989,
        "Master safety limiter must keep output <= -0.1 dBFS, got {max_abs}"
    );
    let (pk_l, pk_r, rms_l, rms_r, _, _) = eng.telemetry().master_meters();
    assert!(pk_l > 0.0 && pk_r > 0.0 && rms_l > 0.0 && rms_r > 0.0);
}

#[test]
fn filter_bandreject_peakpeak_notchnotch_and_combo_slopes() {
    use j_syn_dsp::{
        evaluate_filter_response_db, FilterMode, FilterParams, FilterSlope, SynthEngine,
        SynthParams, WavetableBank,
    };

    for m in [
        FilterMode::Scream,
        FilterMode::Daft,
        FilterMode::Comb,
        FilterMode::Bandreject,
    ] {
        assert!(m.has_fixed_12db_slope(), "{m:?} should report fixed 12 dB slope");
    }
    for m in [
        FilterMode::Lp,
        FilterMode::Bp,
        FilterMode::Hp,
        FilterMode::Notch,
        FilterMode::Peak,
        FilterMode::HpPeak,
        FilterMode::HpNotch,
        FilterMode::LpNotch,
        FilterMode::LpPeak,
        FilterMode::PeakPeak,
        FilterMode::NotchNotch,
    ] {
        assert!(!m.has_fixed_12db_slope(), "{m:?} should allow variable slope");
    }
    assert!(FilterMode::PeakPeak.is_multi());
    assert!(FilterMode::NotchNotch.is_multi());

    // Bandreject dips at cutoff and widens rejected band as resonance increases
    let br_narrow = FilterParams {
        enabled: true,
        mode: FilterMode::Bandreject,
        cutoff_hz: 1_000.0,
        resonance: 0.1,
        aux: 1.0,
        ..FilterParams::default()
    };
    let br_wide = FilterParams {
        resonance: 0.9,
        ..br_narrow
    };
    assert!(
        evaluate_filter_response_db(&br_narrow, 1_000.0)
            < evaluate_filter_response_db(&br_narrow, 100.0) - 8.0
    );
    assert!(
        evaluate_filter_response_db(&br_wide, 600.0)
            < evaluate_filter_response_db(&br_narrow, 600.0) - 2.0
    );

    // Peak+Peak boosts both peaks; Notch+Notch attenuates both notches (cutoff & freq2)
    let pp = FilterParams {
        enabled: true,
        mode: FilterMode::PeakPeak,
        slope: FilterSlope::Db24,
        cutoff_hz: 400.0,
        resonance: 0.7,
        aux: 0.72,
        ..FilterParams::default()
    };
    let nn = FilterParams {
        mode: FilterMode::NotchNotch,
        ..pp
    };
    let freq2 = j_syn_dsp::filter::aux_to_freq2_hz(pp.aux);
    assert!(evaluate_filter_response_db(&pp, 400.0) > 4.0);
    assert!(evaluate_filter_response_db(&pp, freq2) > 4.0);
    assert!(evaluate_filter_response_db(&nn, 400.0) < -10.0);
    assert!(evaluate_filter_response_db(&nn, freq2) < -10.0);

    // Combo filter modes are affected by FilterSlope in response and audio
    for mode in [
        FilterMode::HpPeak,
        FilterMode::HpNotch,
        FilterMode::LpNotch,
        FilterMode::LpPeak,
    ] {
        let stop_hz = match mode {
            FilterMode::HpPeak | FilterMode::HpNotch => 350.0,
            _ => 2_800.0,
        };
        let db12 = evaluate_filter_response_db(
            &FilterParams {
                enabled: true,
                mode,
                slope: FilterSlope::Db12,
                cutoff_hz: 1_000.0,
                resonance: 0.4,
                aux: 0.65,
                ..FilterParams::default()
            },
            stop_hz,
        );
        let db48 = evaluate_filter_response_db(
            &FilterParams {
                enabled: true,
                mode,
                slope: FilterSlope::Db48,
                cutoff_hz: 1_000.0,
                resonance: 0.4,
                aux: 0.65,
                ..FilterParams::default()
            },
            stop_hz,
        );
        assert!(
            db48 < db12 - 12.0,
            "Expected combo {mode:?} 48dB to attenuate stopband more than 12dB: db12={db12}, db48={db48}"
        );
    }

    let bank = WavetableBank::new();
    for mode in [
        FilterMode::Bandreject,
        FilterMode::PeakPeak,
        FilterMode::NotchNotch,
    ] {
        for slope in [
            FilterSlope::Db6,
            FilterSlope::Db12,
            FilterSlope::Db24,
            FilterSlope::Db36,
            FilterSlope::Db48,
        ] {
            let mut p = SynthParams::default();
            p.oscs[0].wt_pos = 0.75;
            p.filters[0] = FilterParams {
                enabled: true,
                mode,
                slope,
                cutoff_hz: 900.0,
                resonance: 0.65,
                drive: 0.4,
                keytrack: 0.0,
                aux: 0.65,
            };
            let mut eng = SynthEngine::new(48_000.0);
            eng.note_on(0, 57, 0.9, None, &p);
            let mut peak = 0.0_f32;
            for _ in 0..2048 {
                let (l, r) = eng.process_sample(&p, &bank);
                assert!(l.is_finite() && r.is_finite() && l.abs() < 8.0 && r.abs() < 8.0);
                peak = peak.max(l.abs()).max(r.abs());
            }
            assert!(peak > 0.005, "Filter {mode:?} {slope:?} produced silence");
        }
    }
}

#[test]
fn fx_phaser_processes_stereo_and_idle_recovery() {
    use j_syn_dsp::{
        FxRackParams, FxRackState, FxSlotParams, FxType, ModUiTelemetry, NUM_MOD_DESTS,
    };

    let mut rack = FxRackState::new(48_000.0);
    let mut rack_params = FxRackParams::default();
    rack_params.buses[0].slots[0] = FxSlotParams {
        fx_type: FxType::Phaser,
        enabled: true,
        stack: 1,
        params: [0.65, 0.85, 0.65, 1.0],
        ..FxSlotParams::default()
    };
    let deltas = [0.0_f32; NUM_MOD_DESTS];
    let tel = ModUiTelemetry::new();

    let mut stereo_diff = 0.0_f32;
    let mut out_peak = 0.0_f32;
    for i in 0..4096 {
        let phase = ((i as f32) * 180.0 / 48_000.0).fract();
        let x = (phase * 2.0 - 1.0) * 0.4;
        let (l, r) = rack.process_buses_sample(
            (x, x),
            (0.0, 0.0),
            (0.0, 0.0),
            &rack_params,
            &deltas,
            120.0,
            Some(&tel),
        );
        assert!(l.is_finite() && r.is_finite() && l.abs() < 4.0 && r.abs() < 4.0);
        out_peak = out_peak.max(l.abs()).max(r.abs());
        stereo_diff = stereo_diff.max((l - r).abs());
    }
    assert!(out_peak > 0.05, "Phaser output too quiet: {out_peak}");
    assert!(stereo_diff > 0.01, "Quadrature phaser should create stereo movement: {stereo_diff}");

    let mut tail_peak = 0.0_f32;
    for _ in 0..8192 {
        let (l, r) = rack.process_buses_sample(
            (0.0, 0.0),
            (0.0, 0.0),
            (0.0, 0.0),
            &rack_params,
            &deltas,
            120.0,
            None,
        );
        assert!(l.is_finite() && r.is_finite());
        tail_peak = tail_peak.max(l.abs()).max(r.abs());
    }
    // Check final tail block settles cleanly to zero
    let mut end_peak = 0.0_f32;
    for _ in 0..512 {
        let (l, r) = rack.process_buses_sample(
            (0.0, 0.0),
            (0.0, 0.0),
            (0.0, 0.0),
            &rack_params,
            &deltas,
            120.0,
            None,
        );
        end_peak = end_peak.max(l.abs()).max(r.abs());
    }
    assert!(end_peak < 1e-6 && tail_peak < 2.0, "Phaser idle decay failed: end_peak={end_peak}");
}

#[test]
fn parallel_fx_buses_and_sub_bus_routing_and_modulation() {
    use j_syn_dsp::{
        DirectDest, FilterBusDest, FilterRouting, FxBusTopology, FxSlotParams, FxType,
        ModDestination, ModMatrixSource, ModSlot, SynthEngine, SynthParams, WavetableBank,
    };

    let bank = WavetableBank::new();
    let sample_rate = 48_000.0;

    // 1. DirectDest::Sub bypasses Filter 1 (even at 30 Hz cutoff) and Bus A/B muted levels,
    //    Sub LP Cutoff rolls off high harmonics, and Sub Clip Drive saturates cleanly.
    let run_sub = |lp_cutoff_hz: f32, drive_db: f32, sub_enabled: bool| -> f32 {
        let mut p = SynthParams::default();
        p.amp_attack_s = 0.001;
        p.oscs[0].level = 0.8;
        p.oscs[0].wt_pos = 0.8; // bright wave
        p.routing.osc_direct[0] = DirectDest::Sub;
        p.filters[0].cutoff_hz = 30.0; // would crush highs if not direct
        p.fx_bus_a.level = 0.0;
        p.fx_bus_b.level = 0.0;
        p.sub_bus.enabled = sub_enabled;
        p.sub_bus.lp_cutoff_hz = lp_cutoff_hz;
        p.sub_bus.clip_drive_db = drive_db;
        p.sub_bus.level = 1.0;

        let mut eng = SynthEngine::new(sample_rate);
        eng.note_on(0, 72, 1.0, None, &p); // C5 (523 Hz)
        let mut sq = 0.0_f32;
        for _ in 0..4096 {
            let (l, r) = eng.process_sample(&p, &bank);
            assert!(l.is_finite() && r.is_finite());
            sq += 0.5 * (l * l + r * r);
        }
        (sq / 4096.0).sqrt()
    };

    let rms_sub_open = run_sub(2_000.0, 0.0, true);
    let rms_sub_driven = run_sub(2_000.0, 24.0, true);
    let rms_sub_low = run_sub(80.0, 0.0, true);
    let rms_sub_off = run_sub(80.0, 24.0, false);
    assert!(
        rms_sub_open > 0.01,
        "Sub bus should carry bright C5 direct signal: open={rms_sub_open}, low={rms_sub_low}, off={rms_sub_off}"
    );
    assert!(
        rms_sub_driven > rms_sub_open * 2.0 && rms_sub_driven < 1.0,
        "Sub Clipper +24 dB drive must saturate & boost RMS cleanly: dry={rms_sub_open}, driven={rms_sub_driven}"
    );
    assert!(rms_sub_low < rms_sub_open * 0.25, "Sub LP 80 Hz must attenuate C5: {rms_sub_low} vs {rms_sub_open}");
    assert!(
        rms_sub_off > rms_sub_low * 3.0 && rms_sub_off < rms_sub_driven * 0.7,
        "Disabled Sub bus must bypass LP & clipper and pass clean Direct Out through: off={rms_sub_off}, low={rms_sub_low}, driven={rms_sub_driven}"
    );

    // 2. Parallel Filter routing: Filter 1 -> Bus A (panned Left), Filter 2 -> Bus B (panned Right)
    let mut mp = SynthParams::default();
    mp.amp_attack_s = 0.001;
    mp.oscs[0].enabled = true;
    mp.oscs[0].level = 0.8;
    mp.oscs[1].enabled = true;
    mp.oscs[1].level = 0.8;
    mp.routing.filter_routing = FilterRouting::Parallel;
    mp.routing.osc_filter_balance[0] = -1.0; // Osc 1 -> Filter 1 only
    mp.routing.osc_filter_balance[1] = 1.0; // Osc 2 -> Filter 2 only
    mp.routing.filter_out_bus = [FilterBusDest::BusA, FilterBusDest::BusB];
    mp.filters[1].enabled = true;
    mp.filters[1].cutoff_hz = 20_000.0;
    mp.fx_bus_a.pan = -1.0; // hard left
    mp.fx_bus_b.pan = 1.0; // hard right
    mp.fx_bus_b.slots[0] = FxSlotParams {
        fx_type: FxType::Distortion,
        enabled: true,
        stack: 1,
        params: [0.0, 0.5, 0.5, 1.0],
        ..FxSlotParams::default()
    };

    let mut eng = SynthEngine::new(sample_rate);
    eng.note_on(0, 57, 1.0, None, &mp);
    let (mut l_sq, mut r_sq) = (0.0_f32, 0.0_f32);
    for _ in 0..2048 {
        let (l, r) = eng.process_sample(&mp, &bank);
        l_sq += l * l;
        r_sq += r * r;
    }
    assert!(l_sq > 0.01 && r_sq > 0.01, "Expected both Bus A (L) and Bus B (R) active: l={l_sq}, r={r_sq}");

    // Now mute Bus A via Macro1 -> BusALevel (-1.0 depth), leaving only Right channel (Bus B)
    mp.mod_matrix.slots[0] = ModSlot {
        source: ModMatrixSource::Macro1,
        destination: ModDestination::BusALevel,
        depth: -1.0,
        bipolar: false,
        enabled: true,
        ..ModSlot::default()
    };
    mp.macros[0] = 1.0;
    let mut eng_muted = SynthEngine::new(sample_rate);
    eng_muted.note_on(0, 57, 1.0, None, &mp);
    let (mut ml_sq, mut mr_sq) = (0.0_f32, 0.0_f32);
    for _ in 0..2048 {
        let (l, r) = eng_muted.process_sample(&mp, &bank);
        ml_sq += l * l;
        mr_sq += r * r;
    }
    assert!(ml_sq < 1e-6 && mr_sq > 0.01, "Expected Bus A muted on L while Bus B remains on R: l={ml_sq}, r={mr_sq}");

    // 3. Serial Bus Topology (A -> B): Bus A output feeds Bus B, and FilterBusDest::Both avoids double-feeding Bus B.
    let run_serial = |f1_dest: FilterBusDest, bus_b_level: f32| -> (f32, f32) {
        let mut sp = SynthParams::default();
        sp.amp_attack_s = 0.001;
        sp.oscs[0].enabled = true;
        sp.oscs[0].level = 0.8;
        sp.routing.osc_filter_balance[0] = -1.0;
        sp.routing.filter_out_bus[0] = f1_dest;
        sp.fx_bus_topology = FxBusTopology::Serial;
        sp.fx_bus_a.level = 1.0;
        sp.fx_bus_b.level = bus_b_level;
        sp.fx_bus_b.pan = 1.0; // hard right: only Bus B output reaches master

        let mut eng_s = SynthEngine::new(sample_rate);
        eng_s.note_on(0, 57, 1.0, None, &sp);
        let (mut sl_sq, mut sr_sq) = (0.0_f32, 0.0_f32);
        for _ in 0..2048 {
            let (l, r) = eng_s.process_sample(&sp, &bank);
            sl_sq += l * l;
            sr_sq += r * r;
        }
        (sl_sq, sr_sq)
    };

    let (ser_a_l, ser_a_r) = run_serial(FilterBusDest::BusA, 1.0);
    let (ser_both_l, ser_both_r) = run_serial(FilterBusDest::Both, 1.0);
    let (ser_muted_l, ser_muted_r) = run_serial(FilterBusDest::BusA, 0.0);
    assert!(
        ser_a_l < 1e-6 && ser_a_r > 0.01,
        "Serial A -> B must feed Bus A into Bus B (panned hard right): l={ser_a_l}, r={ser_a_r}"
    );
    assert!(
        ser_muted_l < 1e-6 && ser_muted_r < 1e-6,
        "Serial A -> B must mute master output when Bus B level is 0: l={ser_muted_l}, r={ser_muted_r}"
    );
    assert!(
        (ser_both_r - ser_a_r).abs() < 1e-5 && ser_both_l < 1e-6,
        "Serial FilterBusDest::Both must route via Bus A -> Bus B once without double-feeding: both_r={ser_both_r}, bus_a_r={ser_a_r}"
    );
}

#[test]
fn fx_diopser_stages_stacking_dispersion_and_idle() {
    use j_syn_dsp::{
        diopser_freq_hz, evaluate_diopser_ripple, FxRackParams, FxRackState, FxSlotParams, FxType,
        ModUiTelemetry, NUM_MOD_DESTS,
    };

    let deltas = [0.0_f32; NUM_MOD_DESTS];
    let tel = ModUiTelemetry::new();

    // Verify ripple curve peaks around center frequency for all stage counts
    let p_diop = [0.5, 0.85, 0.0, 1.0];
    let center_hz = diopser_freq_hz(p_diop[0]);
    for stages_idx in 0..5 {
        let peak_val = evaluate_diopser_ripple(p_diop, stages_idx, 1, center_hz);
        let far_val = evaluate_diopser_ripple(p_diop, stages_idx, 1, 20_000.0);
        assert!(
            peak_val > far_val * 2.0,
            "Diopser ripple should peak near center {center_hz} Hz (stages_idx={stages_idx}: peak={peak_val}, far={far_val})"
        );

        let mut rack = FxRackState::new(48_000.0);
        let mut rp = FxRackParams::default();
        rp.buses[0].slots[0] = FxSlotParams {
            fx_type: FxType::Diopser,
            enabled: true,
            stack: 2,
            params: [0.45, 0.75, 0.35, 1.0],
            diopser_stages: stages_idx as u8,
            ..FxSlotParams::default()
        };

        // Transient impulse dispersion + clean decay to silence
        let mut max_abs = 0.0_f32;
        for i in 0..4096 {
            let imp = if i == 0 { 0.8 } else { 0.0 };
            let (l, r) = rack.process_buses_sample(
                (imp, imp),
                (0.0, 0.0),
                (0.0, 0.0),
                &rp,
                &deltas,
                120.0,
                Some(&tel),
            );
            assert!(l.is_finite() && r.is_finite());
            max_abs = max_abs.max(l.abs()).max(r.abs());
        }
        assert!(max_abs > 0.1 && max_abs <= 1.5, "Unexpected Diopser impulse peak {max_abs}");

        for _ in 0..140_000 {
            let _ = rack.process_buses_sample(
                (0.0, 0.0),
                (0.0, 0.0),
                (0.0, 0.0),
                &rp,
                &deltas,
                120.0,
                None,
            );
        }
        let (end_l, end_r) = rack.process_buses_sample(
            (0.0, 0.0),
            (0.0, 0.0),
            (0.0, 0.0),
            &rp,
            &deltas,
            120.0,
            None,
        );
        assert!(end_l.abs() < 1e-6 && end_r.abs() < 1e-6, "Diopser did not settle idle cleanly");
    }
}

#[test]
fn fx_reverb_delay_chorus_eq_subtabs_and_bus_b_modulation() {
    use j_syn_dsp::{
        FxRackParams, FxRackState, FxSlotParams, FxType, ModDestination, ModUiTelemetry,
        NUM_MOD_DESTS,
    };

    let deltas = [0.0_f32; NUM_MOD_DESTS];

    // Delay Ping-Pong: feed Left channel only -> Right channel receives bounced taps
    let mut rack = FxRackState::new(48_000.0);
    let mut rp = FxRackParams::default();
    rp.buses[1].slots[0] = FxSlotParams {
        fx_type: FxType::Delay,
        enabled: true,
        stack: 1,
        params: [0.0, 0.65, 0.1, 1.0], // 1/16 note (~125 ms at 120 BPM = 6000 samples)
        eq_params: [0.0, 1.0, 1.0, 0.5], // Ping-Pong = 100%, Width = 100%
        ..FxSlotParams::default()
    };

    let mut right_energy = 0.0_f32;
    for i in 0..24_000 {
        let in_l = if i < 64 { 0.5 } else { 0.0 };
        let (_l, r) = rack.process_buses_sample(
            (0.0, 0.0),
            (in_l, 0.0),
            (0.0, 0.0),
            &rp,
            &deltas,
            120.0,
            None,
        );
        if i > 10_000 {
            right_energy += r * r;
        }
    }
    assert!(right_energy > 0.05, "Ping-Pong Delay fed on L should bounce to R: {right_energy}");

    // Reverb Pre-Delay: 0 ms vs 250 ms delays wet onset, and Bus B Eq P5..P8 modulation works cleanly
    let tel = ModUiTelemetry::new();
    let measure_reverb_early_wet = |predelay_norm: f32, mod_eq_cutoff: f32| -> f32 {
        let mut r = FxRackState::new(48_000.0);
        let mut p = FxRackParams::default();
        p.buses[1].slots[2] = FxSlotParams {
            fx_type: FxType::Reverb,
            enabled: true,
            stack: 1,
            params: [0.6, 0.7, 0.2, 1.0],
            eq_params: [0.0, 1.0, predelay_norm, 0.5],
            ..FxSlotParams::default()
        };
        let mut d = [0.0_f32; NUM_MOD_DESTS];
        // Modulate Bus B Slot 2 Eq Param 0 (LOW CUT = P5)
        d[ModDestination::fx_bus_slot_param(1, 2, 4).index()] = mod_eq_cutoff;

        let mut energy = 0.0_f32;
        // First 4800 samples (100 ms) at 48 kHz
        for i in 0..4800 {
            let x = if i < 32 { 0.7 } else { 0.0 };
            let (ol, or) =
                r.process_buses_sample((0.0, 0.0), (x, x), (0.0, 0.0), &p, &d, 120.0, Some(&tel));
            if i > 200 {
                energy += ol * ol + or * or;
            }
        }
        energy
    };

    let immediate = measure_reverb_early_wet(0.0, 0.0);
    let predelayed = measure_reverb_early_wet(1.0, 0.0); // 250 ms pre-delay > 100 ms window
    let high_passed = measure_reverb_early_wet(0.0, 1.0); // 2 kHz low-cut attenuates wet tail
    assert!(immediate > 1e-4, "Immediate reverb should produce tail in first 100ms");
    assert!(predelayed < 1e-6, "250ms pre-delay should hold reverb wet before 100ms: {predelayed}");
    assert!(high_passed < immediate, "Bus B Reverb Low Cut EQ modulation should reduce energy: {high_passed} vs {immediate}");

    // Chorus with EQ Low Cut + Feedback runs finite & cleanly
    let mut rc = FxRackState::new(48_000.0);
    let mut cp = FxRackParams::default();
    cp.buses[0].slots[1] = FxSlotParams {
        fx_type: FxType::Chorus,
        enabled: true,
        stack: 1,
        params: [0.4, 0.6, 0.8, 1.0],
        eq_params: [0.4, 0.6, 0.8, 0.5],
        ..FxSlotParams::default()
    };
    let mut chorus_pk = 0.0_f32;
    for i in 0..2048 {
        let s = (i as f32 * 0.04).sin() * 0.4;
        let (l, r) = rc.process_buses_sample((s, s), (0.0, 0.0), (0.0, 0.0), &cp, &deltas, 120.0, None);
        assert!(l.is_finite() && r.is_finite());
        chorus_pk = chorus_pk.max(l.abs()).max(r.abs());
    }
    assert!(chorus_pk > 0.05 && chorus_pk < 3.0);
}

#[test]
fn mpe_per_voice_pitch_bend_pressure_slide_and_master_broadcast() {
    use j_syn_dsp::{
        ModDestination, ModMatrixSource, ModSlot, SynthEngine, SynthParams, WavetableBank,
    };

    let bank = WavetableBank::new();
    let sr = 48_000.0;
    let mut p = SynthParams::default();
    p.mpe_enabled = true;
    p.mpe_pitch_bend_range = 48.0;
    p.pitch_bend_range = 2.0;
    p.amp_attack_s = 0.001;
    p.oscs[0].enabled = true;
    p.oscs[0].level = 0.8;
    p.mod_matrix.slots[0] = ModSlot {
        source: ModMatrixSource::Slide,
        destination: ModDestination::Osc1Warp,
        depth: 1.0,
        bipolar: false,
        enabled: true,
        ..ModSlot::default()
    };
    p.mod_matrix.slots[1] = ModSlot {
        source: ModMatrixSource::Aftertouch,
        destination: ModDestination::Osc1Level,
        depth: -0.5,
        bipolar: false,
        enabled: true,
        ..ModSlot::default()
    };

    let mut eng = SynthEngine::new(sr);
    // Note on Member Ch 2 (channel=1) and Member Ch 3 (channel=2)
    eng.note_on(1, 60, 1.0, Some(101), &p);
    eng.note_on(2, 60, 1.0, Some(102), &p);

    // Bend ONLY Ch 2 (+24 st) and set Ch 2 Slide=1.0, Ch 3 Aftertouch=1.0
    eng.set_pitch_bend_ch(1, 0.75, &p); // (0.75 * 2 - 1) = +0.5 * 48 = +24 st
    eng.set_slide_ch(1, 1.0, &p);
    eng.set_aftertouch_ch(2, 1.0, &p);

    for _ in 0..1024 {
        let _ = eng.process_sample(&p, &bank);
    }

    let v1 = eng
        .voices
        .iter()
        .find(|v| v.active && v.channel == 1)
        .expect("member ch 1 active");
    let v2 = eng
        .voices
        .iter()
        .find(|v| v.active && v.channel == 2)
        .expect("member ch 2 active");

    assert!(
        (v1.pitch_bend_smoothed - 0.5).abs() < 1e-3,
        "Member ch 1 pitch bend should smooth to +0.5: {}",
        v1.pitch_bend_smoothed
    );
    assert!(
        v2.pitch_bend_smoothed.abs() < 1e-4,
        "Member ch 2 pitch bend must stay isolated at 0: {}",
        v2.pitch_bend_smoothed
    );
    assert!(
        v1.slide_smoothed > 0.99 && v2.slide_smoothed < 1e-4,
        "Slide (CC74) must isolate per member channel: v1={}, v2={}",
        v1.slide_smoothed,
        v2.slide_smoothed
    );
    assert!(
        v2.aftertouch_smoothed > 0.99 && v1.aftertouch_smoothed < 1e-4,
        "Pressure (AT) must isolate per member channel: v1={}, v2={}",
        v1.aftertouch_smoothed,
        v2.aftertouch_smoothed
    );
    assert!(
        v1.mod_offsets[ModDestination::Osc1Warp.index()] > 0.95
            && v2.mod_offsets[ModDestination::Osc1Warp.index()].abs() < 1e-3,
        "Slide mod matrix routing must only affect voice on ch 1"
    );
    assert!(
        v2.mod_offsets[ModDestination::Osc1Level.index()] < -0.45
            && v1.mod_offsets[ModDestination::Osc1Level.index()].abs() < 1e-3,
        "Aftertouch mod matrix routing must only affect voice on ch 2"
    );

    // Master channel (channel=0) pitch bend broadcasts globally + PolyTuning targets voice_id
    eng.set_pitch_bend_ch(0, 1.0, &p);
    eng.set_poly_tuning(2, 60, Some(102), 7.0);
    for _ in 0..1024 {
        let (l, r) = eng.process_sample(&p, &bank);
        assert!(l.is_finite() && r.is_finite());
    }
    let v2_after = eng
        .voices
        .iter()
        .find(|v| v.active && v.channel == 2)
        .unwrap();
    assert!((v2_after.poly_tuning_smoothed - 7.0).abs() < 1e-2);
}

#[test]
fn fx_tube_resonator_tune_filter_dirty_lfo_and_idle() {
    use j_syn_dsp::{
        evaluate_tube_res_scope_response, format_tube_res_tune, tube_res_tune_hz, FxRackParams,
        FxRackState, FxSlotParams, FxType, NUM_MOD_DESTS,
    };

    let mod_offsets = [0.0_f32; NUM_MOD_DESTS];

    let defs = FxSlotParams::default_params_for_type(FxType::TubeResonator);
    let eq_defs = FxSlotParams::default_eq_params_for_type(FxType::TubeResonator);
    assert_eq!(format_tube_res_tune(defs[0]), "C1");
    assert!((tube_res_tune_hz(defs[0]) - 32.703197).abs() < 0.01);
    assert!(format_tube_res_tune(defs[0] + 0.25 / 72.0).contains("+25c"));
    assert_eq!(format_tube_res_tune(1.0), "C6");
    assert!((eq_defs[0] - 0.5).abs() < 1e-6);
    assert!((eq_defs[3] - 1.0).abs() < 1e-6, "Tube Res. damping must default to 100%");

    let mut base_params = FxRackParams::default();
    base_params.buses[0].slots[0] = FxSlotParams {
        enabled: true,
        fx_type: FxType::TubeResonator,
        stack: 1,
        params: defs,
        eq_params: eq_defs,
        tube_res_filter_enabled: true,
        ..FxSlotParams::default()
    };

    let run_burst = |params: &FxRackParams| -> Vec<f32> {
        let mut rack = FxRackState::new(48_000.0);
        let mut out = Vec::with_capacity(4096);
        for n in 0..4096 {
            let x = if n < 64 {
                ((n as f32 * 0.37).sin()) * 0.5
            } else {
                0.0
            };
            let (l, r) =
                rack.process_buses_sample((x, -x * 0.8), (0.0, 0.0), (0.0, 0.0), params, &mod_offsets, 120.0, None);
            assert!(l.is_finite() && r.is_finite());
            out.push(l);
        }
        out
    };

    let out_flt_on = run_burst(&base_params);
    let mut flt_off_params = base_params.clone();
    flt_off_params.buses[0].slots[0].tube_res_filter_enabled = false;
    let out_flt_off = run_burst(&flt_off_params);

    let energy_on: f32 = out_flt_on[64..].iter().map(|s| s * s).sum();
    let energy_off: f32 = out_flt_off[64..].iter().map(|s| s * s).sum();
    assert!(energy_on > 1e-4 && energy_off > 1e-4);
    let flt_diff: f32 = out_flt_on
        .iter()
        .zip(out_flt_off.iter())
        .map(|(a, b)| (a - b).abs())
        .sum();
    assert!(flt_diff > 1.0, "Filter On vs Off must noticeably shape tail");

    // Verify Dirty (EngineMode::Vintage2007) vs Clean (EngineMode::Hq) LFO behavior
    let mut dirty_p = base_params.clone();
    dirty_p.engine_mode = EngineMode::Vintage2007;
    dirty_p.buses[0].slots[0].eq_params[0] = 1.0; // LFO AMT 100%
    dirty_p.buses[0].slots[0].eq_params[1] = 0.85; // fast LFO
    dirty_p.buses[0].slots[0].eq_params[2] = 0.6; // longer feedback decay

    let mut clean_p = dirty_p.clone();
    clean_p.engine_mode = EngineMode::Hq;

    let out_dirty = run_burst(&dirty_p);
    let out_clean = run_burst(&clean_p);
    let lfo_mode_diff: f32 = out_dirty[128..]
        .iter()
        .zip(out_clean[128..].iter())
        .map(|(a, b)| (a - b).abs())
        .sum();
    assert!(
        lfo_mode_diff > 0.01,
        "Dirty vs Clean engine mode must produce distinct Pitch LFO modulation (diff={lfo_mode_diff})"
    );

    // Scope response evaluator should return bounded finite values
    for i in 0..=32 {
        let freq_hz = 20.0 * (1000.0_f32).powf(i as f32 / 32.0);
        let (mag, bp_env) = evaluate_tube_res_scope_response(defs, eq_defs, true, 0.0, freq_hz);
        assert!(mag.is_finite() && (0.0..=1.0).contains(&mag));
        assert!(bp_env.is_finite() && (0.0..=1.0).contains(&bp_env));
    }

    // Long silence should decay cleanly to idle without NaNs
    let mut rack = FxRackState::new(48_000.0);
    let (l0, r0) = rack.process_buses_sample(
        (0.4, 0.4),
        (0.0, 0.0),
        (0.0, 0.0),
        &base_params,
        &mod_offsets,
        120.0,
        None,
    );
    assert!(l0.is_finite() && r0.is_finite());
    for _ in 0..96_000 {
        let (l, r) = rack.process_buses_sample(
            (0.0, 0.0),
            (0.0, 0.0),
            (0.0, 0.0),
            &base_params,
            &mod_offsets,
            120.0,
            None,
        );
        assert!(l.is_finite() && r.is_finite());
    }
    let (l_end, r_end) = rack.process_buses_sample(
        (0.0, 0.0),
        (0.0, 0.0),
        (0.0, 0.0),
        &base_params,
        &mod_offsets,
        120.0,
        None,
    );
    assert!(l_end.abs() < 1e-5 && r_end.abs() < 1e-5);
}

#[test]
fn osc_pitch_ratio_hz_offset_and_modulation() {
    use j_syn_dsp::{ModDestination, ModMatrixSource, ModSlot, OscPitchMode};

    let bank = WavetableBank::new();
    let sample_rate = 48_000.0;

    let count_zero_crossings = |params: &SynthParams, note: u8| -> usize {
        let mut eng = SynthEngine::new(sample_rate);
        eng.note_on(0, note, 0.9, None, params);
        let mut prev = 0.0_f32;
        let mut zc = 0usize;
        for i in 0..4800 {
            let (l, r) = eng.process_sample(params, &bank);
            assert!(l.is_finite() && r.is_finite());
            if i > 200 && prev <= 0.0 && l > 0.0 {
                zc += 1;
            }
            prev = l;
        }
        zc
    };

    let mut base_p = SynthParams::default();
    base_p.amp_attack_s = 0.001;
    base_p.filters[0].enabled = false;
    base_p.oscs[0].pitch_mode = OscPitchMode::Ratio;
    base_p.oscs[0].ratio = 1.0;
    base_p.oscs[0].hz_offset = 0.0;

    let mut ratio2_p = base_p;
    ratio2_p.oscs[0].ratio = 2.0;

    let zc1 = count_zero_crossings(&base_p, 69); // A4 = 440 Hz
    let zc2 = count_zero_crossings(&ratio2_p, 69); // 880 Hz (2x ratio)
    let ratio = zc2 as f32 / (zc1.max(1) as f32);
    assert!(
        (ratio - 2.0).abs() < 0.08,
        "Ratio 2.0 must double oscillator frequency: zc1={zc1}, zc2={zc2}, ratio={ratio}"
    );

    // Ratio = 0.0 with +200 Hz linear offset and keytrack=false produces constant 200 Hz regardless of MIDI note
    let mut fixed_hz_p = base_p;
    fixed_hz_p.oscs[0].keytrack = false;
    fixed_hz_p.oscs[0].ratio = 0.0;
    fixed_hz_p.oscs[0].hz_offset = 200.0;
    let zc_fixed_c3 = count_zero_crossings(&fixed_hz_p, 60);
    let zc_fixed_c5 = count_zero_crossings(&fixed_hz_p, 84);
    assert_eq!(zc_fixed_c3, zc_fixed_c5);
    let est_hz = zc_fixed_c3 as f32 * (sample_rate / 4600.0);
    assert!((est_hz - 200.0).abs() < 15.0, "Expected ~200 Hz fixed beating freq, got {est_hz}");

    // Modulation matrix: Macro1 -> Osc1Ratio raises frequency cleanly
    let mut mod_ratio_p = base_p;
    mod_ratio_p.macros[0] = 1.0;
    mod_ratio_p.mod_matrix.slots[0] = ModSlot {
        source: ModMatrixSource::Macro1,
        destination: ModDestination::Osc1Ratio,
        depth: 0.0625, // +0.0625 * 32 = +2.0 => total ratio 3.0x (1320 Hz on A4)
        bipolar: false,
        enabled: true,
        bypassed: false,
        aux_source: ModMatrixSource::None,
        curve: 0.0,
    };
    let zc_mod = count_zero_crossings(&mod_ratio_p, 69);
    let ratio_mod = zc_mod as f32 / (zc1.max(1) as f32);
    assert!(
        (ratio_mod - 3.0).abs() < 0.15,
        "Osc1Ratio modulation (+2.0 -> 3.0x) failed: zc1={zc1}, zc_mod={zc_mod}"
    );
}

#[test]
fn fx_utility_width_pan_gain_phase_invert_and_modulation() {
    use j_syn_dsp::{
        format_utility_pan, utility_gain_db_from_norm, utility_width_from_norm, FxRackParams,
        FxRackState, FxSlotParams, FxType, ModDestination, NUM_MOD_DESTS,
        UTILITY_WIDTH_DEFAULT_NORM,
    };

    // 1. Curve math verification: Width (100% @ 0.40, 200% @ 0.68, 500% @ 1.00) & Gain (±20 dB ~65% span)
    assert!((utility_width_from_norm(0.0)).abs() < 1e-6);
    assert!((utility_width_from_norm(UTILITY_WIDTH_DEFAULT_NORM) - 1.0).abs() < 1e-4);
    assert!((utility_width_from_norm(0.68) - 2.0).abs() < 1e-4);
    assert!((utility_width_from_norm(1.0) - 5.0).abs() < 1e-4);

    assert!((utility_gain_db_from_norm(0.5)).abs() < 1e-6);
    assert!((utility_gain_db_from_norm(0.0) + 60.0).abs() < 1e-4);
    assert!((utility_gain_db_from_norm(1.0) - 60.0).abs() < 1e-4);
    assert!((utility_gain_db_from_norm(0.825) - 21.4).abs() < 0.25);

    assert_eq!(format_utility_pan(0.0), "50L");
    assert_eq!(format_utility_pan(0.5), "C");
    assert_eq!(format_utility_pan(1.0), "50R");

    let defs = FxSlotParams::default_params_for_type(FxType::Utility);
    let mod_offsets = [0.0_f32; NUM_MOD_DESTS];

    // 2. Unity passthrough at defaults (100% Width, C Pan, 0 dB Gain, 100% Mix, Inv L/R off)
    let mut params = FxRackParams::default();
    params.buses[0].slots[0] = FxSlotParams {
        enabled: true,
        fx_type: FxType::Utility,
        stack: 1,
        params: defs,
        utility_inv_l: false,
        utility_inv_r: false,
        ..FxSlotParams::default()
    };
    let mut rack = FxRackState::new(48_000.0);
    let (ul, ur) = rack.process_buses_sample(
        (0.45, -0.25),
        (0.0, 0.0),
        (0.0, 0.0),
        &params,
        &mod_offsets,
        120.0,
        None,
    );
    assert!((ul - 0.45).abs() < 1e-5 && (ur + 0.25).abs() < 1e-5);

    // 3. Phase invert + 0% Width mono sum
    params.buses[0].slots[0].utility_inv_l = true;
    let mut rack_inv = FxRackState::new(48_000.0);
    let (il, ir) = rack_inv.process_buses_sample(
        (0.4, 0.4),
        (0.0, 0.0),
        (0.0, 0.0),
        &params,
        &mod_offsets,
        120.0,
        None,
    );
    assert!((il + 0.4).abs() < 1e-5 && (ir - 0.4).abs() < 1e-5);

    // 4. 0% Width collapses stereo to mono + 500% boosts Side
    params.buses[0].slots[0].utility_inv_l = false;
    params.buses[0].slots[0].params[0] = 0.0; // 0% Width
    let mut rack_mono = FxRackState::new(48_000.0);
    let (ml, mr) = rack_mono.process_buses_sample(
        (0.6, -0.2),
        (0.0, 0.0),
        (0.0, 0.0),
        &params,
        &mod_offsets,
        120.0,
        None,
    );
    assert!((ml - 0.2).abs() < 1e-5 && (mr - 0.2).abs() < 1e-5);

    // 5. Gain +6 dB via mod matrix raises output level
    params.buses[0].slots[0].params = defs;
    let mut mod_gain = [0.0_f32; NUM_MOD_DESTS];
    mod_gain[ModDestination::fx_bus_slot_param(0, 0, 2).index()] = 0.25; // +~6.9 dB
    let mut rack_mod = FxRackState::new(48_000.0);
    let (gl, gr) = rack_mod.process_buses_sample(
        (0.1, 0.1),
        (0.0, 0.0),
        (0.0, 0.0),
        &params,
        &mod_gain,
        120.0,
        None,
    );
    assert!(gl > 0.18 && (gl - gr).abs() < 1e-5 && gl.is_finite());
}

#[test]
fn sub_bus_lp_off_and_sub_master_clipper_scopes() {
    use j_syn_dsp::{
        DirectDest, FxRackParams, FxRackState, ModUiTelemetry, SynthEngine, SynthParams,
        WavetableBank, FX_SCOPE_LEN, NUM_MOD_DESTS,
    };

    let sr = 48_000.0;
    let mod_offsets = [0.0_f32; NUM_MOD_DESTS];

    // 1. Sub LP Cutoff at 100% (2000 Hz) acts as LP Off and passes 6 kHz unattenuated,
    //    whereas 1500 Hz attenuates 6 kHz strongly.
    let run_6k = |cutoff_hz: f32, drive_db: f32, tel: Option<&ModUiTelemetry>| -> f32 {
        let mut rack = FxRackState::new(sr);
        let mut params = FxRackParams::default();
        params.sub.enabled = true;
        params.sub.lp_cutoff_hz = cutoff_hz;
        params.sub.clip_drive_db = drive_db;
        params.sub.level = 1.0;

        let mut sq = 0.0_f32;
        for n in 0..2048 {
            let x = 0.5 * (2.0 * std::f32::consts::PI * 6_000.0 * n as f32 / sr).sin();
            let (l, r) = rack.process_buses_sample(
                (0.0, 0.0),
                (0.0, 0.0),
                (x, x),
                &params,
                &mod_offsets,
                120.0,
                tel,
            );
            assert!(l.is_finite() && r.is_finite());
            if n >= 512 {
                sq += 0.5 * (l * l + r * r);
            }
        }
        (sq / 1536.0).sqrt()
    };

    let tel = ModUiTelemetry::new();
    let rms_lp_off = run_6k(2_000.0, 0.0, None);
    let rms_lp_1500 = run_6k(1_500.0, 0.0, None);
    let _ = run_6k(2_000.0, 18.0, Some(&tel));

    assert!(
        rms_lp_off > 0.34,
        "Sub LP Off (100% / 2000 Hz) must pass 6 kHz unattenuated: {rms_lp_off}"
    );
    assert!(
        rms_lp_1500 < rms_lp_off * 0.15,
        "Sub LP 1500 Hz must strongly attenuate 6 kHz: {rms_lp_1500} vs {rms_lp_off}"
    );

    let mut sub_pre = [0.0_f32; FX_SCOPE_LEN];
    let mut sub_post = [0.0_f32; FX_SCOPE_LEN];
    let sub_act = tel.sub_scope.read_snapshot(&mut sub_pre, &mut sub_post);
    assert!(
        sub_pre.iter().copied().fold(0.0_f32, f32::max) > 1.2 && sub_act > 0.1,
        "Expected sub_scope pre_peak > 1.2 and non-zero clip activity: act={sub_act}"
    );

    // 2. Master Scope populates when hot synth audio hits the master safety limiter and clears on reset
    let bank = WavetableBank::new();
    let mut sp = SynthParams::default();
    sp.amp_attack_s = 0.001;
    sp.oscs[0].level = 1.0;
    sp.routing.osc_direct[0] = DirectDest::Sub;
    sp.sub_bus.enabled = true;
    sp.sub_bus.lp_cutoff_hz = 2_000.0;
    sp.sub_bus.clip_drive_db = 12.0;
    sp.master_gain = 2.0; // +6 dB driving master limiter
    sp.master_limiter = true;

    let mut eng = SynthEngine::new(sr);
    eng.note_on(0, 60, 1.0, None, &sp);
    for _ in 0..2048 {
        let (l, r) = eng.process_sample(&sp, &bank);
        assert!(l.is_finite() && r.is_finite());
    }
    let eng_tel = eng.telemetry();
    let mut m_pre = [0.0_f32; FX_SCOPE_LEN];
    let mut m_post = [0.0_f32; FX_SCOPE_LEN];
    let m_act = eng_tel.master_scope.read_snapshot(&mut m_pre, &mut m_post);
    assert!(
        m_pre.iter().copied().fold(0.0_f32, f32::max) > 1.0 && m_act > 0.05,
        "Expected master_scope pre_peak > 1.0 and limiter activity > 0.05: act={m_act}"
    );

    eng.reset();
    let m_act_after = eng_tel.master_scope.read_snapshot(&mut m_pre, &mut m_post);
    assert_eq!(m_act_after, 0.0);
}

#[test]
fn fx_limiter_lookahead_scurve_transient_clipping_and_modulation() {
    use j_syn_dsp::{
        limiter_ceil_db, FxRackParams, FxRackState, FxSlotParams, FxType, ModDestination,
        NUM_MOD_DESTS,
    };

    let sr = 48_000.0;
    let mod_offsets = [0.0_f32; NUM_MOD_DESTS];

    // 1. Ceiling invariant at 0.0 ms and 3.0 ms lookahead under +18 dB drive & sharp spikes
    let ceil_norm = 0.75; // -4.5 dB
    let ceil_lin = 10.0_f32.powf(limiter_ceil_db(ceil_norm) * 0.05);
    for la_norm in [0.0_f32, 1.0_f32] {
        let mut rack = FxRackState::new(sr);
        let mut params = FxRackParams::default();
        params.buses[0].slots[0].fx_type = FxType::Limiter;
        params.buses[0].slots[0].enabled = true;
        params.buses[0].slots[0].params = [0.75, ceil_norm, la_norm, 1.0]; // +18 dB gain
        params.buses[0].slots[0].eq_params = FxSlotParams::default_eq_params_for_type(FxType::Limiter);
        params.buses[0].slots[0].limiter_true_peak = true;

        for n in 0..4096 {
            let x = if n % 400 < 20 {
                0.95
            } else {
                0.35 * (2.0 * std::f32::consts::PI * 55.0 * n as f32 / sr).sin()
            };
            let (l, r) = rack.process_buses_sample(
                (x, -x),
                (0.0, 0.0),
                (0.0, 0.0),
                &params,
                &mod_offsets,
                120.0,
                None,
            );
            assert!(l.is_finite() && r.is_finite());
            assert!(
                l.abs() <= ceil_lin + 1e-4 && r.abs() <= ceil_lin + 1e-4,
                "Limiter exceeded ceiling {ceil_lin} at lookahead_norm={la_norm}: l={l}, r={r}"
            );
        }
    }

    // 2. 0 ms lookahead lets micro-transient hit clipper without ducking subsequent audio
    //    as deeply as 2.0 ms lookahead (Pro-L 2 style punch vs kHs lookahead smoothness).
    let measure_post_spike_rms = |la_norm: f32| -> f32 {
        let mut rack = FxRackState::new(sr);
        let mut params = FxRackParams::default();
        params.buses[0].slots[0].fx_type = FxType::Limiter;
        params.buses[0].slots[0].enabled = true;
        params.buses[0].slots[0].params = [0.0, 0.5, la_norm, 1.0]; // 0 dB gain, -9 dB ceil (~0.355)
        params.buses[0].slots[0].eq_params = [0.3, 0.1, 1.0, 0.0];
        params.buses[0].slots[0].limiter_true_peak = false;

        let la_samples = (la_norm * 3.0 * 0.001 * sr).round() as usize;
        let spike_at = 500;
        let mut sq = 0.0_f32;
        let mut count = 0;
        for n in 0..1000 {
            let base = 0.15 * (2.0 * std::f32::consts::PI * 220.0 * n as f32 / sr).sin();
            let spike = if n == spike_at { 1.0 } else { 0.0 };
            let (l, _r) = rack.process_buses_sample(
                (base + spike, base + spike),
                (0.0, 0.0),
                (0.0, 0.0),
                &params,
                &mod_offsets,
                120.0,
                None,
            );
            // Measure 1 ms right before/during spike arrival in delayed stream
            if n >= spike_at + la_samples - 24 && n < spike_at + la_samples {
                sq += l * l;
                count += 1;
            }
        }
        (sq / count.max(1) as f32).sqrt()
    };
    let rms_zero_la = measure_post_spike_rms(0.0);
    let rms_lookahead = measure_post_spike_rms(2.0 / 3.0);
    assert!(
        rms_zero_la > rms_lookahead,
        "0 ms lookahead should preserve pre-spike level rather than ducking ahead: 0ms={rms_zero_la}, 2ms={rms_lookahead}"
    );

    // 3. Delay-compensated Mix=50% at 3.0 ms lookahead has no comb cancellation on unclipped audio
    let mut rack_mix = FxRackState::new(sr);
    let mut p_mix = FxRackParams::default();
    p_mix.buses[0].slots[0].fx_type = FxType::Limiter;
    p_mix.buses[0].slots[0].enabled = true;
    p_mix.buses[0].slots[0].params = [0.0, 1.0, 1.0, 0.5]; // 0 dB gain, 0 dB ceil, 3.0 ms, 50% mix
    p_mix.buses[0].slots[0].eq_params = FxSlotParams::default_eq_params_for_type(FxType::Limiter);
    let mut sq_in = 0.0_f32;
    let mut sq_out = 0.0_f32;
    for n in 0..2048 {
        let x = 0.25 * (2.0 * std::f32::consts::PI * 3000.0 * n as f32 / sr).sin();
        let (l, _r) = rack_mix.process_buses_sample(
            (x, x),
            (0.0, 0.0),
            (0.0, 0.0),
            &p_mix,
            &mod_offsets,
            120.0,
            None,
        );
        if n >= 512 {
            sq_in += x * x;
            sq_out += l * l;
        }
    }
    let ratio = (sq_out / sq_in.max(1e-9)).sqrt();
    assert!(
        (ratio - 1.0).abs() < 0.03,
        "Delay-compensated 50% Mix must preserve amplitude without comb filtering: ratio={ratio}"
    );

    // 4. Mod Matrix modulation on Limiter Gain + idle sleep recovery
    let mut mod_gain = [0.0_f32; NUM_MOD_DESTS];
    mod_gain[ModDestination::fx_bus_slot_param(0, 0, 0).index()] = 0.5;
    let mut rack_mod = FxRackState::new(sr);
    let mut p_mod = FxRackParams::default();
    p_mod.buses[0].slots[0].fx_type = FxType::Limiter;
    p_mod.buses[0].slots[0].enabled = true;
    p_mod.buses[0].slots[0].params = [0.0, 1.0, 0.5, 1.0];
    p_mod.buses[0].slots[0].eq_params = FxSlotParams::default_eq_params_for_type(FxType::Limiter);
    for n in 0..1024 {
        let x = 0.1 * (2.0 * std::f32::consts::PI * 440.0 * n as f32 / sr).sin();
        let (l, r) = rack_mod.process_buses_sample(
            (x, x),
            (0.0, 0.0),
            (0.0, 0.0),
            &p_mod,
            &mod_gain,
            120.0,
            None,
        );
        assert!(l.is_finite() && r.is_finite());
    }
    for _ in 0..1500 {
        let _ = rack_mod.process_buses_sample(
            (0.0, 0.0),
            (0.0, 0.0),
            (0.0, 0.0),
            &p_mod,
            &mod_gain,
            120.0,
            None,
        );
    }
    let (il, ir) = rack_mod.process_buses_sample(
        (0.0, 0.0),
        (0.0, 0.0),
        (0.0, 0.0),
        &p_mod,
        &mod_gain,
        120.0,
        None,
    );
    assert_eq!((il, ir), (0.0, 0.0));
}

#[test]
fn distortion_all_modes_bounded_bias_asymmetry_linear_fold_and_idle() {
    use j_syn_dsp::waveshaper::shape_sample;
    use j_syn_dsp::{
        DistortionMode, FxRackParams, FxRackState, FxSlotParams, FxType, InsertMode,
        InsertPosition, SynthEngine, SynthParams, WavetableBank, NUM_MOD_DESTS,
    };

    let sr = 48_000.0;
    let mod_zero = [0.0_f32; NUM_MOD_DESTS];

    // 1. Every DistortionMode produces finite, bounded audio and enters idle sleep cleanly
    for mode in DistortionMode::ALL {
        let mut rack = FxRackState::new(sr);
        let mut p = FxRackParams::default();
        p.buses[0].slots[0] = FxSlotParams {
            fx_type: FxType::Distortion,
            enabled: true,
            stack: 1,
            distortion_mode: mode,
            params: [0.50, 0.75, 0.50, 1.0],
            ..FxSlotParams::default()
        };
        let mut peak = 0.0_f32;
        for n in 0..1024 {
            let x = 0.6 * (2.0 * std::f32::consts::PI * 220.0 * n as f32 / sr).sin();
            let (l, r) = rack.process_buses_sample((x, x), (0.0, 0.0), (0.0, 0.0), &p, &mod_zero, 120.0, None);
            assert!(l.is_finite() && r.is_finite(), "mode {:?} produced NaN/Inf", mode);
            peak = peak.max(l.abs()).max(r.abs());
        }
        assert!(peak > 0.05 && peak <= 1.5, "mode {:?} peak out of range: {}", mode, peak);

        for _ in 0..1500 {
            let _ = rack.process_buses_sample((0.0, 0.0), (0.0, 0.0), (0.0, 0.0), &p, &mod_zero, 120.0, None);
        }
        let (il, ir) = rack.process_buses_sample((0.0, 0.0), (0.0, 0.0), (0.0, 0.0), &p, &mod_zero, 120.0, None);
        assert_eq!((il, ir), (0.0, 0.0), "mode {:?} did not enter idle sleep", mode);
    }

    // 2. Bias != 0 introduces even-harmonic asymmetry on odd-symmetric shapers (ClassicC, LinearFold, HardClip)
    for mode in [DistortionMode::ClassicC, DistortionMode::LinearFold, DistortionMode::HardClip] {
        let sym_sum = shape_sample(mode, 0.45, 0.8, 0.0) + shape_sample(mode, -0.45, 0.8, 0.0);
        let asym_sum = shape_sample(mode, 0.45, 0.8, 0.6) + shape_sample(mode, -0.45, 0.8, 0.6);
        assert!(sym_sum.abs() < 1e-5, "{:?} should be odd-symmetric at bias=0: {}", mode, sym_sum);
        assert!(asym_sum.abs() > 0.01, "{:?} should be asymmetric at bias=0.6: {}", mode, asym_sum);
    }

    // 3. LinearFold exhibits multi-crossing wavefolding at high drive
    let mut crossings = 0usize;
    let mut prev = shape_sample(DistortionMode::LinearFold, -1.0, 1.0, 0.0);
    for i in 1..=200 {
        let x = -1.0 + 2.0 * (i as f32 / 200.0);
        let cur = shape_sample(DistortionMode::LinearFold, x, 1.0, 0.0);
        if (prev < 0.0 && cur >= 0.0) || (prev > 0.0 && cur <= 0.0) {
            crossings += 1;
        }
        prev = cur;
    }
    assert!(crossings >= 5, "LinearFold should cross zero multiple times at full drive, got {}", crossings);

    // 4. Expanded InsertMode variants produce finite, audible audio
    let bank = WavetableBank::new();
    for ins_mode in [
        InsertMode::ClassicA,
        InsertMode::ClassicB,
        InsertMode::ClassicC,
        InsertMode::DiodeA,
        InsertMode::DiodeB,
        InsertMode::LinearFold,
        InsertMode::SineFold,
        InsertMode::HardClip,
        InsertMode::HalfRectify,
        InsertMode::FullRectify,
    ] {
        let mut eng = SynthEngine::new(sr);
        let mut sp = SynthParams::default();
        sp.oscs[0].enabled = true;
        sp.oscs[0].level = 0.8;
        sp.routing.inserts[0].mode = ins_mode;
        sp.routing.inserts[0].position = InsertPosition::PreFilter;
        sp.routing.inserts[0].drive = 0.7;
        eng.note_on(0, 60, 1.0, None, &sp);
        let mut ins_peak = 0.0_f32;
        for _ in 0..512 {
            let (l, r) = eng.process_sample(&sp, &bank);
            assert!(l.is_finite() && r.is_finite());
            ins_peak = ins_peak.max(l.abs()).max(r.abs());
        }
        assert!(ins_peak > 0.01, "InsertMode {:?} produced silence", ins_mode);
    }
}

#[test]
fn fx_multiband_dist_flat_xover_per_band_harmonics_solo_and_idle() {
    use j_syn_dsp::{
        DistortionMode, FxRackParams, FxRackState, FxSlotParams, FxType, MbDistBandParams,
        ModDestination, MB_BAND_LEVEL_DEFAULT_NORM, MB_XOVER_HIGH_DEFAULT_NORM,
        MB_XOVER_LOW_DEFAULT_NORM, NUM_MOD_DESTS,
    };

    let sr = 96_000.0;
    let mod_zero = [0.0_f32; NUM_MOD_DESTS];

    // 1. Flat unity magnitude sum (+-0.1 dB across 20 Hz .. 20 kHz) when bands are neutral (mix=0 or bypassed)
    for bypass_mode in [false, true] {
        let mut p = FxRackParams::default();
        p.buses[0].slots[0] = FxSlotParams {
            fx_type: FxType::MultibandDist,
            enabled: true,
            stack: 1,
            params: [MB_XOVER_LOW_DEFAULT_NORM, MB_XOVER_HIGH_DEFAULT_NORM, 0.50, 1.0],
            mb_bands: [MbDistBandParams {
                mode: DistortionMode::Tube,
                enabled: !bypass_mode,
                solo: false,
                drive: 0.0,
                bias: 0.5,
                level: MB_BAND_LEVEL_DEFAULT_NORM,
                mix: 0.0,
            }; 3],
            ..FxSlotParams::default()
        };

        for freq_hz in [25.0_f32, 60.0, 150.0, 200.0, 650.0, 1500.0, 2500.0, 6000.0, 12000.0, 18000.0] {
            let mut rack = FxRackState::new(sr);
            let mut rms_in_sq = 0.0_f64;
            let mut rms_out_sq = 0.0_f64;
            // Measure over an exact integer number of cycles after settling the 200 Hz LR4 phase response
            let period_samples = (sr / freq_hz).round().max(1.0) as usize;
            let cycles = ((9_600.0 / period_samples as f32).ceil() as usize).max(4);
            let measure = cycles * period_samples;
            let settle = 24_000;
            for n in 0..(settle + measure) {
                let x = 0.25 * (2.0 * std::f32::consts::PI * freq_hz * n as f32 / sr).sin();
                let (l, r) = rack.process_buses_sample((x, x), (0.0, 0.0), (0.0, 0.0), &p, &mod_zero, 120.0, None);
                assert!(l.is_finite() && r.is_finite());
                if n >= settle {
                    rms_in_sq += (x as f64) * (x as f64);
                    rms_out_sq += (l as f64) * (l as f64);
                }
            }
            let gain_db = 10.0 * (rms_out_sq / rms_in_sq).log10();
            assert!(
                gain_db.abs() <= 0.1,
                "LR4 sum not flat at {freq_hz} Hz (bypass={bypass_mode}): {gain_db:+.4} dB"
            );
        }
    }

    // 2. Independent harmonic generation per band: Low driven with SineFold, High bypassed
    let measure_thd_energy = |freq_hz: f32, bands: [MbDistBandParams; 3]| -> f64 {
        let mut rack = FxRackState::new(sr);
        let mut p = FxRackParams::default();
        p.buses[0].slots[0] = FxSlotParams {
            fx_type: FxType::MultibandDist,
            enabled: true,
            stack: 1,
            params: [MB_XOVER_LOW_DEFAULT_NORM, MB_XOVER_HIGH_DEFAULT_NORM, 0.50, 1.0],
            mb_bands: bands,
            ..FxSlotParams::default()
        };
        let n_fft = 8192;
        let settle = 4096;
        let mut out_buf = Vec::with_capacity(n_fft);
        for n in 0..(settle + n_fft) {
            let x = 0.5 * (2.0 * std::f32::consts::PI * freq_hz * n as f32 / sr).sin();
            let (l, _) = rack.process_buses_sample((x, x), (0.0, 0.0), (0.0, 0.0), &p, &mod_zero, 120.0, None);
            if n >= settle {
                out_buf.push(l as f64);
            }
        }
        // Goertzel at fundamental vs 3rd harmonic
        let goertzel = |target_hz: f32| -> f64 {
            let omega = 2.0 * std::f64::consts::PI * target_hz as f64 / sr as f64;
            let coeff = 2.0 * omega.cos();
            let (mut s1, mut s2) = (0.0, 0.0);
            for &s in &out_buf {
                let s0 = s + coeff * s1 - s2;
                s2 = s1;
                s1 = s0;
            }
            (s1 * s1 + s2 * s2 - coeff * s1 * s2).max(0.0).sqrt()
        };
        let h1 = goertzel(freq_hz).max(1e-9);
        let h3 = goertzel(freq_hz * 3.0);
        h3 / h1
    };

    let low_only_bands = [
        MbDistBandParams {
            mode: DistortionMode::LinearFold,
            enabled: true,
            solo: false,
            drive: 0.85,
            bias: 0.5,
            level: MB_BAND_LEVEL_DEFAULT_NORM,
            mix: 1.0,
        },
        MbDistBandParams {
            enabled: false,
            ..MbDistBandParams::default()
        },
        MbDistBandParams {
            enabled: false,
            ..MbDistBandParams::default()
        },
    ];
    let low_h3 = measure_thd_energy(60.0, low_only_bands);
    let high_h3 = measure_thd_energy(8_000.0, low_only_bands);
    assert!(low_h3 > 0.05, "Low band should generate strong harmonics on 60 Hz: {low_h3}");
    assert!(high_h3 < 0.002, "Bypassed High band should leave 8 kHz clean: {high_h3}");

    // 3. Band solo isolates the soloed band + idle recovery + modulated xover
    let mut solo_rack = FxRackState::new(sr);
    let mut sp = FxRackParams::default();
    sp.buses[0].slots[0] = FxSlotParams {
        fx_type: FxType::MultibandDist,
        enabled: true,
        stack: 1,
        params: [MB_XOVER_LOW_DEFAULT_NORM, MB_XOVER_HIGH_DEFAULT_NORM, 0.50, 1.0],
        mb_bands: [
            MbDistBandParams { solo: true, drive: 0.0, mix: 0.0, ..MbDistBandParams::default() },
            MbDistBandParams { solo: false, drive: 0.0, mix: 0.0, ..MbDistBandParams::default() },
            MbDistBandParams { solo: false, drive: 0.0, mix: 0.0, ..MbDistBandParams::default() },
        ],
        ..FxSlotParams::default()
    };
    let mut high_rms = 0.0_f64;
    for n in 0..8000 {
        let x = 0.4 * (2.0 * std::f32::consts::PI * 8000.0 * n as f32 / sr).sin();
        let (l, _) = solo_rack.process_buses_sample((x, x), (0.0, 0.0), (0.0, 0.0), &sp, &mod_zero, 120.0, None);
        if n >= 4000 {
            high_rms += (l as f64) * (l as f64);
        }
    }
    assert!(high_rms < 1e-3, "Soloing Low band should attenuate 8 kHz high content: {high_rms}");

    // Modulate P1 (LOW XOV) cleanly and verify idle sleep recovery
    let mut mod_xov = [0.0_f32; NUM_MOD_DESTS];
    mod_xov[ModDestination::fx_bus_slot_param(0, 0, 0).index()] = 0.4;
    let (ml, mr) = solo_rack.process_buses_sample((0.1, -0.1), (0.0, 0.0), (0.0, 0.0), &sp, &mod_xov, 120.0, None);
    assert!(ml.is_finite() && mr.is_finite());

    for _ in 0..1500 {
        let _ = solo_rack.process_buses_sample((0.0, 0.0), (0.0, 0.0), (0.0, 0.0), &sp, &mod_zero, 120.0, None);
    }
    let (il, ir) = solo_rack.process_buses_sample((0.0, 0.0), (0.0, 0.0), (0.0, 0.0), &sp, &mod_zero, 120.0, None);
    assert_eq!((il, ir), (0.0, 0.0));
}
