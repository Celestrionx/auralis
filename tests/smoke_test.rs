use auralis::graph::{
    compile_song, compile_song_with_stems, compile_stems, export_audio, export_wav,
    read_wav_loop_points,
};
use auralis::mcp::handle_mcp_request;
use auralis::model::{
    Adsr, Clip, FmConfig, FxConfig, InstrumentConfig, LfoConfig, LoopPoints, NoteEvent, Pattern,
    PitchEnvelope, Song, Track, UnisonConfig, Waveform,
};
use auralis::telemetry::analyze_audio;
use serde_json::{Value, json};
use std::collections::BTreeMap;

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
        patterns: BTreeMap::new(),
        tracks: vec![Track {
            id: "drum_track".to_string(),
            instrument: InstrumentConfig::Synth {
                waveform: Waveform::Noise,
                adsr: Adsr::default(),
                pitch_env: Some(PitchEnvelope {
                    drop_semitones: 24.0,
                    decay_sec: 0.05,
                }),
                unison: None,
                lfo: None,
                fm: None,
            },
            fx: vec![
                FxConfig::Gain { gain_db: -3.0 },
                FxConfig::Pan { pan: -0.5 },
                FxConfig::HighPass {
                    cutoff_hz: 800.0,
                    q: None,
                },
            ],
            clips: Vec::new(),
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
        patterns: BTreeMap::new(),
        tracks: vec![Track {
            id: "piano_track".to_string(),
            instrument: InstrumentConfig::SoundFont {
                soundfont_path: "assets/non_existent.sf2".to_string(),
                bank: Some(0),
                preset: Some(0),
            },
            fx: vec![FxConfig::Gain { gain_db: -2.0 }],
            clips: Vec::new(),
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
        patterns: BTreeMap::new(),
        tracks: vec![Track {
            id: "grand_piano".to_string(),
            instrument: InstrumentConfig::SoundFont {
                soundfont_path: sf2_path.to_string(),
                bank: Some(0),
                preset: Some(0), // Acoustic Grand Piano
            },
            fx: vec![FxConfig::Gain { gain_db: -3.0 }],
            clips: Vec::new(),
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
        patterns: BTreeMap::new(),
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
                    unison: None,
                    lfo: None,
                    fm: None,
                },
                fx: vec![],
                clips: Vec::new(),
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
                    unison: None,
                    lfo: None,
                    fm: None,
                },
                fx: vec![FxConfig::HighPass {
                    cutoff_hz: 1000.0,
                    q: None,
                }],
                clips: Vec::new(),
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
        patterns: BTreeMap::new(),
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
                    unison: None,
                    lfo: None,
                    fm: None,
                },
                fx: vec![FxConfig::Pan { pan: -0.8 }],
                clips: Vec::new(),
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
                    unison: None,
                    lfo: None,
                    fm: None,
                },
                fx: vec![FxConfig::Pan { pan: 0.8 }],
                clips: Vec::new(),
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
        patterns: BTreeMap::new(),
        tracks: vec![Track {
            id: "ambient_loop".to_string(),
            instrument: InstrumentConfig::Synth {
                waveform: Waveform::Sine,
                adsr: Adsr::default(),
                pitch_env: None,
                unison: None,
                lfo: None,
                fm: None,
            },
            fx: vec![],
            clips: Vec::new(),
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
        patterns: BTreeMap::new(),
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
                unison: None,
                lfo: None,
                fm: None,
            },
            fx: vec![],
            clips: Vec::new(),
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
        patterns: BTreeMap::new(),
        tracks: vec![Track {
            id: "hot_synth".to_string(),
            instrument: InstrumentConfig::Synth {
                waveform: Waveform::Sawtooth,
                adsr: Adsr::default(),
                pitch_env: None,
                unison: None,
                lfo: None,
                fm: None,
            },
            fx: vec![
                FxConfig::LowPass {
                    cutoff_hz: 1200.0,
                    q: Some(3.5),
                },
                FxConfig::Saturate { drive: 3.0 },
                FxConfig::Gain { gain_db: 6.0 },
            ],
            clips: Vec::new(),
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
        patterns: BTreeMap::new(),
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
                unison: None,
                lfo: None,
                fm: None,
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
            clips: Vec::new(),
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

#[test]
fn test_pattern_clip_bit_identical_pcm_parity() {
    let base_events = vec![
        NoteEvent {
            start_beats: 0.0,
            duration_beats: 0.5,
            midi_note: 36,
            velocity: 0.9,
            ..Default::default()
        },
        NoteEvent {
            start_beats: 1.0,
            duration_beats: 0.5,
            midi_note: 43,
            slide_to_midi: Some(48),
            glide_beats: Some(0.25),
            velocity: 0.8,
            ..Default::default()
        },
    ];

    let mut flat_events = Vec::new();
    for rep in 0..4 {
        for ev in &base_events {
            let mut copy = ev.clone();
            copy.start_beats += rep as f32 * 2.0;
            flat_events.push(copy);
        }
    }

    let instrument = InstrumentConfig::Synth {
        waveform: Waveform::Sine,
        adsr: Adsr::default(),
        pitch_env: None,
        unison: None,
        lfo: None,
        fm: None,
    };

    let flat_song = Song {
        tempo: 130.0,
        tracks: vec![Track {
            id: "bass".to_string(),
            instrument: instrument.clone(),
            fx: vec![],
            clips: Vec::new(),
            events: flat_events,
        }],
        ..Default::default()
    };

    let mut patterns = BTreeMap::new();
    patterns.insert(
        "bass_cell".to_string(),
        Pattern {
            length_beats: 2.0,
            events: base_events,
        },
    );

    let pattern_song = Song {
        tempo: 130.0,
        patterns,
        tracks: vec![Track {
            id: "bass".to_string(),
            instrument,
            fx: vec![],
            clips: vec![Clip {
                pattern: "bass_cell".to_string(),
                start_beats: 0.0,
                repeat: Some(4),
                transpose_semitones: None,
                velocity_scale: None,
            }],
            events: Vec::new(),
        }],
        ..Default::default()
    };

    let flat_audio = compile_song(&flat_song);
    let pat_audio = compile_song(&pattern_song);
    assert_eq!(flat_audio.samples, pat_audio.samples);
}

#[test]
fn test_pattern_clip_transpose_velocity_and_roundtrip() {
    let mut patterns = BTreeMap::new();
    patterns.insert(
        "triad_riff".to_string(),
        Pattern {
            length_beats: 4.0,
            events: vec![NoteEvent {
                start_beats: 0.0,
                duration_beats: 1.0,
                midi_note: 48,
                chord_notes: Some(vec![48, 52, 55]),
                slide_to_midi: Some(60),
                glide_beats: Some(0.5),
                velocity: 0.8,
                ..Default::default()
            }],
        },
    );

    let track = Track {
        id: "lead".to_string(),
        instrument: InstrumentConfig::Synth {
            waveform: Waveform::Triangle,
            adsr: Adsr::default(),
            pitch_env: None,
            unison: None,
            lfo: None,
            fm: None,
        },
        fx: vec![],
        clips: vec![Clip {
            pattern: "triad_riff".to_string(),
            start_beats: 2.0,
            repeat: Some(2),
            transpose_semitones: Some(12),
            velocity_scale: Some(0.5),
        }],
        events: Vec::new(),
    };

    let resolved = track.resolved_events(&patterns);
    assert_eq!(resolved.len(), 2);
    assert_eq!(resolved[0].start_beats, 2.0);
    assert_eq!(resolved[1].start_beats, 6.0);
    assert_eq!(resolved[0].midi_note, 60);
    assert_eq!(resolved[0].chord_notes, Some(vec![60, 64, 67]));
    assert_eq!(resolved[0].slide_to_midi, Some(72));
    assert!((resolved[0].velocity - 0.4).abs() < 1e-6);

    let song = Song {
        patterns,
        tracks: vec![track],
        ..Default::default()
    };
    let json = serde_json::to_string(&song).expect("Serialize failed");
    let decoded: Song = serde_json::from_str(&json).expect("Deserialize failed");
    assert_eq!(song, decoded);
}

#[test]
fn test_pattern_missing_graceful_fallback() {
    let song = Song {
        tracks: vec![Track {
            id: "broken_ref".to_string(),
            instrument: InstrumentConfig::Synth {
                waveform: Waveform::Sine,
                adsr: Adsr::default(),
                pitch_env: None,
                unison: None,
                lfo: None,
                fm: None,
            },
            fx: vec![],
            clips: vec![Clip {
                pattern: "does_not_exist".to_string(),
                start_beats: 0.0,
                repeat: Some(2),
                transpose_semitones: None,
                velocity_scale: None,
            }],
            events: Vec::new(),
        }],
        ..Default::default()
    };

    let rendered = compile_song(&song);
    assert!(!rendered.samples.is_empty());
}

#[test]
fn test_mcp_tools_list_inspect_soundfont_and_eval_wav() {
    let list_resp = handle_mcp_request(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list"
    }));
    let tools = list_resp["result"]["tools"]
        .as_array()
        .expect("tools array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"auralis_init_manifest"));
    assert!(names.contains(&"auralis_compile_audio"));
    assert!(names.contains(&"auralis_inspect_soundfont"));
    assert!(names.contains(&"auralis_eval_wav"));

    // Error handling on missing files
    let missing_sf = handle_mcp_request(&json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "auralis_inspect_soundfont",
            "arguments": { "soundfont_path": "assets/missing.sf2" }
        }
    }));
    assert_eq!(missing_sf["result"]["isError"], json!(true));

    let missing_wav = handle_mcp_request(&json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "auralis_eval_wav",
            "arguments": { "wav_path": "missing_audio.wav" }
        }
    }));
    assert_eq!(missing_wav["result"]["isError"], json!(true));

    // Real SoundFont preset discovery (if bundled TimGM6mb.sf2 exists)
    let sf2_path = "assets/soundfonts/TimGM6mb.sf2";
    if std::path::Path::new(sf2_path).exists() {
        let sf_resp = handle_mcp_request(&json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "auralis_inspect_soundfont",
                "arguments": { "soundfont_path": sf2_path, "bank": 128 }
            }
        }));
        assert!(sf_resp["result"]["isError"].is_null());
        let text = sf_resp["result"]["content"][0]["text"]
            .as_str()
            .expect("sf json text");
        let parsed: Value = serde_json::from_str(text).expect("valid json");
        let presets = parsed["presets"].as_array().expect("presets array");
        assert!(
            presets
                .iter()
                .any(|p| p["bank"] == 128 && p["preset"] == 25)
        );
    }

    // Compile + evaluate WAV via MCP
    let tmp_wav = std::env::temp_dir().join("auralis_mcp_eval_test.wav");
    let song = Song {
        tempo: 120.0,
        loop_points: Some(LoopPoints {
            start_beat: 0.0,
            end_beat: Some(2.0),
        }),
        tracks: vec![Track {
            id: "tone".to_string(),
            instrument: InstrumentConfig::Synth {
                waveform: Waveform::Sine,
                adsr: Adsr::default(),
                pitch_env: None,
                unison: None,
                lfo: None,
                fm: None,
            },
            fx: vec![],
            clips: Vec::new(),
            events: vec![NoteEvent {
                start_beats: 0.0,
                duration_beats: 2.0,
                midi_note: 69,
                velocity: 0.7,
                ..Default::default()
            }],
        }],
        ..Default::default()
    };

    let compile_resp = handle_mcp_request(&json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "auralis_compile_audio",
            "arguments": {
                "manifest_json": serde_json::to_string(&song).unwrap(),
                "output_wav_path": tmp_wav.to_string_lossy()
            }
        }
    }));
    assert!(compile_resp["result"]["isError"].is_null());

    let eval_resp = handle_mcp_request(&json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "tools/call",
        "params": {
            "name": "auralis_eval_wav",
            "arguments": { "wav_path": tmp_wav.to_string_lossy() }
        }
    }));
    assert!(eval_resp["result"]["isError"].is_null());
    let eval_text = eval_resp["result"]["content"][0]["text"]
        .as_str()
        .expect("eval json text");
    let telemetry: Value = serde_json::from_str(eval_text).expect("valid telemetry json");
    assert!(telemetry["peak_amplitude"].as_f64().unwrap() > 0.1);
    assert!(telemetry["loop_telemetry"].is_object());

    let _ = std::fs::remove_file(tmp_wav);
}

#[test]
fn test_sidechain_ducking_attenuation() {
    let pad_track = Track {
        id: "pad".to_string(),
        instrument: InstrumentConfig::Synth {
            waveform: Waveform::Sine,
            adsr: Adsr {
                attack_sec: 0.005,
                decay_sec: 0.05,
                sustain_level: 0.9,
                release_sec: 0.05,
            },
            pitch_env: None,
            unison: None,
            lfo: None,
            fm: None,
        },
        fx: vec![FxConfig::Sidechain {
            source_track: "kick".to_string(),
            amount: 0.9,
            threshold_db: -24.0,
            attack_sec: 0.002,
            release_sec: 0.10,
        }],
        clips: Vec::new(),
        events: vec![NoteEvent {
            start_beats: 0.0,
            duration_beats: 2.0,
            midi_note: 60,
            velocity: 0.8,
            ..Default::default()
        }],
    };

    let mut dry_pad = pad_track.clone();
    dry_pad.fx.clear();

    let kick_track = Track {
        id: "kick".to_string(),
        instrument: InstrumentConfig::Synth {
            waveform: Waveform::Sine,
            adsr: Adsr {
                attack_sec: 0.001,
                decay_sec: 0.15,
                sustain_level: 0.0,
                release_sec: 0.02,
            },
            pitch_env: None,
            unison: None,
            lfo: None,
            fm: None,
        },
        fx: vec![FxConfig::Gain { gain_db: -40.0 }], // silent in mix, raw trigger still active!
        clips: Vec::new(),
        events: vec![NoteEvent {
            start_beats: 0.0,
            duration_beats: 0.5,
            midi_note: 36,
            velocity: 1.0,
            ..Default::default()
        }],
    };

    let dry_audio = compile_song(&Song {
        tempo: 120.0,
        tracks: vec![dry_pad],
        ..Default::default()
    });
    let ducked_audio = compile_song(&Song {
        tempo: 120.0,
        tracks: vec![kick_track, pad_track],
        ..Default::default()
    });

    // During kick transient (first 150ms), ducked RMS must be noticeably lower than dry RMS
    let window = (44100.0 * 0.15) as usize * 2;
    let dry_rms: f32 = (dry_audio.samples[..window]
        .iter()
        .map(|s| s * s)
        .sum::<f32>()
        / window as f32)
        .sqrt();
    let ducked_rms: f32 = (ducked_audio.samples[..window]
        .iter()
        .map(|s| s * s)
        .sum::<f32>()
        / window as f32)
        .sqrt();
    assert!(ducked_rms < dry_rms * 0.6);

    // Deterministic parallel compile check
    let ducked_again = compile_song(&Song {
        tempo: 120.0,
        tracks: vec![
            Track {
                id: "kick".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Sine,
                    adsr: Adsr {
                        attack_sec: 0.001,
                        decay_sec: 0.15,
                        sustain_level: 0.0,
                        release_sec: 0.02,
                    },
                    pitch_env: None,
                    unison: None,
                    lfo: None,
                    fm: None,
                },
                fx: vec![FxConfig::Gain { gain_db: -40.0 }],
                clips: Vec::new(),
                events: vec![NoteEvent {
                    start_beats: 0.0,
                    duration_beats: 0.5,
                    midi_note: 36,
                    velocity: 1.0,
                    ..Default::default()
                }],
            },
            Track {
                id: "pad".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Sine,
                    adsr: Adsr {
                        attack_sec: 0.005,
                        decay_sec: 0.05,
                        sustain_level: 0.9,
                        release_sec: 0.05,
                    },
                    pitch_env: None,
                    unison: None,
                    lfo: None,
                    fm: None,
                },
                fx: vec![FxConfig::Sidechain {
                    source_track: "kick".to_string(),
                    amount: 0.9,
                    threshold_db: -24.0,
                    attack_sec: 0.002,
                    release_sec: 0.10,
                }],
                clips: Vec::new(),
                events: vec![NoteEvent {
                    start_beats: 0.0,
                    duration_beats: 2.0,
                    midi_note: 60,
                    velocity: 0.8,
                    ..Default::default()
                }],
            },
        ],
        ..Default::default()
    });
    assert_eq!(ducked_audio.samples, ducked_again.samples);
}

#[test]
fn test_compressor_and_stereo_width_mid_side() {
    let song = Song {
        tempo: 120.0,
        tracks: vec![
            Track {
                id: "left_tone".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Sine,
                    adsr: Adsr::default(),
                    pitch_env: None,
                    unison: None,
                    lfo: None,
                    fm: None,
                },
                fx: vec![FxConfig::Pan { pan: -0.9 }],
                clips: Vec::new(),
                events: vec![NoteEvent {
                    start_beats: 0.0,
                    duration_beats: 1.0,
                    midi_note: 60,
                    velocity: 0.9,
                    ..Default::default()
                }],
            },
            Track {
                id: "right_tone".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Triangle,
                    adsr: Adsr::default(),
                    pitch_env: None,
                    unison: None,
                    lfo: None,
                    fm: None,
                },
                fx: vec![FxConfig::Pan { pan: 0.9 }],
                clips: Vec::new(),
                events: vec![NoteEvent {
                    start_beats: 0.0,
                    duration_beats: 1.0,
                    midi_note: 67,
                    velocity: 0.9,
                    ..Default::default()
                }],
            },
        ],
        master_fx: vec![
            FxConfig::Compressor {
                threshold_db: -18.0,
                ratio: 4.0,
                attack_sec: 0.003,
                release_sec: 0.08,
                makeup_db: 0.0,
            },
            FxConfig::StereoWidth { width: 0.0 }, // Collapse to mono
        ],
        ..Default::default()
    };

    let serialized = serde_json::to_string(&song).expect("Serialize failed");
    let decoded: Song = serde_json::from_str(&serialized).expect("Deserialize failed");
    assert_eq!(song, decoded);

    let rendered = compile_song(&song);
    let telemetry = analyze_audio(
        &rendered.samples,
        rendered.sample_rate,
        rendered.channels,
        None,
    );
    assert!(telemetry.stereo_correlation.unwrap() > 0.999);
    assert!(telemetry.peak_amplitude < 0.75);
}

#[test]
fn test_section_telemetry_crest_factor_and_diagnostics() {
    let sr = 44100;
    let total_frames = sr * 2; // 2.0 seconds -> 4 sections of 0.5s each
    let mut samples = vec![0.0_f32; total_frames * 2];

    // Section 0..1 (first 1.0s): quiet 80 Hz sub-bass (-28 dBFS)
    // Section 2..3 (last 1.0s): loud 80 Hz sub-bass (-3 dBFS) -> triggers SUB_BASS_DOMINANT, DULL_HIGH_END, HIGH_SECTION_VARIANCE
    for frame in 0..total_frames {
        let t = frame as f32 / sr as f32;
        let amp = if frame < sr { 0.04 } else { 0.70 };
        let val = (2.0 * std::f32::consts::PI * 80.0 * t).sin() * amp;
        samples[frame * 2] = val;
        samples[frame * 2 + 1] = val;
    }

    let telemetry = analyze_audio(&samples, sr as u32, 2, None);
    assert_eq!(telemetry.sections.len(), 4);
    assert!(telemetry.crest_factor_db > 2.0);
    assert!(telemetry.side_to_mid_ratio.unwrap() < 0.001);
    assert!(
        telemetry
            .diagnostics
            .contains(&"SUB_BASS_DOMINANT".to_string())
    );
    assert!(telemetry.diagnostics.contains(&"DULL_HIGH_END".to_string()));
    assert!(
        telemetry
            .diagnostics
            .contains(&"HIGH_SECTION_VARIANCE".to_string())
    );
    assert!(telemetry.sections[3].rms_dbfs > telemetry.sections[0].rms_dbfs + 18.0);
}

#[test]
fn test_compile_stems_parity_and_loop_markers() {
    let song = Song {
        tempo: 120.0,
        loop_points: Some(LoopPoints {
            start_beat: 0.0,
            end_beat: Some(2.0),
        }),
        tracks: vec![
            Track {
                id: "bass".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Sine,
                    adsr: Adsr::default(),
                    pitch_env: None,
                    unison: None,
                    lfo: None,
                    fm: None,
                },
                fx: vec![FxConfig::Pan { pan: -0.3 }],
                clips: Vec::new(),
                events: vec![NoteEvent {
                    start_beats: 0.0,
                    duration_beats: 2.0,
                    midi_note: 36,
                    velocity: 0.7,
                    ..Default::default()
                }],
            },
            Track {
                id: "lead".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Triangle,
                    adsr: Adsr::default(),
                    pitch_env: None,
                    unison: None,
                    lfo: None,
                    fm: None,
                },
                fx: vec![FxConfig::Pan { pan: 0.3 }],
                clips: Vec::new(),
                events: vec![NoteEvent {
                    start_beats: 0.5,
                    duration_beats: 1.5,
                    midi_note: 67,
                    velocity: 0.6,
                    ..Default::default()
                }],
            },
        ],
        ..Default::default()
    };

    let (full_mix, stems) = compile_song_with_stems(&song);
    let stems_only = compile_stems(&song);
    assert_eq!(stems.len(), 2);
    assert_eq!(stems_only.len(), 2);
    assert_eq!(stems[0].0, "bass");
    assert_eq!(stems[1].0, "lead");
    assert_eq!(stems[0].1.samples.len(), full_mix.samples.len());
    assert_eq!(stems[1].1.samples.len(), full_mix.samples.len());
    assert_eq!(stems[0].1.loop_samples, full_mix.loop_samples);

    // Without master_fx, summing stems equals full_mix sample-for-sample
    for i in 0..full_mix.samples.len() {
        let summed = stems[0].1.samples[i] + stems[1].1.samples[i];
        assert!((summed - full_mix.samples[i]).abs() < 1e-6);
    }
}

#[test]
fn test_export_ogg_with_loop_tags_if_ffmpeg_available() {
    if std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_err()
    {
        return;
    }

    let song = Song {
        tempo: 120.0,
        loop_points: Some(LoopPoints {
            start_beat: 1.0,
            end_beat: Some(3.0),
        }),
        tracks: vec![Track {
            id: "ogg_tone".to_string(),
            instrument: InstrumentConfig::Synth {
                waveform: Waveform::Sine,
                adsr: Adsr::default(),
                pitch_env: None,
                unison: None,
                lfo: None,
                fm: None,
            },
            fx: vec![],
            clips: Vec::new(),
            events: vec![NoteEvent {
                start_beats: 0.0,
                duration_beats: 4.0,
                midi_note: 60,
                velocity: 0.7,
                ..Default::default()
            }],
        }],
        ..Default::default()
    };

    let rendered = compile_song(&song);
    let tmp_ogg = std::env::temp_dir().join("auralis_loop_export_test.ogg");
    export_audio(&rendered, &tmp_ogg).expect("OGG export failed");

    let bytes = std::fs::read(&tmp_ogg).expect("Read OGG failed");
    assert!(bytes.starts_with(b"OggS"));
    let text_lossy = String::from_utf8_lossy(&bytes);
    assert!(text_lossy.contains("LOOPSTART=22050"));
    assert!(text_lossy.contains("LOOPLENGTH=44100"));

    let _ = std::fs::remove_file(tmp_ogg);
}

#[test]
fn test_synth_unison_lfo_and_fm_synthesis() {
    let song = Song {
        tempo: 120.0,
        tracks: vec![
            Track {
                id: "supersaw_lfo".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Sawtooth,
                    adsr: Adsr::default(),
                    pitch_env: None,
                    unison: Some(UnisonConfig {
                        voices: 5,
                        detune_cents: 24.0,
                        ..Default::default()
                    }),
                    lfo: Some(LfoConfig {
                        rate_hz: 5.5,
                        depth_semitones: 0.25,
                    }),
                    fm: None,
                },
                fx: vec![FxConfig::Gain { gain_db: -3.0 }],
                clips: Vec::new(),
                events: vec![NoteEvent {
                    start_beats: 0.0,
                    duration_beats: 1.5,
                    midi_note: 60,
                    velocity: 0.8,
                    ..Default::default()
                }],
            },
            Track {
                id: "fm_bell".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Sine,
                    adsr: Adsr {
                        attack_sec: 0.005,
                        decay_sec: 0.3,
                        sustain_level: 0.2,
                        release_sec: 0.1,
                    },
                    pitch_env: None,
                    unison: None,
                    lfo: None,
                    fm: Some(FmConfig {
                        mod_ratio: 2.0,
                        mod_index: 2.5,
                    }),
                },
                fx: vec![FxConfig::Gain { gain_db: -4.0 }],
                clips: Vec::new(),
                events: vec![NoteEvent {
                    start_beats: 0.0,
                    duration_beats: 1.5,
                    midi_note: 72,
                    velocity: 0.75,
                    ..Default::default()
                }],
            },
        ],
        ..Default::default()
    };

    let serialized = serde_json::to_string(&song).expect("Serialize failed");
    let decoded: Song = serde_json::from_str(&serialized).expect("Deserialize failed");
    assert_eq!(song, decoded);

    let rendered = compile_song(&song);
    let telemetry = analyze_audio(
        &rendered.samples,
        rendered.sample_rate,
        rendered.channels,
        None,
    );
    assert!(telemetry.peak_amplitude > 0.2);
    assert!(telemetry.spectral_bands.high_ratio > 0.05);
    assert!(telemetry.stereo_correlation.unwrap() < 0.85);
    assert!(telemetry.side_to_mid_ratio.unwrap() > 0.25);
    assert!(!telemetry.is_clipping);
}

#[test]
fn test_soundfont_pitch_bend_and_cc64_sustain() {
    let sf2_path = "assets/soundfonts/TimGM6mb.sf2";
    if !std::path::Path::new(sf2_path).exists() {
        return;
    }

    let song = Song {
        tempo: 120.0,
        tracks: vec![Track {
            id: "expressive_strings".to_string(),
            instrument: InstrumentConfig::SoundFont {
                soundfont_path: sf2_path.to_string(),
                bank: Some(0),
                preset: Some(48),
            },
            fx: vec![FxConfig::Gain { gain_db: -2.0 }],
            clips: Vec::new(),
            events: vec![NoteEvent {
                start_beats: 0.0,
                duration_beats: 1.0,
                midi_note: 60,
                slide_to_midi: Some(67),
                glide_beats: Some(0.5),
                sustain: Some(true),
                velocity: 0.8,
                ..Default::default()
            }],
        }],
        ..Default::default()
    };

    let serialized = serde_json::to_string(&song).expect("Serialize failed");
    let decoded: Song = serde_json::from_str(&serialized).expect("Deserialize failed");
    assert_eq!(song, decoded);

    let rendered = compile_song(&song);
    let telemetry = analyze_audio(
        &rendered.samples,
        rendered.sample_rate,
        rendered.channels,
        None,
    );
    assert!(telemetry.peak_amplitude > 0.01);
    assert!(!telemetry.is_clipping);
}
