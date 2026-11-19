use crate::model::{Adsr, Waveform};
use std::f32::consts::PI;

pub fn generate_sample(waveform: Waveform, phase: f32) -> f32 {
    let normalized = phase % (2.0 * PI);
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
    }
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
