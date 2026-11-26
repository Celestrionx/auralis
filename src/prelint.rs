use crate::model::{
    Adsr, Clip, FilterEnvelope, FilterSweepMode, FmConfig, FxConfig, InstrumentConfig, LfoConfig,
    LoopPoints, NoteEvent, Pattern, PitchEnvelope, Song, Track, UnisonConfig, Waveform,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuditSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreLintFinding {
    pub code: String,
    pub severity: AuditSeverity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_id: Option<String>,
    pub message: String,
    pub fix_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManifestAudit {
    pub quality_score: u8,
    pub passed: bool,
    pub track_count: usize,
    pub pattern_count: usize,
    pub resolved_event_count: usize,
    pub estimated_duration_sec: f32,
    pub velocity_std_dev: f32,
    pub findings: Vec<PreLintFinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Archetype {
    Synthwave,
    Ambient,
    Drill,
    Orchestral,
    SfxImpact,
}

impl Archetype {
    pub fn as_str(self) -> &'static str {
        match self {
            Archetype::Synthwave => "synthwave",
            Archetype::Ambient => "ambient",
            Archetype::Drill => "drill",
            Archetype::Orchestral => "orchestral",
            Archetype::SfxImpact => "sfx_impact",
        }
    }
}

pub fn parse_archetype(raw: Option<&str>) -> Result<Archetype, String> {
    match raw.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        None | Some("") | Some("synthwave") | Some("starter") => Ok(Archetype::Synthwave),
        Some("ambient") | Some("cinematic_ambient") => Ok(Archetype::Ambient),
        Some("drill") | Some("uk_drill") => Ok(Archetype::Drill),
        Some("orchestral") => Ok(Archetype::Orchestral),
        Some("sfx") | Some("sfx_impact") => Ok(Archetype::SfxImpact),
        Some(other) => Err(format!(
            "Unknown archetype '{}'. Valid archetypes: synthwave, ambient, drill, orchestral, sfx_impact",
            other
        )),
    }
}

fn std_dev(values: &[f32]) -> f32 {
    if values.len() < 2 {
        return 0.0;
    }
    let n = values.len() as f32;
    let mean = values.iter().sum::<f32>() / n;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / n;
    var.sqrt()
}

fn has_stereo_fx(fx: &[FxConfig]) -> bool {
    fx.iter().any(|effect| match effect {
        FxConfig::Pan { pan } => pan.abs() > 0.02,
        FxConfig::StereoWidth { width } => (*width - 1.0).abs() > 0.05,
        FxConfig::Delay { mix, .. } | FxConfig::Reverb { mix, .. } => *mix > 0.01,
        _ => false,
    })
}

fn has_highpass_or_sidechain(fx: &[FxConfig]) -> bool {
    fx.iter().any(|effect| {
        matches!(
            effect,
            FxConfig::HighPass { .. } | FxConfig::Sidechain { .. } | FxConfig::BandPass { .. }
        )
    })
}

/// Runs deterministic static pre-render intelligence checks on a `Song` manifest
/// before PCM synthesis begins, flagging generic patterns and mix conflicts.
pub fn analyze_manifest(song: &Song) -> ManifestAudit {
    let mut findings = Vec::new();
    let mut all_velocities = Vec::new();
    let mut total_resolved_events = 0usize;
    let mut max_end_beat = 0.0f32;
    let mut has_any_stereo_width = false;
    let mut low_end_tracks_without_carve = 0usize;

    for track in &song.tracks {
        for clip in &track.clips {
            if !song.patterns.contains_key(&clip.pattern) {
                findings.push(PreLintFinding {
                    code: "UNRESOLVED_CLIP_PATTERN".to_string(),
                    severity: AuditSeverity::Error,
                    track_id: Some(track.id.clone()),
                    message: format!(
                        "Track '{}' references missing pattern '{}'.",
                        track.id, clip.pattern
                    ),
                    fix_hint: format!(
                        "Define patterns[\"{}\"] or update clip.pattern to an existing pattern key.",
                        clip.pattern
                    ),
                });
            }
        }

        let resolved = track.resolved_events(&song.patterns);
        total_resolved_events += resolved.len();

        let mut track_velocities = Vec::with_capacity(resolved.len());
        let mut has_low_note = false;

        for ev in &resolved {
            track_velocities.push(ev.velocity);
            all_velocities.push(ev.velocity);
            let end_beat = ev.start_beats + ev.duration_beats.max(0.0);
            if end_beat > max_end_beat {
                max_end_beat = end_beat;
            }
            if ev.notes().into_iter().any(|n| n < 55) {
                has_low_note = true;
            }
        }

        if track_velocities.len() >= 4 && std_dev(&track_velocities) < 0.02 {
            findings.push(PreLintFinding {
                code: "FLAT_VELOCITY".to_string(),
                severity: AuditSeverity::Warning,
                track_id: Some(track.id.clone()),
                message: format!(
                    "Track '{}' has {} events with near-zero velocity variation.",
                    track.id,
                    track_velocities.len()
                ),
                fix_hint: "Vary accent and ghost-note velocities across 0.45-0.95 (or use Clip.velocity_scale across sections).".to_string(),
            });
        }

        if let InstrumentConfig::Synth {
            waveform,
            pitch_env,
            unison,
            lfo,
            fm,
            filter_env,
            ..
        } = &track.instrument
        {
            if let Some(u) = unison
                && u.voices > 1
                && u.stereo_spread > 0.05
            {
                has_any_stereo_width = true;
            }

            let is_bare_osc = *waveform != Waveform::Noise
                && pitch_env.is_none()
                && unison.is_none()
                && lfo.is_none()
                && fm.is_none()
                && filter_env.is_none()
                && track.fx.is_empty();

            if is_bare_osc {
                findings.push(PreLintFinding {
                    code: "BARE_SYNTH_TIMBRE".to_string(),
                    severity: AuditSeverity::Warning,
                    track_id: Some(track.id.clone()),
                    message: format!(
                        "Track '{}' uses a bare {:?} oscillator with no modulation or FX.",
                        track.id, waveform
                    ),
                    fix_hint: "Add unison (with stereo_spread), fm, filter_env, lfo, or track fx (LowPass/Delay/Reverb/Saturate).".to_string(),
                });
            }

            if has_low_note && pitch_env.is_none() && !has_highpass_or_sidechain(&track.fx) {
                low_end_tracks_without_carve += 1;
            }
        } else if has_low_note && !has_highpass_or_sidechain(&track.fx) {
            low_end_tracks_without_carve += 1;
        }

        if has_stereo_fx(&track.fx) {
            has_any_stereo_width = true;
        }
    }

    if song.tracks.is_empty() || total_resolved_events == 0 {
        findings.push(PreLintFinding {
            code: "EMPTY_SONG".to_string(),
            severity: AuditSeverity::Error,
            track_id: None,
            message: "Manifest contains no playable tracks or note events.".to_string(),
            fix_hint: "Scaffold a non-empty archetype with `auralis_init_manifest` or add tracks with patterns/clips.".to_string(),
        });
    }

    if total_resolved_events >= 32 && song.patterns.is_empty() {
        findings.push(PreLintFinding {
            code: "MISSING_PATTERN_REUSE".to_string(),
            severity: AuditSeverity::Warning,
            track_id: None,
            message: format!(
                "Manifest defines {} raw events without reusable Pattern/Clip scheduling.",
                total_resolved_events
            ),
            fix_hint: "Extract repeating bars into `patterns` and arrange via `clips` with `repeat`, `transpose_semitones`, and `velocity_scale`.".to_string(),
        });
    }

    if song.tracks.len() >= 3 && !has_any_stereo_width {
        findings.push(PreLintFinding {
            code: "CENTER_STACKED_MIX".to_string(),
            severity: AuditSeverity::Warning,
            track_id: None,
            message: "Multi-track mix has zero stereo panning, unison spread, or spatial FX.".to_string(),
            fix_hint: "Add `Pan`, `StereoWidth`, `Delay`/`Reverb`, or `unison.stereo_spread` on non-sub tracks.".to_string(),
        });
    }

    if low_end_tracks_without_carve >= 2 {
        findings.push(PreLintFinding {
            code: "UNCARVED_LOW_END".to_string(),
            severity: AuditSeverity::Warning,
            track_id: None,
            message: format!(
                "{} tracks occupy sub/low-mid notes (< MIDI 55) without HighPass or Sidechain separation.",
                low_end_tracks_without_carve
            ),
            fix_hint: "Add `HighPass` (130-220 Hz) on chord/pad tracks or `Sidechain` ducked from the kick track.".to_string(),
        });
    }

    if song.tracks.len() >= 3
        && !song
            .master_fx
            .iter()
            .any(|fx| matches!(fx, FxConfig::SoftClip | FxConfig::Compressor { .. }))
    {
        findings.push(PreLintFinding {
            code: "MISSING_MASTER_DYNAMICS".to_string(),
            severity: AuditSeverity::Warning,
            track_id: None,
            message: "Multi-track mix has no SoftClip or Compressor on `master_fx`.".to_string(),
            fix_hint: "Add `{\"type\": \"softclip\"}` or `Compressor` in `master_fx` to prevent summed peak clipping.".to_string(),
        });
    }

    if let Some(lp) = song.loop_points {
        if let Some(end_beat) = lp.end_beat {
            if lp.start_beat >= end_beat {
                findings.push(PreLintFinding {
                    code: "INVALID_LOOP_RANGE".to_string(),
                    severity: AuditSeverity::Error,
                    track_id: None,
                    message: format!(
                        "loop_points start_beat ({}) must be < end_beat ({}).",
                        lp.start_beat, end_beat
                    ),
                    fix_hint: "Set loop_points.end_beat strictly greater than start_beat aligned to bar boundaries.".to_string(),
                });
            }
        } else {
            findings.push(PreLintFinding {
                code: "UNBOUNDED_LOOP_END".to_string(),
                severity: AuditSeverity::Info,
                track_id: None,
                message:
                    "loop_points.end_beat is omitted; loop end will rely on inferred tail length."
                        .to_string(),
                fix_hint: "Explicitly set loop_points.end_beat to your target bar boundary."
                    .to_string(),
            });
        }
    }

    let mut penalty = 0i32;
    let mut has_errors = false;
    for f in &findings {
        match f.severity {
            AuditSeverity::Error => {
                has_errors = true;
                penalty += 25;
            }
            AuditSeverity::Warning => penalty += 12,
            AuditSeverity::Info => penalty += 4,
        }
    }

    let quality_score = (100 - penalty).clamp(0, 100) as u8;
    let tempo = if song.tempo > 0.0 { song.tempo } else { 120.0 };
    let estimated_duration_sec = max_end_beat * 60.0 / tempo;

    ManifestAudit {
        quality_score,
        passed: !has_errors && quality_score >= 70,
        track_count: song.tracks.len(),
        pattern_count: song.patterns.len(),
        resolved_event_count: total_resolved_events,
        estimated_duration_sec,
        velocity_std_dev: std_dev(&all_velocities),
        findings,
    }
}

/// Builds a non-generic, pre-intelligence-passing starter `Song` for a chosen archetype.
pub fn build_archetype_song(archetype: Archetype, tempo_override: Option<f32>) -> Song {
    match archetype {
        Archetype::Synthwave => build_synthwave(tempo_override.unwrap_or(108.0)),
        Archetype::Ambient => build_ambient(tempo_override.unwrap_or(82.0)),
        Archetype::Drill => build_drill(tempo_override.unwrap_or(142.0)),
        Archetype::Orchestral => build_orchestral(tempo_override.unwrap_or(92.0)),
        Archetype::SfxImpact => build_sfx_impact(tempo_override.unwrap_or(120.0)),
    }
}

fn note(start_beats: f32, duration_beats: f32, midi_note: u8, velocity: f32) -> NoteEvent {
    NoteEvent {
        start_beats,
        duration_beats,
        midi_note,
        velocity,
        ..Default::default()
    }
}

fn chord(start_beats: f32, duration_beats: f32, notes: Vec<u8>, velocity: f32) -> NoteEvent {
    NoteEvent {
        start_beats,
        duration_beats,
        midi_note: notes[0],
        chord_notes: Some(notes),
        velocity,
        ..Default::default()
    }
}

fn build_synthwave(tempo: f32) -> Song {
    let mut patterns = BTreeMap::new();
    patterns.insert(
        "kick_4on4".to_string(),
        Pattern {
            length_beats: 4.0,
            events: vec![
                note(0.0, 0.5, 36, 0.94),
                note(1.0, 0.5, 36, 0.85),
                note(2.0, 0.5, 36, 0.90),
                note(3.0, 0.5, 36, 0.83),
            ],
        },
    );
    patterns.insert(
        "bass_drive".to_string(),
        Pattern {
            length_beats: 4.0,
            events: vec![
                note(0.0, 0.45, 33, 0.88),
                note(0.5, 0.45, 33, 0.68),
                note(1.0, 0.45, 33, 0.84),
                note(1.5, 0.45, 45, 0.72),
                note(2.0, 0.45, 33, 0.86),
                note(2.5, 0.45, 33, 0.66),
                note(3.0, 0.45, 36, 0.82),
                note(3.5, 0.45, 40, 0.76),
            ],
        },
    );
    patterns.insert(
        "pad_progression".to_string(),
        Pattern {
            length_beats: 8.0,
            events: vec![
                chord(0.0, 3.8, vec![57, 60, 64, 67], 0.62), // Am7
                chord(4.0, 3.8, vec![53, 57, 60, 64], 0.56), // Fmaj7
            ],
        },
    );
    patterns.insert(
        "arp_motif".to_string(),
        Pattern {
            length_beats: 4.0,
            events: vec![
                note(0.0, 0.45, 69, 0.78),
                note(0.5, 0.45, 72, 0.62),
                note(1.0, 0.45, 76, 0.82),
                note(1.5, 0.45, 72, 0.65),
                note(2.0, 0.45, 79, 0.86),
                note(2.5, 0.45, 76, 0.60),
                note(3.0, 0.45, 72, 0.74),
                note(3.5, 0.45, 74, 0.68),
            ],
        },
    );
    patterns.insert(
        "hat_offbeat".to_string(),
        Pattern {
            length_beats: 4.0,
            events: vec![
                note(0.5, 0.25, 78, 0.64),
                note(1.25, 0.2, 78, 0.40),
                note(1.5, 0.25, 78, 0.72),
                note(2.5, 0.25, 78, 0.62),
                note(3.25, 0.2, 78, 0.42),
                note(3.5, 0.25, 78, 0.76),
            ],
        },
    );

    Song {
        tempo,
        time_signature: (4, 4),
        sample_rate: Some(44100),
        loop_points: Some(LoopPoints {
            start_beat: 0.0,
            end_beat: Some(16.0),
        }),
        master_fx: vec![
            FxConfig::Compressor {
                threshold_db: -12.0,
                ratio: 3.0,
                attack_sec: 0.01,
                release_sec: 0.14,
                makeup_db: 1.5,
            },
            FxConfig::SoftClip,
        ],
        patterns,
        tracks: vec![
            Track {
                id: "kick".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Sine,
                    adsr: Adsr {
                        attack_sec: 0.002,
                        decay_sec: 0.16,
                        sustain_level: 0.0,
                        release_sec: 0.04,
                    },
                    pitch_env: Some(PitchEnvelope {
                        drop_semitones: 36.0,
                        decay_sec: 0.055,
                    }),
                    unison: None,
                    lfo: None,
                    fm: None,
                    filter_env: None,
                },
                fx: vec![FxConfig::Saturate { drive: 1.3 }],
                clips: vec![Clip {
                    pattern: "kick_4on4".to_string(),
                    start_beats: 0.0,
                    repeat: Some(4),
                    transpose_semitones: None,
                    velocity_scale: None,
                }],
                events: Vec::new(),
            },
            Track {
                id: "analog_bass".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Sawtooth,
                    adsr: Adsr {
                        attack_sec: 0.005,
                        decay_sec: 0.14,
                        sustain_level: 0.55,
                        release_sec: 0.08,
                    },
                    pitch_env: None,
                    unison: Some(UnisonConfig {
                        voices: 2,
                        detune_cents: 8.0,
                        stereo_spread: 0.2,
                    }),
                    lfo: None,
                    fm: None,
                    filter_env: Some(FilterEnvelope {
                        start_cutoff_hz: 1600.0,
                        end_cutoff_hz: 260.0,
                        decay_sec: 0.18,
                        q: 1.3,
                    }),
                },
                fx: vec![
                    FxConfig::LowPass {
                        cutoff_hz: 900.0,
                        q: Some(0.9),
                    },
                    FxConfig::Sidechain {
                        source_track: "kick".to_string(),
                        amount: 0.6,
                        threshold_db: -24.0,
                        attack_sec: 0.005,
                        release_sec: 0.14,
                    },
                    FxConfig::Gain { gain_db: -3.0 },
                ],
                clips: vec![
                    Clip {
                        pattern: "bass_drive".to_string(),
                        start_beats: 0.0,
                        repeat: Some(2),
                        transpose_semitones: None,
                        velocity_scale: None,
                    },
                    Clip {
                        pattern: "bass_drive".to_string(),
                        start_beats: 8.0,
                        repeat: Some(2),
                        transpose_semitones: Some(-4),
                        velocity_scale: Some(0.92),
                    },
                ],
                events: Vec::new(),
            },
            Track {
                id: "wide_pad".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Sawtooth,
                    adsr: Adsr {
                        attack_sec: 0.25,
                        decay_sec: 0.4,
                        sustain_level: 0.72,
                        release_sec: 0.5,
                    },
                    pitch_env: None,
                    unison: Some(UnisonConfig {
                        voices: 4,
                        detune_cents: 16.0,
                        stereo_spread: 0.85,
                    }),
                    lfo: Some(LfoConfig {
                        rate_hz: 4.2,
                        depth_semitones: 0.08,
                    }),
                    fm: None,
                    filter_env: None,
                },
                fx: vec![
                    FxConfig::HighPass {
                        cutoff_hz: 180.0,
                        q: None,
                    },
                    FxConfig::FilterSweep {
                        start_hz: 600.0,
                        end_hz: 3200.0,
                        start_beat: 0.0,
                        end_beat: 16.0,
                        q: 1.1,
                        mode: FilterSweepMode::LowPass,
                    },
                    FxConfig::Sidechain {
                        source_track: "kick".to_string(),
                        amount: 0.45,
                        threshold_db: -24.0,
                        attack_sec: 0.005,
                        release_sec: 0.16,
                    },
                    FxConfig::Reverb {
                        room_size: 0.6,
                        damping: 0.45,
                        mix: 0.25,
                    },
                    FxConfig::Gain { gain_db: -6.0 },
                ],
                clips: vec![Clip {
                    pattern: "pad_progression".to_string(),
                    start_beats: 0.0,
                    repeat: Some(2),
                    transpose_semitones: None,
                    velocity_scale: None,
                }],
                events: Vec::new(),
            },
            Track {
                id: "fm_arp".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Triangle,
                    adsr: Adsr {
                        attack_sec: 0.004,
                        decay_sec: 0.16,
                        sustain_level: 0.2,
                        release_sec: 0.12,
                    },
                    pitch_env: None,
                    unison: None,
                    lfo: None,
                    fm: Some(FmConfig {
                        mod_ratio: 2.0,
                        mod_index: 1.8,
                    }),
                    filter_env: Some(FilterEnvelope {
                        start_cutoff_hz: 4200.0,
                        end_cutoff_hz: 900.0,
                        decay_sec: 0.14,
                        q: 1.2,
                    }),
                },
                fx: vec![
                    FxConfig::HighPass {
                        cutoff_hz: 260.0,
                        q: None,
                    },
                    FxConfig::Pan { pan: 0.28 },
                    FxConfig::Delay {
                        delay_beats: 0.75,
                        feedback: 0.3,
                        mix: 0.22,
                    },
                    FxConfig::Gain { gain_db: -7.0 },
                ],
                clips: vec![
                    Clip {
                        pattern: "arp_motif".to_string(),
                        start_beats: 0.0,
                        repeat: Some(2),
                        transpose_semitones: None,
                        velocity_scale: Some(0.85),
                    },
                    Clip {
                        pattern: "arp_motif".to_string(),
                        start_beats: 8.0,
                        repeat: Some(2),
                        transpose_semitones: Some(-4),
                        velocity_scale: Some(1.0),
                    },
                ],
                events: Vec::new(),
            },
            Track {
                id: "hihat".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Noise,
                    adsr: Adsr {
                        attack_sec: 0.001,
                        decay_sec: 0.045,
                        sustain_level: 0.0,
                        release_sec: 0.02,
                    },
                    pitch_env: None,
                    unison: None,
                    lfo: None,
                    fm: None,
                    filter_env: None,
                },
                fx: vec![
                    FxConfig::HighPass {
                        cutoff_hz: 5200.0,
                        q: None,
                    },
                    FxConfig::Pan { pan: -0.25 },
                    FxConfig::Gain { gain_db: -8.0 },
                ],
                clips: vec![Clip {
                    pattern: "hat_offbeat".to_string(),
                    start_beats: 0.0,
                    repeat: Some(4),
                    transpose_semitones: None,
                    velocity_scale: None,
                }],
                events: Vec::new(),
            },
        ],
    }
}

fn build_ambient(tempo: f32) -> Song {
    let mut patterns = BTreeMap::new();
    patterns.insert(
        "drone_chords".to_string(),
        Pattern {
            length_beats: 8.0,
            events: vec![
                chord(0.0, 7.6, vec![50, 57, 62, 65], 0.58), // Dm9 voicings
                chord(4.0, 3.8, vec![48, 55, 60, 64], 0.48),
            ],
        },
    );
    patterns.insert(
        "glass_bell".to_string(),
        Pattern {
            length_beats: 8.0,
            events: vec![
                note(0.5, 1.5, 74, 0.68),
                note(2.5, 1.5, 77, 0.52),
                note(4.5, 1.5, 81, 0.74),
                note(6.5, 1.25, 79, 0.44),
            ],
        },
    );

    Song {
        tempo,
        time_signature: (4, 4),
        sample_rate: Some(44100),
        loop_points: Some(LoopPoints {
            start_beat: 0.0,
            end_beat: Some(16.0),
        }),
        master_fx: vec![FxConfig::SoftClip],
        patterns,
        tracks: vec![
            Track {
                id: "sub_swell".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Triangle,
                    adsr: Adsr {
                        attack_sec: 0.6,
                        decay_sec: 0.5,
                        sustain_level: 0.8,
                        release_sec: 0.8,
                    },
                    pitch_env: None,
                    unison: None,
                    lfo: None,
                    fm: None,
                    filter_env: Some(FilterEnvelope {
                        start_cutoff_hz: 320.0,
                        end_cutoff_hz: 120.0,
                        decay_sec: 2.0,
                        q: 0.8,
                    }),
                },
                fx: vec![FxConfig::Gain { gain_db: -6.0 }],
                clips: Vec::new(),
                events: vec![note(0.0, 7.8, 38, 0.72), note(8.0, 7.8, 36, 0.60)],
            },
            Track {
                id: "nebula_pad".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Sawtooth,
                    adsr: Adsr {
                        attack_sec: 0.8,
                        decay_sec: 0.6,
                        sustain_level: 0.75,
                        release_sec: 1.0,
                    },
                    pitch_env: None,
                    unison: Some(UnisonConfig {
                        voices: 5,
                        detune_cents: 18.0,
                        stereo_spread: 0.9,
                    }),
                    lfo: Some(LfoConfig {
                        rate_hz: 3.5,
                        depth_semitones: 0.1,
                    }),
                    fm: None,
                    filter_env: None,
                },
                fx: vec![
                    FxConfig::HighPass {
                        cutoff_hz: 160.0,
                        q: None,
                    },
                    FxConfig::FilterSweep {
                        start_hz: 450.0,
                        end_hz: 2400.0,
                        start_beat: 0.0,
                        end_beat: 16.0,
                        q: 1.0,
                        mode: FilterSweepMode::LowPass,
                    },
                    FxConfig::Reverb {
                        room_size: 0.8,
                        damping: 0.35,
                        mix: 0.42,
                    },
                    FxConfig::Gain { gain_db: -6.0 },
                ],
                clips: vec![Clip {
                    pattern: "drone_chords".to_string(),
                    start_beats: 0.0,
                    repeat: Some(2),
                    transpose_semitones: None,
                    velocity_scale: None,
                }],
                events: Vec::new(),
            },
            Track {
                id: "bell_chime".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Sine,
                    adsr: Adsr {
                        attack_sec: 0.01,
                        decay_sec: 0.8,
                        sustain_level: 0.1,
                        release_sec: 0.9,
                    },
                    pitch_env: None,
                    unison: None,
                    lfo: None,
                    fm: Some(FmConfig {
                        mod_ratio: 3.5,
                        mod_index: 2.2,
                    }),
                    filter_env: None,
                },
                fx: vec![
                    FxConfig::Pan { pan: -0.35 },
                    FxConfig::Delay {
                        delay_beats: 1.5,
                        feedback: 0.4,
                        mix: 0.3,
                    },
                    FxConfig::Reverb {
                        room_size: 0.75,
                        damping: 0.4,
                        mix: 0.35,
                    },
                    FxConfig::Gain { gain_db: -7.0 },
                ],
                clips: vec![
                    Clip {
                        pattern: "glass_bell".to_string(),
                        start_beats: 0.0,
                        repeat: Some(1),
                        transpose_semitones: None,
                        velocity_scale: None,
                    },
                    Clip {
                        pattern: "glass_bell".to_string(),
                        start_beats: 8.0,
                        repeat: Some(1),
                        transpose_semitones: Some(-2),
                        velocity_scale: Some(0.85),
                    },
                ],
                events: Vec::new(),
            },
        ],
    }
}

fn build_drill(tempo: f32) -> Song {
    let mut patterns = BTreeMap::new();
    patterns.insert(
        "drill_hats".to_string(),
        Pattern {
            length_beats: 4.0,
            events: vec![
                note(0.0, 0.2, 80, 0.75),
                note(0.75, 0.2, 80, 0.42),
                note(1.5, 0.2, 80, 0.72),
                note(2.25, 0.15, 80, 0.38),
                note(2.5, 0.15, 80, 0.56),
                note(3.0, 0.2, 80, 0.78),
                note(3.66, 0.15, 80, 0.45),
            ],
        },
    );
    patterns.insert(
        "drill_808".to_string(),
        Pattern {
            length_beats: 8.0,
            events: vec![
                NoteEvent {
                    start_beats: 0.0,
                    duration_beats: 1.4,
                    midi_note: 38, // D2
                    velocity: 0.92,
                    ..Default::default()
                },
                NoteEvent {
                    start_beats: 3.5,
                    duration_beats: 1.0,
                    midi_note: 38,
                    velocity: 0.78,
                    slide_to_midi: Some(50),
                    glide_beats: Some(0.35),
                    ..Default::default()
                },
                NoteEvent {
                    start_beats: 6.0,
                    duration_beats: 1.2,
                    midi_note: 39, // Eb2 Phrygian
                    velocity: 0.86,
                    ..Default::default()
                },
            ],
        },
    );

    Song {
        tempo,
        time_signature: (4, 4),
        sample_rate: Some(44100),
        loop_points: Some(LoopPoints {
            start_beat: 0.0,
            end_beat: Some(8.0),
        }),
        master_fx: vec![FxConfig::SoftClip],
        patterns,
        tracks: vec![
            Track {
                id: "kick".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Sine,
                    adsr: Adsr {
                        attack_sec: 0.002,
                        decay_sec: 0.14,
                        sustain_level: 0.0,
                        release_sec: 0.03,
                    },
                    pitch_env: Some(PitchEnvelope {
                        drop_semitones: 34.0,
                        decay_sec: 0.045,
                    }),
                    unison: None,
                    lfo: None,
                    fm: None,
                    filter_env: None,
                },
                fx: vec![FxConfig::Saturate { drive: 1.4 }],
                clips: Vec::new(),
                events: vec![
                    note(0.0, 0.5, 36, 0.95),
                    note(1.5, 0.5, 36, 0.82),
                    note(4.0, 0.5, 36, 0.92),
                    note(6.5, 0.5, 36, 0.76),
                ],
            },
            Track {
                id: "glide_808".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Sine,
                    adsr: Adsr {
                        attack_sec: 0.005,
                        decay_sec: 0.25,
                        sustain_level: 0.7,
                        release_sec: 0.15,
                    },
                    pitch_env: None,
                    unison: None,
                    lfo: None,
                    fm: Some(FmConfig {
                        mod_ratio: 1.0,
                        mod_index: 0.8,
                    }),
                    filter_env: None,
                },
                fx: vec![
                    FxConfig::Saturate { drive: 1.5 },
                    FxConfig::Sidechain {
                        source_track: "kick".to_string(),
                        amount: 0.7,
                        threshold_db: -24.0,
                        attack_sec: 0.004,
                        release_sec: 0.12,
                    },
                    FxConfig::Gain { gain_db: -3.0 },
                ],
                clips: vec![Clip {
                    pattern: "drill_808".to_string(),
                    start_beats: 0.0,
                    repeat: Some(1),
                    transpose_semitones: None,
                    velocity_scale: None,
                }],
                events: Vec::new(),
            },
            Track {
                id: "dark_bell".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Triangle,
                    adsr: Adsr {
                        attack_sec: 0.005,
                        decay_sec: 0.35,
                        sustain_level: 0.15,
                        release_sec: 0.25,
                    },
                    pitch_env: None,
                    unison: Some(UnisonConfig {
                        voices: 3,
                        detune_cents: 12.0,
                        stereo_spread: 0.65,
                    }),
                    lfo: None,
                    fm: Some(FmConfig {
                        mod_ratio: 2.0,
                        mod_index: 2.0,
                    }),
                    filter_env: None,
                },
                fx: vec![
                    FxConfig::HighPass {
                        cutoff_hz: 240.0,
                        q: None,
                    },
                    FxConfig::Pan { pan: 0.25 },
                    FxConfig::Reverb {
                        room_size: 0.55,
                        damping: 0.45,
                        mix: 0.28,
                    },
                    FxConfig::Gain { gain_db: -7.0 },
                ],
                clips: Vec::new(),
                events: vec![
                    note(0.0, 1.5, 62, 0.75),
                    note(2.0, 1.5, 63, 0.60),
                    note(4.0, 1.5, 69, 0.80),
                    note(6.0, 1.5, 65, 0.56),
                ],
            },
            Track {
                id: "counter_hats".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Noise,
                    adsr: Adsr {
                        attack_sec: 0.001,
                        decay_sec: 0.04,
                        sustain_level: 0.0,
                        release_sec: 0.015,
                    },
                    pitch_env: None,
                    unison: None,
                    lfo: None,
                    fm: None,
                    filter_env: None,
                },
                fx: vec![
                    FxConfig::HighPass {
                        cutoff_hz: 5500.0,
                        q: None,
                    },
                    FxConfig::Pan { pan: -0.3 },
                    FxConfig::Gain { gain_db: -8.0 },
                ],
                clips: vec![Clip {
                    pattern: "drill_hats".to_string(),
                    start_beats: 0.0,
                    repeat: Some(2),
                    transpose_semitones: None,
                    velocity_scale: None,
                }],
                events: Vec::new(),
            },
        ],
    }
}

fn build_orchestral(tempo: f32) -> Song {
    let mut patterns = BTreeMap::new();
    patterns.insert(
        "piano_ostinato".to_string(),
        Pattern {
            length_beats: 4.0,
            events: vec![
                NoteEvent {
                    start_beats: 0.0,
                    duration_beats: 1.9,
                    midi_note: 60,
                    chord_notes: Some(vec![60, 63, 67]),
                    velocity: 0.72,
                    sustain: Some(true),
                    ..Default::default()
                },
                note(2.0, 0.9, 67, 0.58),
                note(3.0, 0.9, 70, 0.66),
            ],
        },
    );

    let sf2_path = "assets/soundfonts/TimGM6mb.sf2".to_string();

    Song {
        tempo,
        time_signature: (4, 4),
        sample_rate: Some(44100),
        loop_points: Some(LoopPoints {
            start_beat: 0.0,
            end_beat: Some(8.0),
        }),
        master_fx: vec![FxConfig::SoftClip],
        patterns,
        tracks: vec![
            Track {
                id: "piano".to_string(),
                instrument: InstrumentConfig::SoundFont {
                    soundfont_path: sf2_path.clone(),
                    bank: Some(0),
                    preset: Some(0),
                },
                fx: vec![
                    FxConfig::HighPass {
                        cutoff_hz: 120.0,
                        q: None,
                    },
                    FxConfig::Pan { pan: -0.2 },
                    FxConfig::Reverb {
                        room_size: 0.55,
                        damping: 0.5,
                        mix: 0.22,
                    },
                ],
                clips: vec![
                    Clip {
                        pattern: "piano_ostinato".to_string(),
                        start_beats: 0.0,
                        repeat: Some(1),
                        transpose_semitones: None,
                        velocity_scale: None,
                    },
                    Clip {
                        pattern: "piano_ostinato".to_string(),
                        start_beats: 4.0,
                        repeat: Some(1),
                        transpose_semitones: Some(-2),
                        velocity_scale: Some(0.9),
                    },
                ],
                events: Vec::new(),
            },
            Track {
                id: "strings".to_string(),
                instrument: InstrumentConfig::SoundFont {
                    soundfont_path: sf2_path.clone(),
                    bank: Some(0),
                    preset: Some(48),
                },
                fx: vec![
                    FxConfig::HighPass {
                        cutoff_hz: 140.0,
                        q: None,
                    },
                    FxConfig::StereoWidth { width: 1.3 },
                    FxConfig::Reverb {
                        room_size: 0.65,
                        damping: 0.45,
                        mix: 0.3,
                    },
                    FxConfig::Gain { gain_db: -4.0 },
                ],
                clips: Vec::new(),
                events: vec![
                    chord(0.0, 3.8, vec![55, 60, 63], 0.62),
                    chord(4.0, 3.8, vec![53, 58, 62], 0.54),
                ],
            },
            Track {
                id: "contrabass".to_string(),
                instrument: InstrumentConfig::SoundFont {
                    soundfont_path: sf2_path,
                    bank: Some(0),
                    preset: Some(43),
                },
                fx: vec![FxConfig::Gain { gain_db: -2.0 }],
                clips: Vec::new(),
                events: vec![note(0.0, 3.8, 36, 0.80), note(4.0, 3.8, 34, 0.72)],
            },
        ],
    }
}

fn build_sfx_impact(tempo: f32) -> Song {
    Song {
        tempo,
        time_signature: (4, 4),
        sample_rate: Some(44100),
        loop_points: None,
        master_fx: vec![FxConfig::SoftClip],
        patterns: BTreeMap::new(),
        tracks: vec![
            Track {
                id: "transient_crack".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Noise,
                    adsr: Adsr {
                        attack_sec: 0.001,
                        decay_sec: 0.035,
                        sustain_level: 0.0,
                        release_sec: 0.02,
                    },
                    pitch_env: None,
                    unison: None,
                    lfo: None,
                    fm: None,
                    filter_env: None,
                },
                fx: vec![
                    FxConfig::HighPass {
                        cutoff_hz: 1400.0,
                        q: None,
                    },
                    FxConfig::Pan { pan: 0.1 },
                ],
                clips: Vec::new(),
                events: vec![note(0.0, 0.15, 72, 0.92)],
            },
            Track {
                id: "sub_thump".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Sine,
                    adsr: Adsr {
                        attack_sec: 0.002,
                        decay_sec: 0.25,
                        sustain_level: 0.0,
                        release_sec: 0.08,
                    },
                    pitch_env: Some(PitchEnvelope {
                        drop_semitones: 42.0,
                        decay_sec: 0.08,
                    }),
                    unison: None,
                    lfo: None,
                    fm: None,
                    filter_env: None,
                },
                fx: vec![FxConfig::Saturate { drive: 1.7 }],
                clips: Vec::new(),
                events: vec![note(0.0, 0.6, 40, 0.95)],
            },
            Track {
                id: "air_tail".to_string(),
                instrument: InstrumentConfig::Synth {
                    waveform: Waveform::Noise,
                    adsr: Adsr {
                        attack_sec: 0.01,
                        decay_sec: 0.55,
                        sustain_level: 0.0,
                        release_sec: 0.25,
                    },
                    pitch_env: None,
                    unison: None,
                    lfo: None,
                    fm: None,
                    filter_env: None,
                },
                fx: vec![
                    FxConfig::FilterSweep {
                        start_hz: 4200.0,
                        end_hz: 280.0,
                        start_beat: 0.0,
                        end_beat: 1.5,
                        q: 1.2,
                        mode: FilterSweepMode::LowPass,
                    },
                    FxConfig::Reverb {
                        room_size: 0.6,
                        damping: 0.5,
                        mix: 0.35,
                    },
                    FxConfig::Gain { gain_db: -6.0 },
                ],
                clips: Vec::new(),
                events: vec![note(0.0, 1.5, 60, 0.72)],
            },
        ],
    }
}
