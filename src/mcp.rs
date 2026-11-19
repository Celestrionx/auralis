use crate::graph::{compile_song, export_wav};
use crate::model::Song;
use crate::telemetry::analyze_audio;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

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

        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

        let response = match method {
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
                        json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": serde_json::to_string_pretty(&song).unwrap_or_default()
                                }
                            ]
                        })
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
                                );
                                match export_wav(&rendered, wav_path) {
                                    Ok(_) => json!({
                                        "content": [
                                            {
                                                "type": "text",
                                                "text": format!("Audio successfully written to {}.\nTelemetry:\n{}", wav_path, serde_json::to_string_pretty(&telemetry).unwrap_or_default())
                                            }
                                        ]
                                    }),
                                    Err(e) => json!({
                                        "isError": true,
                                        "content": [{ "type": "text", "text": format!("WAV export error: {}", e) }]
                                    }),
                                }
                            }
                            Err(e) => json!({
                                "isError": true,
                                "content": [{ "type": "text", "text": format!("Manifest parse error: {}", e) }]
                            }),
                        }
                    }
                    _ => json!({
                        "isError": true,
                        "content": [{ "type": "text", "text": format!("Unknown tool: {}", tool_name) }]
                    }),
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
        };

        let response_str = serde_json::to_string(&response)?;
        writeln!(stdout, "{}", response_str)?;
        stdout.flush()?;
    }

    Ok(())
}
