use crate::model::{Adsr, FmConfig, Waveform};
use std::f32::consts::PI;

pub struct DeterministicNoise {
    state: u32,
}

impl DeterministicNoise {
    pub fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 0x5a17b349 } else { seed },
        }
    }

    pub fn next_sample(&mut self) -> f32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        ((x as f32) / (u32::MAX as f32)) * 2.0 - 1.0
    }
}

pub fn generate_sample(waveform: Waveform, phase: f32, noise: &mut DeterministicNoise) -> f32 {
    let normalized = phase.rem_euclid(2.0 * PI);
    match waveform {
        Waveform::Sine => normalized.sin(),
        Waveform::Square => {
            if normalized < PI {
                1.0
            } else {
                -1.0
            }
        }
        Waveform::Sawtooth => 1.0 - (normalized / PI),
        Waveform::Triangle => {
            let normalized_0_to_1 = normalized / (2.0 * PI);
            4.0 * (normalized_0_to_1 - 0.5).abs() - 1.0
        }
        Waveform::Noise => noise.next_sample(),
    }
}

pub fn generate_modulated_sample(
    waveform: Waveform,
    carrier_phase: f32,
    mod_phase: f32,
    fm: Option<&FmConfig>,
    noise: &mut DeterministicNoise,
) -> f32 {
    let effective_phase = match fm {
        Some(cfg) => carrier_phase + cfg.mod_index * mod_phase.sin(),
        None => carrier_phase,
    };
    generate_sample(waveform, effective_phase, noise)
}

pub fn calculate_envelope(time_sec: f32, duration_sec: f32, adsr: &Adsr) -> f32 {
    let total_duration = duration_sec + adsr.release_sec;
    if time_sec < 0.0 || time_sec > total_duration {
        return 0.0;
    }

    if time_sec <= adsr.attack_sec {
        if adsr.attack_sec <= 0.0001 {
            1.0
        } else {
            time_sec / adsr.attack_sec
        }
    } else if time_sec <= adsr.attack_sec + adsr.decay_sec {
        let decay_t = time_sec - adsr.attack_sec;
        if adsr.decay_sec <= 0.0001 {
            adsr.sustain_level
        } else {
            1.0 - (1.0 - adsr.sustain_level) * (decay_t / adsr.decay_sec)
        }
    } else if time_sec <= duration_sec {
        adsr.sustain_level
    } else {
        let release_t = time_sec - duration_sec;
        if adsr.release_sec <= 0.0001 {
            0.0
        } else {
            adsr.sustain_level * (1.0 - (release_t / adsr.release_sec).clamp(0.0, 1.0))
        }
    }
}
