//! Real-time audio-thread zero-allocation verification (`assert_no_alloc`).
//!
//! Runs in its own integration test binary so `AllocDisabler` can act as the
//! `#[global_allocator]` without interfering with allocation-heavy unit tests.

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use j_syn_dsp::{
    DirectDest, EngineMode, FilterBusDest, FilterMode, FilterRouting, FilterSlope, FxBusTopology,
    FxSlotParams, FxType, InsertMode, InsertPosition, MAX_FX_STACK, MAX_VOICES, ModDestination,
    ModMatrixSource, ModSeqMode, ModSeqPlayMode, ModSeqRetrig, ModSlot, ModSource, NoiseType,
    OscPitchMode, PolyMode, SynthEngine, SynthParams, WarpMode, WavetableBank,
};

#[global_allocator]
static ALLOC_GUARD: AllocDisabler = AllocDisabler;

#[test]
fn audio_thread_polyphony_mpe_voice_stealing_and_all_fx_zero_alloc() {
    let bank = WavetableBank::new();
    let wt = bank.factory_table(j_syn_dsp::FactoryWavetable::BasicShapes);
    let tables = [wt, wt, wt, wt];

    let mut engine = Box::new(SynthEngine::new(48_000.0));
    let mut params = SynthParams::default();
    params.engine_mode = EngineMode::Hq;
    params.poly_mode = PolyMode::Poly;
    params.mpe_enabled = true;
    params.mpe_pitch_bend_range = 48.0;
    params.dc_blocker = true;
    params.master_limiter = true;
    params.amp_attack_s = 0.002;
    params.amp_decay_s = 0.08;
    params.amp_sustain = 0.7;
    params.amp_release_s = 0.05;

    for (i, osc) in params.oscs.iter_mut().enumerate() {
        osc.enabled = true;
        osc.level = 0.4;
        osc.unison = 4;
        osc.detune = 0.3;
        osc.spread = 0.7;
        osc.warp_mode = WarpMode::from_index(i);
        osc.warp_amount = 0.4;
        osc.pitch_mode = if i % 2 == 0 {
            OscPitchMode::Semitones
        } else {
            OscPitchMode::Ratio
        };
        osc.hz_offset = if i == 2 { 12.5 } else { 0.0 };
    }
    params.oscs[1].mod_source = ModSource::Osc1;
    params.oscs[1].mod_depth = 0.35;
    params.oscs[2].mod_source = ModSource::Osc2;
    params.oscs[2].mod_depth = 0.25;

    params.noise.enabled = true;
    params.noise.noise_type = NoiseType::Pink;
    params.noise.level = 0.2;

    params.filters[0].enabled = true;
    params.filters[0].mode = FilterMode::Scream;
    params.filters[0].slope = FilterSlope::Db24;
    params.filters[1].enabled = true;
    params.filters[1].mode = FilterMode::Comb;
    params.filters[1].slope = FilterSlope::Db12;

    params.routing.filter_routing = FilterRouting::Parallel;
    params.routing.filter_out_bus = [FilterBusDest::BusA, FilterBusDest::BusB];
    params.routing.osc_direct[3] = DirectDest::Sub;
    params.routing.feedback_amount = 0.25;
    params.routing.inserts[0].enabled = true;
    params.routing.inserts[0].mode = InsertMode::TubeToMech;
    params.routing.inserts[0].position = InsertPosition::PreFilter;
    params.routing.inserts[0].drive = 0.5;
    params.routing.inserts[1].enabled = true;
    params.routing.inserts[1].mode = InsertMode::SineFold;
    params.routing.inserts[1].position = InsertPosition::PostFilter;
    params.routing.inserts[1].drive = 0.4;

    params.sub_bus.enabled = true;
    params.sub_bus.drive_clamp();

    params.modseqs[0].mode = ModSeqMode::Performer;
    params.modseqs[0].retrig = ModSeqRetrig::PolyVoice;
    params.modseqs[1].mode = ModSeqMode::Mseg;
    params.modseqs[1].retrig = ModSeqRetrig::MonoGlobal;
    params.modseqs[1].play_mode = ModSeqPlayMode::PingPong;

    // Populate representative ModMatrix routes across synth + FX destinations
    let routes = [
        (ModMatrixSource::Env2, ModDestination::Filter1Cutoff, 0.5),
        (ModMatrixSource::ModSeq1, ModDestination::Osc1WtPos, 0.6),
        (ModMatrixSource::ModSeq2, ModDestination::BusALevel, 0.25),
        (ModMatrixSource::Aftertouch, ModDestination::Filter2Res, 0.4),
        (ModMatrixSource::Slide, ModDestination::Osc2ModDepth, 0.5),
        (ModMatrixSource::Velocity, ModDestination::Insert1Drive, 0.3),
        (
            ModMatrixSource::Macro1,
            ModDestination::fx_bus_slot_param(0, 0, 0),
            0.4,
        ),
        (
            ModMatrixSource::Macro2,
            ModDestination::fx_bus_slot_param(1, 0, 1),
            -0.3,
        ),
        (
            ModMatrixSource::Macro3,
            ModDestination::fx_bus_slot_param(2, 0, 2),
            0.25,
        ),
    ];
    for (idx, (src, dst, depth)) in routes.into_iter().enumerate() {
        params.mod_matrix.slots[idx] = ModSlot {
            source: src,
            destination: dst,
            depth,
            bipolar: idx % 2 == 0,
            enabled: true,
            bypassed: false,
            aux_source: if idx == 0 {
                ModMatrixSource::ModWheel
            } else {
                ModMatrixSource::None
            },
            curve: 0.2,
        };
    }
    params.macros = [0.6, 0.4, 0.5, 0.2, 0.1, 0.7, 0.8, 0.9];

    let all_fx: Vec<FxType> = FxType::ALL
        .into_iter()
        .filter(|&ty| ty != FxType::Off)
        .collect();
    let topologies = [
        FxBusTopology::Parallel,
        FxBusTopology::Serial,
        FxBusTopology::SerialAll,
        FxBusTopology::ABToC,
        FxBusTopology::AToC,
        FxBusTopology::BToC,
    ];

    let (non_finite_count, peak) = assert_no_alloc(|| {
        let mut non_finite = 0usize;
        let mut max_peak = 0.0_f32;

        // Trigger more than MAX_VOICES notes to force voice stealing + TailVoice crossfade
        for n in 0..(MAX_VOICES + 12) {
            let ch = ((n % 15) + 1) as u8;
            let note = (36 + (n % 48)) as u8;
            engine.note_on(ch, note, 0.8, Some(n as i32), &params);
            engine.set_pitch_bend_ch(ch, 0.55, &params);
            engine.set_aftertouch_ch(ch, 0.4, &params);
            engine.set_slide_ch(ch, 0.6, &params);
            engine.set_poly_pressure(ch, note, Some(n as i32), 0.5);
            engine.set_poly_tuning(ch, note, Some(n as i32), 0.75);
            engine.set_poly_brightness(ch, note, Some(n as i32), 0.65);

            for _ in 0..8 {
                let (l, r) = engine.process_sample_with_tables(&params, tables);
                if !l.is_finite() || !r.is_finite() {
                    non_finite += 1;
                }
                max_peak = max_peak.max(l.abs()).max(r.abs());
            }
        }

        // Sweep every FxType across Bus A/B/C with STACK 1..=MAX_FX_STACK and all topologies
        for (idx, &fx_ty) in all_fx.iter().enumerate() {
            params.fx_bus_topology = topologies[idx % topologies.len()];
            let mut slot = FxSlotParams::init_patch_slot(0);
            slot.fx_type = fx_ty;
            slot.enabled = true;
            slot.stack = ((idx % MAX_FX_STACK) + 1) as u8;
            slot.params = FxSlotParams::default_params_for_type(fx_ty);
            slot.eq_params = FxSlotParams::default_eq_params_for_type(fx_ty);
            params.fx_bus_a.slots[0] = slot;
            params.fx_bus_b.slots[0] = slot;
            params.fx_bus_c.slots[0] = slot;

            for _ in 0..32 {
                let (l, r) = engine.process_sample_with_tables(&params, tables);
                if !l.is_finite() || !r.is_finite() {
                    non_finite += 1;
                }
                max_peak = max_peak.max(l.abs()).max(r.abs());
            }
        }

        // Release all notes and process release tail
        engine.all_notes_off();
        for _ in 0..128 {
            let (l, r) = engine.process_sample_with_tables(&params, tables);
            if !l.is_finite() || !r.is_finite() {
                non_finite += 1;
            }
            max_peak = max_peak.max(l.abs()).max(r.abs());
        }

        (non_finite, max_peak)
    });

    assert_eq!(
        non_finite_count, 0,
        "Audio thread emitted non-finite samples"
    );
    assert!(
        peak > 0.001,
        "Expected audible signal during RT no-alloc torture test"
    );
    assert!(
        peak <= 1.0,
        "Safety limiter should keep output bounded <= 1.0, got {peak}"
    );
}

trait SubBusClampHelper {
    fn drive_clamp(&mut self);
}

impl SubBusClampHelper for j_syn_dsp::SubBusParams {
    fn drive_clamp(&mut self) {
        self.lp_cutoff_hz = 120.0;
        self.clip_drive_db = 6.0;
        self.level = 0.8;
    }
}
