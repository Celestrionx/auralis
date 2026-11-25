use crate::graph::{compile_song, export_audio, read_wav_loop_points};
use crate::model::Song;
use crate::soundfont::get_or_load_soundfont;
use crate::telemetry::analyze_audio;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

fn mcp_error(message: impl Into<String>) -> Value {
    json!({
        "isError": true,
        "content": [{ "type": "text", "text": message.into() }]
    })
}

fn mcp_text(text: impl Into<String>) -> Value {
    json!({
        "content": [{ "type": "text", "text": text.into() }]
    })
}

fn eval_wav_to_json(wav_path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut reader = hound::WavReader::open(wav_path)?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = ((1_i64 << (spec.bits_per_sample.saturating_sub(1).max(1))) - 1) as f32;
            reader
                .samples::<i32>()
                .filter_map(Result::ok)
                .map(|s| s as f32 / max_val)
                .collect()
        }
        hound::SampleFormat::Float => reader.samples::<f32>().filter_map(Result::ok).collect(),
    };
    let loop_samples = read_wav_loop_points(wav_path);
    let telemetry = analyze_audio(&samples, spec.sample_rate, spec.channels, loop_samples);
    Ok(serde_json::to_string_pretty(&telemetry)?)
}

fn inspect_soundfont_to_json(
    soundfont_path: &str,
    bank_filter: Option<i32>,
) -> Result<String, Box<dyn std::error::Error>> {
    let sf = get_or_load_soundfont(soundfont_path)?;
    let mut presets: Vec<(i32, i32, &str)> = sf
        .get_presets()
        .iter()
        .map(|p| (p.get_bank_number(), p.get_patch_number(), p.get_name()))
        .filter(|(b, _, _)| bank_filter.is_none_or(|target| *b == target))
        .collect();
    presets.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let entries: Vec<Value> = presets
        .into_iter()
        .map(|(bank, preset, name)| json!({ "bank": bank, "preset": preset, "name": name }))
        .collect();

    Ok(serde_json::to_string_pretty(&json!({
        "soundfont_path": soundfont_path,
        "preset_count": entries.len(),
        "presets": entries
    }))?)
}

pub fn handle_mcp_request(request: &Value) -> Value {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

    match method {
        "initialize" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2024-11-05",
                "serverInfo": {
                    "name": "auralis",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "tools": {}
                }
            }
        }),
        "tools/list" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "tools": [
                    {
                        "name": "auralis_init_manifest",
                        "description": "Scaffold a baseline declarative song manifest in JSON",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "tempo": { "type": "number", "default": 120.0 }
                            }
                        }
                    },
                    {
                        "name": "auralis_compile_audio",
                        "description": "Compile a declarative song manifest into a WAV file and return telemetry",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "manifest_json": { "type": "string" },
                                "output_wav_path": { "type": "string" }
                            },
                            "required": ["manifest_json", "output_wav_path"]
                        }
                    },
                    {
                        "name": "auralis_inspect_soundfont",
                        "description": "List available SoundFont 2 (SF2) banks, presets, and instrument names",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "soundfont_path": { "type": "string" },
                                "bank": { "type": "integer" }
                            },
                            "required": ["soundfont_path"]
                        }
                    },
                    {
                        "name": "auralis_eval_wav",
                        "description": "Evaluate an existing WAV file and return quantitative auditory telemetry",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "wav_path": { "type": "string" }
                            },
                            "required": ["wav_path"]
                        }
                    }
                ]
            }
        }),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or(Value::Null);
            let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

            let tool_result = match tool_name {
                "auralis_init_manifest" => {
                    let tempo = arguments
                        .get("tempo")
                        .and_then(|t| t.as_f64())
                        .unwrap_or(120.0) as f32;
                    let song = Song {
                        tempo,
                        ..Song::default()
                    };
                    mcp_text(serde_json::to_string_pretty(&song).unwrap_or_default())
                }
                "auralis_compile_audio" => {
                    let manifest_str = arguments
                        .get("manifest_json")
                        .and_then(|s| s.as_str())
                        .unwrap_or("");
                    let wav_path = arguments
                        .get("output_wav_path")
                        .and_then(|s| s.as_str())
                        .unwrap_or("output.wav");

                    match serde_json::from_str::<Song>(manifest_str) {
                        Ok(song) => {
                            let rendered = compile_song(&song);
                            let telemetry = analyze_audio(
                                &rendered.samples,
                                rendered.sample_rate,
                                rendered.channels,
                                rendered.loop_samples,
                            );
                            match export_audio(&rendered, wav_path) {
                                Ok(_) => mcp_text(format!(
                                    "Audio successfully written to {}.\nTelemetry:\n{}",
                                    wav_path,
                                    serde_json::to_string_pretty(&telemetry).unwrap_or_default()
                                )),
                                Err(e) => mcp_error(format!("Audio export error: {}", e)),
                            }
                        }
                        Err(e) => mcp_error(format!("Manifest parse error: {}", e)),
                    }
                }
                "auralis_inspect_soundfont" => {
                    let sf_path = arguments
                        .get("soundfont_path")
                        .and_then(|s| s.as_str())
                        .unwrap_or("");
                    let bank_filter = arguments
                        .get("bank")
                        .and_then(|b| b.as_i64())
                        .map(|b| b as i32);
                    if sf_path.is_empty() {
                        mcp_error("Missing required argument: soundfont_path")
                    } else {
                        match inspect_soundfont_to_json(sf_path, bank_filter) {
                            Ok(json_out) => mcp_text(json_out),
                            Err(e) => mcp_error(format!("SoundFont inspection error: {}", e)),
                        }
                    }
                }
                "auralis_eval_wav" => {
                    let wav_path = arguments
                        .get("wav_path")
                        .and_then(|s| s.as_str())
                        .unwrap_or("");
                    if wav_path.is_empty() {
                        mcp_error("Missing required argument: wav_path")
                    } else {
                        match eval_wav_to_json(wav_path) {
                            Ok(json_out) => mcp_text(json_out),
                            Err(e) => mcp_error(format!("WAV evaluation error: {}", e)),
                        }
                    }
                }
                _ => mcp_error(format!("Unknown tool: {}", tool_name)),
            };

            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": tool_result
            })
        }
        _ => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32601,
                "message": "Method not found"
            }
        }),
    }
}

pub fn run_stdio_mcp_server() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let response = handle_mcp_request(&request);
        let response_str = serde_json::to_string(&response)?;
        writeln!(stdout, "{}", response_str)?;
        stdout.flush()?;
    }

    Ok(())
}
