# Plan v3: JSON Parse Error Handling

**Plan for:** [0009-json-parse-error-handling](./ticket.md)

## Summary

The codebase currently silently swallows JSON parse errors in the persistence layer by using `.unwrap_or_default()` when deserializing JSONB fields from the database. This masks data corruption issues and makes debugging difficult. The fix will add proper error logging while preserving the fallback behavior to maintain backward compatibility.

## Problem Analysis

### Affected Sites

1. **`apps/api/src/adapters/persistence/domain_end_user.rs:13`**
   - `serde_json::from_value(roles_json).unwrap_or_default()` in `row_to_profile()`
   - Parses `roles` JSONB field -> `Vec<String>`

2. **`apps/api/src/adapters/persistence/subscription_plan.rs:18`**
   - `serde_json::from_value(features_json).unwrap_or_default()` in `row_to_profile()`
   - Parses `features` JSONB field -> `Vec<String>`

3. **`apps/api/src/adapters/persistence/user_subscription.rs:160-162`**
   - `serde_json::from_value(features_json).unwrap_or_default()` in `list_by_domain_and_mode()`
   - Parses `p_features` JSONB field -> `Vec<String>`

4. **`apps/api/src/adapters/persistence/domain_end_user.rs:261`**
   - `serde_json::to_value(roles).unwrap_or_default()` in `set_roles()`
   - Serializes `Vec<String>` -> JSON

5. **`apps/api/src/adapters/persistence/subscription_plan.rs:136,213`**
   - `serde_json::to_value(...).unwrap_or(serde_json::json!([]))`
   - Serializes features to JSON

### Root Cause

The `unwrap_or_default()` pattern silently converts parse failures to empty vectors, hiding potential issues like:
- Corrupted JSONB data in the database
- Schema mismatches after migrations
- Type mismatches (e.g., `[1, 2, 3]` instead of `["a", "b", "c"]`)

## Database Schema Analysis (Addressing Feedback)

From migration analysis:

| Table | Column | Definition | NULL Handling |
|-------|--------|------------|---------------|
| `domain_end_users` | `roles` | `JSONB DEFAULT '[]'::jsonb` | No explicit NOT NULL; may be NULL for old rows |
| `subscription_plans` | `features` | `JSONB DEFAULT '[]'::jsonb` | No explicit NOT NULL; may be NULL for old rows |

### SQLx Behavior with JSONB

When a JSONB column is NULL in PostgreSQL:
- `row.get::<serde_json::Value, _>("column")` returns `serde_json::Value::Null` (not Rust `None`)
- This is because SQLx maps SQL NULL to JSON null for `serde_json::Value` type

**Verification approach:** The current code uses `row.get("roles")` returning `serde_json::Value`, which SQLx will populate with `Value::Null` for SQL NULLs. Our helper will handle this case.

### Semantic Analysis: NULL vs Empty Array

| Scenario | Current Behavior | Proposed Behavior | Rationale |
|----------|------------------|-------------------|-----------|
| JSONB is `[]` (empty array) | `Vec::new()` | `Vec::new()` (no log) | Valid empty state |
| JSONB is `["a", "b"]` | `vec!["a", "b"]` | `vec!["a", "b"]` (no log) | Normal parsing |
| JSONB is SQL NULL | `Vec::new()` | `Vec::new()` (no log) | Default not set; treat as empty |
| JSONB is JSON `null` | `Vec::new()` | `Vec::new()` (log warning) | Unexpected; should be `[]` or SQL NULL |
| JSONB is `[1, 2, 3]` | `Vec::new()` | `Vec::new()` (log warning) | Type mismatch corruption |
| JSONB is `{"key": "value"}` | `Vec::new()` | `Vec::new()` (log warning) | Wrong structure |

**Key insight:** SQL NULL (which becomes `Value::Null`) is a valid state meaning "not set" and should not log. Only actual parse failures of non-null JSON should log.

**Decision:** For roles/features, empty vector is a safe default since:
- Empty roles means no special permissions (fail-closed for authorization)
- Empty features means no premium features (fail-closed for billing)

## Implementation Approach

### Strategy: Log + Fallback with Distinct NULL Handling

1. Distinguish between SQL NULL (valid) and parse errors (corruption)
2. Log warnings only for actual parse failures, not for NULL values
3. Use structured log fields for multi-tenant filtering
4. Truncate raw JSON to prevent log bloat
5. Fall back to empty vector for backward compatibility

### Step-by-Step Implementation

#### Step 1: Add helper functions for JSON parsing with logging

**File:** `apps/api/src/adapters/persistence/mod.rs`

```rust
const MAX_JSON_LOG_LEN: usize = 200;

/// Parse JSON value to target type, logging warning on failure.
///
/// Handles NULL gracefully (returns default without logging).
/// Only logs warnings for actual parse failures (type mismatches, corruption).
///
/// # Arguments
/// * `json` - The JSON value to parse (may be Value::Null for SQL NULL)
/// * `field_name` - Name of the field being parsed (for logging)
/// * `entity_type` - Type of entity (e.g., "domain_end_user", "subscription_plan")
/// * `entity_id` - ID of the entity (for log filtering)
pub fn parse_json_with_fallback<T: serde::de::DeserializeOwned + Default>(
    json: &serde_json::Value,
    field_name: &str,
    entity_type: &str,
    entity_id: &str,
) -> T {
    // SQL NULL becomes Value::Null - treat as valid empty state, no warning
    if json.is_null() {
        return T::default();
    }

    serde_json::from_value(json.clone()).unwrap_or_else(|err| {
        // Truncate raw JSON to prevent log bloat from large arrays
        let raw_str = json.to_string();
        let truncated = if raw_str.len() > MAX_JSON_LOG_LEN {
            format!("{}...", &raw_str[..MAX_JSON_LOG_LEN])
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
- Explicit check for `Value::Null` to avoid logging on SQL NULLs (which are valid)
- Uses `&serde_json::Value` to avoid caller cloning; clone happens only in helper when needed for `from_value`
- Separate `entity_type` and `entity_id` fields for structured, filterable logs
- Truncates raw JSON to 200 chars to prevent log bloat
- No `domain_id` added since entity_id is sufficient for querying related data

**Note on clone:** `serde_json::from_value` requires ownership. We clone only when parsing non-null values. The clone is localized and happens once per call.

**PII consideration:** `roles` and `features` contain permission/feature names, not PII. Safe to log.

#### Step 2: Update domain_end_user.rs

**File:** `apps/api/src/adapters/persistence/domain_end_user.rs`

Replace `row_to_profile` function:
```rust
fn row_to_profile(row: sqlx::postgres::PgRow) -> DomainEndUserProfile {
    let id: Uuid = row.get("id");
    let roles_json: serde_json::Value = row.get("roles");
    let roles: Vec<String> = super::parse_json_with_fallback(
        &roles_json,
        "roles",
        "domain_end_user",
        &id.to_string(),
    );

    DomainEndUserProfile {
        id,
        domain_id: row.get("domain_id"),
        email: row.get("email"),
        roles,
        google_id: row.get("google_id"),
        email_verified_at: row.get("email_verified_at"),
        last_login_at: row.get("last_login_at"),
        is_frozen: row.get("is_frozen"),
        is_whitelisted: row.get("is_whitelisted"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}
```

For `set_roles()` (line 261), serialization of `Vec<String>` to JSON cannot fail in practice (no custom serializers, valid UTF-8 strings). However, if it does fail, it indicates a fundamental bug. We'll log at error level and fail the operation rather than silently clearing data:

```rust
async fn set_roles(&self, id: Uuid, roles: &[String]) -> AppResult<()> {
    let roles_json = serde_json::to_value(roles).map_err(|err| {
        tracing::error!(
            error = %err,
            roles_count = roles.len(),
            entity_id = %id,
            "Failed to serialize roles to JSON - this indicates a bug"
        );
        AppError::Internal("Failed to serialize roles".into())
    })?;
    sqlx::query(
        "UPDATE domain_end_users SET roles = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
    )
    .bind(roles_json)
    .bind(id)
    .execute(&self.pool)
    .await
    .map_err(AppError::from)?;
    Ok(())
}
```

**Addressing feedback:** Changed serialization error handling from silent fallback to propagating error. This surfaces the issue rather than silently writing `[]`.

#### Step 3: Update subscription_plan.rs

**File:** `apps/api/src/adapters/persistence/subscription_plan.rs`

Replace `row_to_profile` function:
```rust
fn row_to_profile(row: sqlx::postgres::PgRow) -> SubscriptionPlanProfile {
    let id: Uuid = row.get("id");
    let features_json: serde_json::Value = row.get("features");
    let features: Vec<String> = super::parse_json_with_fallback(
        &features_json,
        "features",
        "subscription_plan",
        &id.to_string(),
    );

    SubscriptionPlanProfile {
        id,
        domain_id: row.get("domain_id"),
        // ... rest unchanged
    }
}
```

For serialization in `create()` and `update()`, apply same error propagation pattern:
```rust
// In create():
let features_json = serde_json::to_value(&input.features).map_err(|err| {
    tracing::error!(error = %err, "Failed to serialize features to JSON");
    AppError::Internal("Failed to serialize features".into())
})?;

// In update():
let features_json = input.features.as_ref()
    .map(|f| serde_json::to_value(f))
    .transpose()
    .map_err(|err| {
        tracing::error!(error = %err, "Failed to serialize features to JSON");
        AppError::Internal("Failed to serialize features".into())
    })?;
```

#### Step 4: Update user_subscription.rs

**File:** `apps/api/src/adapters/persistence/user_subscription.rs`

Replace lines 160-162 in `list_by_domain_and_mode`:
```rust
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
| `apps/api/src/adapters/persistence/mod.rs` | Add `parse_json_with_fallback` helper with NULL handling |
| `apps/api/src/adapters/persistence/domain_end_user.rs` | Update `row_to_profile`, `set_roles` |
| `apps/api/src/adapters/persistence/subscription_plan.rs` | Update `row_to_profile`, `create`, `update` |
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
    fn parse_json_empty_array() {
        let json = serde_json::json!([]);
        let result: Vec<String> = parse_json_with_fallback(&json, "test", "entity", "123");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_json_sql_null_returns_empty_no_warning() {
        // SQL NULL becomes Value::Null - should return default without logging
        let json = serde_json::Value::Null;
        let result: Vec<String> = parse_json_with_fallback(&json, "test", "entity", "123");
        assert!(result.is_empty());
        // Note: Cannot easily verify no log without tracing-test; documented behavior
    }

    #[test]
    fn parse_json_invalid_type_returns_empty() {
        // Type mismatch: numbers instead of strings
        let json = serde_json::json!([1, 2, 3]);
        let result: Vec<String> = parse_json_with_fallback(&json, "test", "entity", "123");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_json_wrong_structure_returns_empty() {
        // Object instead of array
        let json = serde_json::json!({"key": "value"});
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

**Note on log verification:** The codebase does not currently use `tracing-test`. Adding log verification is out of scope. The test for `Value::Null` documents expected behavior (no warning) even if we can't verify it in unit tests.

### Manual Verification

1. Run `./run api:build` to verify compilation
2. Run `./run api:test` to run all tests
3. Optionally seed corrupted data in local DB and verify warning logs appear

## Edge Cases

1. **SQL NULL (Value::Null)**: Returns empty, no warning (valid state)
2. **Empty arrays `[]`**: Parses correctly, no warning
3. **Valid arrays**: Parses correctly, no warning
4. **Type mismatches `[1,2,3]`**: Logs warning, returns empty
5. **Wrong structure `{"a":1}`**: Logs warning, returns empty
6. **Large JSON arrays**: Truncated in logs to 200 chars
7. **Malformed JSON**: PostgreSQL JSONB prevents this at DB level

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Log flooding from bad data | Truncate JSON to 200 chars; structured fields enable log filtering |
| Hidden authorization issues | Empty roles = no permissions (fail-closed is safe) |
| Hidden billing issues | Empty features = no premium access (fail-closed is safe) |
| Serialization errors silently clearing data | **Changed:** Now propagates error instead of returning `[]` |
| SQL NULL treated as error | **Addressed:** Explicit check for `Value::Null` returns default without logging |

## Follow-up Work (Out of Scope)

- **Log rate limiting:** If a tenant has widespread corruption, logs could still be noisy. Consider per-entity deduplication if this becomes an issue.
- **Metrics:** Consider incrementing a counter on parse failures for alerting.
- **Log capture tests:** Add `tracing-test` integration for verifying warnings are emitted.
- **Migration to add NOT NULL:** Consider adding `NOT NULL` constraint to `roles` and `features` columns with default `'[]'::jsonb` for schema enforcement.

## Rollback Plan

If issues arise:
1. Revert the changes to restore `.unwrap_or_default()` behavior
2. The fallback behavior is unchanged for deserialization, so data remains accessible
3. Serialization now returns errors instead of silently writing `[]` - if this causes issues, can revert to fallback behavior
4. No database migrations involved, so no rollback needed there

## History

- 2026-01-01: Initial plan (v1) created
- 2026-01-01: Plan v2 created addressing feedback:
  - Added NULL vs empty array semantic analysis
  - Changed helper to accept `&serde_json::Value` to avoid caller clones
  - Added structured entity_type/entity_id fields instead of context string
  - Added JSON truncation to prevent log bloat
  - Clarified fail-closed safety implications for roles/features
  - Documented follow-up work for log rate limiting and metrics
- 2026-01-01: Plan v3 created addressing feedback-2:
  - Verified SQLx JSONB NULL handling: SQL NULL becomes Value::Null
  - Added explicit check for Value::Null to avoid logging on valid NULL state
  - Changed serialization error handling from silent fallback to error propagation
  - Confirmed roles/features are safe to log (not PII)
  - Added NOT NULL migration as follow-up suggestion
  - Clarified that domain_id is not needed since entity_id is sufficient for queries
