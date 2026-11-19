use crate::model::{FxConfig, Song, Track};
use crate::synth::{DeterministicNoise, calculate_envelope, generate_sample};
use hound::{WavSpec, WavWriter};
use std::f32::consts::PI;
use std::path::Path;

pub struct RenderedAudio {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>, // interleaved: [L0, R0, L1, R1, ...]
}

pub fn compile_song(song: &Song) -> RenderedAudio {
    let sample_rate = song.sample_rate.unwrap_or(44100);
    let sec_per_beat = 60.0 / song.tempo.max(1.0);

    let mut total_duration_sec = 0.0_f32;
    for track in &song.tracks {
        for ev in &track.events {
            let note_end = (ev.start_beats + ev.duration_beats) * sec_per_beat
                + track.instrument.adsr.release_sec;
            if note_end > total_duration_sec {
                total_duration_sec = note_end;
            }
        }
    }

    let total_samples = ((total_duration_sec + 0.1) * sample_rate as f32).ceil() as usize;
    let mut master_left = vec![0.0_f32; total_samples];
    let mut master_right = vec![0.0_f32; total_samples];

    for (t_idx, track) in song.tracks.iter().enumerate() {
        let (track_mono, pan_val) = render_track(
            track,
            sample_rate,
            sec_per_beat,
            total_samples,
            t_idx as u32,
        );

        // Constant power pan law
        let pan = pan_val.clamp(-1.0, 1.0);
        let theta = (pan + 1.0) * (PI / 4.0);
        let left_gain = theta.cos();
        let right_gain = theta.sin();

        for i in 0..total_samples {
            master_left[i] += track_mono[i] * left_gain;
            master_right[i] += track_mono[i] * right_gain;
        }
    }

    // Interleave left and right into stereo stream
    let mut interleaved = Vec::with_capacity(total_samples * 2);
    for i in 0..total_samples {
        interleaved.push(master_left[i]);
        interleaved.push(master_right[i]);
    }

    RenderedAudio {
        sample_rate,
        channels: 2,
        samples: interleaved,
    }
}

fn render_track(
    track: &Track,
    sample_rate: u32,
    sec_per_beat: f32,
    total_samples: usize,
    track_seed: u32,
) -> (Vec<f32>, f32) {
    let mut buffer = vec![0.0_f32; total_samples];
    let mut noise_gen = DeterministicNoise::new(track_seed + 1);

    for (ev_idx, ev) in track.events.iter().enumerate() {
        let start_sec = ev.start_beats * sec_per_beat;
        let duration_sec = ev.duration_beats * sec_per_beat;
        let notes = ev.notes();
        let note_count = notes.len().max(1) as f32;
        // Scale velocity so multi-note chords don't explode track gain
        let velocity_per_voice = ev.velocity / note_count.sqrt();

        let start_sample = (start_sec * sample_rate as f32) as usize;
        let note_total_sec = duration_sec + track.instrument.adsr.release_sec;
        let note_total_samples = (note_total_sec * sample_rate as f32).ceil() as usize;
        let end_sample = (start_sample + note_total_samples).min(total_samples);

        for &midi in &notes {
            for (offset, s) in buffer[start_sample..end_sample].iter_mut().enumerate() {
                let t = offset as f32 / sample_rate as f32;

                let pitch_drop = if let Some(penv) = &track.instrument.pitch_env {
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
                let raw_sample = generate_sample(track.instrument.waveform, phase, &mut noise_gen);
                let env = calculate_envelope(t, duration_sec, &track.instrument.adsr);
                *s += raw_sample * env * velocity_per_voice;
            }
        }

        // Re-seed noise generator per note event for variety
        noise_gen = DeterministicNoise::new(track_seed.wrapping_add(ev_idx as u32 * 31 + 7));
    }

    let mut pan_val = 0.0_f32;

    // Apply Track FX
    for fx in &track.fx {
        match fx {
            FxConfig::Gain { gain_db } => {
                let linear_gain = 10.0_f32.powf(*gain_db / 20.0);
                for s in &mut buffer {
                    *s *= linear_gain;
                }
            }
            FxConfig::LowPass { cutoff_hz } => {
                let rc = 1.0 / (2.0 * PI * cutoff_hz.max(10.0));
                let dt = 1.0 / sample_rate as f32;
                let alpha = dt / (rc + dt);
                let mut prev = 0.0_f32;
                for s in &mut buffer {
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
                for s in &mut buffer {
                    let cur_in = *s;
                    prev_out = alpha * (prev_out + cur_in - prev_in);
                    prev_in = cur_in;
                    *s = prev_out;
                }
            }
            FxConfig::Pan { pan } => {
                pan_val = *pan;
            }
        }
    }

    (buffer, pan_val)
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

    let mut writer = WavWriter::create(path, spec)?;
    for &sample in &audio.samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * i16::MAX as f32) as i16;
        writer.write_sample(sample_i16)?;
    }
    writer.finalize()?;
    Ok(())
}
