# Plan v2: JSON Parse Error Handling

**Plan for:** [0009-json-parse-error-handling](./ticket.md)

## Summary

The codebase currently silently swallows JSON parse errors in the persistence layer by using `.unwrap_or_default()` when deserializing JSONB fields from the database. This masks data corruption issues and makes debugging difficult. The fix will add proper error logging while preserving the fallback behavior to maintain backward compatibility.

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
   - Serializes `Vec<String>` → JSON (unlikely to fail)

5. **`apps/api/src/adapters/persistence/subscription_plan.rs:136,213`**
   - `serde_json::to_value(...).unwrap_or(serde_json::json!([]))`
   - Serializes features to JSON (unlikely to fail)

### Root Cause

The `unwrap_or_default()` pattern silently converts parse failures to empty vectors, hiding potential issues like:
- Corrupted JSONB data in the database
- Schema mismatches after migrations
- Type mismatches (e.g., `[1, 2, 3]` instead of `["a", "b", "c"]`)

## Semantic Analysis: NULL vs Empty Array

Before implementing, we need to establish clear semantics:

| Scenario | Expected Behavior | Rationale |
|----------|-------------------|-----------|
| JSONB is `[]` (empty array) | Return `Vec::new()` | Valid empty state |
| JSONB is `NULL` | Return `Vec::new()` | No value set equals empty |
| JSONB is `["a", "b"]` | Return `vec!["a", "b"]` | Normal parsing |
| JSONB is `[1, 2, 3]` | Log warning, return `Vec::new()` | Corrupted data |
| JSONB is `null` (JSON null) | Log warning, return `Vec::new()` | Unexpected; `NULL` should be SQL NULL |

**Decision:** For roles/features, empty vector is a safe default since:
- Empty roles means no special permissions (fail-closed for authorization)
- Empty features means no premium features (fail-closed for billing)

If a user loses features/roles due to corruption, they lose access rather than gain unauthorized access. This is the safer failure mode.

## Implementation Approach

### Strategy: Log + Fallback with Safeguards

1. Log warnings when JSON parsing fails with structured fields
2. Use reference semantics to avoid unnecessary clones
3. Truncate raw JSON in logs to prevent log bloat
4. Include entity identifiers for filterability
5. Fall back to empty vector for backward compatibility

### Step-by-Step Implementation

#### Step 1: Add helper function for JSON parsing with logging

**File:** `apps/api/src/adapters/persistence/mod.rs`

```rust
use tracing;

const MAX_JSON_LOG_LEN: usize = 200;

/// Parse JSON value to target type, logging warning on failure.
///
/// Uses reference semantics to avoid cloning when parsing succeeds.
/// Falls back to `T::default()` on parse error while logging context.
pub fn parse_json_with_fallback<T: serde::de::DeserializeOwned + Default>(
    json: &serde_json::Value,
    field_name: &str,
    entity_type: &str,
    entity_id: &str,
) -> T {
    serde_json::from_value(json.clone()).unwrap_or_else(|err| {
        // Truncate raw JSON to prevent log bloat
        let raw_str = json.to_string();
        let truncated = if raw_str.len() > MAX_JSON_LOG_LEN {
            format!("{}... (truncated)", &raw_str[..MAX_JSON_LOG_LEN])
        } else {
            raw_str
        };

        tracing::warn!(
            field = field_name,
            entity_type = entity_type,
            entity_id = entity_id,
            raw_json = %truncated,
            error = %err,
            "Failed to parse JSON field, using default value"
        );
        T::default()
    })
}
```

**Addressing feedback:**
- Uses `&serde_json::Value` to avoid caller cloning; clone happens only in the helper and only when needed for `from_value`
- Separate `entity_type` and `entity_id` fields for structured, filterable logs
- Truncates raw JSON to 200 chars to prevent log bloat
- Confirms `tracing` import (already used elsewhere in the crate)

**Note on clone:** `serde_json::from_value` requires ownership, so we must clone. The alternative (`from_str` after `to_string()`) would be equally expensive. The clone is localized and only happens once per call.

#### Step 2: Update domain_end_user.rs

**File:** `apps/api/src/adapters/persistence/domain_end_user.rs`

Replace line 13:
```rust
// Before
let roles: Vec<String> = serde_json::from_value(roles_json).unwrap_or_default();

// After
let id: Uuid = row.get("id");
let roles: Vec<String> = super::parse_json_with_fallback(
    &roles_json,
    "roles",
    "domain_end_user",
    &id.to_string(),
);
```

For `set_roles()` (line 261), serialization of `Vec<String>` to JSON should never fail (no custom serializers, no non-UTF8 strings). If it does, something is fundamentally broken. We'll log but not panic:
```rust
// Before
let roles_json = serde_json::to_value(roles).unwrap_or_default();

// After
let roles_json = serde_json::to_value(&roles).unwrap_or_else(|err| {
    tracing::error!(
        error = %err,
        roles_count = roles.len(),
        "Failed to serialize roles to JSON - this indicates a bug"
    );
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
    &features_json,
    "features",
    "subscription_plan",
    &id.to_string(),
);
```

Lines 136 and 213 are serialization paths; apply same pattern as step 2 serialization.

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
    &features_json,
    "features",
    "subscription_plan",
    &plan_id.to_string(),
);
```

### Files to Modify

| File | Changes |
|------|---------|
| `apps/api/src/adapters/persistence/mod.rs` | Add `parse_json_with_fallback` helper with truncation |
| `apps/api/src/adapters/persistence/domain_end_user.rs` | Update lines 13, 261 |
| `apps/api/src/adapters/persistence/subscription_plan.rs` | Update line 18 (optionally 136, 213) |
| `apps/api/src/adapters/persistence/user_subscription.rs` | Update lines 160-162 |

## Testing Approach

### Unit Tests

Add unit tests for the helper in `apps/api/src/adapters/persistence/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_valid_array() {
        let json = serde_json::json!(["a", "b", "c"]);
        let result: Vec<String> = parse_json_with_fallback(&json, "test", "entity", "123");
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn parse_json_invalid_type_returns_empty() {
        let json = serde_json::json!([1, 2, 3]); // numbers instead of strings
        let result: Vec<String> = parse_json_with_fallback(&json, "test", "entity", "123");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_json_null_returns_empty() {
        let json = serde_json::Value::Null;
        let result: Vec<String> = parse_json_with_fallback(&json, "test", "entity", "123");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_json_empty_array() {
        let json = serde_json::json!([]);
        let result: Vec<String> = parse_json_with_fallback(&json, "test", "entity", "123");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_json_nested_objects_returns_empty() {
        let json = serde_json::json!([{"key": "value"}]);
        let result: Vec<String> = parse_json_with_fallback(&json, "test", "entity", "123");
        assert!(result.is_empty());
    }
}
```

**Note on log capture testing:** The codebase does not currently use `tracing-test` or similar. Adding log verification tests is out of scope for this task but documented as a follow-up below.

### Manual Verification

1. Run `./run api:build` to verify compilation
2. Run `./run api:test` to run all tests
3. Optionally seed corrupted data in local DB and verify warning logs appear

## Edge Cases

1. **NULL JSONB values**: `serde_json::Value::Null` logs warning, returns empty
2. **Empty arrays**: `[]` parses correctly, no warning
3. **Type mismatches**: `[1,2,3]` when expecting strings logs warning, returns empty
4. **Nested objects**: `[{"a": 1}]` when expecting `Vec<String>` logs warning
5. **Malformed JSON**: PostgreSQL JSONB prevents this at DB level, not a concern
6. **Large JSON arrays**: Truncated in logs to 200 chars to prevent bloat

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Log flooding from bad data | Truncate JSON to 200 chars; structured fields enable log filtering |
| Hidden authorization issues | Empty roles = no permissions (fail-closed is safe) |
| Hidden billing issues | Empty features = no premium access (fail-closed is safe) |
| Serialization error swallowing | Use `error!` level + include context; these should never happen |

## Follow-up Work (Out of Scope)

- **Log rate limiting:** If a tenant has widespread corruption, logs could still be noisy. Consider adding per-entity deduplication if this becomes an issue in production.
- **Metrics:** Consider incrementing a counter on parse failures for alerting.
- **Log capture tests:** Add `tracing-test` integration for verifying warnings are emitted.

## Rollback Plan

If issues arise:
1. Revert the changes to restore `.unwrap_or_default()` behavior
2. The fallback behavior is unchanged, so data remains accessible
3. No database migrations involved, so no rollback needed there

## History

- 2026-01-01: Initial plan (v1) created
- 2026-01-01: Plan v2 created addressing feedback:
  - Added NULL vs empty array semantic analysis
  - Changed helper to accept `&serde_json::Value` to avoid caller clones
  - Added structured entity_type/entity_id fields instead of context string
  - Added JSON truncation to prevent log bloat
  - Clarified fail-closed safety implications for roles/features
  - Documented follow-up work for log rate limiting and metrics
