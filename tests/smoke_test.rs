use auralis::graph::{compile_song, export_wav};
use auralis::model::{
    Adsr, FxConfig, InstrumentConfig, NoteEvent, PitchEnvelope, Song, Track, Waveform,
};
use auralis::telemetry::analyze_audio;

#[test]
fn test_manifest_roundtrip() {
    let song = Song {
        tempo: 140.0,
        time_signature: (3, 4),
        sample_rate: Some(48000),
        tracks: vec![Track {
            id: "drum_track".to_string(),
            instrument: InstrumentConfig {
                waveform: Waveform::Noise,
                adsr: Adsr::default(),
                pitch_env: Some(PitchEnvelope {
                    drop_semitones: 24.0,
                    decay_sec: 0.05,
                }),
            },
            fx: vec![
                FxConfig::Gain { gain_db: -3.0 },
                FxConfig::Pan { pan: -0.5 },
                FxConfig::HighPass { cutoff_hz: 800.0 },
            ],
            events: vec![NoteEvent {
                start_beats: 0.0,
                duration_beats: 1.0,
                midi_note: 60,
                chord_notes: Some(vec![60, 64, 67]),
                velocity: 0.8,
            }],
        }],
    };

    let serialized = serde_json::to_string(&song).expect("Serialize failed");
    let deserialized: Song = serde_json::from_str(&serialized).expect("Deserialize failed");
    assert_eq!(song, deserialized);
}

#[test]
fn test_percussion_synthesis() {
    let song = Song {
        tempo: 120.0,
        time_signature: (4, 4),
        sample_rate: Some(44100),
        tracks: vec![
            Track {
                id: "kick".to_string(),
                instrument: InstrumentConfig {
                    waveform: Waveform::Sine,
                    adsr: Adsr {
                        attack_sec: 0.002,
                        decay_sec: 0.12,
                        sustain_level: 0.0,
                        release_sec: 0.03,
                    },
                    pitch_env: Some(PitchEnvelope {
                        drop_semitones: 36.0,
                        decay_sec: 0.04,
                    }),
                },
                fx: vec![],
                events: vec![NoteEvent {
                    start_beats: 0.0,
                    duration_beats: 0.5,
                    midi_note: 36,
                    chord_notes: None,
                    velocity: 0.9,
                }],
            },
            Track {
                id: "snare".to_string(),
                instrument: InstrumentConfig {
                    waveform: Waveform::Noise,
                    adsr: Adsr {
                        attack_sec: 0.001,
                        decay_sec: 0.15,
                        sustain_level: 0.0,
                        release_sec: 0.02,
                    },
                    pitch_env: None,
                },
                fx: vec![FxConfig::HighPass { cutoff_hz: 1000.0 }],
                events: vec![NoteEvent {
                    start_beats: 1.0,
                    duration_beats: 0.5,
                    midi_note: 60,
                    chord_notes: None,
                    velocity: 0.7,
                }],
            },
        ],
    };

    let rendered = compile_song(&song);
    assert_eq!(rendered.channels, 2);
    assert!(!rendered.samples.is_empty());

    let telemetry = analyze_audio(&rendered.samples, rendered.sample_rate, rendered.channels);
    assert!(telemetry.peak_amplitude > 0.0);
    assert!(!telemetry.is_clipping);
}

#[test]
fn test_stereo_panning_and_chords() {
    let song = Song {
        tempo: 120.0,
        time_signature: (4, 4),
        sample_rate: Some(44100),
        tracks: vec![
            Track {
                id: "left_chords".to_string(),
                instrument: InstrumentConfig {
                    waveform: Waveform::Triangle,
                    adsr: Adsr {
                        attack_sec: 0.05,
                        decay_sec: 0.2,
                        sustain_level: 0.7,
                        release_sec: 0.2,
                    },
                    pitch_env: None,
                },
                fx: vec![FxConfig::Pan { pan: -0.8 }],
                events: vec![NoteEvent {
                    start_beats: 0.0,
                    duration_beats: 2.0,
                    midi_note: 60,
                    chord_notes: Some(vec![60, 64, 67, 71]), // Cmaj7
                    velocity: 0.7,
                }],
            },
            Track {
                id: "right_lead".to_string(),
                instrument: InstrumentConfig {
                    waveform: Waveform::Sine,
                    adsr: Adsr::default(),
                    pitch_env: None,
                },
                fx: vec![FxConfig::Pan { pan: 0.8 }],
                events: vec![NoteEvent {
                    start_beats: 0.5,
                    duration_beats: 1.0,
                    midi_note: 72,
                    chord_notes: None,
                    velocity: 0.6,
                }],
            },
        ],
    };

    let rendered = compile_song(&song);
    let telemetry = analyze_audio(&rendered.samples, rendered.sample_rate, rendered.channels);
    assert_eq!(telemetry.channels, 2);
    assert!(telemetry.stereo_correlation.is_some());
    // Since tracks are hard-panned left and right, correlation should be less than 0.99
    assert!(telemetry.stereo_correlation.unwrap() < 0.99);

    let tmp_path = std::env::temp_dir().join("auralis_stereo_test.wav");
    export_wav(&rendered, &tmp_path).expect("Export failed");
    let reader = hound::WavReader::open(&tmp_path).expect("Open failed");
    assert_eq!(reader.spec().channels, 2);
    let _ = std::fs::remove_file(tmp_path);
}
