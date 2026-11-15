use unboundbean_lib::{ai, ble, crypto, protocol, wifi};

#[test]
fn command_frame_contains_length_and_checksum() {
    let frame = protocol::build_command(0x18, 0x82, &[0x01]);
    assert_eq!(
        frame,
        vec![0x08, 0xEE, 0, 0, 0, 0x18, 0x82, 0x0B, 0x00, 0x01, 0x9C]
    );
}

#[test]
fn response_decoder_reassembles_split_notifications() {
    let response = vec![
        0x09, 0xFF, 0, 0, 0x01, 0x01, 0x03, 0x0C, 0, 0x05, 0x07, 0x25,
    ];
    let mut decoder = protocol::FrameDecoder::default();
    assert!(decoder.push(&response[..5]).is_empty());
    let frames = decoder.push(&response[5..]);
    assert_eq!(frames, vec![response]);
}

#[test]
fn response_decoder_preserves_a_magic_prefix_split_between_notifications() {
    let response = vec![
        0x09, 0xFF, 0, 0, 0x01, 0x01, 0x03, 0x0C, 0, 0x05, 0x07, 0x25,
    ];
    let mut decoder = protocol::FrameDecoder::default();
    assert!(decoder.push(&response[..2]).is_empty());
    assert_eq!(decoder.push(&response[2..]), vec![response]);
}

#[test]
fn response_decoder_discards_invalid_checksum() {
    let mut decoder = protocol::FrameDecoder::default();
    let frames = decoder.push(&[0x09, 0xFF, 0, 0, 1, 1, 1, 10, 0, 0]);
    assert!(frames.is_empty());
}

#[test]
fn hkdf_matches_independent_sha256_vector() {
    let key = crypto::derive_session_key(&[0x42; 32]).expect("HKDF should succeed");
    assert_eq!(
        hex::encode(key),
        "fd75e43fce204bcc94897f460ba3e088f5414ce47d0f630a9478af494a435dbc"
    );
}

#[test]
fn audio_chunk_decryption_matches_aes_256_ctr_vector() {
    let file_key: [u8; 32] = std::array::from_fn(|index| index as u8);
    let nonce: [u8; 16] = std::array::from_fn(|index| index as u8);
    let encrypted = hex::decode("b2a2b96214cd444e57345061bf6f68271590f11cbaa436291ad0f4e476465a1f396c281737d8d426f0c2a02d528e13f76dfa272842b99c8d5414437b7bf1fc28252d030bb783658c33b8369abd10775e74d8ac888e8ee9a15928445c45e4b90892b4cea324044c81c852f94253dae7af65d1a7064e0209c0ab8a13c4d30ce01858de18fca062fb6ef08784756641496bdb318a38378aabb29f89361d4696a60e").unwrap();
    let decrypted = crypto::decrypt_audio_chunk(&encrypted, &file_key, &nonce, 7).unwrap();
    assert_eq!(decrypted, (0u8..160).collect::<Vec<_>>());
}

#[test]
fn wrapped_file_key_requires_soundcore_magic() {
    let error = crypto::unwrap_file_key(&[0; 32], &[0; 16], &[0; 46]).unwrap_err();
    assert!(error.contains("soundcored3200"));
}

#[test]
fn recording_list_parser_reads_protocol_entries() {
    let mut frame = vec![0x09, 0xFF, 0, 0, 1, 0x1A, 0x0E, 0, 0];
    frame.extend_from_slice(&2u16.to_le_bytes());
    frame.extend_from_slice(&1234u32.to_le_bytes());
    frame.extend_from_slice(&200u32.to_le_bytes());
    frame.extend_from_slice(&5678u32.to_le_bytes());
    frame.extend_from_slice(&400u32.to_le_bytes());
    frame.extend_from_slice(&0u32.to_le_bytes());
    frame.extend_from_slice(&0i32.to_le_bytes());
    frame.push(0);
    let length = frame.len() as u16;
    frame[7..9].copy_from_slice(&length.to_le_bytes());
    let checksum = protocol::checksum(&frame[..frame.len() - 1]);
    *frame.last_mut().unwrap() = checksum;

    let recordings = ble::parse_recording_list(&frame).unwrap();
    assert_eq!(recordings.len(), 2);
    assert_eq!(recordings[0].file_id, 1234);
    assert_eq!(recordings[1].size_units, 400);
}

#[test]
fn wifi_response_reverses_device_ip_bytes() {
    let mut frame = vec![0x09, 0xFF, 0, 0, 1, 0x1A, 0x05, 16, 0];
    frame.extend_from_slice(&[1, 4, 168, 192]);
    frame.extend_from_slice(&443u16.to_le_bytes());
    frame.push(0);
    let checksum = protocol::checksum(&frame[..frame.len() - 1]);
    *frame.last_mut().unwrap() = checksum;
    let (host, port) = ble::parse_wifi_response(&frame).unwrap();
    assert_eq!(host, "192.168.4.1");
    assert_eq!(port, 443);
}

#[test]
fn encrypted_file_head_rejects_a_different_recording() {
    let mut frame = vec![0u8; 97];
    frame[0..4].copy_from_slice(&[0x09, 0xFF, 0, 0]);
    frame[5] = 0x1A;
    frame[6] = 0x07;
    frame[9..13].copy_from_slice(&99u32.to_le_bytes());
    let error = wifi::parse_encrypted_file_head(&frame, 42).unwrap_err();
    assert!(error.contains("requested 42"));
}

#[test]
fn ai_response_parser_returns_first_message_content() {
    let body = serde_json::json!({
        "choices": [{"message": {"content": "# 会议摘要"}}]
    });
    assert_eq!(ai::parse_summary_response(&body).unwrap(), "# 会议摘要");
}
