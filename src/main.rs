use auralis::graph::{compile_song, export_wav};
use auralis::mcp::run_stdio_mcp_server;
use auralis::model::{Adsr, InstrumentConfig, NoteEvent, PitchEnvelope, Song, Track, Waveform};
use auralis::telemetry::analyze_audio;
use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "auralis")]
#[command(about = "Headless Audio Graph Compiler & Auditory Telemetry Engine", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scaffold a starter declarative song manifest
    Init {
        /// Target JSON manifest path
        path: PathBuf,
    },
    /// Compile a declarative song manifest into audio and telemetry
    Compile {
        /// Input manifest JSON path
        manifest: PathBuf,
        /// Output WAV path
        #[arg(short, long, default_value = "output.wav")]
        output: PathBuf,
    },
    /// Evaluate an existing WAV file and output auditory telemetry
    Eval {
        /// Input WAV path
        path: PathBuf,
    },
    /// Run the Model Context Protocol (MCP) server over stdio
    Serve,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { path } => {
            let song = Song {
                tempo: 120.0,
                time_signature: (4, 4),
                sample_rate: Some(44100),
                tracks: vec![
                    Track {
                        id: "kick_drum".to_string(),
                        instrument: InstrumentConfig {
                            waveform: Waveform::Sine,
                            adsr: Adsr {
                                attack_sec: 0.002,
                                decay_sec: 0.15,
                                sustain_level: 0.0,
                                release_sec: 0.05,
                            },
                            pitch_env: Some(PitchEnvelope {
                                drop_semitones: 36.0,
                                decay_sec: 0.05,
                            }),
                        },
                        fx: vec![],
                        events: vec![
                            NoteEvent {
                                start_beats: 0.0,
                                duration_beats: 1.0,
                                midi_note: 36, // C2
                                chord_notes: None,
                                velocity: 0.9,
                            },
                            NoteEvent {
                                start_beats: 2.0,
                                duration_beats: 1.0,
                                midi_note: 36,
                                chord_notes: None,
                                velocity: 0.9,
                            },
                        ],
                    },
                    Track {
                        id: "chords_pad".to_string(),
                        instrument: InstrumentConfig {
                            waveform: Waveform::Triangle,
                            adsr: Adsr {
                                attack_sec: 0.2,
                                decay_sec: 0.3,
                                sustain_level: 0.7,
                                release_sec: 0.4,
                            },
                            pitch_env: None,
                        },
                        fx: vec![],
                        events: vec![
                            NoteEvent {
                                start_beats: 0.0,
                                duration_beats: 2.0,
                                midi_note: 60,
                                chord_notes: Some(vec![60, 64, 67]), // C major
                                velocity: 0.7,
                            },
                            NoteEvent {
                                start_beats: 2.0,
                                duration_beats: 2.0,
                                midi_note: 57,
                                chord_notes: Some(vec![57, 60, 64]), // A minor
                                velocity: 0.7,
                            },
                        ],
                    },
                ],
            };

            let json_str = serde_json::to_string_pretty(&song)?;
            fs::write(&path, json_str)?;
            println!("Scaffolded song manifest at {}", path.display());
        }
        Commands::Compile { manifest, output } => {
            let content = fs::read_to_string(&manifest)?;
            let song: Song = serde_json::from_str(&content)?;
            let rendered = compile_song(&song);
            export_wav(&rendered, &output)?;
            let telemetry =
                analyze_audio(&rendered.samples, rendered.sample_rate, rendered.channels);
            println!("Compiled audio to {}", output.display());
            println!(
                "Auditory Telemetry:\n{}",
                serde_json::to_string_pretty(&telemetry)?
            );
        }
        Commands::Eval { path } => {
            let mut reader = hound::WavReader::open(&path)?;
            let spec = reader.spec();
            let samples: Vec<f32> = reader
                .samples::<i16>()
                .filter_map(Result::ok)
                .map(|s| s as f32 / i16::MAX as f32)
                .collect();
            let telemetry = analyze_audio(&samples, spec.sample_rate, spec.channels);
            println!("{}", serde_json::to_string_pretty(&telemetry)?);
        }
        Commands::Serve => {
            run_stdio_mcp_server()?;
        }
    }

    Ok(())
}
