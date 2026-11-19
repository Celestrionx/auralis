use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioTelemetry {
    pub sample_count: usize,
    pub duration_seconds: f32,
    pub peak_amplitude: f32,
    pub peak_dbfs: f32,
    pub rms_amplitude: f32,
    pub rms_dbfs: f32,
    pub is_clipping: bool,
    pub spectral_bands: SpectralEnergy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpectralEnergy {
    pub sub_bass_ratio: f32,
    pub mid_ratio: f32,
    pub high_ratio: f32,
}

pub fn analyze_audio(samples: &[f32], sample_rate: u32) -> AudioTelemetry {
    if samples.is_empty() {
        return AudioTelemetry {
            sample_count: 0,
            duration_seconds: 0.0,
            peak_amplitude: 0.0,
            peak_dbfs: -96.0,
            rms_amplitude: 0.0,
            rms_dbfs: -96.0,
            is_clipping: false,
            spectral_bands: SpectralEnergy {
                sub_bass_ratio: 0.0,
                mid_ratio: 0.0,
                high_ratio: 0.0,
            },
        };
    }

    let mut peak = 0.0_f32;
    let mut sum_sq = 0.0_f32;

    for &s in samples {
        let abs = s.abs();
        if abs > peak {
            peak = abs;
        }
        sum_sq += s * s;
    }

    let rms = (sum_sq / samples.len() as f32).sqrt();
    let peak_dbfs = if peak > 1e-5 {
        20.0 * peak.log10()
    } else {
        -96.0
    };
    let rms_dbfs = if rms > 1e-5 {
        20.0 * rms.log10()
    } else {
        -96.0
    };

    let spectral_bands = calculate_spectral_bands(samples, sample_rate);

    AudioTelemetry {
        sample_count: samples.len(),
        duration_seconds: samples.len() as f32 / sample_rate as f32,
        peak_amplitude: peak,
        peak_dbfs,
        rms_amplitude: rms,
        rms_dbfs,
        is_clipping: peak >= 0.999,
        spectral_bands,
    }
}

fn calculate_spectral_bands(samples: &[f32], sample_rate: u32) -> SpectralEnergy {
    let fft_size = 2048.min(samples.len().next_power_of_two());
    if fft_size < 128 {
        return SpectralEnergy {
            sub_bass_ratio: 0.33,
            mid_ratio: 0.33,
            high_ratio: 0.34,
        };
    }

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_size);

    let mut buffer: Vec<Complex<f32>> = samples
        .iter()
        .take(fft_size)
        .map(|&s| Complex { re: s, im: 0.0 })
        .collect();

    while buffer.len() < fft_size {
        buffer.push(Complex { re: 0.0, im: 0.0 });
    }

    fft.process(&mut buffer);

    let bin_hz = sample_rate as f32 / fft_size as f32;
    let mut sub_bass_energy = 0.0_f32;
    let mut mid_energy = 0.0_f32;
    let mut high_energy = 0.0_f32;

    for (i, c) in buffer.iter().take(fft_size / 2).enumerate() {
        let freq = i as f32 * bin_hz;
        let magnitude = (c.re * c.re + c.im * c.im).sqrt();

        if freq <= 250.0 {
            sub_bass_energy += magnitude;
        } else if freq <= 4000.0 {
            mid_energy += magnitude;
        } else {
            high_energy += magnitude;
        }
    }

    let total = sub_bass_energy + mid_energy + high_energy;
    if total <= 1e-6 {
        SpectralEnergy {
            sub_bass_ratio: 0.33,
            mid_ratio: 0.33,
            high_ratio: 0.34,
        }
    } else {
        SpectralEnergy {
            sub_bass_ratio: sub_bass_energy / total,
            mid_ratio: mid_energy / total,
            high_ratio: high_energy / total,
        }
    }
}
