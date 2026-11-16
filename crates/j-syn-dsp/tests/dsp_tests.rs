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
        FilterMode, FilterParams, FilterSlope, InsertMode, InsertPosition, SynthEngine,
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
        params.routing.osc_direct[0] = osc_direct;
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
        evaluate_modseq_curve, evaluate_mseg_curve, evaluate_step_shape, LfoPreset,
        ModDestination, ModMatrixSource, ModSeqMode, ModSeqStep, ModSeqStepShape,
        ModSeqSyncMode, ModSlot, MsegCurve, MsegNode, MAX_MODSEQ_STEPS, MAX_MSEG_NODES,
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
        init_params.fx_slots[idx] = FxSlotParams {
            fx_type,
            enabled: true,
            stack: 1,
            params: FxSlotParams::default_params_for_type(fx_type),
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
    let _ = eng.telemetry().fx_scopes[3].read_snapshot(&mut pre_buf, &mut post_buf);
    assert!(pre_buf.iter().any(|&v| v > 0.0), "Expected live FX scope telemetry");

    // 2. Series STACK 4 vs STACK 1 changes output energy and J-SAT ceiling clipping limits post peak
    let run_peak = |stack: u8, ceil_norm: f32, mod_wheel: f32| -> f32 {
        let mut p = SynthParams::default();
        p.fx_slots = [FxSlotParams::default(); NUM_FX_SLOTS];
        p.fx_slots[0] = FxSlotParams {
            fx_type: FxType::Distortion,
            enabled: true,
            stack,
            params: [0.0, 0.75, 0.5, 1.0],
        };
        p.fx_slots[1] = FxSlotParams {
            fx_type: FxType::JSatClipper,
            enabled: true,
            stack: 1,
            // Drive +12 dB (0.50), Ceiling configurable, Knee 0% (hard clip), Mix 100%
            params: [0.50, ceil_norm, 0.0, 1.0],
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
    use j_syn_dsp::{FxSlotParams, FxType, ModDestination, ModMatrixSource, ModSlot};

    let bank = WavetableBank::new();
    // All 7 FxType::Filter modes process cleanly with finite output
    for mode_idx in 0..7 {
        let mut p = SynthParams::default();
        p.oscs[0].wt_pos = 0.75; // rich harmonics so HP/BP modes are audible
        p.fx_slots[0] = FxSlotParams {
            fx_type: FxType::Filter,
            enabled: true,
            stack: 1,
            params: [0.45, 0.4, (mode_idx as f32) / 6.0, 1.0],
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
