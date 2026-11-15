use std::{
    f32::consts::PI,
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

use super::error::FfmpegError;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Temporary WAV file guard that removes the file when dropped.
#[derive(Debug)]
pub struct TempAudioFile {
    pub path: PathBuf,
}

impl Drop for TempAudioFile {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_file(&self.path) {
            tracing::debug!("Failed to remove temp audio file {}: {}", self.path.display(), err);
        }
    }
}

/// Computes the high-pass and optional low-pass cutoff frequencies (in Hz) for a given
/// `filter_strength` (`0..=100`) and `sample_rate`.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn filter_cutoffs(sample_rate: u32, filter_strength: u8) -> (f32, Option<f32>) {
    let sr = sample_rate.max(1) as f32;
    let strength = f32::from(filter_strength.min(100)) / 100.0;
    // Calibrated so the default strength (60%) gives HPF = 450 Hz and LPF = ~2700 Hz @ 8 kHz.
    let raw_hpf = if strength <= 0.60 {
        80.0 + (strength / 0.60) * 370.0
    } else {
        450.0 + ((strength - 0.60) / 0.40) * 110.0
    };
    let hpf_hz = raw_hpf.min(sr * 0.40);

    let lpf_hz = if filter_strength > 25 {
        let t = (f32::from(filter_strength.min(100)) - 25.0) / 75.0;
        let fc = if sample_rate <= 16_000 {
            (sr * (0.44 - t * 0.22)).max(sr * 0.25)
        } else {
            (12_000.0 - t * 7_500.0).min(sr * 0.45)
        };
        Some(fc.max(hpf_hz + 150.0).min(sr * 0.48))
    } else {
        None
    };

    (hpf_hz, lpf_hz)
}

/// Decodes audio from `input_video` into interleaved `f32` PCM samples using `ffmpeg`.
pub fn extract_audio_f32(
    ffmpeg_path: &Path,
    input_video: &Path,
    sample_rate: u32,
    channels: u16,
    start_secs: Option<f64>,
    duration_secs: Option<f64>,
) -> Result<Vec<f32>, FfmpegError> {
    let mut cmd = Command::new(ffmpeg_path);
    #[cfg(target_os = "windows")]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, crate::util::CREATE_NO_WINDOW);

    cmd.args(["-v", "error"]);
    if let Some(start) = start_secs {
        cmd.args(["-ss", &format!("{start:.3}")]);
    }
    cmd.arg("-i").arg(input_video);
    if let Some(dur) = duration_secs {
        cmd.args(["-t", &format!("{dur:.3}")]);
    }

    cmd.args([
        "-vn",
        "-sn",
        "-dn",
        "-f",
        "f32le",
        "-acodec",
        "pcm_f32le",
        "-ar",
        &sample_rate.to_string(),
        "-ac",
        &channels.to_string(),
        "pipe:1",
    ])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

    let output = cmd.output()?;
    if !output.status.success() && output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!("ffmpeg audio extraction failed: {}", stderr.trim());
        return Err(FfmpegError::AudioProcessing {
            message: stderr.trim().to_string(),
        });
    }

    let samples = output
        .stdout
        .as_chunks::<4>()
        .0
        .iter()
        .copied()
        .map(f32::from_le_bytes)
        .collect();
    Ok(samples)
}

/// Applies wraparound repair, high-pass wind removal, optional harshness low-pass, gain,
/// and soft-knee peak limiting to interleaved `f32` PCM audio.
#[must_use]
pub fn process_audio_samples(
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
    filter_strength: u8,
    gain_db: i8,
) -> Vec<f32> {
    if samples.is_empty() || channels == 0 || sample_rate == 0 {
        return samples.to_vec();
    }

    let ch_count = usize::from(channels);
    let frame_count = samples.len() / ch_count;
    let mut out = vec![0.0_f32; frame_count * ch_count];

    for ch in 0..ch_count {
        let mut mono = Vec::with_capacity(frame_count);
        for f in 0..frame_count {
            mono.push(samples[f * ch_count + ch]);
        }

        process_mono_channel(&mut mono, sample_rate, filter_strength, gain_db);

        for (f, &val) in mono.iter().enumerate() {
            out[f * ch_count + ch] = val;
        }
    }

    out
}

struct EnvelopeSpec {
    threshold: f32,
    ratio_exp: f32,
    idle_gain: f32,
    min_gain: f32,
    att_ms: f32,
    rel_ms: f32,
}

fn process_mono_channel(samples: &mut [f32], sample_rate: u32, filter_strength: u8, gain_db: i8) {
    unwrap_int16_overflow(samples, sample_rate);

    let (hpf_hz, lpf_hz) = filter_cutoffs(sample_rate, filter_strength);

    if filter_strength > 0 {
        declick_overload_bursts(samples, sample_rate);
    }

    // 4th-order Butterworth high-pass removes wind rumble and sub-motor turbulence.
    apply_4th_order_hp(samples, sample_rate, hpf_hz);

    if filter_strength > 0 {
        #[allow(clippy::cast_precision_loss)]
        let sr = sample_rate as f32;
        let strength = f32::from(filter_strength.min(100)) / 100.0;
        let w = (strength / 0.60).min(1.50);
        let w01 = (strength / 0.60).min(1.0);

        if let Some(lpf_fc) = lpf_hz {
            let low_split = 850.0_f32.min(sr * 0.30).max(hpf_hz + 80.0);
            let mid_split = 1850.0_f32.min(sr * 0.40).max(low_split + 200.0);

            if lpf_fc > mid_split + 150.0 {
                // 3-band time-domain wind & hiss suppression: keeps motor tones (850-1850 Hz) natural
                // while attenuating wind roar below 850 Hz and upper turbulence hiss above 1850 Hz.
                let mut b_low = samples.to_vec();
                apply_4th_order_lp(&mut b_low, sample_rate, low_split);
                compress_band_rms(
                    &mut b_low,
                    sample_rate,
                    EnvelopeSpec {
                        threshold: 0.025,
                        ratio_exp: 1.40 * w.min(1.25),
                        idle_gain: 1.0 - 0.75 * w01,
                        min_gain: 1.0 - 0.975 * w01,
                        att_ms: 8.0,
                        rel_ms: 90.0,
                    },
                );

                let mut b_upper = samples.to_vec();
                apply_4th_order_hp(&mut b_upper, sample_rate, low_split);

                let mut b_mid = b_upper.clone();
                apply_4th_order_lp(&mut b_mid, sample_rate, mid_split);

                let mut b_hi = b_upper;
                apply_4th_order_hp(&mut b_hi, sample_rate, mid_split);
                apply_4th_order_lp(&mut b_hi, sample_rate, lpf_fc);
                compress_band_rms(
                    &mut b_hi,
                    sample_rate,
                    EnvelopeSpec {
                        threshold: 0.045,
                        ratio_exp: 1.10 * w.min(1.25),
                        idle_gain: 1.0 - 0.25 * w01,
                        min_gain: 1.0 - 0.82 * w01,
                        att_ms: 8.0,
                        rel_ms: 90.0,
                    },
                );

                for i in 0..samples.len() {
                    samples[i] = b_low[i] + b_mid[i] + b_hi[i];
                }
            } else {
                apply_4th_order_lp(samples, sample_rate, lpf_fc);
            }
        }

        // Smooth pre-gain overload compressor balances heavy wind/motor blasts against normal flight.
        compress_band_rms(
            samples,
            sample_rate,
            EnvelopeSpec {
                threshold: 0.34 - 0.14 * w01,
                ratio_exp: 0.75 * w.min(1.20),
                idle_gain: 1.0,
                min_gain: 1.0 - 0.68 * w01,
                att_ms: 10.0,
                rel_ms: 140.0,
            },
        );
    }

    let gain = 10.0_f32.powf(f32::from(gain_db) / 20.0);
    // Soft-knee ceiling prevents inter-sample AAC clipping on playback.
    let threshold = 0.65_f32;
    let ceiling = 0.85_f32;
    let headroom = ceiling - threshold;

    for s in samples.iter_mut() {
        let scaled = *s * gain;
        let abs = scaled.abs();
        *s = if abs > threshold {
            let compressed = threshold + headroom * ((abs - threshold) / headroom).tanh();
            compressed.copysign(scaled)
        } else {
            scaled
        };
    }
}

fn apply_biquad_cascade(samples: &mut [f32], mut stage1: Biquad, mut stage2: Biquad) {
    for s in samples.iter_mut() {
        *s = stage2.process(stage1.process(*s));
    }
}

fn apply_4th_order_hp(samples: &mut [f32], sample_rate: u32, cutoff_hz: f32) {
    apply_biquad_cascade(
        samples,
        Biquad::highpass(sample_rate, cutoff_hz, std::f32::consts::FRAC_1_SQRT_2),
        Biquad::highpass(sample_rate, cutoff_hz, std::f32::consts::FRAC_1_SQRT_2),
    );
}

fn apply_4th_order_lp(samples: &mut [f32], sample_rate: u32, cutoff_hz: f32) {
    apply_biquad_cascade(
        samples,
        Biquad::lowpass(sample_rate, cutoff_hz, std::f32::consts::FRAC_1_SQRT_2),
        Biquad::lowpass(sample_rate, cutoff_hz, std::f32::consts::FRAC_1_SQRT_2),
    );
}

/// Suppresses unrecoverable single-sample impulsive crackles during overload bursts
/// while leaving normal high-frequency propeller oscillations untouched.
fn declick_overload_bursts(samples: &mut [f32], sample_rate: u32) {
    let n = samples.len();
    if n < 6 {
        return;
    }

    let win = (sample_rate as usize * 25 / 1000).max(16);
    let half = win / 2;

    // Prefix sum of large 1-sample jumps gives O(n) local crackle density over a 25 ms window.
    let mut jump_prefix = Vec::with_capacity(n + 1);
    jump_prefix.push(0_u32);
    for i in 0..n {
        let prev = if i > 0 { samples[i - 1] } else { samples[0] };
        let is_jump = u32::from((samples[i] - prev).abs() > 1.05);
        jump_prefix.push(jump_prefix[i] + is_jump);
    }

    #[allow(clippy::cast_precision_loss)]
    let inv_win = 1.0 / win as f32;
    let mut prev_buf = Vec::with_capacity(n);

    for _ in 0..2 {
        prev_buf.clear();
        prev_buf.extend_from_slice(samples);
        for i in 2..(n - 2) {
            let lo = i.saturating_sub(half);
            let hi = (i + half).min(n);
            #[allow(clippy::cast_precision_loss)]
            let density = (jump_prefix[hi] - jump_prefix[lo]) as f32 * inv_win;
            let thr = if density > 0.02 { 0.75 } else { 1.05 };
            let mid = 0.5 * (prev_buf[i - 1] + prev_buf[i + 1]);
            let delta = prev_buf[i] - mid;
            if delta.abs() > thr {
                samples[i] = mid + delta.clamp(-0.35, 0.35);
            }
        }
    }
}

/// Smooth RMS envelope follower that attenuates excess level above `spec.threshold`.
fn compress_band_rms(samples: &mut [f32], sample_rate: u32, spec: EnvelopeSpec) {
    if samples.is_empty() || sample_rate == 0 {
        return;
    }

    #[allow(clippy::cast_precision_loss)]
    let sr = sample_rate as f32;
    let att = 1.0 - (-1.0 / (spec.att_ms * 0.001 * sr)).exp();
    let rel = 1.0 - (-1.0 / (spec.rel_ms * 0.001 * sr)).exp();

    let mut env2 = samples[0] * samples[0];
    for s in samples.iter_mut() {
        let v2 = *s * *s;
        let coef = if v2 > env2 { att } else { rel };
        env2 += coef * (v2 - env2);
        let env = env2.sqrt();
        let gain = if env > spec.threshold {
            (spec.idle_gain * (spec.threshold / env.max(1.0e-5)).powf(spec.ratio_exp))
                .min(spec.idle_gain)
                .max(spec.min_gain)
        } else {
            spec.idle_gain
        };
        *s *= gain;
    }
}

fn diff_energy(samples: &[f32]) -> f32 {
    samples.windows(2).map(|w| (w[1] - w[0]) * (w[1] - w[0])).sum()
}

fn hpf_energy(samples: &[f32], sample_rate: u32) -> f32 {
    let mut hp = Biquad::highpass(sample_rate, 140.0, std::f32::consts::FRAC_1_SQRT_2);
    samples
        .iter()
        .map(|&s| {
            let y = hp.process(s);
            y * y
        })
        .sum()
}

/// Repairs signed 16-bit integer wraparound (`±2.0` discontinuities) using bounded Viterbi
/// unwrapping followed by iterative single-sample snap correction.
fn unwrap_int16_overflow(samples: &mut [f32], sample_rate: u32) {
    let n = samples.len();
    if n < 10 {
        return;
    }

    // 5-sample moving average suppresses 1.5–4 kHz motor harmonics while preserving baseline jumps.
    let mut ma5 = Vec::with_capacity(n);
    for i in 0..n {
        let mut sum = 0.0_f32;
        for offset in -2_isize..=2 {
            let idx = i.saturating_add_signed(offset).min(n - 1);
            sum += samples[idx];
        }
        ma5.push(sum * 0.2);
    }

    let pad = (sample_rate as usize * 15 / 1000).max(24);
    let max_len = (sample_rate as usize * 120 / 1000).max(128);
    let mut intervals: Vec<(usize, usize)> = Vec::new();
    for i in 0..(n - 1) {
        let ma_jump = i + 3 < n && (ma5[i + 3] - ma5[i]).abs() > 1.05;
        let opposite_rail = (samples[i + 1] - samples[i]).abs() > 1.35 && (samples[i] * samples[i + 1] < -0.25);
        if ma_jump || opposite_rail {
            let s = i.saturating_sub(pad);
            let e = (i + pad).min(n);
            if let Some(last) = intervals.last_mut() {
                if s <= last.1 && (e - last.0 <= max_len) {
                    last.1 = last.1.max(e);
                    continue;
                }
            }
            intervals.push((s, e));
        }
    }

    const STATES: [f32; 3] = [-1.0, 0.0, 1.0];
    const ZERO_IDX: usize = 1;
    const W_AMP: f32 = 0.008;
    const W_K: f32 = 0.015;

    for (start, end) in intervals {
        let len = end - start;
        if len < 2 {
            continue;
        }
        let orig = &samples[start..end];
        let mut seg = orig.to_vec();
        let mut dp = [1.0e9_f32; 3];
        dp[ZERO_IDX] = 0.0;
        let mut back = vec![[0_u8; 3]; len];

        for i in 1..len {
            let mut next_dp = [0.0_f32; 3];
            for (cur_idx, &cur_k) in STATES.iter().enumerate() {
                let cur_val = seg[i] + 2.0 * cur_k;
                let mut best_cost = f32::INFINITY;
                let mut best_prev = ZERO_IDX;
                for (prev_idx, &prev_k) in STATES.iter().enumerate() {
                    let prev_val = seg[i - 1] + 2.0 * prev_k;
                    let diff = cur_val - prev_val;
                    let cost = dp[prev_idx] + diff * diff;
                    if cost < best_cost {
                        best_cost = cost;
                        best_prev = prev_idx;
                    }
                }
                #[allow(clippy::cast_possible_truncation)]
                {
                    back[i][cur_idx] = best_prev as u8;
                }
                next_dp[cur_idx] = best_cost + cur_val * cur_val * W_AMP + cur_k * cur_k * W_K;
            }
            dp = next_dp;
        }

        let mut chosen = vec![ZERO_IDX; len];
        chosen[len - 1] = ZERO_IDX;
        for i in (1..len).rev() {
            chosen[i - 1] = usize::from(back[i][chosen[i]]);
        }

        if chosen.iter().any(|&c| c != ZERO_IDX) {
            for (i, &state_idx) in chosen.iter().enumerate() {
                seg[i] += 2.0 * STATES[state_idx];
            }

            let rough_before = diff_energy(orig);
            let rough_after = diff_energy(&seg);
            let hp_before = hpf_energy(orig, sample_rate) + 1.0e-6;
            let hp_after = hpf_energy(&seg, sample_rate);
            let max_abs = seg.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);

            if rough_after < rough_before * 0.90 && hp_after <= hp_before * 1.15 && max_abs < 2.95 {
                samples[start..end].copy_from_slice(&seg);
            }
        }
    }

    // Iterative single-sample wraparound snap repairs isolated ±2.0 spikes skipped by the block guard.
    for _ in 0..2 {
        for i in 1..(n - 1) {
            let mid = 0.5 * (samples[i - 1] + samples[i + 1]);
            let err0 = (samples[i] - mid).abs();
            if err0 > 1.10 {
                let mut best_val = samples[i];
                let mut best_err = err0;
                for shift in [-2.0_f32, 2.0_f32] {
                    let cand = samples[i] + shift;
                    if cand.abs() < 2.2 {
                        let err = (cand - mid).abs();
                        if err < best_err * 0.55 {
                            best_err = err;
                            best_val = cand;
                        }
                    }
                }
                samples[i] = best_val;
            }
        }
    }
}

/// Writes interleaved `f32` PCM samples to a standard 32-bit IEEE float WAV file.
pub fn write_wav_f32(path: &Path, samples: &[f32], sample_rate: u32, channels: u16) -> std::io::Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    let data_bytes = u32::try_from(samples.len().saturating_mul(4)).unwrap_or(u32::MAX);
    let riff_size = data_bytes.saturating_add(36);
    let block_align = channels.saturating_mul(4);
    let byte_rate = sample_rate.saturating_mul(u32::from(block_align));

    writer.write_all(b"RIFF")?;
    writer.write_all(&riff_size.to_le_bytes())?;
    writer.write_all(b"WAVE")?;
    writer.write_all(b"fmt ")?;
    writer.write_all(&16_u32.to_le_bytes())?;
    writer.write_all(&3_u16.to_le_bytes())?; // WAVE_FORMAT_IEEE_FLOAT
    writer.write_all(&channels.to_le_bytes())?;
    writer.write_all(&sample_rate.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&32_u16.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&data_bytes.to_le_bytes())?;

    for &sample in samples {
        writer.write_all(&sample.to_le_bytes())?;
    }
    writer.flush()
}

/// Extracts, cleans, and writes a temporary 32-bit float WAV file ready for encoder muxing.
pub fn prepare_cleaned_audio_wav(
    ffmpeg_path: &Path,
    input_video: &Path,
    sample_rate: u32,
    channels: u16,
    filter_strength: u8,
    gain_db: i8,
) -> Result<Option<TempAudioFile>, FfmpegError> {
    let raw = extract_audio_f32(ffmpeg_path, input_video, sample_rate, channels, None, None)?;
    if raw.is_empty() {
        return Ok(None);
    }

    let cleaned = process_audio_samples(&raw, sample_rate, channels, filter_strength, gain_db);
    let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_path = std::env::temp_dir().join(format!("walksnail_osd_audio_{}_{id}.wav", std::process::id()));
    write_wav_f32(&temp_path, &cleaned, sample_rate, channels).map_err(|source| FfmpegError::AudioProcessing {
        message: source.to_string(),
    })?;

    Ok(Some(TempAudioFile { path: temp_path }))
}

struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    #[allow(clippy::cast_precision_loss)]
    fn highpass(sample_rate: u32, fc: f32, q: f32) -> Self {
        let w0 = 2.0 * PI * fc / sample_rate as f32;
        let cos_w = w0.cos();
        let alpha = w0.sin() / (2.0 * q);

        let a0 = 1.0 + alpha;
        Self {
            b0: ((1.0 + cos_w) * 0.5) / a0,
            b1: -(1.0 + cos_w) / a0,
            b2: ((1.0 + cos_w) * 0.5) / a0,
            a1: (-2.0 * cos_w) / a0,
            a2: (1.0 - alpha) / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn lowpass(sample_rate: u32, fc: f32, q: f32) -> Self {
        let w0 = 2.0 * PI * fc / sample_rate as f32;
        let cos_w = w0.cos();
        let alpha = w0.sin() / (2.0 * q);

        let a0 = 1.0 + alpha;
        Self {
            b0: ((1.0 - cos_w) * 0.5) / a0,
            b1: (1.0 - cos_w) / a0,
            b2: ((1.0 - cos_w) * 0.5) / a0,
            a1: (-2.0 * cos_w) / a0,
            a2: (1.0 - alpha) / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

#[cfg(test)]
#[allow(clippy::cast_precision_loss)]
mod tests {
    use super::*;

    #[test]
    fn test_unwrap_int16_overflow_fixes_wraparound() {
        let sr = 8000;
        let mut clean = Vec::with_capacity(1200);
        for i in 0..1200 {
            let t = i as f32 / sr as f32;
            // Slow wind pulse peaking at +1.4 plus quiet 1.2 kHz motor tone
            let wind = if (0.04..0.11).contains(&t) {
                1.4 * ((t - 0.04) / 0.07 * PI).sin()
            } else {
                0.0
            };
            let motor = 0.15 * (2.0 * PI * 1200.0 * t).sin();
            clean.push(wind + motor);
        }

        let mut wrapped: Vec<f32> = clean
            .iter()
            .map(|&v| {
                if v > 1.0 {
                    v - 2.0
                } else if v < -1.0 {
                    v + 2.0
                } else {
                    v
                }
            })
            .collect();

        let max_wrapped_diff = wrapped.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0_f32, f32::max);
        assert!(max_wrapped_diff > 1.5, "expected wraparound discontinuity");

        unwrap_int16_overflow(&mut wrapped, sr);

        let max_unwrapped_diff = wrapped.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0_f32, f32::max);
        assert!(
            max_unwrapped_diff < 0.35,
            "expected smooth unwrapped signal, got max diff {max_unwrapped_diff}"
        );
    }

    #[test]
    fn test_unwrap_ignores_clean_2khz_signal() {
        let sr = 8000;
        let original: Vec<f32> = (0..800)
            .map(|i| 0.92 * (2.0 * PI * 2000.0 * (i as f32 / sr as f32)).sin())
            .collect();
        let mut processed = original.clone();

        unwrap_int16_overflow(&mut processed, sr);

        assert_eq!(original, processed);
    }

    #[test]
    fn test_process_audio_reduces_gain_and_limits_peaks() {
        let sr = 8000;
        let samples: Vec<f32> = (0..1600)
            .map(|i| {
                let t = i as f32 / sr as f32;
                1.3 * (2.0 * PI * 30.0 * t).sin() + 0.9 * (2.0 * PI * 1200.0 * t).sin()
            })
            .collect();

        let out = process_audio_samples(&samples, sr, 1, 60, -10);
        let peak = out.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
        assert!(peak <= 0.895, "peak {peak} exceeded soft ceiling");

        // Second half (after HPF settles) should have substantially lower RMS than input
        let rms_in = (samples[800..].iter().map(|v| v * v).sum::<f32>() / 800.0).sqrt();
        let rms_out = (out[800..].iter().map(|v| v * v).sum::<f32>() / 800.0).sqrt();
        assert!(rms_out < rms_in * 0.3, "expected significant rumble+gain reduction");
    }

    #[test]
    fn test_unwrap_never_inflates_dense_highfreq_overload() {
        let sr = 8000;
        // Simulate dense overloaded high-frequency bursts jumping near ±0.95 (no artificial low-band drift).
        let samples: Vec<f32> = (0..3200)
            .map(|i| {
                let t = i as f32 / sr as f32;
                let raw = 0.95 * (2.0 * PI * 1900.0 * t).sin() + 0.55 * (2.0 * PI * 3100.0 * t).sin();
                if raw > 1.0 {
                    raw - 2.0
                } else if raw < -1.0 {
                    raw + 2.0
                } else {
                    raw
                }
            })
            .collect();

        let out = process_audio_samples(&samples, sr, 1, 60, -10);
        let peak = out.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
        let rms = (out[800..].iter().map(|v| v * v).sum::<f32>() / (out.len() - 800) as f32).sqrt();
        assert!(peak < 0.55, "overload blast peak too high: {peak}");
        assert!(rms < 0.14, "overload blast rms too loud: {rms}");
    }

    #[test]
    fn test_three_band_attenuates_wind_more_than_motors() {
        let sr = 8000;
        let wind_600hz: Vec<f32> = (0..1600)
            .map(|i| 0.35 * (2.0 * PI * 600.0 * (i as f32 / sr as f32)).sin())
            .collect();
        let motor_1200hz: Vec<f32> = (0..1600)
            .map(|i| 0.35 * (2.0 * PI * 1200.0 * (i as f32 / sr as f32)).sin())
            .collect();

        let out_wind = process_audio_samples(&wind_600hz, sr, 1, 60, -4);
        let out_motor = process_audio_samples(&motor_1200hz, sr, 1, 60, -4);

        let rms_wind = (out_wind[800..].iter().map(|v| v * v).sum::<f32>() / 800.0).sqrt();
        let rms_motor = (out_motor[800..].iter().map(|v| v * v).sum::<f32>() / 800.0).sqrt();
        assert!(
            rms_wind < rms_motor * 0.25,
            "expected strong wind band attenuation: wind={rms_wind}, motor={rms_motor}"
        );
    }
}
