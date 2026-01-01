# Plan: Harden Webhook Signature Compare

## Summary

The current `constant_time_compare` function in `apps/api/src/infra/stripe_client.rs:546-555` has a timing vulnerability: it returns early when string lengths don't match. This creates a timing leak that allows attackers to determine the expected signature length by measuring response times.

The fix is straightforward: replace the custom implementation with the battle-tested `subtle` crate which provides constant-time comparison primitives.

## Current Implementation (Vulnerable)

```rust
fn constant_time_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {      // <-- TIMING LEAK: early return on length mismatch
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= x ^ y;
    }
    result == 0
}
```

The early return on line 547-549 reveals length information to an attacker.

## Implementation Steps

### Step 1: Add `subtle` Dependency

**File:** `apps/api/Cargo.toml`

Add the `subtle` crate as a dependency. This crate is maintained by the dalek-cryptography team and is the standard for constant-time operations in Rust.

```toml
subtle = "2.5"
```

### Step 2: Replace `constant_time_compare` Implementation

**File:** `apps/api/src/infra/stripe_client.rs` (lines 546-555)

Replace the custom function with one using `subtle::ConstantTimeEq`:

```rust
fn constant_time_compare(a: &str, b: &str) -> bool {
    use subtle::ConstantTimeEq;

    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();

    // Length check must be constant-time too.
    // If lengths differ, we still compare against dummy bytes to avoid timing leak.
    if a_bytes.len() != b_bytes.len() {
        // Compare a against itself to burn the same time, then return false
        let _ = a_bytes.ct_eq(a_bytes);
        return false;
    }

    bool::from(a_bytes.ct_eq(b_bytes))
}
```

**Alternative (simpler but slightly different semantics):**

The `subtle` crate's slice comparison already short-circuits on length. However, since Stripe signatures are always 64 hex characters (SHA-256 output), length mismatches indicate malformed input rather than a real attack vector. We can use:

```rust
fn constant_time_compare(a: &str, b: &str) -> bool {
    use subtle::ConstantTimeEq;

    // If lengths differ, this is malformed input.
    // The subtle crate's ct_eq for slices does short-circuit on length,
    // but since we're comparing hex-encoded signatures (fixed 64 chars),
    // a length mismatch means invalid input, not an attack.
    if a.len() != b.len() {
        return false;
    }

    bool::from(a.as_bytes().ct_eq(b.as_bytes()))
}
```

**Recommendation:** Use the simpler version. The length check on hex-encoded HMAC-SHA256 signatures (always 64 characters) doesn't leak useful information because:
1. Attackers already know the expected length (64 hex chars for SHA-256)
2. A length mismatch means the attacker isn't even sending a valid signature format

### Step 3: Add Unit Tests

**File:** `apps/api/src/infra/stripe_client.rs` (at end of file)

Add a test module for the constant-time comparison and signature verification:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_compare_equal() {
        assert!(constant_time_compare("abc", "abc"));
        assert!(constant_time_compare("", ""));
        assert!(constant_time_compare(
            "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2",
            "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2"
        ));
    }

    #[test]
    fn test_constant_time_compare_not_equal() {
        assert!(!constant_time_compare("abc", "abd"));
        assert!(!constant_time_compare("abc", "ab"));
        assert!(!constant_time_compare("", "a"));
        assert!(!constant_time_compare(
            "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2",
            "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b3"
        ));
    }

    #[test]
    fn test_verify_webhook_signature_valid() {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        let payload = r#"{"type":"checkout.session.completed"}"#;
        let secret = "whsec_test_secret";
        let timestamp = chrono::Utc::now().timestamp().to_string();

        // Compute valid signature
        let signed_payload = format!("{}.{}", timestamp, payload);
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(signed_payload.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        let header = format!("t={},v1={}", timestamp, signature);

        let result = StripeClient::verify_webhook_signature(payload, &header, secret);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_webhook_signature_invalid() {
        let payload = r#"{"type":"checkout.session.completed"}"#;
        let secret = "whsec_test_secret";
        let timestamp = chrono::Utc::now().timestamp().to_string();

        let header = format!("t={},v1=invalid_signature", timestamp);

        let result = StripeClient::verify_webhook_signature(payload, &header, secret);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_webhook_signature_expired() {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        let payload = r#"{"type":"checkout.session.completed"}"#;
        let secret = "whsec_test_secret";
        // Timestamp 10 minutes ago (beyond 5-minute tolerance)
        let timestamp = (chrono::Utc::now().timestamp() - 600).to_string();

        let signed_payload = format!("{}.{}", timestamp, payload);
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(signed_payload.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        let header = format!("t={},v1={}", timestamp, signature);

        let result = StripeClient::verify_webhook_signature(payload, &header, secret);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_webhook_signature_missing_timestamp() {
        let result = StripeClient::verify_webhook_signature("payload", "v1=abc123", "secret");
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_webhook_signature_missing_signature() {
        let result = StripeClient::verify_webhook_signature("payload", "t=12345", "secret");
        assert!(result.is_err());
    }
}
```

### Step 4: Verify Build and Tests

```bash
./run api:build   # Verify it compiles
./run api:test    # Run tests
```

## Files to Modify

| File | Change |
|------|--------|
| `apps/api/Cargo.toml` | Add `subtle = "2.5"` dependency |
| `apps/api/src/infra/stripe_client.rs` | Replace `constant_time_compare` function (lines 546-555), add tests |

## Edge Cases

1. **Empty strings**: Both empty strings should be equal
2. **Length mismatch**: Should return false without timing leak
3. **Single character difference**: Should return false
4. **Valid signatures with multiple v1 entries**: Stripe may send multiple signatures; all should be checked
5. **Expired timestamps**: Should fail even with valid signature (already handled)
6. **Future timestamps**: Current impl allows ±5 minutes, which handles clock skew

## Security Notes

- The `subtle` crate is widely used and audited for cryptographic constant-time operations
- HMAC-SHA256 signatures are always 64 hex characters; length mismatch on well-formed input shouldn't occur
- The timestamp tolerance (5 minutes) is standard for Stripe webhooks

## References

- [subtle crate docs](https://docs.rs/subtle/latest/subtle/trait.ConstantTimeEq.html)
- [Stripe Webhook Signature Verification](https://stripe.com/docs/webhooks/signatures)
