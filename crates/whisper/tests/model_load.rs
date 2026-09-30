#[cfg(feature = "whisper-engine")]
#[test]
fn test_whisper_model_load_direct() {
    let model_path = "../../models/ggml-base.en.bin";
    assert!(
        std::path::Path::new(model_path).exists(),
        "Model file not found at {model_path}"
    );

    eprintln!("Attempting direct whisper-rs model load from {model_path}...");
    let ctx = whisper_rs::WhisperContext::new_with_params(
        model_path,
        whisper_rs::WhisperContextParameters::default(),
    )
    .expect("whisper-rs failed to load model");

    let _state = ctx
        .create_state()
        .expect("whisper-rs failed to create state");
    eprintln!("Model loaded and state created successfully");
}

#[cfg(feature = "whisper-engine")]
#[test]
fn test_whisper_model_load_in_multithread_runtime() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let model_path = "../../models/ggml-base.en.bin".to_string();
    assert!(
        std::path::Path::new(&model_path).exists(),
        "Model file not found at {model_path}"
    );

    eprintln!("Attempting whisper-rs model load inside multi-threaded tokio runtime via spawn_blocking...");
    rt.block_on(async move {
        tokio::task::spawn_blocking(move || {
            eprintln!("spawn_blocking: loading model...");
            let ctx = whisper_rs::WhisperContext::new_with_params(
                &model_path,
                whisper_rs::WhisperContextParameters::default(),
            )
            .expect("whisper-rs failed to load model");

            let _state = ctx
                .create_state()
                .expect("whisper-rs failed to create state");
            eprintln!("spawn_blocking: model loaded OK");
        })
        .await
        .expect("spawn_blocking panicked");
    });
    eprintln!("Multi-thread runtime test passed!");
}

#[cfg(feature = "whisper-engine")]
#[test]
fn test_whisper_model_load_sync_on_async_thread() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let model_path = "../../models/ggml-base.en.bin".to_string();
    assert!(
        std::path::Path::new(&model_path).exists(),
        "Model file not found at {model_path}"
    );

    eprintln!("Attempting synchronous whisper-rs model load on multi-threaded runtime worker...");
    rt.block_on(async move {
        eprintln!("Loading model synchronously on async worker thread...");
        let ctx = whisper_rs::WhisperContext::new_with_params(
            &model_path,
            whisper_rs::WhisperContextParameters::default(),
        )
        .expect("whisper-rs failed to load model");

        let _state = ctx
            .create_state()
            .expect("whisper-rs failed to create state");
        eprintln!("Model loaded OK synchronously on async worker thread");
    });
    eprintln!("Sync in multi-thread runtime test passed!");
}
