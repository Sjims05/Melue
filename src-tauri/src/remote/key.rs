//! The key that makes your devices yours: a random secret in `.melue/remote-key` on the share (made by the first
//! device that needs it). Only devices with the share can read it, so only they can play on each other. A request
//! carries proof of it: who sends it, when, and a signature of both and the request with the key (HMAC-SHA256), so the
//! key itself never goes over the network. A proof is good for a few minutes (devices' clocks differ a little).

use std::sync::Mutex;

use hmac::{Hmac, Mac};
use sha2::Sha256;
use tauri::AppHandle;

use crate::share::{Share, ShareError};

const FILE: &str = ".melue/remote-key";
/// How old (or how far ahead) a proof may be.
const GOOD_FOR_MS: i64 = 5 * 60_000;

/// The key, once read (it doesn't change).
static KEY: Mutex<Option<Vec<u8>>> = Mutex::new(None);

/// The key from the share, made there if there's none yet.
pub fn read_or_make(share: &dyn Share) -> Result<Vec<u8>, String> {
    match share.read(FILE) {
        Ok(text) => {
            let key = hex::decode(String::from_utf8_lossy(&text).trim()).map_err(|e| format!("the share's remote key is damaged: {e}"))?;
            if key.len() >= 32 {
                return Ok(key);
            }
            Err("the share's remote key is too short".into())
        }
        Err(ShareError::NotFound { .. }) => {
            let mut key = vec![0u8; 32];
            getrandom::getrandom(&mut key).map_err(|e| e.to_string())?;
            share.create_dir_all(".melue")?;
            share.write(FILE, hex::encode(&key).as_bytes())?;
            Ok(key)
        }
        Err(e) => Err(e.to_string()),
    }
}

/// The key (read from the share the first time).
pub fn key(app: &AppHandle) -> Result<Vec<u8>, String> {
    if let Some(key) = KEY.lock().unwrap().clone() {
        return Ok(key);
    }
    let key = crate::sync::shared::with_share(app, |share| read_or_make(share))?;
    *KEY.lock().unwrap() = Some(key.clone());
    Ok(key)
}

fn signature(key: &[u8], device: &str, at_ms: i64, request: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("any key length");
    mac.update(format!("{device}\n{at_ms}\n{request}").as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// The proof for `request` (e.g. "GET /ping") from `device` at `at_ms`: "device.time.signature".
pub fn proof(key: &[u8], device: &str, at_ms: i64, request: &str) -> String {
    format!("{device}.{at_ms}.{}", signature(key, device, at_ms, request))
}

/// The device a proof comes from, when it's good for `request` at `now_ms`.
pub fn check(key: &[u8], proof: &str, request: &str, now_ms: i64) -> Option<String> {
    let mut parts = proof.rsplitn(3, '.');
    let (given, at, device) = (parts.next()?, parts.next()?.parse::<i64>().ok()?, parts.next()?);
    if (now_ms - at).abs() > GOOD_FOR_MS {
        return None;
    }
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("any key length");
    mac.update(format!("{device}\n{at}\n{request}").as_bytes());
    // Compared in constant time.
    mac.verify_slice(&hex::decode(given).ok()?).ok()?;
    Some(device.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::share::testkit::TestShare;

    #[test]
    fn one_key_on_the_share_for_every_device() {
        let share = TestShare::new("remote-key");
        let first = read_or_make(&share.share).unwrap();
        assert_eq!(first.len(), 32);
        assert_eq!(read_or_make(&share.share).unwrap(), first, "the second device reads the same one");
    }

    #[test]
    fn a_proof_is_good_only_with_the_key_for_its_request_and_for_a_while() {
        let key = [7u8; 32];
        let proof = proof(&key, "phone-1", 1_000_000, "GET /ping");
        assert_eq!(check(&key, &proof, "GET /ping", 1_000_000 + 60_000).as_deref(), Some("phone-1"));
        assert_eq!(check(&[8u8; 32], &proof, "GET /ping", 1_000_000), None, "another key");
        assert_eq!(check(&key, &proof, "POST /play", 1_000_000), None, "another request");
        assert_eq!(check(&key, &proof, "GET /ping", 1_000_000 + 10 * 60_000), None, "too old");
        assert_eq!(check(&key, &proof.replace("phone-1", "laptop"), "GET /ping", 1_000_000), None, "another device");
    }
}
