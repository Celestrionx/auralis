pub mod graph;
pub mod mcp;
pub mod model;
pub mod soundfont;
pub mod synth;
pub mod telemetry;

pub use graph::{RenderedAudio, compile_song, export_wav};
pub use model::{
    Adsr, FxConfig, InstrumentConfig, NoteEvent, PitchEnvelope, Song, Track, Waveform,
};
pub use telemetry::{AudioTelemetry, SpectralEnergy, analyze_audio};
