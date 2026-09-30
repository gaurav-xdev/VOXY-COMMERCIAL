use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

fn main() {
    let host = cpal::default_host();
    println!("=== CPAL Audio Diagnostics ===");
    println!("Default Host: {:?}", host.id());

    println!("\n--- Output Devices (Speakers) ---");
    if let Ok(devices) = host.output_devices() {
        for (i, d) in devices.enumerate() {
            let name = d
                .description()
                .map(|desc| desc.name().to_string())
                .unwrap_or_else(|_| "Unknown".into());
            println!("  [{}] {}", i, name);
        }
    }

    if let Some(def_out) = host.default_output_device() {
        let name = def_out
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| "Unknown".into());
        println!("Default Output: {}", name);
        if let Ok(cfg) = def_out.default_output_config() {
            println!(
                "  Default Config: {}Hz, {}ch, format: {:?}",
                cfg.sample_rate(),
                cfg.channels(),
                cfg.sample_format()
            );
            println!("  Testing speaker playback (tone)...");
            let sample_rate = cfg.sample_rate();
            let channels = cfg.channels() as usize;
            let mut sample_clock = 0f32;
            let stream_cfg: cpal::StreamConfig = cfg.into();
            let stream = def_out.build_output_stream(
                stream_cfg,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    for frame in data.chunks_mut(channels) {
                        let value = (sample_clock * 440.0 * 2.0 * std::f32::consts::PI
                            / sample_rate as f32)
                            .sin()
                            * 0.15;
                        sample_clock += 1.0;
                        for sample in frame.iter_mut() {
                            *sample = value;
                        }
                    }
                },
                |err| eprintln!("Output stream error: {}", err),
                None,
            );
            match stream {
                Ok(s) => {
                    if let Ok(()) = s.play() {
                        println!("  -> Speaker playback stream started successfully!");
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        println!("  -> Speaker playback verified!");
                    } else {
                        println!("  -> Speaker play() failed.");
                    }
                }
                Err(e) => println!("  -> Speaker build_output_stream failed: {}", e),
            }
        }
    } else {
        println!("No default output device found.");
    }

    println!("\n--- Input Devices (Microphones) ---");
    if let Ok(devices) = host.input_devices() {
        for (i, d) in devices.enumerate() {
            let name = d
                .description()
                .map(|desc| desc.name().to_string())
                .unwrap_or_else(|_| "Unknown".into());
            println!("  [{}] {}", i, name);
        }
    }

    if let Some(def_in) = host.default_input_device() {
        let name = def_in
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| "Unknown".into());
        println!("Default Input: {}", name);
        if let Ok(cfg) = def_in.default_input_config() {
            println!(
                "  Default Config: {}Hz, {}ch, format: {:?}",
                cfg.sample_rate(),
                cfg.channels(),
                cfg.sample_format()
            );
            println!("  Testing microphone capture with default_input_config...");
            let stream_cfg: cpal::StreamConfig = cfg.into();
            let stream = def_in.build_input_stream(
                stream_cfg,
                |_data: &[f32], _: &cpal::InputCallbackInfo| {},
                |err| eprintln!("Input stream error: {}", err),
                None,
            );
            match stream {
                Ok(s) => {
                    if let Ok(()) = s.play() {
                        println!("  -> Microphone capture stream started successfully with default config!");
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        println!("  -> Microphone capture verified!");
                    } else {
                        println!("  -> Microphone play() failed.");
                    }
                }
                Err(e) => println!(
                    "  -> Microphone build_input_stream (default config) failed: {}",
                    e
                ),
            }
        } else {
            println!("  Failed to get default_input_config");
        }
    } else {
        println!("No default input device found.");
    }
}
