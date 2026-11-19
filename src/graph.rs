use crate::model::{FxConfig, Song, Track};
use crate::synth::{calculate_envelope, generate_sample};
use hound::{WavSpec, WavWriter};
use std::f32::consts::PI;
use std::path::Path;

pub struct RenderedAudio {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
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
    let mut master_buffer = vec![0.0_f32; total_samples];

    for track in &song.tracks {
        let track_buffer = render_track(track, sample_rate, sec_per_beat, total_samples);
        for (m, t) in master_buffer.iter_mut().zip(track_buffer.iter()) {
            *m += *t;
        }
    }

    RenderedAudio {
        sample_rate,
        channels: 1,
        samples: master_buffer,
    }
}

fn render_track(
    track: &Track,
    sample_rate: u32,
    sec_per_beat: f32,
    total_samples: usize,
) -> Vec<f32> {
    let mut buffer = vec![0.0_f32; total_samples];

    for ev in &track.events {
        let start_sec = ev.start_beats * sec_per_beat;
        let duration_sec = ev.duration_beats * sec_per_beat;
        let freq = ev.frequency_hz();

        let start_sample = (start_sec * sample_rate as f32) as usize;
        let note_total_sec = duration_sec + track.instrument.adsr.release_sec;
        let note_total_samples = (note_total_sec * sample_rate as f32).ceil() as usize;

        let end_sample = (start_sample + note_total_samples).min(total_samples);

        for (offset, s) in buffer[start_sample..end_sample].iter_mut().enumerate() {
            let t = offset as f32 / sample_rate as f32;
            let phase = 2.0 * PI * freq * t;
            let raw_sample = generate_sample(track.instrument.waveform, phase);
            let env = calculate_envelope(t, duration_sec, &track.instrument.adsr);
            *s += raw_sample * env * ev.velocity;
        }
    }

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
                let rc = 1.0 / (2.0 * PI * cutoff_hz);
                let dt = 1.0 / sample_rate as f32;
                let alpha = dt / (rc + dt);
                let mut prev = 0.0_f32;
                for s in &mut buffer {
                    *s = prev + alpha * (*s - prev);
                    prev = *s;
                }
            }
        }
    }

    buffer
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
