pub mod graph;
pub mod mcp;
pub mod model;
pub mod prelint;
pub mod sampler;
pub mod soundfont;
pub mod synth;
pub mod telemetry;

pub use graph::{
    RenderedAudio, compile_song, compile_song_with_stems, compile_stems, export_audio, export_ogg,
    export_wav, read_wav_loop_points,
};
pub use model::{
    Adsr, Clip, FilterEnvelope, FilterSweepMode, FmConfig, FxConfig, InstrumentConfig, LfoConfig,
    LoopPoints, NoteEvent, Pattern, PitchEnvelope, Song, Track, UnisonConfig, Waveform,
};
pub use prelint::{
    Archetype, AuditSeverity, ManifestAudit, PreLintFinding, analyze_manifest,
    build_archetype_song, parse_archetype,
};
pub use telemetry::{
    AudioTelemetry, LoopTelemetry, SectionTelemetry, SpectralEnergy, analyze_audio,
};
