use auralis::graph::{compile_song, compile_song_with_stems, export_audio, read_wav_loop_points};
use auralis::mcp::run_stdio_mcp_server;
use auralis::model::Song;
use auralis::prelint::{analyze_manifest, build_archetype_song, parse_archetype};
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
    /// Scaffold a non-generic starter song manifest from an archetype
    Init {
        /// Target JSON manifest path
        path: PathBuf,
        /// Starter archetype (synthwave, ambient, drill, orchestral, sfx_impact)
        #[arg(short, long, default_value = "synthwave")]
        archetype: String,
        /// Optional tempo override in BPM
        #[arg(short, long)]
        tempo: Option<f32>,
    },
    /// Run deterministic pre-render intelligence checks on a manifest before compiling
    Lint {
        /// Input manifest JSON path
        manifest: PathBuf,
        /// Exit with non-zero status if pre-render quality gate fails
        #[arg(long)]
        strict: bool,
    },
    /// Compile a declarative song manifest into audio and telemetry
    Compile {
        /// Input manifest JSON path
        manifest: PathBuf,
        /// Output audio path (.wav or .ogg)
        #[arg(short, long, default_value = "output.wav")]
        output: PathBuf,
        /// Also export individual equal-length per-track stems alongside output
        #[arg(long)]
        stems: bool,
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
        Commands::Init {
            path,
            archetype,
            tempo,
        } => {
            let parsed = parse_archetype(Some(&archetype)).map_err(std::io::Error::other)?;
            let song = build_archetype_song(parsed, tempo);
            let json_str = serde_json::to_string_pretty(&song)?;
            fs::write(&path, json_str)?;
            println!(
                "Scaffolded '{}' manifest at {}",
                parsed.as_str(),
                path.display()
            );
        }
        Commands::Lint { manifest, strict } => {
            let content = fs::read_to_string(&manifest)?;
            let song: Song = serde_json::from_str(&content)?;
            let audit = analyze_manifest(&song);
            println!("{}", serde_json::to_string_pretty(&audit)?);
            if strict && !audit.passed {
                return Err(format!(
                    "Pre-render intelligence gate failed (score={}/100, findings={})",
                    audit.quality_score,
                    audit.findings.len()
                )
                .into());
            }
        }
        Commands::Compile {
            manifest,
            output,
            stems,
        } => {
            let content = fs::read_to_string(&manifest)?;
            let song: Song = serde_json::from_str(&content)?;
            let pre_audit = analyze_manifest(&song);
            let (rendered, stem_buffers) = if stems {
                compile_song_with_stems(&song)
            } else {
                (compile_song(&song), Vec::new())
            };
            export_audio(&rendered, &output)?;
            if stems {
                let parent = output
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or_else(|| std::path::Path::new("."));
                let base = output
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("output");
                let ext = output.extension().and_then(|e| e.to_str()).unwrap_or("wav");
                for (track_id, stem_audio) in &stem_buffers {
                    let safe_id: String = track_id
                        .chars()
                        .map(|c| {
                            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                                c
                            } else {
                                '_'
                            }
                        })
                        .collect();
                    let stem_path = parent.join(format!("{}_{}.{}", base, safe_id, ext));
                    export_audio(stem_audio, &stem_path)?;
                    println!("Exported stem to {}", stem_path.display());
                }
            }
            let telemetry = analyze_audio(
                &rendered.samples,
                rendered.sample_rate,
                rendered.channels,
                rendered.loop_samples,
            );
            println!("Compiled audio to {}", output.display());
            println!(
                "Pre-Lint Score: {}/100 (passed={}, findings={})",
                pre_audit.quality_score,
                pre_audit.passed,
                pre_audit.findings.len()
            );
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
            let loop_samples = read_wav_loop_points(&path);
            let telemetry = analyze_audio(&samples, spec.sample_rate, spec.channels, loop_samples);
            println!("{}", serde_json::to_string_pretty(&telemetry)?);
        }
        Commands::Serve => {
            run_stdio_mcp_server()?;
        }
    }

    Ok(())
}
