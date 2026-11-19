use crate::model::{FxConfig, InstrumentConfig, Song, Track};
use crate::soundfont::render_soundfont_track;
use crate::synth::{DeterministicNoise, calculate_envelope, generate_sample};
use hound::{WavSpec, WavWriter};
use std::f32::consts::PI;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

pub struct RenderedAudio {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>, // interleaved: [L0, R0, L1, R1, ...]
    pub loop_samples: Option<(u32, u32)>,
}

pub fn compile_song(song: &Song) -> RenderedAudio {
    let sample_rate = song.sample_rate.unwrap_or(44100);
    let sec_per_beat = 60.0 / song.tempo.max(1.0);

    let mut total_duration_sec = 0.0_f32;
    for track in &song.tracks {
        let release = match &track.instrument {
            InstrumentConfig::Synth { adsr, .. } => adsr.release_sec,
            InstrumentConfig::SoundFont { .. } => 0.5,
        };
        for ev in &track.events {
            let note_end = (ev.start_beats + ev.duration_beats) * sec_per_beat + release;
            if note_end > total_duration_sec {
                total_duration_sec = note_end;
            }
        }
    }

    let total_samples = ((total_duration_sec + 0.1) * sample_rate as f32).ceil() as usize;
    let mut master_left = vec![0.0_f32; total_samples];
    let mut master_right = vec![0.0_f32; total_samples];

    for (t_idx, track) in song.tracks.iter().enumerate() {
        match &track.instrument {
            InstrumentConfig::SoundFont {
                soundfont_path,
                bank,
                preset,
            } => {
                let (mut left, mut right) = match render_soundfont_track(
                    track,
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
                };

                apply_stereo_fx(&mut left, &mut right, &track.fx, sample_rate);

                for i in 0..total_samples {
                    master_left[i] += left[i];
                    master_right[i] += right[i];
                }
            }
            InstrumentConfig::Synth { .. } => {
                let (mut track_mono, pan_val) = render_synth_track(
                    track,
                    sample_rate,
                    sec_per_beat,
                    total_samples,
                    t_idx as u32,
                );

                apply_mono_fx(&mut track_mono, &track.fx, sample_rate);

                let pan = pan_val.clamp(-1.0, 1.0);
                let theta = (pan + 1.0) * (PI / 4.0);
                let left_gain = theta.cos();
                let right_gain = theta.sin();

                for i in 0..total_samples {
                    master_left[i] += track_mono[i] * left_gain;
                    master_right[i] += track_mono[i] * right_gain;
                }
            }
        }
    }

    // Interleave left and right into stereo stream
    let mut interleaved = Vec::with_capacity(total_samples * 2);
    for i in 0..total_samples {
        interleaved.push(master_left[i]);
        interleaved.push(master_right[i]);
    }

    let loop_samples = song.loop_points.map(|lp| {
        let start_sample = (lp.start_beat * sec_per_beat * sample_rate as f32).round() as u32;
        let end_sample = match lp.end_beat {
            Some(eb) => (eb * sec_per_beat * sample_rate as f32).round() as u32,
            None => total_samples as u32,
        };
        (start_sample, end_sample)
    });

    RenderedAudio {
        sample_rate,
        channels: 2,
        samples: interleaved,
        loop_samples,
    }
}

fn render_synth_track(
    track: &Track,
    sample_rate: u32,
    sec_per_beat: f32,
    total_samples: usize,
    track_seed: u32,
) -> (Vec<f32>, f32) {
    let (waveform, adsr, pitch_env) = match &track.instrument {
        InstrumentConfig::Synth {
            waveform,
            adsr,
            pitch_env,
        } => (*waveform, adsr, pitch_env.as_ref()),
        _ => return (vec![0.0; total_samples], 0.0),
    };

    let mut buffer = vec![0.0_f32; total_samples];
    let mut noise_gen = DeterministicNoise::new(track_seed + 1);

    for (ev_idx, ev) in track.events.iter().enumerate() {
        let start_sec = ev.start_beats * sec_per_beat;
        let duration_sec = ev.duration_beats * sec_per_beat;
        let notes = ev.notes();
        let note_count = notes.len().max(1) as f32;
        let velocity_per_voice = ev.velocity / note_count.sqrt();

        let start_sample = (start_sec * sample_rate as f32) as usize;
        let note_total_sec = duration_sec + adsr.release_sec;
        let note_total_samples = (note_total_sec * sample_rate as f32).ceil() as usize;
        let end_sample = (start_sample + note_total_samples).min(total_samples);

        for &midi in &notes {
            for (offset, s) in buffer[start_sample..end_sample].iter_mut().enumerate() {
                let t = offset as f32 / sample_rate as f32;

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

                let effective_midi = (midi as f32 + pitch_drop).clamp(0.0, 127.0);
                let freq = 440.0 * 2.0_f32.powf((effective_midi - 69.0) / 12.0);

                let phase = 2.0 * PI * freq * t;
                let raw_sample = generate_sample(waveform, phase, &mut noise_gen);
                let env = calculate_envelope(t, duration_sec, adsr);
                *s += raw_sample * env * velocity_per_voice;
            }
        }

        noise_gen = DeterministicNoise::new(track_seed.wrapping_add(ev_idx as u32 * 31 + 7));
    }

    let mut pan_val = 0.0_f32;
    for fx in &track.fx {
        if let FxConfig::Pan { pan } = fx {
            pan_val = *pan;
        }
    }

    (buffer, pan_val)
}

fn apply_mono_fx(buffer: &mut [f32], fx_list: &[FxConfig], sample_rate: u32) {
    for fx in fx_list {
        match fx {
            FxConfig::Gain { gain_db } => {
                let linear_gain = 10.0_f32.powf(*gain_db / 20.0);
                for s in buffer.iter_mut() {
                    *s *= linear_gain;
                }
            }
            FxConfig::LowPass { cutoff_hz } => {
                let rc = 1.0 / (2.0 * PI * cutoff_hz.max(10.0));
                let dt = 1.0 / sample_rate as f32;
                let alpha = dt / (rc + dt);
                let mut prev = 0.0_f32;
                for s in buffer.iter_mut() {
                    *s = prev + alpha * (*s - prev);
                    prev = *s;
                }
            }
            FxConfig::HighPass { cutoff_hz } => {
                let rc = 1.0 / (2.0 * PI * cutoff_hz.max(10.0));
                let dt = 1.0 / sample_rate as f32;
                let alpha = rc / (rc + dt);
                let mut prev_out = 0.0_f32;
                let mut prev_in = 0.0_f32;
                for s in buffer.iter_mut() {
                    let cur_in = *s;
                    prev_out = alpha * (prev_out + cur_in - prev_in);
                    prev_in = cur_in;
                    *s = prev_out;
                }
            }
            FxConfig::Pan { .. } => {}
        }
    }
}

fn apply_stereo_fx(left: &mut [f32], right: &mut [f32], fx_list: &[FxConfig], sample_rate: u32) {
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
                let left_gain = theta.cos() * std::f32::consts::SQRT_2;
                let right_gain = theta.sin() * std::f32::consts::SQRT_2;
                for (l, r) in left.iter_mut().zip(right.iter_mut()) {
                    *l *= left_gain;
                    *r *= right_gain;
                }
            }
            FxConfig::LowPass { cutoff_hz } => {
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
            FxConfig::HighPass { cutoff_hz } => {
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
