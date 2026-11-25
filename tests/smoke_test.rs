use auralis::graph::{compile_song, export_wav, read_wav_loop_points};
use auralis::model::{
    Adsr, FxConfig, InstrumentConfig, LoopPoints, NoteEvent, PitchEnvelope, Song, Track, Waveform,
};
use auralis::telemetry::analyze_audio;

#[test]
fn test_manifest_roundtrip() {
    let song = Song {
        tempo: 140.0,
        time_signature: (3, 4),
        sample_rate: Some(48000),
        loop_points: Some(LoopPoints {
            start_beat: 1.0,
            end_beat: Some(7.0),
        }),
        master_fx: Vec::new(),
        tracks: vec![Track {
            id: "drum_track".to_string(),
            instrument: InstrumentConfig::Synth {
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
                FxConfig::HighPass {
                    cutoff_hz: 800.0,
                    q: None,
                },
            ],
            events: vec![NoteEvent {
                start_beats: 0.0,
                duration_beats: 1.0,
                midi_note: 60,
                chord_notes: Some(vec![60, 64, 67]),
                velocity: 0.8,
                ..Default::default()
            }],
        }],
    };

    let serialized = serde_json::to_string(&song).expect("Serialize failed");
    let deserialized: Song = serde_json::from_str(&serialized).expect("Deserialize failed");
    assert_eq!(song, deserialized);
}

#[test]
fn test_soundfont_manifest_roundtrip_and_fallback() {
    let song = Song {
        tempo: 120.0,
        time_signature: (4, 4),
        sample_rate: Some(44100),
        loop_points: None,
        master_fx: Vec::new(),
        tracks: vec![Track {
            id: "piano_track".to_string(),
            instrument: InstrumentConfig::SoundFont {
                soundfont_path: "assets/non_existent.sf2".to_string(),
                bank: Some(0),
                preset: Some(0),
            },
            fx: vec![FxConfig::Gain { gain_db: -2.0 }],
            events: vec![NoteEvent {
                start_beats: 0.0,
                duration_beats: 1.0,
                midi_note: 60,
                velocity: 0.8,
                ..Default::default()
            }],
        }],
    };

    let serialized = serde_json::to_string(&song).expect("Serialize failed");
    let deserialized: Song = serde_json::from_str(&serialized).expect("Deserialize failed");
    assert_eq!(song, deserialized);

    let rendered = compile_song(&song);
    assert_eq!(rendered.channels, 2);
    assert!(!rendered.samples.is_empty());
}

#[test]
fn test_real_soundfont_synthesis() {
    let sf2_path = "assets/soundfonts/TimGM6mb.sf2";
    if !std::path::Path::new(sf2_path).exists() {
        return;
    }

    let song = Song {
        tempo: 100.0,
        time_signature: (4, 4),
        sample_rate: Some(44100),
        loop_points: None,
        master_fx: Vec::new(),
        tracks: vec![Track {
            id: "grand_piano".to_string(),
            instrument: InstrumentConfig::SoundFont {
                soundfont_path: sf2_path.to_string(),
                bank: Some(0),
                preset: Some(0), // Acoustic Grand Piano
            },
            fx: vec![FxConfig::Gain { gain_db: -3.0 }],
            events: vec![NoteEvent {
                start_beats: 0.0,
                duration_beats: 2.0,
                midi_note: 60,
                chord_notes: Some(vec![60, 64, 67]), // C Major triad
                velocity: 0.8,
                ..Default::default()
            }],
        }],
    };

    let rendered = compile_song(&song);
    assert_eq!(rendered.channels, 2);
    assert!(!rendered.samples.is_empty());

    let telemetry = analyze_audio(
        &rendered.samples,
        rendered.sample_rate,
        rendered.channels,
        rendered.loop_samples,
    );
    assert!(telemetry.peak_amplitude > 0.01);
    assert!(!telemetry.is_clipping);
}

#[test]
fn test_percussion_synthesis() {
    let song = Song {
        tempo: 120.0,
        time_signature: (4, 4),
        sample_rate: Some(44100),
        loop_points: None,
        master_fx: Vec::new(),
        tracks: vec![
            Track {
                id: "kick".to_string(),
                instrument: InstrumentConfig::Synth {
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
                    velocity: 0.9,
                    ..Default::default()
                }],
            },
            Track {
                id: "snare".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Noise,
                    adsr: Adsr {
                        attack_sec: 0.001,
                        decay_sec: 0.15,
                        sustain_level: 0.0,
                        release_sec: 0.02,
                    },
                    pitch_env: None,
                },
                fx: vec![FxConfig::HighPass {
                    cutoff_hz: 1000.0,
                    q: None,
                }],
                events: vec![NoteEvent {
                    start_beats: 1.0,
                    duration_beats: 0.5,
                    midi_note: 60,
                    velocity: 0.7,
                    ..Default::default()
                }],
            },
        ],
    };

    let rendered = compile_song(&song);
    assert_eq!(rendered.channels, 2);
    assert!(!rendered.samples.is_empty());

    let telemetry = analyze_audio(
        &rendered.samples,
        rendered.sample_rate,
        rendered.channels,
        rendered.loop_samples,
    );
    assert!(telemetry.peak_amplitude > 0.0);
    assert!(!telemetry.is_clipping);
}

#[test]
fn test_stereo_panning_and_chords() {
    let song = Song {
        tempo: 120.0,
        time_signature: (4, 4),
        sample_rate: Some(44100),
        loop_points: None,
        master_fx: Vec::new(),
        tracks: vec![
            Track {
                id: "left_chords".to_string(),
                instrument: InstrumentConfig::Synth {
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
                    ..Default::default()
                }],
            },
            Track {
                id: "right_lead".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Sine,
                    adsr: Adsr::default(),
                    pitch_env: None,
                },
                fx: vec![FxConfig::Pan { pan: 0.8 }],
                events: vec![NoteEvent {
                    start_beats: 0.5,
                    duration_beats: 1.0,
                    midi_note: 72,
                    velocity: 0.6,
                    ..Default::default()
                }],
            },
        ],
    };

    let rendered = compile_song(&song);
    let telemetry = analyze_audio(
        &rendered.samples,
        rendered.sample_rate,
        rendered.channels,
        rendered.loop_samples,
    );
    assert_eq!(telemetry.channels, 2);
    assert!(telemetry.stereo_correlation.is_some());
    assert!(telemetry.stereo_correlation.unwrap() < 0.99);

    let tmp_path = std::env::temp_dir().join("auralis_stereo_test.wav");
    export_wav(&rendered, &tmp_path).expect("Export failed");
    let reader = hound::WavReader::open(&tmp_path).expect("Open failed");
    assert_eq!(reader.spec().channels, 2);
    let _ = std::fs::remove_file(tmp_path);
}

#[test]
fn test_seamless_loop_metadata_export_and_read() {
    let song = Song {
        tempo: 120.0,
        time_signature: (4, 4),
        sample_rate: Some(44100),
        loop_points: Some(LoopPoints {
            start_beat: 4.0,      // 2.0 seconds = 88200 samples
            end_beat: Some(12.0), // 6.0 seconds = 264600 samples
        }),
        master_fx: Vec::new(),
        tracks: vec![Track {
            id: "ambient_loop".to_string(),
            instrument: InstrumentConfig::Synth {
                waveform: Waveform::Sine,
                adsr: Adsr::default(),
                pitch_env: None,
            },
            fx: vec![],
            events: vec![NoteEvent {
                start_beats: 0.0,
                duration_beats: 16.0,
                midi_note: 60,
                velocity: 0.7,
                ..Default::default()
            }],
        }],
    };

    let rendered = compile_song(&song);
    assert_eq!(rendered.loop_samples, Some((88200, 264600)));

    let tmp_path = std::env::temp_dir().join("auralis_loop_test.wav");
    export_wav(&rendered, &tmp_path).expect("Export failed");

    // Verify RIFF 'smpl' chunk reader parses exact loop markers
    let read_loops = read_wav_loop_points(&tmp_path);
    assert_eq!(read_loops, Some((88200, 264600)));

    let telemetry = analyze_audio(
        &rendered.samples,
        rendered.sample_rate,
        rendered.channels,
        rendered.loop_samples,
    );
    assert!(telemetry.loop_telemetry.is_some());
    let lt = telemetry.loop_telemetry.unwrap();
    assert_eq!(lt.start_sample, 88200);
    assert_eq!(lt.end_sample, 264600);
    assert!((lt.start_seconds - 2.0).abs() < 1e-4);
    assert!((lt.end_seconds - 6.0).abs() < 1e-4);
    assert!((lt.loop_duration_seconds - 4.0).abs() < 1e-4);

    let _ = std::fs::remove_file(tmp_path);
}

#[test]
fn test_portamento_glide_and_phase_continuity() {
    let song = Song {
        tempo: 120.0,
        time_signature: (4, 4),
        sample_rate: Some(44100),
        loop_points: None,
        master_fx: Vec::new(),
        tracks: vec![Track {
            id: "glide_bass".to_string(),
            instrument: InstrumentConfig::Synth {
                waveform: Waveform::Sine,
                adsr: Adsr {
                    attack_sec: 0.01,
                    decay_sec: 0.1,
                    sustain_level: 0.8,
                    release_sec: 0.05,
                },
                pitch_env: None,
            },
            fx: vec![],
            events: vec![NoteEvent {
                start_beats: 0.0,
                duration_beats: 1.0,
                midi_note: 36,
                slide_to_midi: Some(48),
                glide_beats: Some(0.5),
                velocity: 0.85,
                ..Default::default()
            }],
        }],
    };

    let serialized = serde_json::to_string(&song).expect("Serialize failed");
    let deserialized: Song = serde_json::from_str(&serialized).expect("Deserialize failed");
    assert_eq!(song, deserialized);

    let rendered = compile_song(&song);
    let telemetry = analyze_audio(
        &rendered.samples,
        rendered.sample_rate,
        rendered.channels,
        None,
    );
    assert!(telemetry.peak_amplitude > 0.3);
    assert!(!telemetry.is_clipping);
}

#[test]
fn test_resonant_biquad_saturation_and_master_softclip() {
    let song = Song {
        tempo: 120.0,
        time_signature: (4, 4),
        sample_rate: Some(44100),
        loop_points: None,
        master_fx: vec![FxConfig::SoftClip],
        tracks: vec![Track {
            id: "hot_synth".to_string(),
            instrument: InstrumentConfig::Synth {
                waveform: Waveform::Sawtooth,
                adsr: Adsr::default(),
                pitch_env: None,
            },
            fx: vec![
                FxConfig::LowPass {
                    cutoff_hz: 1200.0,
                    q: Some(3.5),
                },
                FxConfig::Saturate { drive: 3.0 },
                FxConfig::Gain { gain_db: 6.0 },
            ],
            events: vec![NoteEvent {
                start_beats: 0.0,
                duration_beats: 1.0,
                midi_note: 48,
                chord_notes: Some(vec![48, 55, 60]),
                velocity: 1.0,
                ..Default::default()
            }],
        }],
    };

    let rendered = compile_song(&song);
    let telemetry = analyze_audio(
        &rendered.samples,
        rendered.sample_rate,
        rendered.channels,
        None,
    );
    assert!(telemetry.peak_amplitude > 0.7);
    assert!(!telemetry.is_clipping);
}

#[test]
fn test_tempo_delay_and_schroeder_reverb_spatial() {
    let song = Song {
        tempo: 120.0,
        time_signature: (4, 4),
        sample_rate: Some(44100),
        loop_points: None,
        master_fx: Vec::new(),
        tracks: vec![Track {
            id: "ambient_bell".to_string(),
            instrument: InstrumentConfig::Synth {
                waveform: Waveform::Triangle,
                adsr: Adsr {
                    attack_sec: 0.005,
                    decay_sec: 0.1,
                    sustain_level: 0.0,
                    release_sec: 0.05,
                },
                pitch_env: None,
            },
            fx: vec![
                FxConfig::Delay {
                    delay_beats: 0.5,
                    feedback: 0.45,
                    mix: 0.35,
                },
                FxConfig::Reverb {
                    room_size: 0.75,
                    damping: 0.4,
                    mix: 0.35,
                },
            ],
            events: vec![NoteEvent {
                start_beats: 0.0,
                duration_beats: 0.5,
                midi_note: 72,
                velocity: 0.8,
                ..Default::default()
            }],
        }],
    };

    let rendered = compile_song(&song);
    // Tail should exceed dry duration (0.25s + 0.05s = 0.3s) due to spatial FX tail
    let telemetry = analyze_audio(
        &rendered.samples,
        rendered.sample_rate,
        rendered.channels,
        None,
    );
    assert!(telemetry.duration_seconds > 1.0);
    assert!(telemetry.stereo_correlation.unwrap() < 0.99);
    assert!(!telemetry.is_clipping);
}

#[test]
fn test_multi_window_spectral_telemetry_past_intro() {
    // Silent first second, then bright 6 kHz tone later in the track
    let sr = 44100;
    let mut samples = vec![0.0_f32; sr * 2 * 2]; // 2 seconds stereo
    for (i, slot) in samples.iter_mut().enumerate().skip(sr * 2) {
        let frame = (i - sr * 2) / 2;
        let t = frame as f32 / sr as f32;
        *slot = (2.0 * std::f32::consts::PI * 6000.0 * t).sin() * 0.5;
    }

    let telemetry = analyze_audio(&samples, sr as u32, 2, None);
    assert!(telemetry.spectral_bands.high_ratio > 0.7);
    assert!(telemetry.spectral_bands.sub_bass_ratio < 0.15);
}
