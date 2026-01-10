# Plan v2: Harden Webhook Signature Compare

## Summary

The `constant_time_compare` function in `apps/api/src/infra/stripe_client.rs` has a timing vulnerability: early return on length mismatch leaks information. The goal is to eliminate timing leaks entirely.

## Key Change from v1

**Decision: Use `hmac::Mac::verify_slice` instead of custom compare logic.**

The `hmac` crate (already a dependency) provides `verify_slice()` which is inherently constant-time. This eliminates both:
1. The need for a custom `constant_time_compare` function
2. The need to add the `subtle` crate

This approach is simpler and leverages a battle-tested implementation.

## Current Implementation (Vulnerable)

```rust
// apps/api/src/infra/stripe_client.rs — verify_webhook_signature method

// Compute expected signature
let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(payload));
let mut mac = Hmac::<Sha256>::new_from_slice(webhook_secret.as_bytes())
    .map_err(|_| AppError::Internal("HMAC error".into()))?;
mac.update(signed_payload.as_bytes());
let expected = hex::encode(mac.finalize().into_bytes());  // <-- finalize() consumes mac

// Check if any signature matches
for sig in signatures {
    if constant_time_compare(sig, &expected) {  // <-- vulnerable custom compare
        ...
    }
}
```

```rust
// apps/api/src/infra/stripe_client.rs — constant_time_compare function

fn constant_time_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {      // <-- TIMING LEAK
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= x ^ y;
    }
    result == 0
}
```

The early return on line 547 reveals length information.

## Implementation Steps

### Step 1: Refactor to Use `verify_slice`

**File:** `apps/api/src/infra/stripe_client.rs`

Replace the signature verification logic in `verify_webhook_signature` to use `Mac::verify_slice` on decoded bytes rather than comparing hex strings.

**Before:**
```rust
// Compute expected signature
let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(payload));
let mut mac = Hmac::<Sha256>::new_from_slice(webhook_secret.as_bytes())
    .map_err(|_| AppError::Internal("HMAC error".into()))?;
mac.update(signed_payload.as_bytes());
let expected = hex::encode(mac.finalize().into_bytes());

// Check if any signature matches
for sig in signatures {
    if constant_time_compare(sig, &expected) {
```

**After:**
```rust
// Compute expected MAC
let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(payload));

// Check if any signature matches (constant-time via verify_slice)
for sig in &signatures {
    // Decode hex signature; skip malformed entries
    let sig_bytes = match hex::decode(sig) {
        Ok(bytes) => bytes,
        Err(_) => continue,  // malformed hex, try next signature
    };

    let mut mac = Hmac::<Sha256>::new_from_slice(webhook_secret.as_bytes())
        .map_err(|_| AppError::Internal("HMAC error".into()))?;
    mac.update(signed_payload.as_bytes());

    // verify_slice performs constant-time comparison
    if mac.verify_slice(&sig_bytes).is_ok() {
```

**Key points:**
- `verify_slice` is constant-time internally (uses `subtle` under the hood in the `hmac` crate)
- We decode the hex signature to bytes, handling malformed hex gracefully
- We create a fresh MAC for each signature attempt (necessary since `verify_slice` consumes the MAC)
- Length mismatch in `verify_slice` returns error without timing leak

### Step 2: Remove `constant_time_compare` Function

Delete the `constant_time_compare` function entirely since it's no longer needed.

**File:** `apps/api/src/infra/stripe_client.rs`

Remove:
```rust
fn constant_time_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= x ^ y;
    }
    result == 0
}
```

### Step 3: Add Comprehensive Unit Tests

**File:** `apps/api/src/infra/stripe_client.rs` (add at end of file)

Add a test module covering:
1. Valid signature verification
2. Invalid signature verification
3. Expired timestamp (using fixed timestamps)
4. Missing timestamp in header
5. Missing signature in header
6. **Multiple v1 entries** (second signature valid)
7. **Malformed hex signature** (non-hex chars, odd length)
8. **Empty signature value**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    const TEST_SECRET: &str = "whsec_test_secret_key";
    const TOLERANCE_SECONDS: i64 = 300;

    fn compute_signature(payload: &str, timestamp: i64, secret: &str) -> String {
        let signed_payload = format!("{}.{}", timestamp, payload);
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(signed_payload.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    fn current_timestamp() -> i64 {
        chrono::Utc::now().timestamp()
    }

    #[test]
    fn test_verify_webhook_signature_valid() {
        let payload = r#"{"type":"checkout.session.completed"}"#;
        let ts = current_timestamp();
        let sig = compute_signature(payload, ts, TEST_SECRET);
        let header = format!("t={},v1={}", ts, sig);

        let result = StripeClient::verify_webhook_signature(payload, &header, TEST_SECRET);
        assert!(result.is_ok(), "Expected valid signature to pass");
    }

    #[test]
    fn test_verify_webhook_signature_invalid() {
        let payload = r#"{"type":"checkout.session.completed"}"#;
        let ts = current_timestamp();
        let header = format!("t={},v1=0000000000000000000000000000000000000000000000000000000000000000", ts);

        let result = StripeClient::verify_webhook_signature(payload, &header, TEST_SECRET);
        assert!(result.is_err(), "Expected invalid signature to fail");
    }

    #[test]
    fn test_verify_webhook_signature_expired() {
        let payload = r#"{"type":"checkout.session.completed"}"#;
        // Timestamp beyond tolerance (10 minutes ago)
        let ts = current_timestamp() - (TOLERANCE_SECONDS + 300);
        let sig = compute_signature(payload, ts, TEST_SECRET);
        let header = format!("t={},v1={}", ts, sig);

        let result = StripeClient::verify_webhook_signature(payload, &header, TEST_SECRET);
        assert!(result.is_err(), "Expected expired timestamp to fail");
    }

    #[test]
    fn test_verify_webhook_signature_missing_timestamp() {
        let result = StripeClient::verify_webhook_signature("payload", "v1=abc123def456", TEST_SECRET);
        assert!(result.is_err(), "Expected missing timestamp to fail");
    }

    #[test]
    fn test_verify_webhook_signature_missing_signature() {
        let ts = current_timestamp();
        let result = StripeClient::verify_webhook_signature("payload", &format!("t={}", ts), TEST_SECRET);
        assert!(result.is_err(), "Expected missing signature to fail");
    }

    #[test]
    fn test_verify_webhook_signature_multiple_v1_entries() {
        // Stripe may send multiple v1 signatures; verification passes if any match
        let payload = r#"{"type":"test"}"#;
        let ts = current_timestamp();
        let valid_sig = compute_signature(payload, ts, TEST_SECRET);
        let invalid_sig = "0000000000000000000000000000000000000000000000000000000000000000";

        // Invalid first, valid second
        let header = format!("t={},v1={},v1={}", ts, invalid_sig, valid_sig);

        let result = StripeClient::verify_webhook_signature(payload, &header, TEST_SECRET);
        assert!(result.is_ok(), "Expected second valid signature to pass");
    }

    #[test]
    fn test_verify_webhook_signature_malformed_hex() {
        let payload = r#"{"type":"test"}"#;
        let ts = current_timestamp();

        // Non-hex characters
        let header_non_hex = format!("t={},v1=ZZZZ0000000000000000000000000000000000000000000000000000000000", ts);
        let result = StripeClient::verify_webhook_signature(payload, &header_non_hex, TEST_SECRET);
        assert!(result.is_err(), "Expected malformed hex to fail");

        // Odd-length hex (invalid)
        let header_odd = format!("t={},v1=abc", ts);
        let result = StripeClient::verify_webhook_signature(payload, &header_odd, TEST_SECRET);
        assert!(result.is_err(), "Expected odd-length hex to fail");
    }

    #[test]
    fn test_verify_webhook_signature_empty_signature() {
        let payload = r#"{"type":"test"}"#;
        let ts = current_timestamp();
        let header = format!("t={},v1=", ts);

        let result = StripeClient::verify_webhook_signature(payload, &header, TEST_SECRET);
        assert!(result.is_err(), "Expected empty signature to fail");
    }

    #[test]
    fn test_verify_webhook_signature_wrong_length() {
        // Signature with wrong length (not 64 hex chars / 32 bytes)
        let payload = r#"{"type":"test"}"#;
        let ts = current_timestamp();
        let header = format!("t={},v1=abcd1234", ts); // only 8 hex chars

        let result = StripeClient::verify_webhook_signature(payload, &header, TEST_SECRET);
        assert!(result.is_err(), "Expected wrong-length signature to fail");
    }
}
```

### Step 4: Verify Build and Tests

```bash
./run api:build   # Verify it compiles
./run api:test    # Run tests including new ones
```

## Files to Modify

| File | Change |
|------|--------|
| `apps/api/src/infra/stripe_client.rs` | Refactor `verify_webhook_signature` to use `verify_slice`, delete `constant_time_compare`, add tests |

**No new dependencies required** — `hmac` is already in `Cargo.toml`.

## Call Sites

The `constant_time_compare` function is only used in one place:
- `apps/api/src/infra/stripe_client.rs` in `verify_webhook_signature`

This was confirmed by grepping the codebase.

## Security Analysis

### Why `verify_slice` is constant-time

The `hmac` crate's `verify_slice` method internally uses `subtle::ConstantTimeEq` for comparison. From the `hmac` source:

```rust
fn verify_slice(self, tag: &[u8]) -> Result<(), MacError> {
    let choice = self.finalize().into_bytes().ct_eq(tag.into());
    if choice.unwrap_u8() == 1 {
        Ok(())
    } else {
        Err(MacError)
    }
}
```

This eliminates timing side channels for both length and content comparison.

### Handling malformed input

- **Invalid hex:** Decoded with `hex::decode`; failures skip to next signature
- **Wrong length:** `verify_slice` returns error for length mismatch (constant-time)
- **Multiple signatures:** All are tried; first valid match succeeds

### Timestamp tolerance

The existing 5-minute (300 second) tolerance is preserved. Tests use `current_timestamp() - (TOLERANCE_SECONDS + 300)` to ensure expired timestamps fail without time-dependent flakiness.

## Edge Cases

| Case | Handling |
|------|----------|
| Empty strings | Empty hex decodes to empty bytes; verify_slice fails (wrong length) |
| Length mismatch | verify_slice handles this constant-time |
| Single char difference | verify_slice handles this constant-time |
| Multiple v1 signatures | All checked; first valid match succeeds |
| Malformed hex | hex::decode fails; skip to next signature |
| Expired timestamp | Checked after signature match; returns error |
| Future timestamp | Allowed within ±5 minute window (clock skew) |

## References

- [hmac crate docs — verify_slice](https://docs.rs/hmac/latest/hmac/trait.Mac.html#method.verify_slice)
- [subtle crate — used internally by hmac](https://docs.rs/subtle/latest/subtle/)
- [Stripe Webhook Signature Verification](https://stripe.com/docs/webhooks/signatures)
