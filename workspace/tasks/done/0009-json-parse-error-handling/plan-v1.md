# Plan: JSON Parse Error Handling

**Plan for:** [0009-json-parse-error-handling](./ticket.md)

## Summary

The codebase currently silently swallows JSON parse errors in persistence layer by using `.unwrap_or_default()` when deserializing JSONB fields from the database. This masks data corruption issues and makes debugging difficult. The fix will add proper error logging while preserving the fallback behavior to maintain backward compatibility.

## Problem Analysis

### Affected Sites

1. **`apps/api/src/adapters/persistence/domain_end_user.rs:13`**
   - `serde_json::from_value(roles_json).unwrap_or_default()` in `row_to_profile()`
   - Parses `roles` JSONB field → `Vec<String>`

2. **`apps/api/src/adapters/persistence/subscription_plan.rs:18`**
   - `serde_json::from_value(features_json).unwrap_or_default()` in `row_to_profile()`
   - Parses `features` JSONB field → `Vec<String>`

3. **`apps/api/src/adapters/persistence/user_subscription.rs:162`**
   - `serde_json::from_value(features_json).unwrap_or_default()` in `list_by_domain_and_mode()`
   - Parses `p_features` JSONB field → `Vec<String>`

4. **`apps/api/src/adapters/persistence/domain_end_user.rs:261`**
   - `serde_json::to_value(roles).unwrap_or_default()` in `set_roles()`
   - Serializes `Vec<String>` → JSON (this is unlikely to fail, but should still be handled)

5. **`apps/api/src/adapters/persistence/subscription_plan.rs:136,213`**
   - `serde_json::to_value(...).unwrap_or(serde_json::json!([]))`
   - Serializes features to JSON (unlikely to fail, but uses different fallback pattern)

### Root Cause

The `unwrap_or_default()` pattern silently converts parse failures to empty vectors, hiding potential issues like:
- Corrupted JSONB data in the database
- Schema mismatches after migrations
- Type mismatches (e.g., `[1, 2, 3]` instead of `["a", "b", "c"]`)

## Implementation Approach

### Strategy: Log + Fallback

Rather than propagating errors (which would break the entire request for a non-critical field), we will:
1. Log warnings when JSON parsing fails
2. Include context: entity ID, field name, raw JSON value
3. Fall back to empty vector for backward compatibility

This approach balances observability with reliability.

### Step-by-Step Implementation

#### Step 1: Add helper function for JSON parsing with logging

Create a reusable helper in the persistence module:

**File:** `apps/api/src/adapters/persistence/mod.rs`

Add a helper function:
```rust
/// Parse JSON value to target type, logging warning on failure
pub fn parse_json_with_fallback<T: serde::de::DeserializeOwned + Default>(
    json: serde_json::Value,
    field_name: &str,
    context: &str,
) -> T {
    serde_json::from_value(json.clone()).unwrap_or_else(|err| {
        tracing::warn!(
            field = field_name,
            context = context,
            raw_json = %json,
            error = %err,
            "Failed to parse JSON field, using default value"
        );
        T::default()
    })
}
```

#### Step 2: Update domain_end_user.rs

**File:** `apps/api/src/adapters/persistence/domain_end_user.rs`

Replace line 13:
```rust
// Before
let roles: Vec<String> = serde_json::from_value(roles_json).unwrap_or_default();

// After
let id: Uuid = row.get("id");
let roles: Vec<String> = super::parse_json_with_fallback(
    roles_json,
    "roles",
    &format!("domain_end_user:{}", id),
);
```

For `set_roles()` (line 261), serialization of `Vec<String>` should never fail, but we can add a debug log for completeness:
```rust
// Before
let roles_json = serde_json::to_value(roles).unwrap_or_default();

// After
let roles_json = serde_json::to_value(roles).unwrap_or_else(|err| {
    tracing::error!(error = %err, "Failed to serialize roles - this should never happen");
    serde_json::json!([])
});
```

#### Step 3: Update subscription_plan.rs

**File:** `apps/api/src/adapters/persistence/subscription_plan.rs`

Replace line 18:
```rust
// Before
let features: Vec<String> = serde_json::from_value(features_json).unwrap_or_default();

// After
let id: Uuid = row.get("id");
let features: Vec<String> = super::parse_json_with_fallback(
    features_json,
    "features",
    &format!("subscription_plan:{}", id),
);
```

Lines 136 and 213 are serialization (unlikely to fail), but can be updated similarly for consistency.

#### Step 4: Update user_subscription.rs

**File:** `apps/api/src/adapters/persistence/user_subscription.rs`

Replace lines 160-162:
```rust
// Before
let features_json: serde_json::Value = row.get("p_features");
let features: Vec<String> = serde_json::from_value(features_json).unwrap_or_default();

// After
let features_json: serde_json::Value = row.get("p_features");
let plan_id: Uuid = row.get("p_id");
let features: Vec<String> = super::parse_json_with_fallback(
    features_json,
    "features",
    &format!("subscription_plan:{}", plan_id),
);
```

### Files to Modify

| File | Changes |
|------|---------|
| `apps/api/src/adapters/persistence/mod.rs` | Add `parse_json_with_fallback` helper |
| `apps/api/src/adapters/persistence/domain_end_user.rs` | Update lines 13, 261 |
| `apps/api/src/adapters/persistence/subscription_plan.rs` | Update line 18 (optionally 136, 213) |
| `apps/api/src/adapters/persistence/user_subscription.rs` | Update lines 160-162 |

## Testing Approach

### Unit Tests

Add unit tests for the new helper function in `apps/api/src/adapters/persistence/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_with_fallback_valid_array() {
        let json = serde_json::json!(["a", "b", "c"]);
        let result: Vec<String> = parse_json_with_fallback(json, "test", "ctx");
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn parse_json_with_fallback_invalid_type() {
        let json = serde_json::json!([1, 2, 3]); // numbers instead of strings
        let result: Vec<String> = parse_json_with_fallback(json, "test", "ctx");
        assert_eq!(result, Vec::<String>::new());
    }

    #[test]
    fn parse_json_with_fallback_null() {
        let json = serde_json::Value::Null;
        let result: Vec<String> = parse_json_with_fallback(json, "test", "ctx");
        assert_eq!(result, Vec::<String>::new());
    }

    #[test]
    fn parse_json_with_fallback_empty_array() {
        let json = serde_json::json!([]);
        let result: Vec<String> = parse_json_with_fallback(json, "test", "ctx");
        assert_eq!(result, Vec::<String>::new());
    }
}
```

### Manual Verification

1. Run `./run api:build` to verify compilation
2. Run `./run api:test` to run all tests
3. Optionally seed corrupted data and verify warning logs appear

## Edge Cases

1. **NULL JSONB values**: `serde_json::Value::Null` should parse as empty vec (handled by fallback)
2. **Empty arrays**: `[]` should parse correctly (no warning)
3. **Type mismatches**: `[1,2,3]` when expecting strings should log warning and return empty
4. **Nested objects**: `[{"a": 1}]` when expecting `Vec<String>` should log warning
5. **Malformed JSON**: PostgreSQL JSONB prevents this at DB level, so not a concern

## Rollback Plan

If issues arise:
1. Revert the changes to restore `.unwrap_or_default()` behavior
2. The fallback behavior is unchanged, so data remains accessible

## History

- 2026-01-01: Initial plan created
