//! WhatsApp media: key derivation, AES-256-CBC encrypt/decrypt, and the
//! HTTP blob transport (download from / upload to WhatsApp's MMX blob store).
//!
//! Mirrors whatsmeow's `util/media` + `upload/download` logic:
//!     * keys:  HKDF-SHA256(mediaKey, "", "WhatsApp <Type> Keys", 112)
//!             → [iv 16][cipherKey 32][macKey 32][refKey 32]
//!     * on disk: ciphertext ‖ HMAC-SHA256(macKey, iv‖ct)[..10]
//!     * encryption: AES-256-CBC with PKCS#7, deterministic IV from the key.

use base64::Engine as _;
use hmac::{Hmac, Mac as _};
use sha2::{Digest as _, Sha256};

pub const MT_IMAGE: &str = "WhatsApp Image Keys";
pub const MT_VIDEO: &str = "WhatsApp Video Keys";
pub const MT_AUDIO: &str = "WhatsApp Audio Keys";
pub const MT_DOCUMENT: &str = "WhatsApp Document Keys";
pub const MT_HISTORY: &str = "WhatsApp History Keys";
pub const MT_APP_STATE: &str = "WhatsApp App State Keys";
pub const MT_STICKER_PACK: &str = "WhatsApp Sticker Pack Keys";

const MAC_LEN: usize = 10;

pub struct MediaKeys {
    pub iv: [u8; 16],
    pub cipher_key: [u8; 32],
    pub mac_key: [u8; 32],
}

pub fn media_keys(media_key: &[u8], app_info: &[u8]) -> MediaKeys {
    let expanded = crate::wa2::hkdf_sha256(&[], media_key, app_info, 112);
    let mut mk = MediaKeys {
        iv: [0u8; 16],
        cipher_key: [0u8; 32],
        mac_key: [0u8; 32],
    };
    mk.iv.copy_from_slice(&expanded[0..16]);
    mk.cipher_key.copy_from_slice(&expanded[16..48]);
    mk.mac_key.copy_from_slice(&expanded[48..80]);
    mk
}

fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

/// PKCS#7 pad (block 16).
fn pkcs7_pad(plaintext: &[u8]) -> Vec<u8> {
    let n = 16 - (plaintext.len() % 16);
    let mut out = plaintext.to_vec();
    out.extend(std::iter::repeat_n(n as u8, n));
    out
}

fn pkcs7_unpad(data: &[u8]) -> Result<&[u8], String> {
    let Some((&last, _)) = data.split_last() else {
        return Err("empty ciphertext".into());
    };
    let n = last as usize;
    if !(1..=16).contains(&n) || n > data.len() {
        return Err("invalid pkcs7 padding".into());
    }
    for b in &data[data.len() - n..] {
        if *b != last {
            return Err("invalid pkcs7 padding".into());
        }
    }
    Ok(&data[..data.len() - n])
}

fn aes_cbc_encrypt(key: &[u8; 32], iv: &[u8; 16], plaintext: &[u8]) -> Vec<u8> {
    use aes::cipher::{BlockEncrypt, KeyInit as _};
    let cipher = aes::Aes256::new_from_slice(key).expect("32-byte key");
    let padded = pkcs7_pad(plaintext);
    let mut out = Vec::with_capacity(padded.len());
    let mut prev = *iv;
    for chunk in padded.chunks(16) {
        let mut block: [u8; 16] = chunk.try_into().expect("16-byte block");
        for i in 0..16 {
            block[i] ^= prev[i];
        }
        cipher.encrypt_block((&mut block).into());
        out.extend_from_slice(&block);
        prev = block;
    }
    out
}

fn aes_cbc_decrypt(key: &[u8; 32], iv: &[u8; 16], ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    use aes::cipher::{BlockDecrypt, KeyInit as _};
    if ciphertext.is_empty() || !ciphertext.len().is_multiple_of(16) {
        return Err("ciphertext length must be a multiple of 16".into());
    }
    let cipher = aes::Aes256::new_from_slice(key).expect("32-byte key");
    let mut out = Vec::with_capacity(ciphertext.len());
    let mut prev = *iv;
    for chunk in ciphertext.chunks(16) {
        let mut block: [u8; 16] = chunk.try_into().expect("16-byte block");
        cipher.decrypt_block((&mut block).into());
        for i in 0..16 {
            block[i] ^= prev[i];
        }
        out.extend_from_slice(&block);
        prev.copy_from_slice(chunk);
    }
    Ok(pkcs7_unpad(&out)?.to_vec())
}

/// Prepare an attachment for upload: encrypt `plaintext` and return
/// `ciphertext ‖ mac`, plus the two hashes that go into the message proto.
pub struct EncryptedMedia {
    pub data: Vec<u8>,
    pub file_sha256: [u8; 32],
    pub file_enc_sha256: [u8; 32],
    pub file_length: u64,
}

pub fn encrypt_media(plaintext: &[u8], media_key: &[u8; 32], app_info: &[u8]) -> EncryptedMedia {
    let k = media_keys(media_key, app_info);
    let ct = aes_cbc_encrypt(&k.cipher_key, &k.iv, plaintext);
    let mut h = Hmac::<Sha256>::new_from_slice(&k.mac_key).expect("hmac key");
    h.update(&k.iv);
    h.update(&ct);
    let mac = &h.finalize().into_bytes()[..MAC_LEN];
    let mut data = ct;
    data.extend_from_slice(mac);
    EncryptedMedia {
        file_sha256: sha256(plaintext),
        file_enc_sha256: sha256(&data),
        file_length: plaintext.len() as u64,
        data,
    }
}

/// Decrypt the blob fetched from the server. Verifies the 10-byte mac and
/// (when provided) the encrypted/plaintext sha256 fingerprints.
pub fn decrypt_media(
    file: &[u8],
    media_key: &[u8; 32],
    app_info: &[u8],
    file_enc_sha256: Option<&[u8]>,
    file_sha256: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
    if file.len() <= MAC_LEN {
        return Err("media file too short".into());
    }
    if let Some(e) = file_enc_sha256 {
        if e.len() != 32 || sha256(file) != <[u8; 32]>::try_from(e).unwrap() {
            return Err("media file sha256 mismatch".into());
        }
    }
    let (ct_bytes, mac) = file.split_at(file.len() - MAC_LEN);
    let k = media_keys(media_key, app_info);
    let mut h = Hmac::<Sha256>::new_from_slice(&k.mac_key).expect("hmac key");
    h.update(&k.iv);
    h.update(ct_bytes);
    let expected = &h.finalize().into_bytes()[..MAC_LEN];
    if !mac.eq(expected) {
        return Err("media mac mismatch".into());
    }
    let plaintext = aes_cbc_decrypt(&k.cipher_key, &k.iv, ct_bytes)?;
    if let Some(f) = file_sha256 {
        if f.len() != 32 || sha256(&plaintext) != <[u8; 32]>::try_from(f).unwrap() {
            return Err("media plaintext sha256 mismatch".into());
        }
    }
    Ok(plaintext)
}

/// The `mms-type` query value for a media kind (`image`, `video`, `audio`,
/// `document`, ...) used in both download and upload URLs.
pub fn mms_type(app_info: &str) -> &'static str {
    match app_info {
        MT_IMAGE => "image",
        MT_VIDEO => "video",
        MT_AUDIO => "audio",
        MT_DOCUMENT => "document",
        MT_HISTORY => "md-msg-hist",
        MT_APP_STATE => "md-app-state",
        MT_STICKER_PACK => "sticker-pack",
        _ => "image",
    }
}

fn http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .user_agent("WhatsApp/2.25.1 (zen native)")
        .build()
        .map_err(|e| format!("http client: {e}"))
}

/// Fetch a media blob from `url`.
pub fn download(url: &str) -> Result<Vec<u8>, String> {
    let resp = http_client()?
        .get(url)
        .send()
        .map_err(|e| format!("media download {url}: {e}"))?;
    let status = resp.status();
    let body = resp
        .bytes()
        .map_err(|e| format!("media download body: {e}"))?
        .to_vec();
    if !status.is_success() {
        return Err(format!("media download failed with HTTP {status}"));
    }
    Ok(body)
}

/// Result of the MMX upload; the URL/direct path go into the message proto.
pub struct UploadResponse {
    pub url: String,
    pub direct_path: String,
}

/// Upload `data` (ciphertext‖mac) to the blob store.
pub fn upload(
    host: &str,
    auth: &str,
    mms_type: &str,
    token: &str,
    data: &[u8],
) -> Result<UploadResponse, String> {
    let upload_url = format!(
        "https://{host}/mms/{mms_type}/{token}?auth={}&token={token}",
        urlencode(auth)
    );
    let resp = http_client()?
        .post(&upload_url)
        .header("Origin", "https://web.whatsapp.com")
        .header("Referer", "https://web.whatsapp.com/")
        .header("Content-Length", data.len().to_string())
        .body(data.to_vec())
        .send()
        .map_err(|e| format!("media upload {upload_url}: {e}"))?;
    let status = resp.status();
    let body = resp
        .bytes()
        .map_err(|e| format!("media upload body: {e}"))?
        .to_vec();
    if !status.is_success() {
        return Err(format!("media upload failed with HTTP {status}"));
    }
    let v: serde_json::Value =
        serde_json::from_slice(&body).map_err(|e| format!("media upload response parse: {e}"))?;
    Ok(UploadResponse {
        url: v["url"].as_str().unwrap_or_default().to_string(),
        direct_path: v["direct_path"].as_str().unwrap_or_default().to_string(),
    })
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// URL-safe base64 (with padding) of a hash — matches whatsmeow's
/// `base64.URLEncoding` used for the `hash=` and upload `token`.
pub fn b64url(data: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE.encode(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic media key: 32 bytes 0x42.
    fn key() -> [u8; 32] {
        [0x42; 32]
    }

    #[test]
    fn encrypt_then_decrypt_round_trips() {
        let plaintext = b"hello zen media".repeat(20);
        let enc = encrypt_media(&plaintext, &key(), MT_IMAGE.as_bytes());
        assert_eq!(enc.file_length, plaintext.len() as u64);
        let ct_len = plaintext.len().div_ceil(16) * 16;
        assert_eq!(enc.data.len(), ct_len + MAC_LEN);
        let dec = decrypt_media(
            &enc.data,
            &key(),
            MT_IMAGE.as_bytes(),
            Some(&enc.file_enc_sha256),
            Some(&enc.file_sha256),
        )
        .unwrap();
        assert_eq!(dec, plaintext);
    }

    #[test]
    fn mac_is_detected_when_tampered() {
        let plaintext = b"integrity check";
        let enc = encrypt_media(plaintext, &key(), MT_IMAGE.as_bytes());
        let mut data = enc.data;
        data[0] ^= 0xff;
        assert!(decrypt_media(&data, &key(), MT_IMAGE.as_bytes(), None, None).is_err());
    }

    #[test]
    fn media_keys_match_known_partition_layout() {
        let k = media_keys(&key(), MT_IMAGE.as_bytes());
        assert_eq!(k.iv.len(), 16);
        assert_eq!(k.cipher_key.len(), 32);
        assert_eq!(k.mac_key.len(), 32);
        // The full 112-byte expansion equals IV‖cipherKey‖macKey‖ref.
        let expanded = crate::wa2::hkdf_sha256(&[], &key(), MT_IMAGE.as_bytes(), 112);
        assert_eq!(&expanded[0..16], &k.iv);
        assert_eq!(&expanded[16..48], &k.cipher_key);
        assert_eq!(&expanded[48..80], &k.mac_key);
    }

    #[test]
    fn key_derivation_is_deterministic() {
        let a = media_keys(&key(), MT_IMAGE.as_bytes());
        let b = media_keys(&key(), MT_IMAGE.as_bytes());
        assert_eq!(a.iv, b.iv);
        assert_eq!(a.cipher_key, b.cipher_key);
        // Different appInfo → different keys.
        let c = media_keys(&key(), MT_AUDIO.as_bytes());
        assert_ne!(a.cipher_key, c.cipher_key);
        assert_ne!(a.iv, c.iv);
    }

    #[test]
    fn mms_type_maps() {
        assert_eq!(mms_type(MT_IMAGE), "image");
        assert_eq!(mms_type(MT_VIDEO), "video");
        assert_eq!(mms_type(MT_AUDIO), "audio");
        assert_eq!(mms_type(MT_DOCUMENT), "document");
    }

    #[test]
    fn b64url_is_padded() {
        // Go's base64.URLEncoding (whatsmeow) keeps `=` padding.
        assert_eq!(b64url(&[0u8; 2]), "AAA=");
        assert_eq!(b64url(&[0u8; 1]), "AA==");
    }
}