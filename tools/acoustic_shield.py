#!/usr/bin/env python3
"""
VOXY Laptop Acoustic Shield - Production DSP Front-End
Implements a multi-stage real-time speech enhancement engine:
Stage 1: Dual-Microphone Delay-and-Sum Beamformer + Spatial Coherence Post-Filter
Stage 2: Frequency-Domain Adaptive Filter (FDAF / Block NLMS) AEC with Geigel DTD
Stage 3: Decision-Directed Multi-Band Wiener Noise Suppressor with Musical Noise Elimination
Stage 4: Formant & Consonant Preserver (anti-robotic / anti-underwater psychoacoustic floor)
Stage 5: Adaptive Gain Control (AGC) with Soft-Knee Limiter

Maintains strict separation:
RAW MIC ───┬──> Acoustic Shield ──> STT
           └──> Clap Detector   ──> UI Overlay
"""

import collections
import math
import time
from typing import Dict, List, Optional, Tuple
import numpy as np
from scipy import signal


class DualMicBeamformer:
    """
    Stage 1: Broadside dual-microphone delay-and-sum beamformer
    with inter-channel spatial coherence post-filtering.
    """
    def __init__(self, sample_rate: int = 16000):
        self.sample_rate = sample_rate

    def process(self, stereo_block: np.ndarray) -> np.ndarray:
        """
        Input: (N, 2) stereo float32 array from Intel SST left/right capsules.
        Output: (N,) beamformed single-channel audio focused on-axis.
        """
        if stereo_block.ndim == 1:
            return stereo_block.flatten()
        if stereo_block.shape[1] == 1:
            return stereo_block[:, 0]

        left = stereo_block[:, 0]
        right = stereo_block[:, 1]

        # Broadside delay-and-sum: on-axis user speech is in-phase
        broadside = 0.5 * (left + right)
        diff = 0.5 * (left - right)

        # Estimate inter-channel correlation in time domain
        e_sum = np.sum(broadside ** 2) + 1e-8
        e_diff = np.sum(diff ** 2) + 1e-8
        
        # Spatial coherence factor: 1.0 for perfect in-phase broadside, lower for lateral noise
        coherence = max(0.25, min(1.0, (e_sum - e_diff) / e_sum))
        
        # Spatial filtering: preserve on-axis speech, attenuate diffuse / off-axis noise
        return broadside * (0.65 + 0.35 * coherence)


class AcousticEchoCanceller:
    """
    Stage 2: Frequency-Domain Adaptive Filter (FDAF) AEC with Geigel Double-Talk Detection.
    Subtracts laptop speaker playback from microphone signal with sub-millisecond execution.
    """
    def __init__(self, block_size: int = 256, step_size: float = 0.12, dtd_threshold: float = 2.0):
        self.block_size = block_size
        self.fft_size = block_size * 2  # 512
        self.step_size = step_size
        self.dtd_threshold = dtd_threshold
        
        # Frequency domain weights (512-point)
        self.W = np.zeros(self.fft_size, dtype=np.complex64)
        
        # Power spectral density normalizer for step size
        self.power = np.ones(self.fft_size, dtype=np.float32) * 1e-2
        self.alpha_p = 0.85
        
        # Reference history (last 512 samples)
        self.x_buf = np.zeros(self.fft_size, dtype=np.float32)
        self.echo_return_loss = 0.0

    def process(self, mic_input: np.ndarray, ref_input: Optional[np.ndarray]) -> np.ndarray:
        """
        mic_input: (256,) microphone samples
        ref_input: (256,) speaker reference samples (or None if speakers silent)
        """
        N = self.block_size
        d = mic_input.flatten()[:N]

        if ref_input is None or len(ref_input) == 0 or np.max(np.abs(ref_input)) < 1e-4:
            # Shift buffer with zeros
            self.x_buf[:-N] = self.x_buf[N:]
            self.x_buf[-N:] = 0.0
            return d

        ref = ref_input.flatten()[:N]
        # Shift reference buffer
        self.x_buf[:-N] = self.x_buf[N:]
        self.x_buf[-N:] = ref

        # Compute FFT of reference block
        X = np.fft.fft(self.x_buf)

        # Update reference power estimate
        X_mag2 = np.abs(X) ** 2
        self.power = self.alpha_p * self.power + (1.0 - self.alpha_p) * X_mag2

        # Estimated echo in time domain
        Y_hat_freq = self.W * X
        y_hat_time = np.fft.ifft(Y_hat_freq).real
        y_hat = y_hat_time[N:]  # Last N samples (linear convolution constraint)

        # Time-domain error (mic minus estimated echo)
        e = d - y_hat

        # Geigel Double-Talk Detector
        max_ref = np.max(np.abs(self.x_buf)) + 1e-5
        geigel_ratio = np.max(np.abs(d)) / max_ref

        # Measure Echo Return Loss Enhancement (ERLE)
        mic_pwr = np.mean(d ** 2) + 1e-8
        err_pwr = np.mean(e ** 2) + 1e-8
        if mic_pwr > err_pwr:
            self.echo_return_loss = 10.0 * math.log10(mic_pwr / err_pwr)

        # Adapt filter only when user is not speaking over VOXY (single-talk)
        if geigel_ratio < self.dtd_threshold:
            # Frequency-domain error with zero-padding in first half
            e_padded = np.zeros(self.fft_size, dtype=np.float32)
            e_padded[N:] = e
            E = np.fft.fft(e_padded)

            # Gradient calculation with power normalization
            normalized_step = self.step_size / np.maximum(self.power, 1e-3)
            gradient = normalized_step * E * np.conj(X)

            # Gradient time-domain constraint (first N samples only)
            grad_time = np.fft.ifft(gradient).real
            grad_time[N:] = 0.0
            self.W += np.fft.fft(grad_time)
            
            # Leakage / regularization
            self.W *= 0.9999

        return e.astype(np.float32)


class SpectralNoiseSuppressor:
    """
    Stage 3 & 4: Decision-Directed Wiener Filter with Psychoacoustic Masking & Consonant Protection.
    Suppresses continuous mechanical/fan noise, typing clicks, and room reverberation.
    """
    def __init__(self, sample_rate: int = 16000, n_fft: int = 512, hop_size: int = 256):
        self.sample_rate = sample_rate
        self.n_fft = n_fft
        self.hop_size = hop_size
        self.window = np.hanning(n_fft).astype(np.float32)
        
        # Buffers for overlap-add synthesis
        self.in_buffer = np.zeros(n_fft, dtype=np.float32)
        self.out_buffer = np.zeros(n_fft, dtype=np.float32)
        
        # Noise spectrum estimate
        self.noise_psd = np.ones(n_fft // 2 + 1, dtype=np.float32) * 1e-4
        self.prev_speech_psd = np.zeros(n_fft // 2 + 1, dtype=np.float32)
        self.alpha_noise = 0.96
        self.alpha_dd = 0.92  # Decision-directed smoothing
        self.beta_floor = 0.08  # -22 dB spectral floor (prevents musical noise / underwater sound)
        self.frame_count = 0

        # Frequency bin thresholds for consonant protection (>2200 Hz)
        freq_bins = np.linspace(0, sample_rate / 2, n_fft // 2 + 1)
        self.consonant_mask = freq_bins > 2200.0

    def process(self, block: np.ndarray) -> np.ndarray:
        """Processes block of audio samples (typically hop_size long)."""
        n_samples = len(block)
        
        # Shift input buffer
        self.in_buffer[:-n_samples] = self.in_buffer[n_samples:]
        self.in_buffer[-n_samples:] = block
        
        # Apply Hanning window & FFT
        windowed = self.in_buffer * self.window
        spec = np.fft.rfft(windowed)
        mag = np.abs(spec)
        phase = np.angle(spec)
        psd = mag ** 2

        self.frame_count += 1

        # Initial noise estimation during quiet startup
        if self.frame_count < 15:
            self.noise_psd = 0.8 * self.noise_psd + 0.2 * psd
            return block

        # Compute a-posteriori SNR: gamma = |Y|^2 / N
        gamma = np.maximum(psd / np.maximum(self.noise_psd, 1e-8), 1e-3)

        # Compute a-priori SNR via Decision-Directed approach
        xi = self.alpha_dd * (self.prev_speech_psd / np.maximum(self.noise_psd, 1e-8)) + \
             (1.0 - self.alpha_dd) * np.maximum(gamma - 1.0, 0.0)

        # Wiener gain: H = xi / (xi + 1)
        gain = xi / (xi + 1.0)

        # Consonant preservation: bound attenuation in high-frequency bins
        gain[self.consonant_mask] = np.maximum(gain[self.consonant_mask], 0.22)

        # Apply psychoacoustic spectral floor to eliminate musical noise
        gain = np.maximum(gain, self.beta_floor)

        # Filter magnitude & store for next frame
        clean_mag = gain * mag
        self.prev_speech_psd = clean_mag ** 2

        # Update noise estimate slowly when speech is absent
        speech_presence = np.mean(gamma) > 2.2
        if not speech_presence:
            self.noise_psd = self.alpha_noise * self.noise_psd + (1.0 - self.alpha_noise) * psd

        # Inverse FFT & overlap-add
        clean_spec = clean_mag * np.exp(1j * phase)
        clean_time = np.fft.irfft(clean_spec) * self.window

        self.out_buffer += clean_time
        out_chunk = self.out_buffer[:n_samples].copy()
        
        self.out_buffer[:-n_samples] = self.out_buffer[n_samples:]
        self.out_buffer[-n_samples:] = 0.0

        return out_chunk


class AdaptiveGainControl:
    """
    Stage 5: Voice-aware Adaptive Gain Control with Soft-Knee Limiter.
    Ensures optimal dynamic range for downstream STT without boosting ambient silence.
    """
    def __init__(self, target_rms: float = 0.14, max_gain_db: float = 18.0, min_gain_db: float = -12.0):
        self.target_rms = target_rms
        self.max_gain = 10.0 ** (max_gain_db / 20.0)
        self.min_gain = 10.0 ** (min_gain_db / 20.0)
        self.current_gain = 1.0

    def process(self, audio: np.ndarray) -> np.ndarray:
        rms = float(np.sqrt(np.mean(audio ** 2))) + 1e-6
        
        # Only adjust gain if speech is likely present (rms > 0.015)
        if rms > 0.015:
            desired_gain = self.target_rms / rms
            desired_gain = max(self.min_gain, min(self.max_gain, desired_gain))
            self.current_gain = 0.92 * self.current_gain + 0.08 * desired_gain
        else:
            self.current_gain = 0.98 * self.current_gain + 0.02 * 1.0

        boosted = audio * self.current_gain
        # Soft-knee tanh peak limiter to prevent digital clipping
        return np.tanh(boosted * 1.1) / 1.1


class LaptopAcousticShield:
    """
    Complete VOXY Laptop Acoustic Shield Front-End.
    Orchestrates Beamforming, AEC, Spectral Suppression, Consonant Protection, and AGC.
    """
    def __init__(self, sample_rate: int = 16000, block_size: int = 256):
        self.sample_rate = sample_rate
        self.block_size = block_size
        
        # Highpass filter for rumble (<75 Hz mechanical & fan rumble)
        self.hp_b, self.hp_a = signal.butter(2, 75.0 / (sample_rate / 2.0), btype='high')
        self.hp_zi_stereo = np.zeros((len(self.hp_b) - 1, 2), dtype=np.float32)
        self.hp_zi_mono = np.zeros(len(self.hp_b) - 1, dtype=np.float32)
        
        # Subsystems
        self.beamformer = DualMicBeamformer(sample_rate=sample_rate)
        self.aec = AcousticEchoCanceller(block_size=block_size, step_size=0.12)
        self.noise_suppressor = SpectralNoiseSuppressor(sample_rate=sample_rate, n_fft=512, hop_size=block_size)
        self.agc = AdaptiveGainControl(target_rms=0.14)

        # Performance metrics
        self.last_proc_latency_ms = 0.0
        self.snr_improvement_db = 0.0

    def process(self, raw_mic_audio: np.ndarray, speaker_ref: Optional[np.ndarray] = None) -> Tuple[np.ndarray, Dict]:
        """
        Process incoming raw microphone audio from Intel SST.
        Returns: (clean_audio_mono, metrics_dict)
        """
        t0 = time.perf_counter()
        
        # 1. Rumble removal (<75 Hz)
        if raw_mic_audio.ndim > 1 and raw_mic_audio.shape[1] > 1:
            raw_mono = np.mean(raw_mic_audio, axis=1)
            raw_filtered, self.hp_zi_stereo = signal.lfilter(self.hp_b, self.hp_a, raw_mic_audio, axis=0, zi=self.hp_zi_stereo)
        else:
            raw_mono = raw_mic_audio.flatten()
            raw_filtered, self.hp_zi_mono = signal.lfilter(self.hp_b, self.hp_a, raw_mono, zi=self.hp_zi_mono)

        in_noise_rms = float(np.sqrt(np.mean(raw_mono ** 2))) + 1e-6

        # Stage 1: Dual-Capsule Delay-and-Sum Beamforming + Spatial Filter
        stage1_bf = self.beamformer.process(raw_filtered)

        # Stage 2: Frequency-Domain Adaptive Filter Acoustic Echo Cancellation
        stage2_aec = self.aec.process(stage1_bf, speaker_ref)

        # Stage 3 & 4: Spectral Suppression with Consonant Protection
        stage3_denoised = self.noise_suppressor.process(stage2_aec)

        # Stage 5: Adaptive Gain Control
        clean_output = self.agc.process(stage3_denoised)

        out_noise_rms = float(np.sqrt(np.mean(clean_output ** 2))) + 1e-6
        dt_ms = (time.perf_counter() - t0) * 1000.0
        self.last_proc_latency_ms = dt_ms

        snr_gain = max(0.0, 10.0 * math.log10(in_noise_rms / max(out_noise_rms, 1e-6)))
        self.snr_improvement_db = snr_gain

        metrics = {
            "dsp_latency_ms": round(dt_ms, 3),
            "input_rms": round(in_noise_rms, 5),
            "output_rms": round(out_noise_rms, 5),
            "snr_improvement_db": round(snr_gain, 2),
            "erle_db": round(self.aec.echo_return_loss, 2),
            "agc_gain": round(self.agc.current_gain, 3),
        }

        return clean_output, metrics
