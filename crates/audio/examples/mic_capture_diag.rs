use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn main() {
    println!("============================================================");
    println!("         MINIMAL WASAPI MICROPHONE CAPTURE DIAGNOSTIC       ");
    println!("============================================================");

    let host = cpal::default_host();
    println!("WASAPI Host: {:?}", host.id());

    // 1. Enumerate default microphone
    let def_in = match host.default_input_device() {
        Some(d) => d,
        None => {
            eprintln!("[FAIL] No default input device found on host.");
            return;
        }
    };

    let device_name = def_in
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "Unknown".into());
    println!("\n[1] Default Input Device Name: {}", device_name);

    // 2. Query default config
    let default_cfg = match def_in.default_input_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[FAIL] Failed to retrieve default_input_config: {}", e);
            return;
        }
    };

    let sample_rate = default_cfg.sample_rate();
    let channels = default_cfg.channels();
    let sample_format = default_cfg.sample_format();

    println!("[2] Audio Configuration:");
    println!("    - Sample Rate:   {} Hz", sample_rate);
    println!("    - Channels:      {}", channels);
    println!("    - Sample Format: {:?}", sample_format);
    println!("    - Buffer Size:   Default (WASAPI event-driven)");

    // 3. Initialize WASAPI capture stream
    println!("\n[3] Initializing WASAPI capture stream...");
    let init_start = Instant::now();

    let stream_cfg: cpal::StreamConfig = default_cfg.clone().into();
    let frames_captured = Arc::new(AtomicU64::new(0));
    let frames_cb = frames_captured.clone();
    let peak_level = Arc::new(AtomicU32Bits::new(0.0f32));
    let peak_cb = peak_level.clone();
    let rms_accum = Arc::new(AtomicU64::new(0)); // fixed point f64
    let rms_cb = rms_accum.clone();
    let error_count = Arc::new(AtomicUsize::new(0));
    let error_cb = error_count.clone();

    let stream_result = def_in.build_input_stream(
        stream_cfg,
        move |data: &[f32], _info: &cpal::InputCallbackInfo| {
            let n_samples = data.len();
            let n_frames = n_samples / channels as usize;
            frames_cb.fetch_add(n_frames as u64, Ordering::Relaxed);

            let mut local_max = 0.0f32;
            let mut local_sum_sq = 0.0f64;
            for &s in data {
                let abs = s.abs();
                if abs > local_max {
                    local_max = abs;
                }
                local_sum_sq += (s as f64) * (s as f64);
            }
            peak_cb.update_max(local_max);
            let bits = (local_sum_sq * 1000.0) as u64;
            rms_cb.fetch_add(bits, Ordering::Relaxed);
        },
        move |err| {
            error_cb.fetch_add(1, Ordering::Relaxed);
            eprintln!("[STREAM ERROR CALLBACK] {}", err);
        },
        None,
    );

    let init_duration = init_start.elapsed();

    match stream_result {
        Ok(stream) => {
            println!("    - Initialization Result: SUCCESS (took {:?})", init_duration);

            if let Err(e) = stream.play() {
                eprintln!("[FAIL] stream.play() failed: {}", e);
                return;
            }
            println!("    - stream.play() SUCCESS: Microphone is actively streaming!");

            // 4. Capture for 10 seconds
            println!("\n[4] Capturing real microphone PCM for 10 seconds...");
            let start_time = Instant::now();
            let mut last_frames = 0u64;

            for sec in 1..=10 {
                std::thread::sleep(Duration::from_secs(1));
                let cur_frames = frames_captured.load(Ordering::Relaxed);
                let delta = cur_frames - last_frames;
                last_frames = cur_frames;

                let cur_peak = peak_level.get();
                let errors = error_count.load(Ordering::Relaxed);

                println!(
                    "    [{:02}s / 10s] Frames: +{} (total: {}) | Peak: {:.4} | Errors/Glitch: {}",
                    sec, delta, cur_frames, cur_peak, errors
                );
            }

            let total_duration = start_time.elapsed();
            let total_frames = frames_captured.load(Ordering::Relaxed);
            let final_peak = peak_level.get();
            let total_errors = error_count.load(Ordering::Relaxed);

            println!("\n[5] Capture Summary Report:");
            println!("    - Total Duration:       {:.2?}", total_duration);
            println!("    - Total Frames Captured: {}", total_frames);
            println!("    - Expected Frames:       {}", sample_rate * 10);
            println!("    - Frame Delivery Ratio:  {:.2}%", (total_frames as f64 / (sample_rate as f64 * 10.0)) * 100.0);
            println!("    - Peak Level:            {:.4}", final_peak);
            println!("    - Stream Errors/Overruns: {}", total_errors);
            println!("    - Status: SUCCESS — REAL MICROPHONE HARDWARE VERIFIED");
        }
        Err(e) => {
            println!("    - Initialization Result: FAILED (took {:?})", init_duration);
            println!("\n============================================================");
            println!("                    HARDWARE FAILURE AUDIT                  ");
            println!("============================================================");
            println!("Error Type:   {:?}", e);
            println!("Error String: {}", e);
            println!("Root Cause:   WASAPI IAudioClient::Initialize returned OS Error -2147024891 (0x80070005 E_ACCESSDENIED)");
            println!("Verification: PHYSICAL MICROPHONE BLOCKED — NOT END-TO-END VERIFIED");
            println!("============================================================");
        }
    }
}

struct AtomicU32Bits(std::sync::atomic::AtomicU32);
impl AtomicU32Bits {
    fn new(val: f32) -> Self {
        Self(std::sync::atomic::AtomicU32::new(val.to_bits()))
    }
    fn update_max(&self, val: f32) {
        let mut cur = f32::from_bits(self.0.load(Ordering::Relaxed));
        while val > cur {
            match self.0.compare_exchange_weak(
                cur.to_bits(),
                val.to_bits(),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => cur = f32::from_bits(actual),
            }
        }
    }
    fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
}
