use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub fn sign_webhook_payload(secret: &str, timestamp: i64, body: &str) -> String {
    let signed_content = format!("{}.{}", timestamp, body);
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(signed_content.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());
    format!("t={},v1={}", timestamp, signature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_is_deterministic() {
        let sig1 = sign_webhook_payload("whsec_test_secret", 1706500000, r#"{"id":"evt_1"}"#);
        let sig2 = sign_webhook_payload("whsec_test_secret", 1706500000, r#"{"id":"evt_1"}"#);
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn signature_changes_with_different_secret() {
        let sig1 = sign_webhook_payload("whsec_secret_a", 1706500000, r#"{"id":"evt_1"}"#);
        let sig2 = sign_webhook_payload("whsec_secret_b", 1706500000, r#"{"id":"evt_1"}"#);
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn signature_changes_with_different_timestamp() {
        let sig1 = sign_webhook_payload("whsec_test_secret", 1706500000, r#"{"id":"evt_1"}"#);
        let sig2 = sign_webhook_payload("whsec_test_secret", 1706500001, r#"{"id":"evt_1"}"#);
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn signature_changes_with_different_body() {
        let sig1 = sign_webhook_payload("whsec_test_secret", 1706500000, r#"{"id":"evt_1"}"#);
        let sig2 = sign_webhook_payload("whsec_test_secret", 1706500000, r#"{"id":"evt_2"}"#);
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn signature_has_correct_format() {
        let sig = sign_webhook_payload("whsec_test_secret", 1706500000, r#"{"id":"evt_1"}"#);
        assert!(sig.starts_with("t=1706500000,v1="));
        let hex_part = sig.strip_prefix("t=1706500000,v1=").unwrap();
        assert_eq!(hex_part.len(), 64); // SHA-256 hex = 64 chars
        assert!(hex_part.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
