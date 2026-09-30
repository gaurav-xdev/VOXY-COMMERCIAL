/// FFT-based noise suppression using spectral subtraction.
///
/// Uses proper DFT-based STFT with Hann windowing, spectral subtraction,
/// and overlap-add synthesis.
pub struct SpectralDenoiser {
    fft_size: usize,
    hop_size: usize,
    window: Vec<f32>,
    overlap_buffer: Vec<f32>,
    noise_spectrum: Vec<f32>,
    input_buffer: Vec<f32>,
    write_pos: usize,
    buffer_ready: bool,
    noise_smoothing: f32,
    min_noise_floor: f32,
    oversubtraction: f32,
    spectral_floor: f32,
    frames_processed: usize,
    warmup_frames: usize,
}

impl SpectralDenoiser {
    pub fn new(_sample_rate: u32) -> Self {
        let fft_size = 512;
        let hop_size = 256;
        let window = Self::hann_window(fft_size);

        Self {
            fft_size,
            hop_size,
            window: window.clone(),
            overlap_buffer: vec![0.0; fft_size],
            noise_spectrum: vec![0.0; fft_size / 2 + 1],
            input_buffer: vec![0.0; fft_size],
            write_pos: 0,
            buffer_ready: false,
            noise_smoothing: 0.05,
            min_noise_floor: 0.001,
            oversubtraction: 1.5,
            spectral_floor: 0.05,
            frames_processed: 0,
            warmup_frames: 20,
        }
    }

    pub fn with_oversubtraction(mut self, factor: f32) -> Self {
        self.oversubtraction = factor.clamp(1.0, 4.0);
        self
    }

    pub fn with_spectral_floor(mut self, floor: f32) -> Self {
        self.spectral_floor = floor.clamp(0.001, 0.5);
        self
    }

    #[allow(clippy::needless_range_loop)]
    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) {
        output.clear();
        output.reserve(input.len());

        for &sample in input {
            self.input_buffer[self.write_pos] = sample;
            self.write_pos += 1;

            if self.write_pos >= self.fft_size {
                let denoised = self.process_frame();
                for i in 0..self.hop_size {
                    let out_sample = self.overlap_buffer[i] + denoised[i];
                    output.push(out_sample);
                }
                self.overlap_buffer.rotate_left(self.hop_size);
                for i in self.fft_size - self.hop_size..self.fft_size {
                    self.overlap_buffer[i] = 0.0;
                }
                self.input_buffer.rotate_left(self.hop_size);
                self.write_pos = self.fft_size - self.hop_size;
                self.buffer_ready = true;
                self.frames_processed += 1;
            }
        }

        if self.write_pos > 0 && !self.buffer_ready {
            output.extend_from_slice(&self.input_buffer[..self.write_pos]);
        }
    }

    #[allow(clippy::needless_range_loop)]
    fn process_frame(&mut self) -> Vec<f32> {
        let n = self.fft_size;
        let num_bins = n / 2 + 1;

        // Window the input
        let windowed: Vec<f32> = self
            .input_buffer
            .iter()
            .zip(self.window.iter())
            .map(|(s, w)| s * w)
            .collect();

        // Forward DFT: compute real part and imaginary part for each bin
        let mut re = vec![0.0f32; num_bins];
        let mut im = vec![0.0f32; num_bins];
        for k in 0..num_bins {
            let mut sum_re = 0.0f32;
            let mut sum_im = 0.0f32;
            for j in 0..n {
                let angle = 2.0 * std::f32::consts::PI * k as f32 * j as f32 / n as f32;
                sum_re += windowed[j] * angle.cos();
                sum_im -= windowed[j] * angle.sin();
            }
            re[k] = sum_re;
            im[k] = sum_im;
        }

        // Magnitude and phase
        let mut magnitude = Vec::with_capacity(num_bins);
        let mut phase = Vec::with_capacity(num_bins);
        for k in 0..num_bins {
            magnitude.push((re[k] * re[k] + im[k] * im[k]).sqrt());
            phase.push(im[k].atan2(re[k]));
        }

        // Update noise estimate
        if self.frames_processed < self.warmup_frames {
            for k in 0..num_bins {
                self.noise_spectrum[k] = self.noise_spectrum[k] * (1.0 - self.noise_smoothing)
                    + magnitude[k] * self.noise_smoothing;
            }
        } else {
            for k in 0..num_bins {
                if magnitude[k] < self.noise_spectrum[k] * 2.0 {
                    self.noise_spectrum[k] = self.noise_spectrum[k] * (1.0 - self.noise_smoothing)
                        + magnitude[k] * self.noise_smoothing;
                }
            }
        }

        // Spectral subtraction
        let mut denoised_re = vec![0.0f32; num_bins];
        let mut denoised_im = vec![0.0f32; num_bins];
        for k in 0..num_bins {
            let noise = self.noise_spectrum[k].max(self.min_noise_floor);
            let subtracted = magnitude[k] - self.oversubtraction * noise;
            let gain = (subtracted / magnitude[k].max(1e-10)).max(self.spectral_floor);
            let new_mag = magnitude[k] * gain;
            denoised_re[k] = new_mag * phase[k].cos();
            denoised_im[k] = new_mag * phase[k].sin();
        }

        // Inverse DFT to get time-domain output
        let mut output_frame = vec![0.0f32; n];
        for j in 0..n {
            let mut sample = 0.0f32;
            for k in 0..num_bins {
                let angle = 2.0 * std::f32::consts::PI * k as f32 * j as f32 / n as f32;
                sample += denoised_re[k] * angle.cos() - denoised_im[k] * angle.sin();
                // Mirror bins (k > 0 and k < n/2) contribute twice due to symmetry
                if k > 0 && k < n / 2 {
                    sample += denoised_re[k] * angle.cos() - denoised_im[k] * angle.sin();
                }
            }
            output_frame[j] = sample / n as f32;
        }

        // Apply synthesis window
        for i in 0..n {
            output_frame[i] *= self.window[i];
        }

        output_frame
    }

    fn hann_window(size: usize) -> Vec<f32> {
        (0..size)
            .map(|i| {
                let n = size as f32;
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / n).cos())
            })
            .collect()
    }

    pub fn reset(&mut self) {
        self.noise_spectrum.fill(self.min_noise_floor);
        self.overlap_buffer.fill(0.0);
        self.input_buffer.fill(0.0);
        self.write_pos = 0;
        self.buffer_ready = false;
        self.frames_processed = 0;
    }

    pub fn noise_floor(&self) -> f32 {
        self.noise_spectrum.iter().copied().fold(0.0f32, f32::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denoiser_creation() {
        let d = SpectralDenoiser::new(16000);
        assert_eq!(d.fft_size, 512);
        assert_eq!(d.hop_size, 256);
        assert_eq!(d.window.len(), 512);
    }

    #[test]
    fn denoiser_passthrough_loud_signal() {
        let mut d = SpectralDenoiser::new(16000);
        let input: Vec<f32> = (0..2048).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
        let mut output = Vec::new();
        d.process(&input, &mut output);
        assert!(!output.is_empty());
        let output_rms = compute_rms(&output);
        assert!(output_rms > 0.01, "output_rms too low: {output_rms}");
    }

    #[test]
    fn denoiser_reduces_noise() {
        let mut d = SpectralDenoiser::new(16000).with_oversubtraction(2.0);
        // Feed noise floor for warmup
        for _ in 0..30 {
            let noise: Vec<f32> = (0..512).map(|i| (i as f32 * 0.01).sin() * 0.001).collect();
            let mut out = Vec::new();
            d.process(&noise, &mut out);
        }
        // Now feed quiet signal
        let quiet: Vec<f32> = vec![0.0005; 512];
        let mut output = Vec::new();
        d.process(&quiet, &mut output);
        if !output.is_empty() {
            let output_rms = compute_rms(&output);
            assert!(
                output_rms < 0.005,
                "noise not suppressed enough: {output_rms}"
            );
        }
    }

    #[test]
    fn denoiser_reset() {
        let mut d = SpectralDenoiser::new(16000);
        let input = vec![0.5; 512];
        let mut output = Vec::new();
        d.process(&input, &mut output);
        d.reset();
        assert_eq!(d.frames_processed, 0);
    }

    #[test]
    fn denoiser_hann_window() {
        let window = SpectralDenoiser::hann_window(512);
        assert_eq!(window.len(), 512);
        assert!(window[0].abs() < 0.01);
        assert!(window[511].abs() < 0.01);
        assert!(window[256].abs() > 0.9);
    }

    #[test]
    fn denoiser_custom_params() {
        let d = SpectralDenoiser::new(16000)
            .with_oversubtraction(3.0)
            .with_spectral_floor(0.1);
        assert_eq!(d.oversubtraction, 3.0);
        assert_eq!(d.spectral_floor, 0.1);
    }

    #[test]
    fn denoiser_small_input() {
        let mut d = SpectralDenoiser::new(16000);
        let input = vec![0.1; 100];
        let mut output = Vec::new();
        d.process(&input, &mut output);
        assert_eq!(output.len(), 100);
    }

    fn compute_rms(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = samples.iter().map(|&s| s as f64 * s as f64).sum();
        (sum / samples.len() as f64).sqrt() as f32
    }
}
