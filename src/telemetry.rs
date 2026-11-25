use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioTelemetry {
    pub channels: u16,
    pub sample_count: usize,
    pub duration_seconds: f32,
    pub peak_amplitude: f32,
    pub peak_dbfs: f32,
    pub rms_amplitude: f32,
    pub rms_dbfs: f32,
    pub is_clipping: bool,
    #[serde(default)]
    pub stereo_correlation: Option<f32>,
    pub spectral_bands: SpectralEnergy,
    #[serde(default)]
    pub loop_telemetry: Option<LoopTelemetry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoopTelemetry {
    pub start_sample: u32,
    pub end_sample: u32,
    pub start_seconds: f32,
    pub end_seconds: f32,
    pub loop_duration_seconds: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpectralEnergy {
    pub sub_bass_ratio: f32,
    pub mid_ratio: f32,
    pub high_ratio: f32,
}

pub fn analyze_audio(
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
    loop_samples: Option<(u32, u32)>,
) -> AudioTelemetry {
    let ch = channels.max(1) as usize;
    if samples.is_empty() {
        return AudioTelemetry {
            channels,
            sample_count: 0,
            duration_seconds: 0.0,
            peak_amplitude: 0.0,
            peak_dbfs: -96.0,
            rms_amplitude: 0.0,
            rms_dbfs: -96.0,
            is_clipping: false,
            stereo_correlation: None,
            spectral_bands: SpectralEnergy {
                sub_bass_ratio: 0.0,
                mid_ratio: 0.0,
                high_ratio: 0.0,
            },
            loop_telemetry: None,
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

    let stereo_correlation = if ch == 2 && samples.len() >= 4 {
        let mut sum_lr = 0.0_f32;
        let mut sum_l2 = 0.0_f32;
        let mut sum_r2 = 0.0_f32;

        let (pairs, _) = samples.as_chunks::<2>();
        for chunk in pairs {
            let l = chunk[0];
            let r = chunk[1];
            sum_lr += l * r;
            sum_l2 += l * l;
            sum_r2 += r * r;
        }

        let denom = (sum_l2 * sum_r2).sqrt();
        if denom > 1e-6 {
            Some((sum_lr / denom).clamp(-1.0, 1.0))
        } else {
            Some(1.0)
        }
    } else {
        None
    };

    let mono_samples: Vec<f32> = if ch == 2 {
        let (pairs, _) = samples.as_chunks::<2>();
        pairs.iter().map(|c| (c[0] + c[1]) * 0.5).collect()
    } else {
        samples.to_vec()
    };

    let spectral_bands = calculate_spectral_bands(&mono_samples, sample_rate);

    let frames = samples.len() / ch;

    let loop_telemetry = loop_samples.map(|(start, end)| LoopTelemetry {
        start_sample: start,
        end_sample: end,
        start_seconds: start as f32 / sample_rate as f32,
        end_seconds: end as f32 / sample_rate as f32,
        loop_duration_seconds: (end.saturating_sub(start)) as f32 / sample_rate as f32,
    });

    AudioTelemetry {
        channels,
        sample_count: frames,
        duration_seconds: frames as f32 / sample_rate as f32,
        peak_amplitude: peak,
        peak_dbfs,
        rms_amplitude: rms,
        rms_dbfs,
        is_clipping: peak >= 0.999,
        stereo_correlation,
        spectral_bands,
        loop_telemetry,
    }
}

fn calculate_spectral_bands(samples: &[f32], sample_rate: u32) -> SpectralEnergy {
    if samples.len() < 128 {
        return SpectralEnergy {
            sub_bass_ratio: 0.33,
            mid_ratio: 0.33,
            high_ratio: 0.34,
        };
    }

    let fft_size = 2048.min(samples.len().next_power_of_two());
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_size);
    let bin_hz = sample_rate as f32 / fft_size as f32;

    let max_windows = 32usize;
    let window_starts: Vec<usize> = if samples.len() <= fft_size {
        vec![0]
    } else {
        let span = samples.len() - fft_size;
        let count = max_windows.min((samples.len() / fft_size).max(1));
        if count <= 1 {
            vec![0]
        } else {
            (0..count).map(|i| (i * span) / (count - 1)).collect()
        }
    };

    let mut sub_bass_energy = 0.0_f32;
    let mut mid_energy = 0.0_f32;
    let mut high_energy = 0.0_f32;
    let mut buffer = vec![Complex { re: 0.0, im: 0.0 }; fft_size];

    for start in window_starts {
        for (i, slot) in buffer.iter_mut().enumerate() {
            // Hann window reduces spectral leakage across arbitrary cut points
            let hann =
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / fft_size as f32).cos());
            let s = samples.get(start + i).copied().unwrap_or(0.0);
            *slot = Complex {
                re: s * hann,
                im: 0.0,
            };
        }

        fft.process(&mut buffer);

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
