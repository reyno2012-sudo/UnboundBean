use aes::Aes256;
use ctr::cipher::{KeyIvInit, StreamCipher};
use hkdf::Hkdf;
use sha2::Sha256;

type Aes256Ctr = ctr::Ctr128BE<Aes256>;
const HKDF_CONTEXT: &[u8] = &[1, 2, 3];
const FILE_KEY_MAGIC: &[u8] = b"soundcored3200";

pub fn derive_session_key(shared_secret: &[u8]) -> Result<[u8; 32], String> {
    if shared_secret.is_empty() {
        return Err("ECDH shared secret is empty".to_string());
    }
    let hkdf = Hkdf::<Sha256>::new(Some(HKDF_CONTEXT), shared_secret);
    let mut session_key = [0u8; 32];
    hkdf.expand(HKDF_CONTEXT, &mut session_key)
        .map_err(|_| "HKDF-SHA256 session key derivation failed".to_string())?;
    Ok(session_key)
}

pub fn unwrap_file_key(
    session_key: &[u8; 32],
    session_nonce: &[u8; 16],
    encrypted_key: &[u8],
) -> Result<[u8; 32], String> {
    if encrypted_key.len() != 46 {
        return Err(format!(
            "wrapped file key must be 46 bytes, got {}",
            encrypted_key.len()
        ));
    }
    let mut material = encrypted_key.to_vec();
    let mut cipher = Aes256Ctr::new(session_key.into(), session_nonce.into());
    cipher.apply_keystream(&mut material);
    if !material.starts_with(FILE_KEY_MAGIC) {
        return Err("wrapped file key is missing soundcored3200 magic".to_string());
    }
    material[FILE_KEY_MAGIC.len()..]
        .try_into()
        .map_err(|_| "wrapped file key payload is not 32 bytes".to_string())
}

pub fn decrypt_audio_chunk(
    encrypted_data: &[u8],
    file_key: &[u8; 32],
    audio_nonce: &[u8; 16],
    sequence: u32,
) -> Result<Vec<u8>, String> {
    if encrypted_data.len() != 160 {
        return Err(format!(
            "audio chunk must be exactly 160 bytes, got {}",
            encrypted_data.len()
        ));
    }
    let mut counter = [0u8; 16];
    counter[..12].copy_from_slice(&audio_nonce[..12]);
    counter[12..].copy_from_slice(&sequence.wrapping_mul(10).to_be_bytes());

    let mut decrypted = encrypted_data.to_vec();
    let mut cipher = Aes256Ctr::new(file_key.into(), (&counter).into());
    cipher.apply_keystream(&mut decrypted);
    Ok(decrypted)
}
