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
    #[serde(default)]
    pub crest_factor_db: f32,
    pub is_clipping: bool,
    #[serde(default)]
    pub stereo_correlation: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side_to_mid_ratio: Option<f32>,
    pub spectral_bands: SpectralEnergy,
    #[serde(default)]
    pub loop_telemetry: Option<LoopTelemetry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<SectionTelemetry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SectionTelemetry {
    pub index: usize,
    pub start_seconds: f32,
    pub end_seconds: f32,
    pub peak_dbfs: f32,
    pub rms_dbfs: f32,
    pub crest_factor_db: f32,
    pub is_clipping: bool,
    pub spectral_bands: SpectralEnergy,
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

fn amplitude_to_dbfs(amp: f32) -> f32 {
    if amp > 1e-5 {
        20.0 * amp.log10()
    } else {
        -96.0
    }
}

fn compute_level_metrics(samples: &[f32]) -> (f32, f32, f32, f32, f32) {
    if samples.is_empty() {
        return (0.0, -96.0, 0.0, -96.0, 0.0);
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
    let peak_dbfs = amplitude_to_dbfs(peak);
    let rms_dbfs = amplitude_to_dbfs(rms);
    let crest_factor_db = (peak_dbfs - rms_dbfs).max(0.0);
    (peak, peak_dbfs, rms, rms_dbfs, crest_factor_db)
}

fn compute_stereo_metrics(samples: &[f32], ch: usize) -> (Option<f32>, Option<f32>) {
    if ch != 2 || samples.len() < 4 {
        return (None, None);
    }

    let mut sum_lr = 0.0_f32;
    let mut sum_l2 = 0.0_f32;
    let mut sum_r2 = 0.0_f32;
    let mut sum_mid2 = 0.0_f32;
    let mut sum_side2 = 0.0_f32;

    let (pairs, _) = samples.as_chunks::<2>();
    for chunk in pairs {
        let l = chunk[0];
        let r = chunk[1];
        sum_lr += l * r;
        sum_l2 += l * l;
        sum_r2 += r * r;
        let mid = (l + r) * 0.5;
        let side = (l - r) * 0.5;
        sum_mid2 += mid * mid;
        sum_side2 += side * side;
    }

    let denom = (sum_l2 * sum_r2).sqrt();
    let correlation = if denom > 1e-6 {
        (sum_lr / denom).clamp(-1.0, 1.0)
    } else {
        1.0
    };

    let side_to_mid = if sum_mid2 > 1e-10 {
        (sum_side2 / sum_mid2).sqrt()
    } else {
        0.0
    };

    (Some(correlation), Some(side_to_mid))
}

fn analyze_sections(
    samples: &[f32],
    mono_samples: &[f32],
    sample_rate: u32,
    ch: usize,
    frames: usize,
) -> Vec<SectionTelemetry> {
    let sr = sample_rate.max(1) as f32;
    let duration = frames as f32 / sr;
    let count = if duration >= 8.0 {
        8
    } else if duration >= 1.0 {
        4
    } else {
        0
    };
    if count == 0 {
        return Vec::new();
    }

    (0..count)
        .map(|idx| {
            let f0 = (idx * frames) / count;
            let f1 = ((idx + 1) * frames) / count;
            let seg = &samples[(f0 * ch)..(f1 * ch).min(samples.len())];
            let mono_seg = &mono_samples[f0..f1.min(mono_samples.len())];
            let (peak, peak_dbfs, _rms, rms_dbfs, crest_factor_db) = compute_level_metrics(seg);
            SectionTelemetry {
                index: idx,
                start_seconds: f0 as f32 / sr,
                end_seconds: f1 as f32 / sr,
                peak_dbfs,
                rms_dbfs,
                crest_factor_db,
                is_clipping: peak >= 0.999,
                spectral_bands: calculate_spectral_bands(mono_seg, sample_rate),
            }
        })
        .collect()
}

fn build_diagnostics(
    is_clipping: bool,
    rms_dbfs: f32,
    crest_factor_db: f32,
    stereo_correlation: Option<f32>,
    spectral_bands: &SpectralEnergy,
    sections: &[SectionTelemetry],
) -> Vec<String> {
    let mut flags = Vec::new();
    if is_clipping {
        flags.push("CLIPPING_DETECTED".to_string());
    }
    if rms_dbfs > -40.0 && spectral_bands.sub_bass_ratio > 0.65 {
        flags.push("SUB_BASS_DOMINANT".to_string());
    }
    if rms_dbfs > -40.0 && spectral_bands.high_ratio < 0.05 {
        flags.push("DULL_HIGH_END".to_string());
    }
    if rms_dbfs > -18.0 && crest_factor_db < 4.0 {
        flags.push("LOW_DYNAMIC_RANGE".to_string());
    }
    if stereo_correlation.is_some_and(|c| c < 0.0) {
        flags.push("PHASE_CANCELLATION_RISK".to_string());
    }
    let active_rms: Vec<f32> = sections
        .iter()
        .map(|s| s.rms_dbfs)
        .filter(|&r| r > -50.0)
        .collect();
    if active_rms.len() >= 2 {
        let min_r = active_rms.iter().copied().fold(f32::INFINITY, f32::min);
        let max_r = active_rms.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        if max_r - min_r > 16.0 {
            flags.push("HIGH_SECTION_VARIANCE".to_string());
        }
    }
    flags
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
            crest_factor_db: 0.0,
            is_clipping: false,
            stereo_correlation: None,
            side_to_mid_ratio: None,
            spectral_bands: SpectralEnergy {
                sub_bass_ratio: 0.0,
                mid_ratio: 0.0,
                high_ratio: 0.0,
            },
            loop_telemetry: None,
            sections: Vec::new(),
            diagnostics: Vec::new(),
        };
    }

    let (peak, peak_dbfs, rms, rms_dbfs, crest_factor_db) = compute_level_metrics(samples);
    let (stereo_correlation, side_to_mid_ratio) = compute_stereo_metrics(samples, ch);

    let mono_samples: Vec<f32> = if ch == 2 {
        let (pairs, _) = samples.as_chunks::<2>();
        pairs.iter().map(|c| (c[0] + c[1]) * 0.5).collect()
    } else {
        samples.to_vec()
    };

    let spectral_bands = calculate_spectral_bands(&mono_samples, sample_rate);
    let frames = samples.len() / ch;
    let sections = analyze_sections(samples, &mono_samples, sample_rate, ch, frames);
    let is_clipping = peak >= 0.999;
    let diagnostics = build_diagnostics(
        is_clipping,
        rms_dbfs,
        crest_factor_db,
        stereo_correlation,
        &spectral_bands,
        &sections,
    );

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
        crest_factor_db,
        is_clipping,
        stereo_correlation,
        side_to_mid_ratio,
        spectral_bands,
        loop_telemetry,
        sections,
        diagnostics,
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
