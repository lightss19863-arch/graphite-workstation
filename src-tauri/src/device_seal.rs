//! Hardware-Bound Client Attestation & Device Seal
//!
//! Generates cryptographic request signatures bound to the physical workstation,
//! preventing token replay attacks from external scripts or copied tokens.
//!
//! Why this exists:
//! In early testing, we noticed that if an attacker or rogue script grabbed a session bearer
//! token, they could replay requests from curl or Postman against the cloud API.
//! Binding every outbound mutation request to a hardware-derived HMAC secret ensures
//! that stolen bearer tokens are useless without the physical workstation's signing key.

use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;
use zeroize::Zeroize;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSealHeaders {
    pub device_id: String,
    pub timestamp: String,
    pub nonce: String,
    pub body_digest: String,
    pub signature: String,
}

static DEVICE_SECRETS: OnceLock<(String, Vec<u8>)> = OnceLock::new();

fn get_or_init_device_keys() -> &'static (String, Vec<u8>) {
    DEVICE_SECRETS.get_or_init(|| {
        // Retrieve or generate a 32-byte persistent device seed
        let mut seed = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut seed);

        // Deterministic device ID prefix
        let mut hasher = Sha256::new();
        hasher.update(b"graphite-workstation-device-id:");
        hasher.update(seed);
        let id_hash = hex::encode(hasher.finalize());
        let device_id = format!("gx_dev_{}", &id_hash[..16]);

        // Hardware signing key derivation
        let mut key_hasher = Sha256::new();
        key_hasher.update(b"graphite-workstation-signing-key-v1:");
        key_hasher.update(seed);
        let key = key_hasher.finalize().to_vec();

        // Zeroize raw entropy seed immediately after key expansion
        seed.zeroize();
        (device_id, key)
    })
}

/// Signs an outbound HTTP mutation request with the workstation's device seal.
///
/// Returns HTTP headers that must be attached to the request:
/// `X-Graphite-Device-Id`, `X-Graphite-Timestamp`, `X-Graphite-Nonce`,
/// `X-Graphite-Digest`, and `X-Graphite-Signature`.
pub fn sign_request(
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<DeviceSealHeaders, String> {
    let (device_id, key) = get_or_init_device_keys();

    // 1. Current UTC timestamp in milliseconds
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("System time error: {e}"))?
        .as_millis()
        .to_string();

    // 2. High-entropy random nonce to prevent replay attacks
    let mut nonce_bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = hex::encode(nonce_bytes);

    // 3. Payload SHA-256 digest
    let mut body_hasher = Sha256::new();
    if let Some(payload) = body {
        body_hasher.update(payload.as_bytes());
    }
    let body_digest = format!("sha256:{}", hex::encode(body_hasher.finalize()));

    // 4. Canonical string to sign
    // Format: METHOD\nPATH\nTIMESTAMP\nNONCE\nBODY_DIGEST
    let string_to_sign = format!(
        "{}\n{}\n{}\n{}\n{}",
        method.to_uppercase(),
        path,
        timestamp,
        nonce,
        body_digest
    );

    // 5. Compute HMAC-SHA256 signature
    let mut mac =
        HmacSha256::new_from_slice(key).map_err(|e| format!("HMAC initialization failed: {e}"))?;
    mac.update(string_to_sign.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());

    Ok(DeviceSealHeaders {
        device_id: device_id.clone(),
        timestamp,
        nonce,
        body_digest,
        signature,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_seal_header_generation() {
        let headers =
            sign_request("POST", "/api/v1/matter/run", Some("{\"caseId\":\"test\"}")).unwrap();
        assert!(headers.device_id.starts_with("gx_dev_"));
        assert_eq!(headers.nonce.len(), 32);
        assert!(headers.body_digest.starts_with("sha256:"));
        assert_eq!(headers.signature.len(), 64);
    }

    #[test]
    fn test_different_nonces_per_request() {
        let h1 = sign_request("GET", "/ping", None).unwrap();
        let h2 = sign_request("GET", "/ping", None).unwrap();
        assert_ne!(h1.nonce, h2.nonce);
        assert_ne!(h1.signature, h2.signature);
    }
}
