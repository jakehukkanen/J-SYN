//! Per-voice oscillator/noise/filter/insert modulation & bus routing pipeline.

use crate::filter::{
    FilterParams, FilterState, FilterSvfCore, cutoff_hz_to_norm, cutoff_norm_to_hz,
};
use crate::fx::FxBusTopology;
use crate::insert::{
    DirectDest, FilterBusDest, FilterRouting, InsertPosition, InsertShaperState, RoutingParams,
    VoiceFeedbackState,
};
use crate::mod_matrix::{ModDestination, NUM_VOICE_MOD_DESTS};
use crate::noise::{NoiseParams, NoiseState};
use crate::oscillator::{
    EngineMode, ModSource, OscParams, OscillatorState, osc_hz_offset_from_norm,
    osc_hz_offset_to_norm,
};
use crate::wavetable::Wavetable;

use super::{SynthParams, VoiceBusOutput, VoiceControlOverrides};

pub(super) trait StereoFilterStage {
    fn process_filter(
        &mut self,
        in_l: f32,
        in_r: f32,
        params: &FilterParams,
        midi_note: f32,
        sample_rate: f32,
        engine_mode: EngineMode,
    ) -> (f32, f32);
}

impl StereoFilterStage for FilterState {
    #[inline]
    fn process_filter(
        &mut self,
        in_l: f32,
        in_r: f32,
        params: &FilterParams,
        midi_note: f32,
        sample_rate: f32,
        engine_mode: EngineMode,
    ) -> (f32, f32) {
        self.process_stereo(in_l, in_r, params, midi_note, sample_rate, engine_mode)
    }
}

impl StereoFilterStage for FilterSvfCore {
    #[inline]
    fn process_filter(
        &mut self,
        in_l: f32,
        in_r: f32,
        params: &FilterParams,
        midi_note: f32,
        sample_rate: f32,
        engine_mode: EngineMode,
    ) -> (f32, f32) {
        self.process_core_stereo(in_l, in_r, params, midi_note, sample_rate, engine_mode)
    }
}

#[inline]
fn split_filter_send(
    l: f32,
    r: f32,
    balance: f32,
    f1_l: &mut f32,
    f1_r: &mut f32,
    f2_l: &mut f32,
    f2_r: &mut f32,
) {
    let t = (balance.clamp(-1.0, 1.0) + 1.0) * 0.5;
    let w1 = 1.0 - t;
    let w2 = t;
    *f1_l += l * w1;
    *f1_r += r * w1;
    *f2_l += l * w2;
    *f2_r += r * w2;
}

#[inline]
fn run_inserts_at(
    pos: InsertPosition,
    inserts: &mut [InsertShaperState; 2],
    routing: &RoutingParams,
    engine_mode: EngineMode,
    sample_rate: f32,
    mut l: f32,
    mut r: f32,
) -> (f32, f32) {
    for idx in 0..2 {
        if routing.inserts[idx].position == pos {
            (l, r) =
                inserts[idx].process_stereo(l, r, &routing.inserts[idx], engine_mode, sample_rate);
        }
    }
    (l, r)
}

#[inline]
fn apply_osc_modulation(
    osc_p: &OscParams,
    overrides: &VoiceControlOverrides,
    idx: usize,
    mod_offsets: &[f32; NUM_VOICE_MOD_DESTS],
) -> OscParams {
    let mut out = *osc_p;
    out.wt_pos = (overrides.osc_wt_pos[idx] + mod_offsets[ModDestination::osc_wt_pos(idx).index()])
        .clamp(0.0, 1.0);
    out.warp_amount = (overrides.osc_warp_amount[idx]
        + mod_offsets[ModDestination::osc_warp(idx).index()])
    .clamp(0.0, 1.0);
    out.mod_depth = (overrides.osc_mod_depth[idx]
        + mod_offsets[ModDestination::osc_mod_depth(idx).index()])
    .clamp(0.0, 1.0);
    out.pan = (out.pan + mod_offsets[ModDestination::osc_pan(idx).index()] * 2.0).clamp(-1.0, 1.0);
    out.level = (out.level + mod_offsets[ModDestination::osc_level(idx).index()]).clamp(0.0, 1.0);
    out.pitch_semitones = (out.pitch_semitones
        + mod_offsets[ModDestination::osc_pitch(idx).index()] * 96.0)
        .clamp(-48.0, 48.0);
    out.ratio = (out.ratio + mod_offsets[ModDestination::osc_ratio(idx).index()] * 32.0).max(0.0);
    let d_hz = mod_offsets[ModDestination::osc_hz_offset(idx).index()];
    if d_hz.abs() > 1e-6 {
        let norm = osc_hz_offset_to_norm(out.hz_offset);
        out.hz_offset = osc_hz_offset_from_norm((norm + d_hz).clamp(0.0, 1.0));
    }
    out.phase_offset =
        (out.phase_offset + mod_offsets[ModDestination::osc_phase(idx).index()]).rem_euclid(1.0);
    out.detune =
        (out.detune + mod_offsets[ModDestination::osc_detune(idx).index()]).clamp(0.0, 1.0);
    out.spread =
        (out.spread + mod_offsets[ModDestination::osc_spread(idx).index()]).clamp(0.0, 1.0);
    out
}

#[inline]
fn apply_noise_modulation(
    noise: &NoiseParams,
    mod_offsets: &[f32; NUM_VOICE_MOD_DESTS],
) -> NoiseParams {
    let mut out = *noise;
    out.color =
        (out.color + mod_offsets[ModDestination::NoiseColor.index()] * 2.0).clamp(-1.0, 1.0);
    out.stereo = (out.stereo + mod_offsets[ModDestination::NoiseStereo.index()]).clamp(0.0, 1.0);
    out.level = (out.level + mod_offsets[ModDestination::NoiseLevel.index()]).clamp(0.0, 1.0);
    out
}

#[inline]
pub fn apply_filter_modulation(
    flt: &FilterParams,
    idx: usize,
    mod_offsets: &[f32],
) -> FilterParams {
    let mut out = *flt;
    let d_cut = mod_offsets[ModDestination::filter_cutoff(idx).index()];
    if d_cut.abs() > 1e-6 {
        let norm = cutoff_hz_to_norm(out.cutoff_hz);
        out.cutoff_hz = cutoff_norm_to_hz((norm + d_cut).clamp(0.0, 1.0));
    }
    out.resonance =
        (out.resonance + mod_offsets[ModDestination::filter_res(idx).index()]).clamp(0.0, 1.0);
    out.drive =
        (out.drive + mod_offsets[ModDestination::filter_drive(idx).index()]).clamp(0.0, 1.0);
    out.aux = (out.aux + mod_offsets[ModDestination::filter_aux(idx).index()]).clamp(0.0, 1.0);
    out
}

#[inline]
fn apply_filter_modulation_with_overrides(
    flt: &FilterParams,
    overrides: &VoiceControlOverrides,
    idx: usize,
    mod_offsets: &[f32; NUM_VOICE_MOD_DESTS],
) -> FilterParams {
    let mut base = *flt;
    base.cutoff_hz = overrides.filter_cutoff_hz[idx];
    base.resonance = overrides.filter_res[idx];
    base.drive = overrides.filter_drive[idx];
    apply_filter_modulation(&base, idx, mod_offsets)
}

#[inline]
fn apply_routing_modulation(
    routing: &RoutingParams,
    mod_offsets: &[f32; NUM_VOICE_MOD_DESTS],
) -> RoutingParams {
    let mut out = *routing;
    for i in 0..4 {
        out.osc_filter_balance[i] = (out.osc_filter_balance[i]
            + mod_offsets[ModDestination::osc_filter_send(i).index()] * 2.0)
            .clamp(-1.0, 1.0);
    }
    out.noise_filter_balance = (out.noise_filter_balance
        + mod_offsets[ModDestination::NoiseFilterSend.index()] * 2.0)
        .clamp(-1.0, 1.0);
    out.filter_mix =
        (out.filter_mix + mod_offsets[ModDestination::FilterMix.index()] * 2.0).clamp(-1.0, 1.0);
    out.feedback_amount =
        (out.feedback_amount + mod_offsets[ModDestination::FeedbackAmount.index()]).clamp(0.0, 1.0);
    for idx in 0..2 {
        out.inserts[idx].drive = (out.inserts[idx].drive
            + mod_offsets[ModDestination::insert_drive(idx).index()])
        .clamp(0.0, 1.0);
        out.inserts[idx].mix = (out.inserts[idx].mix
            + mod_offsets[ModDestination::insert_mix(idx).index()])
        .clamp(0.0, 1.0);
    }
    out
}

#[inline]
fn route_direct_to_bus(out: &mut VoiceBusOutput, dest: DirectDest, l: f32, r: f32) {
    match dest {
        DirectDest::Off => {}
        DirectDest::BusA => {
            out.bus_a_l += l;
            out.bus_a_r += r;
        }
        DirectDest::BusB => {
            out.bus_b_l += l;
            out.bus_b_r += r;
        }
        DirectDest::Sub => {
            out.sub_l += l;
            out.sub_r += r;
        }
    }
}

#[inline]
fn route_filter_to_bus(
    out: &mut VoiceBusOutput,
    dest: FilterBusDest,
    topology: FxBusTopology,
    l: f32,
    r: f32,
) {
    let (want_a, mut want_b, mut want_c) = match dest {
        FilterBusDest::BusA => (true, false, false),
        FilterBusDest::BusB => (false, true, false),
        FilterBusDest::Both => (true, true, false),
        FilterBusDest::Sub => (false, false, true),
        FilterBusDest::AC => (true, false, true),
        FilterBusDest::BC => (false, true, true),
        FilterBusDest::All => (true, true, true),
    };
    let a_target = topology.bus_a_target();
    let b_target = topology.bus_b_target();
    if want_a && want_b && a_target == 1 {
        want_b = false;
    }
    if want_c
        && ((want_a && a_target == 2)
            || (want_b && b_target == 2)
            || (want_a && a_target == 1 && b_target == 2))
    {
        want_c = false;
    }
    if want_a {
        out.bus_a_l += l;
        out.bus_a_r += r;
    }
    if want_b {
        out.bus_b_l += l;
        out.bus_b_r += r;
    }
    if want_c {
        out.sub_l += l;
        out.sub_r += r;
    }
}

#[inline]
#[allow(clippy::too_many_arguments)]
pub(super) fn process_voice_bus<F: StereoFilterStage>(
    oscillators: &mut [OscillatorState; 4],
    osc_out: &mut [f32; 4],
    prev_osc_out: &mut [f32; 4],
    noise: &mut NoiseState,
    filters: &mut [F; 2],
    inserts: &mut [InsertShaperState; 2],
    inserts_aux: &mut [InsertShaperState; 2],
    feedback: &mut VoiceFeedbackState,
    params: &SynthParams,
    overrides: &VoiceControlOverrides,
    mod_offsets: &[f32; NUM_VOICE_MOD_DESTS],
    tables: [&Wavetable; 4],
    effective_note: f32,
    sample_rate: f32,
) -> VoiceBusOutput {
    // C3 (MIDI 60) = 440 * 2^(-9/12) Hz
    const C3_HZ: f32 = 261.625_58;
    let base_hz = 440.0 * (2.0_f32).powf((effective_note - 69.0) / 12.0);
    let mod_routing = apply_routing_modulation(&params.routing, mod_offsets);
    let mut f1_l = 0.0_f32;
    let mut f1_r = 0.0_f32;
    let mut f2_l = 0.0_f32;
    let mut f2_r = 0.0_f32;
    let mut bus_out = VoiceBusOutput::default();

    let mod_oscs: [OscParams; 4] =
        std::array::from_fn(|i| apply_osc_modulation(&params.oscs[i], overrides, i, mod_offsets));
    let mut osc_active = [false; 4];
    for i in 0..4 {
        if mod_oscs[i].enabled && mod_oscs[i].level > 0.0001 {
            osc_active[i] = true;
        }
    }
    for _ in 0..3 {
        for i in 0..4 {
            if osc_active[i]
                && let Some(src_idx) = mod_oscs[i].mod_source.osc_index()
                && mod_oscs[src_idx].enabled
            {
                osc_active[src_idx] = true;
            }
        }
    }

    // Evaluate oscillators 1 -> 4 sequentially for zero-latency forward / 1-sample feedback cross-PM.
    for a in 0..4 {
        if !osc_active[a] {
            oscillators[a].fb_lp_state = 0.0;
            oscillators[a].fb_dc_x1 = 0.0;
            oscillators[a].fb_dc_y1 = 0.0;
            osc_out[a] = 0.0;
            continue;
        }
        let osc_p = mod_oscs[a];

        let (mod_sig, is_fb) = match osc_p.mod_source {
            ModSource::Off => (0.0, false),
            ModSource::Osc1 => {
                let b = 0;
                if b < a {
                    (osc_out[b], false)
                } else {
                    (prev_osc_out[b], true)
                }
            }
            ModSource::Osc2 => {
                let b = 1;
                if b < a {
                    (osc_out[b], false)
                } else {
                    (prev_osc_out[b], true)
                }
            }
            ModSource::Osc3 => {
                let b = 2;
                if b < a {
                    (osc_out[b], false)
                } else {
                    (prev_osc_out[b], true)
                }
            }
            ModSource::Osc4 => {
                let b = 3;
                if b < a {
                    (osc_out[b], false)
                } else {
                    (prev_osc_out[b], true)
                }
            }
        };

        let osc_hz = if osc_p.keytrack { base_hz } else { C3_HZ };
        let (raw, l, r) = oscillators[a].process_sample(
            a,
            &osc_p,
            tables[a],
            osc_hz,
            sample_rate,
            params.engine_mode,
            mod_sig,
            is_fb,
        );
        osc_out[a] = raw;
        if osc_p.level > 0.0001 {
            if mod_routing.osc_direct[a] != DirectDest::Off {
                route_direct_to_bus(&mut bus_out, mod_routing.osc_direct[a], l, r);
            } else {
                split_filter_send(
                    l,
                    r,
                    mod_routing.osc_filter_balance[a],
                    &mut f1_l,
                    &mut f1_r,
                    &mut f2_l,
                    &mut f2_r,
                );
            }
        }
    }

    *prev_osc_out = *osc_out;

    // Noise Generator
    let mod_noise = apply_noise_modulation(&params.noise, mod_offsets);
    let (nl, nr) = noise.process_sample(&mod_noise, sample_rate);
    if mod_routing.noise_direct != DirectDest::Off {
        route_direct_to_bus(&mut bus_out, mod_routing.noise_direct, nl, nr);
    } else {
        split_filter_send(
            nl,
            nr,
            mod_routing.noise_filter_balance,
            &mut f1_l,
            &mut f1_r,
            &mut f2_l,
            &mut f2_r,
        );
    }

    // Voice Feedback Bus (1-sample delayed post-filter output back into pre-filter sum)
    let (fb_l, fb_r) = feedback.read_feedback(mod_routing.feedback_amount, sample_rate);
    let mod_flt0 =
        apply_filter_modulation_with_overrides(&params.filters[0], overrides, 0, mod_offsets);
    let mod_flt1 =
        apply_filter_modulation_with_overrides(&params.filters[1], overrides, 1, mod_offsets);

    f1_l += fb_l;
    f1_r += fb_r;
    if mod_routing.filter_routing == FilterRouting::Parallel && mod_flt1.enabled {
        f2_l += fb_l;
        f2_r += fb_r;
    }

    let f1_has = f1_l.abs() + f1_r.abs() > 0.0;
    let f2_has = f2_l.abs() + f2_r.abs() > 0.0;
    if f1_has || !f2_has {
        (f1_l, f1_r) = run_inserts_at(
            InsertPosition::PreFilter,
            inserts,
            &mod_routing,
            params.engine_mode,
            sample_rate,
            f1_l,
            f1_r,
        );
    }
    if f2_has {
        (f2_l, f2_r) = run_inserts_at(
            InsertPosition::PreFilter,
            inserts_aux,
            &mod_routing,
            params.engine_mode,
            sample_rate,
            f2_l,
            f2_r,
        );
    }

    // Filter 1
    let (mut out1_l, mut out1_r) = filters[0].process_filter(
        f1_l,
        f1_r,
        &mod_flt0,
        effective_note,
        sample_rate,
        params.engine_mode,
    );

    (out1_l, out1_r) = run_inserts_at(
        InsertPosition::BetweenFilters,
        inserts,
        &mod_routing,
        params.engine_mode,
        sample_rate,
        out1_l,
        out1_r,
    );

    // Filter 2 + Serial / Parallel Bus Routing + Parallel FX Bus Assignment
    let mix = mod_routing.filter_mix.clamp(-1.0, 1.0);
    let gain_f1 = (1.0 - mix).clamp(0.0, 1.0);
    let gain_f2 = (1.0 + mix).clamp(0.0, 1.0);

    match mod_routing.filter_routing {
        FilterRouting::Serial => {
            let (mut post_l, mut post_r, out_dest) = if mod_flt1.enabled {
                let (out2_l, out2_r) = filters[1].process_filter(
                    out1_l + f2_l,
                    out1_r + f2_r,
                    &mod_flt1,
                    effective_note,
                    sample_rate,
                    params.engine_mode,
                );
                let mixed = if mix >= 0.0 {
                    (out2_l, out2_r)
                } else {
                    let w1 = -mix;
                    let w2 = 1.0 + mix;
                    (out1_l * w1 + out2_l * w2, out1_r * w1 + out2_r * w2)
                };
                (mixed.0, mixed.1, mod_routing.filter_out_bus[1])
            } else {
                (
                    out1_l * gain_f1 + f2_l * gain_f2,
                    out1_r * gain_f1 + f2_r * gain_f2,
                    mod_routing.filter_out_bus[0],
                )
            };

            (post_l, post_r) = run_inserts_at(
                InsertPosition::PostFilter,
                inserts,
                &mod_routing,
                params.engine_mode,
                sample_rate,
                post_l,
                post_r,
            );
            feedback.store_post_filter(post_l, post_r);
            route_filter_to_bus(
                &mut bus_out,
                out_dest,
                params.fx_bus_topology,
                post_l,
                post_r,
            );
        }
        FilterRouting::Parallel => {
            let (out2_l, out2_r) = filters[1].process_filter(
                f2_l,
                f2_r,
                &mod_flt1,
                effective_note,
                sample_rate,
                params.engine_mode,
            );
            let b1_l = out1_l * gain_f1;
            let b1_r = out1_r * gain_f1;
            let b2_l = out2_l * gain_f2;
            let b2_r = out2_r * gain_f2;

            if mod_routing.filter_out_bus[0] == mod_routing.filter_out_bus[1] {
                let (mut post_l, mut post_r) = (b1_l + b2_l, b1_r + b2_r);
                (post_l, post_r) = run_inserts_at(
                    InsertPosition::PostFilter,
                    inserts,
                    &mod_routing,
                    params.engine_mode,
                    sample_rate,
                    post_l,
                    post_r,
                );
                feedback.store_post_filter(post_l, post_r);
                route_filter_to_bus(
                    &mut bus_out,
                    mod_routing.filter_out_bus[0],
                    params.fx_bus_topology,
                    post_l,
                    post_r,
                );
            } else {
                let (post1_l, post1_r) = run_inserts_at(
                    InsertPosition::PostFilter,
                    inserts,
                    &mod_routing,
                    params.engine_mode,
                    sample_rate,
                    b1_l,
                    b1_r,
                );
                let (post2_l, post2_r) = run_inserts_at(
                    InsertPosition::PostFilter,
                    inserts_aux,
                    &mod_routing,
                    params.engine_mode,
                    sample_rate,
                    b2_l,
                    b2_r,
                );
                feedback.store_post_filter(post1_l + post2_l, post1_r + post2_r);
                route_filter_to_bus(
                    &mut bus_out,
                    mod_routing.filter_out_bus[0],
                    params.fx_bus_topology,
                    post1_l,
                    post1_r,
                );
                route_filter_to_bus(
                    &mut bus_out,
                    mod_routing.filter_out_bus[1],
                    params.fx_bus_topology,
                    post2_l,
                    post2_r,
                );
            }
        }
    }

    bus_out
}
