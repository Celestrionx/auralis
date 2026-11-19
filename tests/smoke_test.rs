use auralis::graph::{compile_song, export_wav};
use auralis::model::{Adsr, FxConfig, InstrumentConfig, NoteEvent, Song, Track, Waveform};
use auralis::telemetry::analyze_audio;

#[test]
fn test_manifest_roundtrip() {
    let song = Song {
        tempo: 140.0,
        time_signature: (3, 4),
        sample_rate: Some(48000),
        tracks: vec![Track {
            id: "test_track".to_string(),
            instrument: InstrumentConfig {
                waveform: Waveform::Square,
                adsr: Adsr::default(),
            },
            fx: vec![FxConfig::Gain { gain_db: -3.0 }],
            events: vec![NoteEvent {
                start_beats: 0.0,
                duration_beats: 2.0,
                midi_note: 69, // A4
                velocity: 0.75,
            }],
        }],
    };

    let serialized = serde_json::to_string(&song).expect("Serialize failed");
    let deserialized: Song = serde_json::from_str(&serialized).expect("Deserialize failed");
    assert_eq!(song, deserialized);
}

#[test]
fn test_synthesis_and_telemetry() {
    let song = Song {
        tempo: 120.0,
        time_signature: (4, 4),
        sample_rate: Some(44100),
        tracks: vec![Track {
            id: "sine_tone".to_string(),
            instrument: InstrumentConfig {
                waveform: Waveform::Sine,
                adsr: Adsr {
                    attack_sec: 0.01,
                    decay_sec: 0.05,
                    sustain_level: 0.8,
                    release_sec: 0.05,
                },
            },
            fx: vec![],
            events: vec![NoteEvent {
                start_beats: 0.0,
                duration_beats: 1.0,
                midi_note: 69, // 440 Hz
                velocity: 0.8,
            }],
        }],
    };

    let rendered = compile_song(&song);
    assert!(!rendered.samples.is_empty());

    let telemetry = analyze_audio(&rendered.samples, rendered.sample_rate);
    assert!(telemetry.peak_amplitude > 0.0);
    assert!(telemetry.peak_amplitude <= 1.0);
    assert!(!telemetry.is_clipping);
    assert!(telemetry.duration_seconds > 0.5);
    // 440Hz sits squarely in mid range (250Hz - 4000Hz)
    assert!(telemetry.spectral_bands.mid_ratio > telemetry.spectral_bands.high_ratio);
}

#[test]
fn test_wav_export_roundtrip() {
    let song = Song {
        tempo: 120.0,
        time_signature: (4, 4),
        sample_rate: Some(44100),
        tracks: vec![Track {
            id: "short_beep".to_string(),
            instrument: InstrumentConfig {
                waveform: Waveform::Triangle,
                adsr: Adsr::default(),
            },
            fx: vec![],
            events: vec![NoteEvent {
                start_beats: 0.0,
                duration_beats: 0.25,
                midi_note: 60,
                velocity: 0.5,
            }],
        }],
    };

    let rendered = compile_song(&song);
    let tmp_path = std::env::temp_dir().join("auralis_test_output.wav");
    export_wav(&rendered, &tmp_path).expect("Export WAV failed");

    let reader = hound::WavReader::open(&tmp_path).expect("Open WAV failed");
    assert_eq!(reader.spec().channels, 1);
    assert_eq!(reader.spec().sample_rate, 44100);
    assert!(reader.len() > 0);

    let _ = std::fs::remove_file(tmp_path);
}
