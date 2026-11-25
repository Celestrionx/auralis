use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Song {
    pub tempo: f32,
    pub time_signature: (u8, u8),
    #[serde(default)]
    pub sample_rate: Option<u32>,
    #[serde(default)]
    pub loop_points: Option<LoopPoints>,
    #[serde(default)]
    pub master_fx: Vec<FxConfig>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub patterns: BTreeMap<String, Pattern>,
    pub tracks: Vec<Track>,
}

impl Default for Song {
    fn default() -> Self {
        Self {
            tempo: 120.0,
            time_signature: (4, 4),
            sample_rate: Some(44100),
            loop_points: None,
            master_fx: Vec::new(),
            patterns: BTreeMap::new(),
            tracks: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct LoopPoints {
    pub start_beat: f32,
    #[serde(default)]
    pub end_beat: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pattern {
    pub length_beats: f32,
    pub events: Vec<NoteEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Clip {
    pub pattern: String,
    pub start_beats: f32,
    #[serde(default)]
    pub repeat: Option<u32>,
    #[serde(default)]
    pub transpose_semitones: Option<i8>,
    #[serde(default)]
    pub velocity_scale: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Track {
    pub id: String,
    pub instrument: InstrumentConfig,
    #[serde(default)]
    pub fx: Vec<FxConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clips: Vec<Clip>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<NoteEvent>,
}

fn shift_midi(note: u8, delta: i8) -> u8 {
    (note as i16 + delta as i16).clamp(0, 127) as u8
}

impl Track {
    /// Expands all referenced `clips` from `patterns` and merges them with direct `events`
    /// in deterministic chronological order.
    pub fn resolved_events(&self, patterns: &BTreeMap<String, Pattern>) -> Vec<NoteEvent> {
        let mut out = self.events.clone();

        for clip in &self.clips {
            let Some(pattern) = patterns.get(&clip.pattern) else {
                eprintln!(
                    "Warning: Track '{}' references missing pattern '{}'",
                    self.id, clip.pattern
                );
                continue;
            };

            let repeats = clip.repeat.unwrap_or(1);
            let transpose = clip.transpose_semitones.unwrap_or(0);
            let vel_scale = clip.velocity_scale.unwrap_or(1.0);
            let step_beats = pattern.length_beats.max(0.0);

            for rep in 0..repeats {
                let offset = clip.start_beats + (rep as f32) * step_beats;
                for ev in &pattern.events {
                    let mut resolved = ev.clone();
                    resolved.start_beats += offset;
                    resolved.velocity = (resolved.velocity * vel_scale).clamp(0.0, 1.0);
                    if transpose != 0 {
                        resolved.midi_note = shift_midi(resolved.midi_note, transpose);
                        if let Some(ref mut chord) = resolved.chord_notes {
                            for n in chord.iter_mut() {
                                *n = shift_midi(*n, transpose);
                            }
                        }
                        if let Some(slide) = resolved.slide_to_midi {
                            resolved.slide_to_midi = Some(shift_midi(slide, transpose));
                        }
                    }
                    out.push(resolved);
                }
            }
        }

        out.sort_by(|a, b| a.start_beats.total_cmp(&b.start_beats));
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum InstrumentConfig {
    SoundFont {
        soundfont_path: String,
        #[serde(default)]
        bank: Option<u32>,
        #[serde(default)]
        preset: Option<u32>,
    },
    Synth {
        waveform: Waveform,
        #[serde(default)]
        adsr: Adsr,
        #[serde(default)]
        pitch_env: Option<PitchEnvelope>,
    },
}

impl InstrumentConfig {
    pub fn is_soundfont(&self) -> bool {
        matches!(self, InstrumentConfig::SoundFont { .. })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct PitchEnvelope {
    pub drop_semitones: f32,
    pub decay_sec: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Waveform {
    Sine,
    Square,
    Sawtooth,
    Triangle,
    Noise,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Adsr {
    pub attack_sec: f32,
    pub decay_sec: f32,
    pub sustain_level: f32,
    pub release_sec: f32,
}

impl Default for Adsr {
    fn default() -> Self {
        Self {
            attack_sec: 0.01,
            decay_sec: 0.1,
            sustain_level: 0.7,
            release_sec: 0.1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoteEvent {
    pub start_beats: f32,
    pub duration_beats: f32,
    #[serde(default)]
    pub midi_note: u8,
    #[serde(default)]
    pub chord_notes: Option<Vec<u8>>,
    #[serde(default = "default_velocity")]
    pub velocity: f32,
    #[serde(default)]
    pub slide_to_midi: Option<u8>,
    #[serde(default)]
    pub glide_beats: Option<f32>,
}

impl Default for NoteEvent {
    fn default() -> Self {
        Self {
            start_beats: 0.0,
            duration_beats: 1.0,
            midi_note: 60,
            chord_notes: None,
            velocity: default_velocity(),
            slide_to_midi: None,
            glide_beats: None,
        }
    }
}

fn default_velocity() -> f32 {
    0.8
}

impl NoteEvent {
    pub fn notes(&self) -> Vec<u8> {
        if let Some(ref list) = self.chord_notes {
            list.clone()
        } else {
            vec![self.midi_note]
        }
    }

    pub fn frequency_hz_for_note(midi: u8) -> f32 {
        440.0 * 2.0_f32.powf((midi as f32 - 69.0) / 12.0)
    }

    pub fn frequency_hz(&self) -> f32 {
        Self::frequency_hz_for_note(self.midi_note)
    }
}

fn default_sidechain_threshold_db() -> f32 {
    -24.0
}

fn default_sidechain_attack_sec() -> f32 {
    0.005
}

fn default_sidechain_release_sec() -> f32 {
    0.15
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum FxConfig {
    Gain {
        gain_db: f32,
    },
    LowPass {
        cutoff_hz: f32,
        #[serde(default)]
        q: Option<f32>,
    },
    HighPass {
        cutoff_hz: f32,
        #[serde(default)]
        q: Option<f32>,
    },
    Pan {
        pan: f32,
    },
    Saturate {
        drive: f32,
    },
    SoftClip,
    Delay {
        delay_beats: f32,
        feedback: f32,
        mix: f32,
    },
    Reverb {
        room_size: f32,
        damping: f32,
        mix: f32,
    },
    Sidechain {
        source_track: String,
        amount: f32,
        #[serde(default = "default_sidechain_threshold_db")]
        threshold_db: f32,
        #[serde(default = "default_sidechain_attack_sec")]
        attack_sec: f32,
        #[serde(default = "default_sidechain_release_sec")]
        release_sec: f32,
    },
    Compressor {
        threshold_db: f32,
        ratio: f32,
        attack_sec: f32,
        release_sec: f32,
        #[serde(default)]
        makeup_db: f32,
    },
    StereoWidth {
        width: f32,
    },
}
