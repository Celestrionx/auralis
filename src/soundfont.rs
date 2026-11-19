use crate::model::Track;
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

static SOUNDFONT_CACHE: Mutex<Option<HashMap<PathBuf, Arc<SoundFont>>>> = Mutex::new(None);

pub fn get_or_load_soundfont<P: AsRef<Path>>(
    path: P,
) -> Result<Arc<SoundFont>, Box<dyn std::error::Error>> {
    let path_buf = path.as_ref().to_path_buf();
    let mut guard = SOUNDFONT_CACHE.lock().map_err(|e| e.to_string())?;
    let cache = guard.get_or_insert_with(HashMap::new);

    if let Some(sf) = cache.get(&path_buf) {
        return Ok(Arc::clone(sf));
    }

    let mut file = File::open(&path_buf)?;
    let sf = Arc::new(SoundFont::new(&mut file)?);
    cache.insert(path_buf, Arc::clone(&sf));
    Ok(sf)
}

pub fn render_soundfont_track(
    track: &Track,
    soundfont_path: &str,
    bank: Option<u32>,
    preset: Option<u32>,
    sample_rate: u32,
    sec_per_beat: f32,
    total_samples: usize,
) -> Result<(Vec<f32>, Vec<f32>), Box<dyn std::error::Error>> {
    let sound_font = get_or_load_soundfont(soundfont_path)?;
    let settings = SynthesizerSettings::new(sample_rate as i32);
    let mut synth = Synthesizer::new(&sound_font, &settings)?;

    let channel = 0;
    if let Some(b) = bank {
        synth.process_midi_message(channel, 0xB0, 0x00, b as i32);
    }
    if let Some(p) = preset {
        synth.process_midi_message(channel, 0xC0, p as i32, 0);
    }

    // Build timeline event queue: (sample_index, is_note_on, note, velocity)
    #[derive(Clone, Copy)]
    enum MidiAction {
        NoteOn(u8, f32),
        NoteOff(u8),
    }

    let mut timeline: Vec<(usize, MidiAction)> = Vec::new();
    for ev in &track.events {
        let start_sample = (ev.start_beats * sec_per_beat * sample_rate as f32) as usize;
        let duration_samples = (ev.duration_beats * sec_per_beat * sample_rate as f32) as usize;
        let end_sample = start_sample + duration_samples;

        for &midi in &ev.notes() {
            timeline.push((start_sample, MidiAction::NoteOn(midi, ev.velocity)));
            timeline.push((end_sample, MidiAction::NoteOff(midi)));
        }
    }

    // Sort chronologically; note offs before note ons at identical timestamps
    timeline.sort_by(|a, b| {
        if a.0 != b.0 {
            a.0.cmp(&b.0)
        } else {
            match (a.1, b.1) {
                (MidiAction::NoteOff(_), MidiAction::NoteOn(_, _)) => std::cmp::Ordering::Less,
                (MidiAction::NoteOn(_, _), MidiAction::NoteOff(_)) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            }
        }
    });

    let mut out_left = vec![0.0_f32; total_samples];
    let mut out_right = vec![0.0_f32; total_samples];

    let block_size = 64;
    let mut current_sample = 0;
    let mut timeline_idx = 0;

    let mut block_left = vec![0.0_f32; block_size];
    let mut block_right = vec![0.0_f32; block_size];

    while current_sample < total_samples {
        let chunk_len = block_size.min(total_samples - current_sample);

        // Process any MIDI actions due up to current_sample + chunk_len
        while timeline_idx < timeline.len() && timeline[timeline_idx].0 < current_sample + chunk_len
        {
            let (target_sample, action) = timeline[timeline_idx];
            if target_sample > current_sample {
                let sub_chunk = target_sample - current_sample;
                synth.render(&mut block_left[..sub_chunk], &mut block_right[..sub_chunk]);
                for i in 0..sub_chunk {
                    out_left[current_sample + i] += block_left[i];
                    out_right[current_sample + i] += block_right[i];
                }
                current_sample += sub_chunk;
            }

            match action {
                MidiAction::NoteOn(note, vel) => {
                    let midi_vel = ((vel * 127.0).clamp(1.0, 127.0)) as i32;
                    synth.note_on(channel, note as i32, midi_vel);
                }
                MidiAction::NoteOff(note) => {
                    synth.note_off(channel, note as i32);
                }
            }
            timeline_idx += 1;
        }

        // Render remaining portion of block
        if current_sample < total_samples {
            let remaining = block_size.min(total_samples - current_sample);
            synth.render(&mut block_left[..remaining], &mut block_right[..remaining]);
            for i in 0..remaining {
                out_left[current_sample + i] += block_left[i];
                out_right[current_sample + i] += block_right[i];
            }
            current_sample += remaining;
        }
    }

    Ok((out_left, out_right))
}
