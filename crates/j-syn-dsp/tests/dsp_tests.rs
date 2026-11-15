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
    params.oscs[0].enabled = true;
    params.oscs[0].level = 0.0;
    params.oscs[0].mod_source = ModSource::Osc2; // backward mod 2 -> 1
    params.oscs[0].mod_type = ModType::Pm;
    params.oscs[0].mod_depth = 0.9;

    params.oscs[1].enabled = true;
    params.oscs[1].level = 0.8;
    params.oscs[1].mod_source = ModSource::Osc1; // forward mod 1 -> 2
    params.oscs[1].mod_type = ModType::Pm;
    params.oscs[1].mod_depth = 0.85;

    // Self-modulation on OSC 3
    params.oscs[2].enabled = true;
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

    // Trigger 20 notes on 16-voice engine
    for n in 48..68 {
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
    params.oscs[0].enabled = true;
    params.oscs[0].wt_pos = 0.0;
    params.oscs[0].level = 0.8;
    params.oscs[0].pan = 0.0;
    params.oscs[0].retrig = true;
    params.oscs[0].phase_offset = (0.0_f32 / 360.0).rem_euclid(1.0).fract();

    params.oscs[1].enabled = true;
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
    params.oscs[0].enabled = true;
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
        params_base.oscs[0].enabled = true;
        params_base.oscs[0].level = 0.8;
        params_base.oscs[1].enabled = true;
        params_base.oscs[1].level = 0.0; // Silent modulator
        params_base.oscs[1].semi = 7;

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
        params_base.oscs[0].enabled = true;
        params_base.oscs[0].level = 0.0; // Silent modulator
        params_base.oscs[0].semi = 7;
        params_base.oscs[1].enabled = true;
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
