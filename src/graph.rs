use crate::model::{FxConfig, InstrumentConfig, NoteEvent, Song, Track};
use crate::soundfont::render_soundfont_track;
use crate::synth::{DeterministicNoise, calculate_envelope, generate_modulated_sample};
use hound::{WavSpec, WavWriter};
use std::collections::{HashMap, HashSet};
use std::f32::consts::{FRAC_1_SQRT_2, PI, SQRT_2};
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_OGG_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct RenderedAudio {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>, // interleaved: [L0, R0, L1, R1, ...]
    pub loop_samples: Option<(u32, u32)>,
}

fn render_raw_track(
    t_idx: usize,
    track: &Track,
    events: &[NoteEvent],
    sample_rate: u32,
    sec_per_beat: f32,
    total_samples: usize,
) -> (Vec<f32>, Vec<f32>) {
    match &track.instrument {
        InstrumentConfig::SoundFont {
            soundfont_path,
            bank,
            preset,
        } => match render_soundfont_track(
            events,
            soundfont_path,
            *bank,
            *preset,
            sample_rate,
            sec_per_beat,
            total_samples,
        ) {
            Ok(buffers) => buffers,
            Err(err) => {
                eprintln!(
                    "Warning: Failed to render soundfont track '{}' ({}): {}",
                    track.id, soundfont_path, err
                );
                (vec![0.0; total_samples], vec![0.0; total_samples])
            }
        },
        InstrumentConfig::Synth { .. } => {
            let track_mono = render_synth_track(
                &track.instrument,
                events,
                sample_rate,
                sec_per_beat,
                total_samples,
                t_idx as u32,
            );
            // Constant-power mono-to-stereo baseline before unified stereo FX chain
            let left: Vec<f32> = track_mono.iter().map(|&s| s * FRAC_1_SQRT_2).collect();
            let right = left.clone();
            (left, right)
        }
    }
}

fn interleave_stereo(left: &[f32], right: &[f32]) -> Vec<f32> {
    let mut interleaved = Vec::with_capacity(left.len() * 2);
    for (&l, &r) in left.iter().zip(right.iter()) {
        interleaved.push(l);
        interleaved.push(r);
    }
    interleaved
}

fn compile_song_internal(
    song: &Song,
    include_stems: bool,
) -> (RenderedAudio, Vec<(String, RenderedAudio)>) {
    let sample_rate = song.sample_rate.unwrap_or(44100);
    let sec_per_beat = 60.0 / song.tempo.max(1.0);

    let resolved_events: Vec<Vec<NoteEvent>> = song
        .tracks
        .iter()
        .map(|track| track.resolved_events(&song.patterns))
        .collect();

    let mut total_duration_sec = 0.0_f32;
    for (track, events) in song.tracks.iter().zip(resolved_events.iter()) {
        let release = match &track.instrument {
            InstrumentConfig::Synth { adsr, .. } => adsr.release_sec,
            InstrumentConfig::SoundFont { .. } => 0.5,
        };
        for ev in events {
            let note_end = (ev.start_beats + ev.duration_beats) * sec_per_beat + release;
            if note_end > total_duration_sec {
                total_duration_sec = note_end;
            }
        }
    }

    let has_tail_fx = song.loop_points.is_none()
        && (has_spatial_fx(&song.master_fx) || song.tracks.iter().any(|t| has_spatial_fx(&t.fx)));
    let tail_sec = if has_tail_fx { 1.0_f32 } else { 0.1_f32 };

    let total_samples = ((total_duration_sec + tail_sec) * sample_rate as f32).ceil() as usize;

    // Pass 1: Parallel stem synthesis via std::thread::scope
    let mut track_buffers: Vec<(Vec<f32>, Vec<f32>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = song
            .tracks
            .iter()
            .zip(resolved_events.iter())
            .enumerate()
            .map(|(t_idx, (track, events))| {
                scope.spawn(move || {
                    render_raw_track(
                        t_idx,
                        track,
                        events,
                        sample_rate,
                        sec_per_beat,
                        total_samples,
                    )
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("Track render thread panicked"))
            .collect()
    });

    // Extract mono detector signals only for tracks referenced by Sidechain FX
    let sc_targets: HashSet<&str> = song
        .tracks
        .iter()
        .flat_map(|t| t.fx.iter())
        .chain(song.master_fx.iter())
        .filter_map(|fx| match fx {
            FxConfig::Sidechain { source_track, .. } => Some(source_track.as_str()),
            _ => None,
        })
        .collect();

    let sidechain_refs: HashMap<String, Vec<f32>> = if sc_targets.is_empty() {
        HashMap::new()
    } else {
        song.tracks
            .iter()
            .zip(track_buffers.iter())
            .filter(|(t, _)| sc_targets.contains(t.id.as_str()))
            .map(|(t, (l, r))| {
                let mono: Vec<f32> = l
                    .iter()
                    .zip(r.iter())
                    .map(|(&lv, &rv)| lv.abs().max(rv.abs()))
                    .collect();
                (t.id.clone(), mono)
            })
            .collect()
    };

    // Pass 2: Parallel track FX chains (including Sidechain, Compressor, StereoWidth)
    std::thread::scope(|scope| {
        let sc_map = &sidechain_refs;
        for (track, (left, right)) in song.tracks.iter().zip(track_buffers.iter_mut()) {
            scope.spawn(move || {
                apply_stereo_fx(left, right, &track.fx, sample_rate, sec_per_beat, sc_map);
            });
        }
    });

    let loop_samples = song.loop_points.map(|lp| {
        let start_sample = (lp.start_beat * sec_per_beat * sample_rate as f32).round() as u32;
        let end_sample = match lp.end_beat {
            Some(eb) => (eb * sec_per_beat * sample_rate as f32).round() as u32,
            None => total_samples as u32,
        };
        (start_sample, end_sample)
    });

    let stems = if include_stems {
        song.tracks
            .iter()
            .zip(track_buffers.iter())
            .map(|(track, (left, right))| {
                (
                    track.id.clone(),
                    RenderedAudio {
                        sample_rate,
                        channels: 2,
                        samples: interleave_stereo(left, right),
                        loop_samples,
                    },
                )
            })
            .collect()
    } else {
        Vec::new()
    };

    let mut master_left = vec![0.0_f32; total_samples];
    let mut master_right = vec![0.0_f32; total_samples];
    for (left, right) in &track_buffers {
        for i in 0..total_samples {
            master_left[i] += left[i];
            master_right[i] += right[i];
        }
    }

    if !song.master_fx.is_empty() {
        apply_stereo_fx(
            &mut master_left,
            &mut master_right,
            &song.master_fx,
            sample_rate,
            sec_per_beat,
            &sidechain_refs,
        );
    }

    let full_mix = RenderedAudio {
        sample_rate,
        channels: 2,
        samples: interleave_stereo(&master_left, &master_right),
        loop_samples,
    };

    (full_mix, stems)
}

pub fn compile_song(song: &Song) -> RenderedAudio {
    compile_song_internal(song, false).0
}

pub fn compile_stems(song: &Song) -> Vec<(String, RenderedAudio)> {
    compile_song_internal(song, true).1
}

pub fn compile_song_with_stems(song: &Song) -> (RenderedAudio, Vec<(String, RenderedAudio)>) {
    compile_song_internal(song, true)
}

fn has_spatial_fx(fx_list: &[FxConfig]) -> bool {
    fx_list
        .iter()
        .any(|fx| matches!(fx, FxConfig::Delay { .. } | FxConfig::Reverb { .. }))
}

fn render_synth_track(
    instrument: &InstrumentConfig,
    events: &[NoteEvent],
    sample_rate: u32,
    sec_per_beat: f32,
    total_samples: usize,
    track_seed: u32,
) -> Vec<f32> {
    let (waveform, adsr, pitch_env, unison, lfo, fm) = match instrument {
        InstrumentConfig::Synth {
            waveform,
            adsr,
            pitch_env,
            unison,
            lfo,
            fm,
        } => (
            *waveform,
            adsr,
            pitch_env.as_ref(),
            unison.as_ref(),
            lfo.as_ref(),
            fm.as_ref(),
        ),
        _ => return vec![0.0; total_samples],
    };

    let mut buffer = vec![0.0_f32; total_samples];
    let mut noise_gen = DeterministicNoise::new(track_seed + 1);
    let two_pi = 2.0 * PI;
    let two_pi_over_sr = two_pi / sample_rate as f32;

    let voice_count = unison.map(|u| u.voices.clamp(1, 7) as usize).unwrap_or(1);
    let detune_cents = unison
        .map(|u| u.detune_cents.clamp(0.0, 200.0))
        .unwrap_or(0.0);
    let unison_norm = 1.0 / (voice_count as f32).sqrt();
    let voice_detunes: Vec<f32> = (0..voice_count)
        .map(|v| {
            if voice_count == 1 {
                0.0
            } else {
                let frac = (v as f32 / (voice_count - 1) as f32) * 2.0 - 1.0;
                (frac * detune_cents * 0.5) / 100.0
            }
        })
        .collect();

    for (ev_idx, ev) in events.iter().enumerate() {
        let start_sec = ev.start_beats * sec_per_beat;
        let duration_sec = ev.duration_beats * sec_per_beat;
        let glide_dur_sec = ev.glide_beats.unwrap_or(ev.duration_beats).max(0.0001) * sec_per_beat;
        let notes = ev.notes();
        let note_count = notes.len().max(1) as f32;
        let velocity_per_voice = (ev.velocity / note_count.sqrt()) * unison_norm;

        let start_sample = (start_sec * sample_rate as f32) as usize;
        let note_total_sec = duration_sec + adsr.release_sec;
        let note_total_samples = (note_total_sec * sample_rate as f32).ceil() as usize;
        let end_sample = (start_sample + note_total_samples).min(total_samples);

        for &midi in &notes {
            for (v_idx, &detune_semi) in voice_detunes.iter().enumerate() {
                let mut phase = (v_idx as f32 / voice_count as f32) * two_pi;
                let mut mod_phase = 0.0_f32;

                for (offset, s) in buffer[start_sample..end_sample].iter_mut().enumerate() {
                    let t = offset as f32 / sample_rate as f32;

                    let base_midi = if let Some(target_midi) = ev.slide_to_midi {
                        let frac = (t / glide_dur_sec).clamp(0.0, 1.0);
                        midi as f32 + (target_midi as f32 - midi as f32) * frac
                    } else {
                        midi as f32
                    };

                    let pitch_drop = if let Some(penv) = pitch_env {
                        if penv.decay_sec > 0.0001 && t <= penv.decay_sec {
                            let factor = 1.0 - (t / penv.decay_sec);
                            penv.drop_semitones * factor
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    };

                    let lfo_semi = if let Some(lfo_cfg) = lfo {
                        lfo_cfg.depth_semitones * (two_pi * lfo_cfg.rate_hz.max(0.0) * t).sin()
                    } else {
                        0.0
                    };

                    let effective_midi =
                        (base_midi + pitch_drop + detune_semi + lfo_semi).clamp(0.0, 127.0);
                    let freq = 440.0 * 2.0_f32.powf((effective_midi - 69.0) / 12.0);

                    let raw_sample =
                        generate_modulated_sample(waveform, phase, mod_phase, fm, &mut noise_gen);
                    phase = (phase + two_pi_over_sr * freq) % two_pi;
                    if let Some(fm_cfg) = fm {
                        mod_phase = (mod_phase
                            + two_pi_over_sr * freq * fm_cfg.mod_ratio.max(0.01))
                            % two_pi;
                    }
                    let env = calculate_envelope(t, duration_sec, adsr);
                    *s += raw_sample * env * velocity_per_voice;
                }
            }
        }

        noise_gen = DeterministicNoise::new(track_seed.wrapping_add(ev_idx as u32 * 31 + 7));
    }

    buffer
}

fn apply_biquad_channel(samples: &mut [f32], b0: f32, b1: f32, b2: f32, a1: f32, a2: f32) {
    let (mut x1, mut x2, mut y1, mut y2) = (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32);
    for s in samples.iter_mut() {
        let x0 = *s;
        let y0 = b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        x2 = x1;
        x1 = x0;
        y2 = y1;
        y1 = y0;
        *s = y0;
    }
}

fn apply_biquad_filter(
    left: &mut [f32],
    right: &mut [f32],
    sample_rate: u32,
    cutoff_hz: f32,
    q: f32,
    is_highpass: bool,
) {
    let nyquist_guard = (sample_rate as f32 * 0.45).max(100.0);
    let freq = cutoff_hz.clamp(20.0, nyquist_guard);
    let q_clamped = q.clamp(0.1, 20.0);
    let w0 = 2.0 * PI * freq / sample_rate as f32;
    let cos_w0 = w0.cos();
    let alpha = w0.sin() / (2.0 * q_clamped);
    let a0 = 1.0 + alpha;

    let (b0, b1, b2) = if is_highpass {
        let c = (1.0 + cos_w0) * 0.5;
        (c / a0, -(1.0 + cos_w0) / a0, c / a0)
    } else {
        let c = (1.0 - cos_w0) * 0.5;
        (c / a0, (1.0 - cos_w0) / a0, c / a0)
    };
    let a1 = (-2.0 * cos_w0) / a0;
    let a2 = (1.0 - alpha) / a0;

    apply_biquad_channel(left, b0, b1, b2, a1, a2);
    apply_biquad_channel(right, b0, b1, b2, a1, a2);
}

fn apply_stereo_delay(
    left: &mut [f32],
    right: &mut [f32],
    sample_rate: u32,
    sec_per_beat: f32,
    delay_beats: f32,
    feedback: f32,
    mix: f32,
) {
    let delay_samples = (delay_beats.max(0.01) * sec_per_beat * sample_rate as f32)
        .round()
        .max(1.0) as usize;
    let fb = feedback.clamp(0.0, 0.92);
    let wet = mix.clamp(0.0, 1.0);
    let dry = 1.0 - wet;

    let mut buf_l = vec![0.0_f32; delay_samples];
    let mut buf_r = vec![0.0_f32; delay_samples];
    let mut idx = 0usize;

    for (l, r) in left.iter_mut().zip(right.iter_mut()) {
        let in_l = *l;
        let in_r = *r;
        let del_l = buf_l[idx];
        let del_r = buf_r[idx];

        // Cross-feed ping-pong gives stereo motion while preserving exact beat timing
        buf_l[idx] = in_l + del_r * fb;
        buf_r[idx] = in_r * 0.7 + del_l * fb;
        idx = (idx + 1) % delay_samples;

        *l = in_l * dry + del_l * wet;
        *r = in_r * dry + del_r * wet;
    }
}

struct CombFilter {
    buf: Vec<f32>,
    idx: usize,
    filter_store: f32,
}

impl CombFilter {
    fn new(size: usize) -> Self {
        Self {
            buf: vec![0.0; size.max(1)],
            idx: 0,
            filter_store: 0.0,
        }
    }

    fn process(&mut self, input: f32, feedback: f32, damp1: f32, damp2: f32) -> f32 {
        let out = self.buf[self.idx];
        self.filter_store = out * damp2 + self.filter_store * damp1;
        self.buf[self.idx] = input + self.filter_store * feedback;
        self.idx = (self.idx + 1) % self.buf.len();
        out
    }
}

struct AllpassFilter {
    buf: Vec<f32>,
    idx: usize,
}

impl AllpassFilter {
    fn new(size: usize) -> Self {
        Self {
            buf: vec![0.0; size.max(1)],
            idx: 0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let buf_out = self.buf[self.idx];
        let out = -input + buf_out;
        self.buf[self.idx] = input + buf_out * 0.5;
        self.idx = (self.idx + 1) % self.buf.len();
        out
    }
}

fn process_schroeder_channel(
    samples: &mut [f32],
    sr_scale: f32,
    spread: usize,
    feedback: f32,
    damp1: f32,
    damp2: f32,
    wet: f32,
) {
    let mut combs = [
        CombFilter::new(((1116.0 * sr_scale) as usize) + spread),
        CombFilter::new(((1188.0 * sr_scale) as usize) + spread),
        CombFilter::new(((1277.0 * sr_scale) as usize) + spread),
        CombFilter::new(((1356.0 * sr_scale) as usize) + spread),
    ];
    let mut allpasses = [
        AllpassFilter::new(((556.0 * sr_scale) as usize) + spread),
        AllpassFilter::new(((441.0 * sr_scale) as usize) + spread),
    ];

    let dry = 1.0 - wet;
    for s in samples.iter_mut() {
        let inp = *s;
        let scaled = inp * 0.15;
        let mut acc = 0.0_f32;
        for c in &mut combs {
            acc += c.process(scaled, feedback, damp1, damp2);
        }
        for ap in &mut allpasses {
            acc = ap.process(acc);
        }
        *s = inp * dry + acc * wet;
    }
}

fn apply_schroeder_reverb(
    left: &mut [f32],
    right: &mut [f32],
    sample_rate: u32,
    room_size: f32,
    damping: f32,
    mix: f32,
) {
    let sr_scale = sample_rate as f32 / 44100.0;
    let feedback = 0.70 + room_size.clamp(0.0, 0.98) * 0.26;
    let damp1 = damping.clamp(0.0, 1.0) * 0.45;
    let damp2 = 1.0 - damp1;
    let wet = mix.clamp(0.0, 1.0);

    process_schroeder_channel(left, sr_scale, 0, feedback, damp1, damp2, wet);
    process_schroeder_channel(right, sr_scale, 23, feedback, damp1, damp2, wet);
}

struct DynamicsParams {
    threshold_db: f32,
    slope: f32,
    attack_sec: f32,
    release_sec: f32,
    makeup_db: f32,
}

fn apply_envelope_dynamics(
    left: &mut [f32],
    right: &mut [f32],
    detector_signal: Option<&[f32]>,
    sample_rate: u32,
    params: DynamicsParams,
) {
    let atk_coeff = (-1.0 / (params.attack_sec.max(0.0005) * sample_rate as f32)).exp();
    let rel_coeff = (-1.0 / (params.release_sec.max(0.005) * sample_rate as f32)).exp();
    let mut env = 0.0_f32;

    for (i, (l, r)) in left.iter_mut().zip(right.iter_mut()).enumerate() {
        let det = match detector_signal {
            Some(sig) => sig.get(i).copied().unwrap_or(0.0),
            None => l.abs().max(r.abs()),
        };
        let coeff = if det > env { atk_coeff } else { rel_coeff };
        env = coeff * env + (1.0 - coeff) * det;

        let env_db = 20.0 * env.max(1e-6).log10();
        let excess_db = (env_db - params.threshold_db).max(0.0);
        let gain_db = -excess_db * params.slope + params.makeup_db;
        let gain = 10.0_f32.powf(gain_db / 20.0);
        *l *= gain;
        *r *= gain;
    }
}

fn apply_stereo_fx(
    left: &mut [f32],
    right: &mut [f32],
    fx_list: &[FxConfig],
    sample_rate: u32,
    sec_per_beat: f32,
    sidechain_refs: &HashMap<String, Vec<f32>>,
) {
    for fx in fx_list {
        match fx {
            FxConfig::Gain { gain_db } => {
                let linear_gain = 10.0_f32.powf(*gain_db / 20.0);
                for (l, r) in left.iter_mut().zip(right.iter_mut()) {
                    *l *= linear_gain;
                    *r *= linear_gain;
                }
            }
            FxConfig::Pan { pan } => {
                let pan_norm = pan.clamp(-1.0, 1.0);
                let theta = (pan_norm + 1.0) * (PI / 4.0);
                let left_gain = theta.cos() * SQRT_2;
                let right_gain = theta.sin() * SQRT_2;
                for (l, r) in left.iter_mut().zip(right.iter_mut()) {
                    *l *= left_gain;
                    *r *= right_gain;
                }
            }
            FxConfig::LowPass { cutoff_hz, q } => {
                if let Some(q_val) = q {
                    apply_biquad_filter(left, right, sample_rate, *cutoff_hz, *q_val, false);
                } else {
                    let rc = 1.0 / (2.0 * PI * cutoff_hz.max(10.0));
                    let dt = 1.0 / sample_rate as f32;
                    let alpha = dt / (rc + dt);

                    let mut prev_l = 0.0_f32;
                    for l in left.iter_mut() {
                        *l = prev_l + alpha * (*l - prev_l);
                        prev_l = *l;
                    }

                    let mut prev_r = 0.0_f32;
                    for r in right.iter_mut() {
                        *r = prev_r + alpha * (*r - prev_r);
                        prev_r = *r;
                    }
                }
            }
            FxConfig::HighPass { cutoff_hz, q } => {
                if let Some(q_val) = q {
                    apply_biquad_filter(left, right, sample_rate, *cutoff_hz, *q_val, true);
                } else {
                    let rc = 1.0 / (2.0 * PI * cutoff_hz.max(10.0));
                    let dt = 1.0 / sample_rate as f32;
                    let alpha = rc / (rc + dt);

                    let mut prev_out_l = 0.0_f32;
                    let mut prev_in_l = 0.0_f32;
                    for l in left.iter_mut() {
                        let cur_in = *l;
                        prev_out_l = alpha * (prev_out_l + cur_in - prev_in_l);
                        prev_in_l = cur_in;
                        *l = prev_out_l;
                    }

                    let mut prev_out_r = 0.0_f32;
                    let mut prev_in_r = 0.0_f32;
                    for r in right.iter_mut() {
                        let cur_in = *r;
                        prev_out_r = alpha * (prev_out_r + cur_in - prev_in_r);
                        prev_in_r = cur_in;
                        *r = prev_out_r;
                    }
                }
            }
            FxConfig::Saturate { drive } => {
                let d = drive.clamp(0.1, 20.0);
                let norm = d.tanh().max(1e-4);
                for (l, r) in left.iter_mut().zip(right.iter_mut()) {
                    *l = (*l * d).tanh() / norm;
                    *r = (*r * d).tanh() / norm;
                }
            }
            FxConfig::SoftClip => {
                for (l, r) in left.iter_mut().zip(right.iter_mut()) {
                    *l = l.tanh();
                    *r = r.tanh();
                }
            }
            FxConfig::Delay {
                delay_beats,
                feedback,
                mix,
            } => {
                apply_stereo_delay(
                    left,
                    right,
                    sample_rate,
                    sec_per_beat,
                    *delay_beats,
                    *feedback,
                    *mix,
                );
            }
            FxConfig::Reverb {
                room_size,
                damping,
                mix,
            } => {
                apply_schroeder_reverb(left, right, sample_rate, *room_size, *damping, *mix);
            }
            FxConfig::Sidechain {
                source_track,
                amount,
                threshold_db,
                attack_sec,
                release_sec,
            } => {
                if let Some(detector) = sidechain_refs.get(source_track) {
                    apply_envelope_dynamics(
                        left,
                        right,
                        Some(detector.as_slice()),
                        sample_rate,
                        DynamicsParams {
                            threshold_db: *threshold_db,
                            slope: amount.clamp(0.0, 1.0),
                            attack_sec: *attack_sec,
                            release_sec: *release_sec,
                            makeup_db: 0.0,
                        },
                    );
                } else {
                    eprintln!(
                        "Warning: Sidechain source_track '{}' not found",
                        source_track
                    );
                }
            }
            FxConfig::Compressor {
                threshold_db,
                ratio,
                attack_sec,
                release_sec,
                makeup_db,
            } => {
                let slope = 1.0 - 1.0 / ratio.max(1.0);
                apply_envelope_dynamics(
                    left,
                    right,
                    None,
                    sample_rate,
                    DynamicsParams {
                        threshold_db: *threshold_db,
                        slope,
                        attack_sec: *attack_sec,
                        release_sec: *release_sec,
                        makeup_db: *makeup_db,
                    },
                );
            }
            FxConfig::StereoWidth { width } => {
                let w = width.clamp(0.0, 3.0);
                for (l, r) in left.iter_mut().zip(right.iter_mut()) {
                    let mid = (*l + *r) * 0.5;
                    let side = (*l - *r) * 0.5 * w;
                    *l = mid + side;
                    *r = mid - side;
                }
            }
        }
    }
}

pub fn export_wav<P: AsRef<Path>>(
    audio: &RenderedAudio,
    path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    let spec = WavSpec {
        channels: audio.channels,
        sample_rate: audio.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let p = path.as_ref();
    {
        let mut writer = WavWriter::create(p, spec)?;
        for &sample in &audio.samples {
            let clamped = sample.clamp(-1.0, 1.0);
            let sample_i16 = (clamped * i16::MAX as f32) as i16;
            writer.write_sample(sample_i16)?;
        }
        writer.finalize()?;
    }

    if let Some((start_sample, end_sample)) = audio.loop_samples {
        inject_smpl_chunk(p, audio.sample_rate, start_sample, end_sample)?;
    }

    Ok(())
}

/// Exports rendered audio as OGG Vorbis via `ffmpeg` (`libvorbis`), embedding
/// Godot-compatible `LOOPSTART` and `LOOPLENGTH` Vorbis comment tags when loop points exist.
pub fn export_ogg<P: AsRef<Path>>(
    audio: &RenderedAudio,
    path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    let seq = TEMP_OGG_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_wav =
        std::env::temp_dir().join(format!("auralis_ogg_{}_{}.wav", std::process::id(), seq));
    export_wav(audio, &tmp_wav)?;

    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-y")
        .arg("-v")
        .arg("error")
        .arg("-i")
        .arg(&tmp_wav)
        .arg("-c:a")
        .arg("libvorbis")
        .arg("-q:a")
        .arg("5");

    if let Some((start, end)) = audio.loop_samples {
        let length = end.saturating_sub(start);
        cmd.arg("-metadata")
            .arg(format!("LOOPSTART={}", start))
            .arg("-metadata")
            .arg(format!("LOOPLENGTH={}", length));
    }

    let out_path = path.as_ref();
    cmd.arg(out_path);

    let output = cmd.output();
    let _ = std::fs::remove_file(&tmp_wav);

    let output = output.map_err(|e| {
        format!(
            "Failed to invoke ffmpeg for OGG export ({}): {}",
            out_path.display(),
            e
        )
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg OGG export failed: {}", stderr.trim()).into());
    }

    Ok(())
}

/// Exports audio according to output file extension (`.ogg` -> Vorbis, otherwise WAV).
pub fn export_audio<P: AsRef<Path>>(
    audio: &RenderedAudio,
    path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    let p = path.as_ref();
    if p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("ogg"))
    {
        export_ogg(audio, p)
    } else {
        export_wav(audio, p)
    }
}

fn inject_smpl_chunk(
    path: &Path,
    sample_rate: u32,
    start_sample: u32,
    end_sample: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = OpenOptions::new().read(true).write(true).open(path)?;

    let file_len = file.metadata()?.len();
    if file_len < 12 {
        return Ok(());
    }

    // RIFF 'smpl' chunk (60 bytes data + 8 bytes header = 68 bytes)
    let mut chunk = Vec::with_capacity(68);
    chunk.extend_from_slice(b"smpl");
    chunk.extend_from_slice(&60u32.to_le_bytes()); // Chunk data size = 60

    chunk.extend_from_slice(&0u32.to_le_bytes()); // Manufacturer
    chunk.extend_from_slice(&0u32.to_le_bytes()); // Product
    let sample_period = (1_000_000_000.0 / sample_rate as f64).round() as u32;
    chunk.extend_from_slice(&sample_period.to_le_bytes()); // Nanoseconds per sample
    chunk.extend_from_slice(&60u32.to_le_bytes()); // MIDI Unity Note (60 = C4)
    chunk.extend_from_slice(&0u32.to_le_bytes()); // MIDI Pitch Fraction
    chunk.extend_from_slice(&0u32.to_le_bytes()); // SMPTE Format
    chunk.extend_from_slice(&0u32.to_le_bytes()); // SMPTE Offset
    chunk.extend_from_slice(&1u32.to_le_bytes()); // Num Sample Loops = 1
    chunk.extend_from_slice(&0u32.to_le_bytes()); // Sampler Data = 0

    // Loop 0 (24 bytes):
    chunk.extend_from_slice(&0u32.to_le_bytes()); // Cue Point ID
    chunk.extend_from_slice(&0u32.to_le_bytes()); // Type = 0 (Forward loop)
    chunk.extend_from_slice(&start_sample.to_le_bytes()); // Loop Start
    chunk.extend_from_slice(&end_sample.to_le_bytes()); // Loop End
    chunk.extend_from_slice(&0u32.to_le_bytes()); // Fraction
    chunk.extend_from_slice(&0u32.to_le_bytes()); // Play Count (0 = Infinite)

    file.seek(SeekFrom::End(0))?;
    file.write_all(&chunk)?;

    // Update RIFF chunk size at offset 4..8 (total file len - 8)
    let updated_riff_size = (file_len + chunk.len() as u64 - 8) as u32;
    file.seek(SeekFrom::Start(4))?;
    file.write_all(&updated_riff_size.to_le_bytes())?;
    file.flush()?;

    Ok(())
}

pub fn read_wav_loop_points<P: AsRef<Path>>(path: P) -> Option<(u32, u32)> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut header = [0u8; 12];
    file.read_exact(&mut header).ok()?;

    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return None;
    }

    let mut chunk_header = [0u8; 8];
    while file.read_exact(&mut chunk_header).is_ok() {
        let chunk_id = &chunk_header[0..4];
        let chunk_size = u32::from_le_bytes([
            chunk_header[4],
            chunk_header[5],
            chunk_header[6],
            chunk_header[7],
        ]);

        if chunk_id == b"smpl" && chunk_size >= 60 {
            let mut data = vec![0u8; chunk_size as usize];
            file.read_exact(&mut data).ok()?;
            let start = u32::from_le_bytes([data[44], data[45], data[46], data[47]]);
            let end = u32::from_le_bytes([data[48], data[49], data[50], data[51]]);
            return Some((start, end));
        } else {
            let skip_bytes = if chunk_size % 2 != 0 {
                chunk_size as i64 + 1
            } else {
                chunk_size as i64
            };
            if file.seek(SeekFrom::Current(skip_bytes)).is_err() {
                break;
            }
        }
    }

    None
}
