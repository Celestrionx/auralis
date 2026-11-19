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
    #[serde(default)]
    pub pitch_env: Option<PitchEnvelope>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum FxConfig {
    Gain { gain_db: f32 },
    LowPass { cutoff_hz: f32 },
    HighPass { cutoff_hz: f32 },
    Pan { pan: f32 },
}
