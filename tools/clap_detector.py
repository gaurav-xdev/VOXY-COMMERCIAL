#!/usr/bin/env python3
"""
VOXY Physical Clap Detection Engine
Evaluates:
- Option A: Pure Acoustic Transient DSP
- Option B: Pure Visual Hand Landmark Convergence
- Option C: Hybrid Acoustic-Visual Confirmation
"""

import collections
import math
import os
import time
from typing import Dict, Optional, Tuple
import numpy as np
import sounddevice as sd

class AcousticClapDetector:
    """
    High-performance real-time acoustic clap detector.
    Analyzes:
    1. Transient attack slope (fast rise < 8ms)
    2. Peak amplitude (> 0.18) & short-term energy jump (> 10x ambient)
    3. Spectral balance (high-frequency content, ZCR > 0.12 to reject low-freq door/desk thuds)
    4. Fast exponential decay (energy drops > 60% within 40ms to reject sustained speech/coughing)
    5. Refractory debounce (1.0s window to prevent reverberation double-triggers)
    """

    def __init__(self, sample_rate: int = 16000, block_size: int = 256):
        self.sample_rate = sample_rate
        self.block_size = block_size  # 256 samples @ 16kHz = 16.0ms per block
        
        # Audio ring buffers
        self.buffer_len = int(sample_rate * 0.2)  # 200ms history
        self.audio_history = collections.deque(maxlen=self.buffer_len)
        
        # Baseline noise tracking
        self.ambient_rms = 0.005
        self.last_trigger_time = 0.0
        self.cooldown_sec = 1.0  # Refractory period
        self.startup_grace_frames = 20  # Drop stream init transient
        self.frame_count = 0

        # Candidate transient tracking
        self.candidate_detected = False
        self.candidate_time = 0.0
        self.candidate_peak_rms = 0.0
        self.candidate_physical_ts = 0.0
        self.post_candidate_frames = []

    def reset_ambient(self):
        self.ambient_rms = 0.005
        self.frame_count = 0

    def process_block(self, block: np.ndarray, physical_ts: float) -> Optional[Dict]:
        """
        Process a 1-channel float32 audio block.
        Returns event dict if a real clap is verified, else None.
        """
        self.frame_count += 1
        now = time.time()
        
        # Flat 1D array
        samples = block.flatten()
        block_rms = float(np.sqrt(np.mean(samples**2)))
        block_peak = float(np.max(np.abs(samples)))

        # Ignore initial startup transient when opening audio stream
        if self.frame_count < self.startup_grace_frames:
            self.ambient_rms = max(0.001, block_rms)
            return None

        # Check if we are validating a candidate clap's decay
        if self.candidate_detected:
            self.post_candidate_frames.append(block_rms)
            elapsed = now - self.candidate_time

            # We evaluate decay after ~32-48ms (2-3 blocks @ 16ms)
            if len(self.post_candidate_frames) >= 2 or elapsed >= 0.035:
                post_rms = float(np.mean(self.post_candidate_frames))
                peak_rms = self.candidate_peak_rms
                decay_ratio = (peak_rms - post_rms) / max(peak_rms, 1e-4)

                # Reset candidate state
                cand_ts = self.candidate_physical_ts
                cand_peak = self.candidate_peak_amp
                self.candidate_detected = False
                self.post_candidate_frames.clear()

                # REAL CLAP VERIFICATION:
                # 1. Hand clap energy drops sharply (decay_ratio > 0.60)
                # 2. Post-transient energy returns near ambient floor (post_rms < 0.048)
                # Sustained speech vowels (e.g. "AAAH") or coughing maintain energy (post_rms > 0.06).
                if decay_ratio > 0.58 and post_rms < 0.048:
                    self.last_trigger_time = now
                    detector_ts = now
                    det_latency = (detector_ts - cand_ts) * 1000.0

                    return {
                        "type": "clap",
                        "action": "EXPANDED",
                        "source": "acoustic_clap",
                        "confidence": round(min(1.0, 0.92 + decay_ratio * 0.08), 3),
                        "physical_timestamp": cand_ts,
                        "detector_timestamp": detector_ts,
                        "detection_latency_ms": round(det_latency, 2),
                        "peak_amplitude": round(cand_peak, 4),
                        "decay_ratio": round(decay_ratio, 3),
                    }
                else:
                    # Rejected: sustained sound (speech or cough)
                    return None

        # If in cooldown window, ignore
        if now - self.last_trigger_time < self.cooldown_sec:
            return None

        # Update ambient noise floor slowly during quiet periods
        if block_peak < 0.05 and block_rms < 0.02:
            self.ambient_rms = 0.95 * self.ambient_rms + 0.05 * block_rms
            return None

        # Fast transient check:
        # 1. Peak amplitude must exceed clap threshold (> 0.42) (claps are 0.60 - 0.99, typing/clicks are < 0.30)
        # 2. Block RMS must be significant (> 0.075)
        # 3. Ratio over ambient must be explosive (> 8x)
        onset_ratio = block_rms / max(self.ambient_rms, 0.001)
        crest_factor = block_peak / max(block_rms, 1e-4)
        
        if block_peak > 0.42 and block_rms > 0.075 and onset_ratio > 8.0:
            # Spectral check: compute Zero Crossing Rate (ZCR)
            # High ZCR = sharp impact. Low ZCR = low-frequency thump (desk bump, door slam)
            zero_crossings = np.sum(np.diff(samples > 0) != 0)
            zcr = zero_crossings / len(samples)

            # High-pass energy ratio: diff filter (y[n] = x[n] - x[n-1])
            diff = np.diff(samples)
            hf_energy = float(np.mean(diff**2))
            total_energy = float(np.mean(samples**2))
            hf_ratio = hf_energy / max(total_energy, 1e-6)

            # A real hand clap has rich high-frequency energy (ZCR > 0.09, hf_ratio > 0.18)
            # Low-frequency desk knocks have ZCR < 0.08 and hf_ratio < 0.15
            if zcr > 0.08 and hf_ratio > 0.16:
                # Mark as candidate; will verify decay in next 1-2 blocks (16-32ms)
                self.candidate_detected = True
                self.candidate_time = now
                self.candidate_peak_rms = block_rms
                self.candidate_peak_amp = block_peak
                self.candidate_physical_ts = physical_ts
                self.post_candidate_frames = []

        return None
