use crate::model::NoteEvent;
use crate::soundfont::resolve_asset_path;
use hound::{SampleFormat, WavReader};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct LoadedSample {
    pub sample_rate: u32,
    pub left: Vec<f32>,
    pub right: Vec<f32>,
}

static SAMPLE_CACHE: Mutex<Option<HashMap<PathBuf, Arc<LoadedSample>>>> = Mutex::new(None);

pub fn get_or_load_sample<P: AsRef<Path>>(
    path: P,
) -> Result<Arc<LoadedSample>, Box<dyn std::error::Error>> {
    let path_buf = resolve_asset_path(path);
    let mut guard = SAMPLE_CACHE.lock().map_err(|e| e.to_string())?;
    let cache = guard.get_or_insert_with(HashMap::new);

    if let Some(sample) = cache.get(&path_buf) {
        return Ok(Arc::clone(sample));
    }

    let mut reader = WavReader::open(&path_buf)?;
    let spec = reader.spec();
    let ch = spec.channels.max(1) as usize;

    let interleaved: Vec<f32> = match spec.sample_format {
        SampleFormat::Int => {
            let max_val = ((1_i64 << spec.bits_per_sample.saturating_sub(1).max(1)) - 1) as f32;
            reader
                .samples::<i32>()
                .filter_map(Result::ok)
                .map(|s| s as f32 / max_val)
                .collect()
        }
        SampleFormat::Float => reader.samples::<f32>().filter_map(Result::ok).collect(),
    };

    let frames = interleaved.len() / ch;
    let mut left = Vec::with_capacity(frames);
    let mut right = Vec::with_capacity(frames);

    for frame in 0..frames {
        let base = frame * ch;
        let l = interleaved[base];
        let r = if ch >= 2 { interleaved[base + 1] } else { l };
        left.push(l);
        right.push(r);
    }

    let loaded = Arc::new(LoadedSample {
        sample_rate: spec.sample_rate,
        left,
        right,
    });
    cache.insert(path_buf, Arc::clone(&loaded));
    Ok(loaded)
}

pub fn render_sample_track(
    events: &[NoteEvent],
    wav_path: &str,
    root_midi: u8,
    sample_rate: u32,
    sec_per_beat: f32,
    total_samples: usize,
) -> Result<(Vec<f32>, Vec<f32>), Box<dyn std::error::Error>> {
    let sample = get_or_load_sample(wav_path)?;
    let mut out_left = vec![0.0_f32; total_samples];
    let mut out_right = vec![0.0_f32; total_samples];

    if sample.left.is_empty() {
        return Ok((out_left, out_right));
    }

    let sr_ratio = sample.sample_rate as f64 / sample_rate.max(1) as f64;
    let fade_target = (0.01 * sample_rate as f32).round().max(1.0) as usize;

    for ev in events {
        let start_sample = (ev.start_beats * sec_per_beat * sample_rate as f32) as usize;
        let dur_samples = (ev.duration_beats * sec_per_beat * sample_rate as f32).round() as usize;
        if dur_samples == 0 || start_sample >= total_samples {
            continue;
        }

        let fade_samples = fade_target.min((dur_samples / 4).max(1));
        let glide_samples = (ev.glide_beats.unwrap_or(ev.duration_beats).max(0.0001)
            * sec_per_beat
            * sample_rate as f32)
            .max(1.0);
        let notes = ev.notes();
        let vel_per_voice = ev.velocity / (notes.len().max(1) as f32).sqrt();
        let max_offset = dur_samples.min(total_samples - start_sample);

        for &midi in &notes {
            let mut src_pos = 0.0_f64;
            for offset in 0..max_offset {
                let idx0 = src_pos.floor() as usize;
                if idx0 >= sample.left.len() {
                    break;
                }
                let frac = (src_pos - idx0 as f64) as f32;
                let l0 = sample.left[idx0];
                let l1 = sample.left.get(idx0 + 1).copied().unwrap_or(0.0);
                let r0 = sample.right[idx0];
                let r1 = sample.right.get(idx0 + 1).copied().unwrap_or(0.0);

                let l_val = l0 + (l1 - l0) * frac;
                let r_val = r0 + (r1 - r0) * frac;
                let env = if offset >= dur_samples.saturating_sub(fade_samples) {
                    (dur_samples - offset) as f32 / fade_samples as f32
                } else {
                    1.0
                };

                let dst = start_sample + offset;
                out_left[dst] += l_val * env * vel_per_voice;
                out_right[dst] += r_val * env * vel_per_voice;

                let cur_midi = if let Some(target_midi) = ev.slide_to_midi {
                    let g_frac = (offset as f32 / glide_samples).clamp(0.0, 1.0);
                    midi as f32 + (target_midi as f32 - midi as f32) * g_frac
                } else {
                    midi as f32
                };
                let pitch_ratio = 2.0_f64.powf(((cur_midi - root_midi as f32) / 12.0) as f64);
                src_pos += sr_ratio * pitch_ratio;
            }
        }
    }

    Ok((out_left, out_right))
}
