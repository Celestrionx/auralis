pub mod graph;
pub mod mcp;
pub mod model;
pub mod soundfont;
pub mod synth;
pub mod telemetry;

pub use graph::{
    RenderedAudio, compile_song, compile_song_with_stems, compile_stems, export_audio, export_ogg,
    export_wav, read_wav_loop_points,
};
pub use model::{
    Adsr, Clip, FmConfig, FxConfig, InstrumentConfig, LfoConfig, LoopPoints, NoteEvent, Pattern,
    PitchEnvelope, Song, Track, UnisonConfig, Waveform,
};
pub use telemetry::{
    AudioTelemetry, LoopTelemetry, SectionTelemetry, SpectralEnergy, analyze_audio,
};
