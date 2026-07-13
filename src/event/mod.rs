//! Event callback verification, decryption and parsing.

use aes::Aes256;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use http::HeaderMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;

const HEADER_NONCE: &str = "x-lark-request-nonce";
const HEADER_TIMESTAMP: &str = "x-lark-request-timestamp";
const HEADER_SIGNATURE: &str = "x-lark-signature";

type Aes256CbcDecryptor = cbc::Decryptor<Aes256>;

/// Event parsing and cryptographic errors.
#[derive(Debug, Error)]
pub enum EventError {
    /// Required callback header is missing.
    #[error("missing event header: {0}")]
    MissingHeader(&'static str),
    /// Signature verification failed.
    #[error("event signature verification failed")]
    InvalidSignature,
    /// Verification token does not match.
    #[error("event verification token does not match")]
    InvalidVerificationToken,
    /// Encrypted body is malformed.
    #[error("event decryption failed: {0}")]
    Decryption(String),
    /// Body is not valid UTF-8.
    #[error("event body is not UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    /// Event JSON is invalid.
    #[error("event JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    /// Callback did not contain an event type or challenge.
    #[error("invalid event callback: {0}")]
    InvalidEvent(String),
}

/// Event v2 header.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EventHeader {
    pub event_id: Option<String>,
    pub event_type: Option<String>,
    pub app_id: Option<String>,
    pub tenant_key: Option<String>,
    pub create_time: Option<String>,
    pub token: Option<String>,
}

/// Parsed callback result.
#[derive(Clone, Debug)]
pub enum ParsedEvent {
    /// URL-verification challenge.
    Challenge {
        /// Challenge string to echo in the HTTP response.
        challenge: String,
    },
    /// Business event callback.
    Event {
        /// Event type used for dispatching.
        event_type: String,
        /// Optional v2 event header.
        header: Option<EventHeader>,
        /// Event payload.
        event: Value,
        /// Fully decoded callback JSON.
        raw: Value,
    },
}

/// Configurable callback parser.
#[derive(Clone, Debug, Default)]
pub struct EventParser {
    verification_token: Option<String>,
    encrypt_key: Option<String>,
    skip_signature_verification: bool,
}

impl EventParser {
    /// Creates a parser without verification settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the verification token configured in the developer console.
    pub fn verification_token(mut self, token: impl Into<String>) -> Self {
        self.verification_token = Some(token.into());
        self
    }

    /// Sets the event encryption key.
    pub fn encrypt_key(mut self, encrypt_key: impl Into<String>) -> Self {
        self.encrypt_key = Some(encrypt_key.into());
        self
    }

    /// Disables callback signature verification. Intended only for controlled tests.
    pub fn skip_signature_verification(mut self, skip: bool) -> Self {
        self.skip_signature_verification = skip;
        self
    }

    /// Verifies, decrypts and parses one callback request.
    pub fn parse(&self, headers: &HeaderMap, body: &[u8]) -> Result<ParsedEvent, EventError> {
        let body_text = std::str::from_utf8(body)?;
        if let Some(encrypt_key) = self.encrypt_key.as_deref() {
            if !self.skip_signature_verification {
                verify_request_signature(headers, encrypt_key, body_text)?;
            }
        }

        let mut value: Value = serde_json::from_str(body_text)?;
        if let Some(encrypted) = value.get("encrypt").and_then(Value::as_str) {
            let encrypt_key = self.encrypt_key.as_deref().ok_or_else(|| {
                EventError::Decryption("encrypt_key is required for encrypted callbacks".into())
            })?;
            let plaintext = decrypt_event(encrypted, encrypt_key)?;
            value = serde_json::from_slice(&plaintext)?;
        }

        self.verify_token(&value)?;

        if let Some(challenge) = value.get("challenge").and_then(Value::as_str) {
            return Ok(ParsedEvent::Challenge {
                challenge: challenge.to_owned(),
            });
        }

        let header: Option<EventHeader> = value
            .get("header")
            .cloned()
            .map(serde_json::from_value)
            .transpose()?;
        let event_type = header
            .as_ref()
            .and_then(|header: &EventHeader| header.event_type.clone())
            .or_else(|| {
                value
                    .get("event")
                    .and_then(|event| event.get("type"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
            .ok_or_else(|| EventError::InvalidEvent("event type is missing".into()))?;
        let event = value.get("event").cloned().unwrap_or(Value::Null);

        Ok(ParsedEvent::Event {
            event_type,
            header,
            event,
            raw: value,
        })
    }

    fn verify_token(&self, value: &Value) -> Result<(), EventError> {
        let Some(expected) = self.verification_token.as_deref() else {
            return Ok(());
        };
        let actual = value
            .get("token")
            .and_then(Value::as_str)
            .or_else(|| {
                value
                    .get("header")
                    .and_then(|header| header.get("token"))
                    .and_then(Value::as_str)
            })
            .unwrap_or_default();
        if expected.as_bytes().ct_eq(actual.as_bytes()).into() {
            Ok(())
        } else {
            Err(EventError::InvalidVerificationToken)
        }
    }
}

/// Computes the Feishu/Lark SHA-256 callback signature.
pub fn signature(timestamp: &str, nonce: &str, encrypt_key: &str, body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(timestamp.as_bytes());
    hasher.update(nonce.as_bytes());
    hasher.update(encrypt_key.as_bytes());
    hasher.update(body.as_bytes());
    hex::encode(hasher.finalize())
}

/// Verifies callback signature headers against the raw body.
pub fn verify_request_signature(
    headers: &HeaderMap,
    encrypt_key: &str,
    body: &str,
) -> Result<(), EventError> {
    let timestamp = header(headers, HEADER_TIMESTAMP)
        .ok_or(EventError::MissingHeader("X-Lark-Request-Timestamp"))?;
    let nonce =
        header(headers, HEADER_NONCE).ok_or(EventError::MissingHeader("X-Lark-Request-Nonce"))?;
    let supplied =
        header(headers, HEADER_SIGNATURE).ok_or(EventError::MissingHeader("X-Lark-Signature"))?;
    let expected = signature(timestamp, nonce, encrypt_key, body);

    if expected.as_bytes().ct_eq(supplied.as_bytes()).into() {
        Ok(())
    } else {
        Err(EventError::InvalidSignature)
    }
}

/// Decrypts an encrypted event body using AES-256-CBC.
pub fn decrypt_event(encrypted: &str, encrypt_key: &str) -> Result<Vec<u8>, EventError> {
    let decoded = STANDARD
        .decode(encrypted)
        .map_err(|error| EventError::Decryption(format!("invalid base64: {error}")))?;
    if decoded.len() < 32 || (decoded.len() - 16) % 16 != 0 {
        return Err(EventError::Decryption(
            "ciphertext length is invalid".into(),
        ));
    }

    let key = Sha256::digest(encrypt_key.as_bytes());
    let (iv, ciphertext) = decoded.split_at(16);
    let plaintext = Aes256CbcDecryptor::new_from_slices(&key, iv)
        .map_err(|error| EventError::Decryption(error.to_string()))?
        .decrypt_padded_vec_mut::<Pkcs7>(ciphertext)
        .map_err(|error| EventError::Decryption(error.to_string()))?;

    // The official implementations tolerate non-JSON prefix/suffix bytes and
    // extract the JSON object after decryption.
    match (
        plaintext.iter().position(|byte| *byte == b'{'),
        plaintext.iter().rposition(|byte| *byte == b'}'),
    ) {
        (Some(start), Some(end)) if start <= end => {
            return Ok(plaintext[start..=end].to_vec());
        }
        _ => {}
    }
    Ok(plaintext)
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;

    #[test]
    fn signature_matches_official_concatenation_order() {
        assert_eq!(
            signature("1", "2", "3", "4"),
            "03ac674216f3e15c761ee1a5e255f067953623c8b388b4459e13f978d7c846f4"
        );
    }

    #[test]
    fn verifies_headers() {
        let body = r#"{"type":"event_callback"}"#;
        let mut headers = HeaderMap::new();
        headers.insert(HEADER_TIMESTAMP, HeaderValue::from_static("1"));
        headers.insert(HEADER_NONCE, HeaderValue::from_static("2"));
        headers.insert(
            HEADER_SIGNATURE,
            HeaderValue::from_str(&signature("1", "2", "secret", body)).unwrap(),
        );
        verify_request_signature(&headers, "secret", body).unwrap();
    }
}
