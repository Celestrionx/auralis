pub mod graph;
pub mod mcp;
pub mod model;
pub mod soundfont;
pub mod synth;
pub mod telemetry;

pub use graph::{RenderedAudio, compile_song, export_wav, read_wav_loop_points};
pub use model::{
    Adsr, FxConfig, InstrumentConfig, LoopPoints, NoteEvent, PitchEnvelope, Song, Track, Waveform,
};
pub use telemetry::{AudioTelemetry, LoopTelemetry, SpectralEnergy, analyze_audio};
