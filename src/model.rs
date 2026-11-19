use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Song {
    pub tempo: f32,
    pub time_signature: (u8, u8),
    #[serde(default)]
    pub sample_rate: Option<u32>,
    pub tracks: Vec<Track>,
}

impl Default for Song {
    fn default() -> Self {
        Self {
            tempo: 120.0,
            time_signature: (4, 4),
            sample_rate: Some(44100),
            tracks: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Track {
    pub id: String,
    pub instrument: InstrumentConfig,
    #[serde(default)]
    pub fx: Vec<FxConfig>,
    pub events: Vec<NoteEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstrumentConfig {
    pub waveform: Waveform,
    #[serde(default)]
    pub adsr: Adsr,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Waveform {
    Sine,
    Square,
    Sawtooth,
    Triangle,
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
    pub midi_note: u8,
    #[serde(default = "default_velocity")]
    pub velocity: f32,
}

fn default_velocity() -> f32 {
    0.8
}

impl NoteEvent {
    pub fn frequency_hz(&self) -> f32 {
        440.0 * 2.0_f32.powf((self.midi_note as f32 - 69.0) / 12.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum FxConfig {
    Gain { gain_db: f32 },
    LowPass { cutoff_hz: f32 },
}
