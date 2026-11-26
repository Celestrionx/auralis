use crate::model::NoteEvent;
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

static SOUNDFONT_CACHE: Mutex<Option<HashMap<PathBuf, Arc<SoundFont>>>> = Mutex::new(None);

/// Resolves an asset path (`TimGM6mb.sf2` or WAV sample) deterministically across:
/// 1) exact path / current working directory,
/// 2) `AURALIS_SOUNDFONT` (for `.sf2`) and `AURALIS_ASSETS_DIR`,
/// 3) executable-relative directories (`<exe>`, `<exe>/..`, `<exe>/../share/auralis`),
/// 4) XDG data directory (`$XDG_DATA_HOME/auralis` or `$HOME/.local/share/auralis`).
pub fn resolve_asset_path<P: AsRef<Path>>(raw: P) -> PathBuf {
    let raw_path = raw.as_ref();
    if raw_path.as_os_str().is_empty() || raw_path.exists() {
        return raw_path.to_path_buf();
    }

    if raw_path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("sf2"))
        && let Ok(sf_env) = std::env::var("AURALIS_SOUNDFONT")
    {
        let candidate = PathBuf::from(sf_env);
        if candidate.exists() {
            return candidate;
        }
    }

    let stripped = raw_path.strip_prefix("assets").ok();
    let mut search_roots: Vec<PathBuf> = Vec::new();

    if let Ok(assets_dir) = std::env::var("AURALIS_ASSETS_DIR") {
        let dir = PathBuf::from(assets_dir);
        search_roots.push(dir.clone());
        if let Some(parent) = dir.parent() {
            search_roots.push(parent.to_path_buf());
        }
    }

    if let Ok(exe) = std::env::current_exe()
        && let Some(exe_dir) = exe.parent()
    {
        search_roots.push(exe_dir.to_path_buf());
        search_roots.push(exe_dir.join("../share/auralis"));
        if let Some(p1) = exe_dir.parent() {
            search_roots.push(p1.to_path_buf());
            if let Some(p2) = p1.parent() {
                search_roots.push(p2.to_path_buf());
            }
        }
    }

    if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
        search_roots.push(PathBuf::from(xdg_data).join("auralis"));
    } else if let Ok(home) = std::env::var("HOME") {
        search_roots.push(PathBuf::from(home).join(".local/share/auralis"));
    }

    for root in search_roots {
        let direct = root.join(raw_path);
        if direct.exists() {
            return direct;
        }
        if let Some(rel) = stripped {
            let under_assets = root.join(rel);
            if under_assets.exists() {
                return under_assets;
            }
        }
    }

    raw_path.to_path_buf()
}

pub fn get_or_load_soundfont<P: AsRef<Path>>(
    path: P,
) -> Result<Arc<SoundFont>, Box<dyn std::error::Error>> {
    let path_buf = resolve_asset_path(path);
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
    events: &[NoteEvent],
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
    // Configure GM RPN 0x0000 Pitch Bend Sensitivity to +/- 12 semitones
    synth.process_midi_message(channel, 0xB0, 101, 0);
    synth.process_midi_message(channel, 0xB0, 100, 0);
    synth.process_midi_message(channel, 0xB0, 6, 12);
    synth.process_midi_message(channel, 0xB0, 38, 0);

    // Build timeline event queue
    #[derive(Clone, Copy)]
    enum MidiAction {
        NoteOff(u8),
        ControlChange(i32, i32),
        PitchBend(i32),
        NoteOn(u8, f32),
    }

    impl MidiAction {
        fn order_rank(&self) -> u8 {
            match self {
                MidiAction::NoteOff(_) => 0,
                MidiAction::ControlChange(_, _) => 1,
                MidiAction::PitchBend(_) => 2,
                MidiAction::NoteOn(_, _) => 3,
            }
        }
    }

    let mut timeline: Vec<(usize, MidiAction)> = Vec::new();
    for ev in events {
        let start_sample = (ev.start_beats * sec_per_beat * sample_rate as f32) as usize;
        let duration_samples = (ev.duration_beats * sec_per_beat * sample_rate as f32) as usize;
        let end_sample = start_sample + duration_samples;

        if let Some(sustain_on) = ev.sustain {
            if sustain_on {
                timeline.push((start_sample, MidiAction::ControlChange(64, 127)));
                timeline.push((end_sample, MidiAction::ControlChange(64, 0)));
            } else {
                timeline.push((start_sample, MidiAction::ControlChange(64, 0)));
            }
        }

        if let Some(target_midi) = ev.slide_to_midi {
            let delta_semi = (target_midi as f32 - ev.midi_note as f32).clamp(-12.0, 12.0);
            let glide_beats = ev.glide_beats.unwrap_or(ev.duration_beats).max(0.0001);
            let glide_samples = (glide_beats * sec_per_beat * sample_rate as f32) as usize;
            let glide_end = (start_sample + glide_samples).min(end_sample);
            let steps = 16usize;
            for step in 0..=steps {
                let frac = step as f32 / steps as f32;
                let s_idx = start_sample + ((glide_end - start_sample) * step) / steps;
                let bend =
                    (8192 + ((delta_semi * frac / 12.0) * 8191.0).round() as i32).clamp(0, 16383);
                timeline.push((s_idx, MidiAction::PitchBend(bend)));
            }
            timeline.push((end_sample, MidiAction::PitchBend(8192)));
        }

        for &midi in &ev.notes() {
            timeline.push((start_sample, MidiAction::NoteOn(midi, ev.velocity)));
            timeline.push((end_sample, MidiAction::NoteOff(midi)));
        }
    }

    // Sort chronologically; NoteOff < ControlChange < PitchBend < NoteOn at identical timestamps
    timeline.sort_by(|a, b| {
        if a.0 != b.0 {
            a.0.cmp(&b.0)
        } else {
            a.1.order_rank().cmp(&b.1.order_rank())
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
                MidiAction::ControlChange(cc, val) => {
                    synth.process_midi_message(channel, 0xB0, cc, val);
                }
                MidiAction::PitchBend(bend) => {
                    synth.process_midi_message(channel, 0xE0, bend & 0x7F, (bend >> 7) & 0x7F);
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
