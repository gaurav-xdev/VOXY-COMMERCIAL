use voxy_ipc::*;

#[test]
fn protocol_frame_lifecycle() {
    let frame = Frame::new(MessageType::MethodCall)
        .with_payload(b"request-data".to_vec())
        .with_request_id(uuid::Uuid::new_v4())
        .with_stream_id(1);

    assert_eq!(frame.message_type, MessageType::MethodCall);
    assert_eq!(frame.payload, b"request-data");
    assert!(frame.request_id.is_some());
    assert_eq!(frame.stream_id, Some(1));
}

#[test]
fn json_codec_roundtrip_complex_frame() {
    let codec = JsonCodec::new();
    let original = Frame::new(MessageType::StreamData)
        .with_payload(b"stream-chunk-42".to_vec())
        .with_stream_id(7)
        .with_flags(FrameFlags {
            priority_high: true,
            compressed: false,
            encrypted: false,
            requires_ack: true,
            ..Default::default()
        });

    let encoded = codec.encode(&original).unwrap();
    let decoded = codec.decode(&encoded).unwrap();

    assert_eq!(original.message_type, decoded.message_type);
    assert_eq!(original.payload, decoded.payload);
    assert_eq!(original.stream_id, decoded.stream_id);
    assert_eq!(original.flags.requires_ack, decoded.flags.requires_ack);
    assert_eq!(original.flags.priority_high, decoded.flags.priority_high);
}

#[test]
fn capability_token_full_lifecycle() {
    let claims = vec![
        CapabilityClaim {
            capability: "voice:transcribe".to_string(),
            resource: Some("english".to_string()),
            constraints: vec!["max_duration:300s".to_string()],
        },
        CapabilityClaim {
            capability: "storage:write".to_string(),
            resource: Some("namespace:user-data".to_string()),
            constraints: vec!["max_size:10MB".to_string()],
        },
    ];

    let token = CapabilityToken::new("user-42", "voxy-auth", claims, 900, vec![0xAB, 0xCD]);

    assert_eq!(token.subject, "user-42");
    assert_eq!(token.issuer, "voxy-auth");
    assert!(!token.is_expired());
    assert!(token.is_valid());
    assert!(token.has_capability("voice:transcribe"));
    assert!(token.has_capability("storage:write"));
    assert!(!token.has_capability("admin:*"));

    let remaining = token.remaining_ttl_secs();
    assert!(remaining > 0 && remaining <= 900);
}

#[test]
fn cancellation_tokens_work() {
    let token = CancellationToken::new();
    let handle = CancellationHandle::new(token.clone());

    assert!(!token.is_cancelled());
    handle.cancel();
    assert!(token.is_cancelled());
    assert!(handle.token().is_cancelled());
}

#[test]
fn version_negotiation_logic() {
    let client_version = Version::new(1, 2, 0);
    let server_version = Version::new(1, 3, 0);
    let incompatible = Version::new(2, 0, 0);

    assert!(client_version.compatible_with(&server_version));
    assert!(server_version.compatible_with(&client_version));
    assert!(!client_version.compatible_with(&incompatible));

    let negotiated = if client_version.compatible_with(&server_version) {
        server_version.clone()
    } else {
        client_version
    };
    assert_eq!(negotiated, server_version);
}

#[test]
fn replay_subscription_roundtrip() {
    let sub = ReplaySubscription {
        subscription_id: uuid::Uuid::new_v4(),
        topic: "voxy.voice.transcript".to_string(),
        mode: ReplayMode::Replay,
        replay_from: Some(chrono::Utc::now()),
        replay_to: None,
        replay_rate: Some(2.0),
        checkpoint_id: None,
        filter: serde_json::json!({}),
        delivery: DeliveryGuarantee::AtLeastOnce,
    };

    let json = serde_json::to_string(&sub).unwrap();
    let restored: ReplaySubscription = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.topic, "voxy.voice.transcript");
    assert_eq!(restored.mode, ReplayMode::Replay);
    assert_eq!(restored.replay_rate, Some(2.0));
}

#[test]
fn stream_open_full_config() {
    let mut params = std::collections::HashMap::new();
    params.insert("width".to_string(), "1920".to_string());
    params.insert("height".to_string(), "1080".to_string());
    params.insert("fps".to_string(), "60".to_string());

    let open = StreamOpen {
        stream_type: StreamType::Video,
        content_type: "video/mp4".to_string(),
        parameters: params,
    };

    assert_eq!(open.stream_type, StreamType::Video);
    assert_eq!(open.parameters.get("width").unwrap(), "1920");
    assert_eq!(open.parameters.len(), 3);
}

#[test]
fn method_call_with_idempotency() {
    let idem_key = "idem-001".to_string();
    let call = MethodCall {
        method: "device.turn_on".to_string(),
        params: serde_json::json!({"device_id": "light-1"}),
        timeout_ms: Some(10000),
        idempotency_key: Some(idem_key.clone()),
    };

    let frame = Frame::from_json(MessageType::MethodCall, &call).unwrap();
    let restored: MethodCall = frame.payload_json().unwrap();

    assert_eq!(restored.method, "device.turn_on");
    assert_eq!(restored.idempotency_key, Some(idem_key));
    assert_eq!(restored.timeout_ms, Some(10000));
}

#[test]
fn auth_method_display_all() {
    assert_eq!(AuthMethod::Token.to_string(), "token");
    assert_eq!(AuthMethod::Certificate.to_string(), "certificate");
    assert_eq!(AuthMethod::ApiKey.to_string(), "api_key");
    assert_eq!(AuthMethod::OAuth2.to_string(), "oauth2");
    assert_eq!(AuthMethod::MutualTls.to_string(), "mutual_tls");
    assert_eq!(AuthMethod::Anonymous.to_string(), "anonymous");
}

#[test]
fn frame_flags_all_bits() {
    let flags = FrameFlags {
        priority_high: true,
        priority_critical: true,
        compressed: true,
        encrypted: true,
        requires_ack: true,
        is_cancelled: true,
        is_replay: true,
        is_recovery: true,
    };
    let bits = flags.as_u16();
    let restored = FrameFlags::from_u16(bits);
    assert_eq!(flags.as_u16(), restored.as_u16());
}
