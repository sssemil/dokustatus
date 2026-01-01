# Implementation Plan: Batch Domain Auth Configs (N+1 Fix) — v3

## Summary

The `list_domains` endpoint in `apps/api/src/adapters/http/routes/domain.rs` (lines 211-257) has an N+1 query issue. For each domain returned, it calls `get_auth_config()` individually to check whether auth methods are enabled (`has_auth_methods` field). This results in N additional database queries for N domains.

The fix involves adding a batch method to fetch auth configs for multiple domain IDs in a single query, then using that in the route handler.

## Current Behavior Analysis

### Default Behavior in `get_auth_config` (line 549-564)

When no auth config row exists for a domain, `get_auth_config` returns a **synthetic default**:

```rust
.unwrap_or(DomainAuthConfigProfile {
    magic_link_enabled: true, // enabled by default with fallback
    google_oauth_enabled: false,
    // ...
})
```

This means `has_auth_methods = true` for domains without explicit config, regardless of whether fallback email config is actually available.

### Public Config Behavior in `get_public_config` (line 352-366)

For public-facing auth pages, the logic is different — it checks actual fallback availability:

```rust
let magic_link_fallback_available =
    self.fallback_resend_api_key.is_some() && self.fallback_email_domain.is_some();
let magic_link_enabled = auth_config
    .as_ref()
    .map(|c| c.magic_link_enabled)
    .unwrap_or(magic_link_fallback_available);
```

### Decision: Preserve Current Behavior

For the domain listing, we will **preserve the current `get_auth_config` semantics**: domains without explicit config default to `has_auth_methods = true` (magic link assumed enabled). This avoids any UI behavior change.

If the team later wants to align with `get_public_config` semantics (check actual fallback availability), that's a separate task.

## Step-by-Step Implementation

### Step 1: Add batch method to `DomainAuthConfigRepo` trait

**File:** `apps/api/src/application/use_cases/domain_auth.rs`

Add a new method to the `DomainAuthConfigRepo` trait:

```rust
async fn get_by_domain_ids(&self, domain_ids: &[Uuid]) -> AppResult<Vec<DomainAuthConfigProfile>>;
```

### Step 2: Implement batch method in PostgresPersistence

**File:** `apps/api/src/adapters/persistence/domain_auth_config.rs`

Add the implementation using `ANY($1)` pattern (same as `count_by_domain_ids` in domain_end_user.rs):

```rust
async fn get_by_domain_ids(&self, domain_ids: &[Uuid]) -> AppResult<Vec<DomainAuthConfigProfile>> {
    if domain_ids.is_empty() {
        return Ok(vec![]);
    }
    let rows = sqlx::query(
        "SELECT id, domain_id, magic_link_enabled, google_oauth_enabled FROM domain_auth_config WHERE domain_id = ANY($1)",
    )
    .bind(domain_ids)
    .fetch_all(&self.pool)
    .await
    .map_err(AppError::from)?;

    Ok(rows.into_iter().map(|row| DomainAuthConfigProfile {
        id: row.get("id"),
        domain_id: row.get("domain_id"),
        magic_link_enabled: row.get("magic_link_enabled"),
        google_oauth_enabled: row.get("google_oauth_enabled"),
        // Fields not needed for has_auth_methods check; set to defaults
        redirect_url: None,
        whitelist_enabled: false,
        access_token_ttl_secs: 0,
        refresh_token_ttl_days: 0,
        created_at: None,
        updated_at: None,
    }).collect())
}
```

**Note:** We only SELECT the columns needed for `has_auth_methods` logic (`magic_link_enabled`, `google_oauth_enabled`) plus identifiers. This reduces payload size since other fields (`redirect_url`, TTL fields, etc.) are unused for this specific query.

### Step 3: Add batch helper method to `DomainAuthUseCases`

**File:** `apps/api/src/application/use_cases/domain_auth.rs`

Add a **private** method scoped to owner-authorized domain IDs. The name reflects its intended context:

```rust
use std::collections::HashMap;

/// Batch check if domains have auth methods enabled.
///
/// IMPORTANT: This method does NOT verify domain ownership. It must only be
/// called with domain IDs that have already been authorized (e.g., from
/// `list_domains` which already filters by owner).
///
/// Returns a map from domain_id -> has_auth_methods.
/// Domains without explicit config default to `true` (matching `get_auth_config` semantics).
pub(crate) async fn has_auth_methods_for_owner_domains(
    &self,
    domain_ids: &[Uuid],
) -> AppResult<HashMap<Uuid, bool>> {
    if domain_ids.is_empty() {
        return Ok(HashMap::new());
    }

    // Fetch all configs in one query (no deduplication needed; input comes from list_domains which is already unique)
    let configs = self.auth_config_repo.get_by_domain_ids(domain_ids).await?;

    // Build lookup map: domain_id -> has_auth_methods
    let mut result: HashMap<Uuid, bool> = HashMap::with_capacity(domain_ids.len());

    for config in configs {
        let has_methods = config.magic_link_enabled || config.google_oauth_enabled;
        result.insert(config.domain_id, has_methods);
    }

    // Domains without explicit config default to true (magic_link_enabled: true)
    // This matches get_auth_config's synthetic default behavior
    for &domain_id in domain_ids {
        result.entry(domain_id).or_insert(true);
    }

    Ok(result)
}
```

**Key design decisions:**
- Method is `pub(crate)` (not public) to discourage misuse outside the application layer
- Name includes `for_owner_domains` to signal authorization assumption
- Doc comment explicitly warns that ownership is not verified
- Input comes from `list_domains` which already returns unique domain IDs (no deduplication needed)
- Defaults to `true` matching existing `get_auth_config` behavior
- Uses `HashMap::with_capacity` for efficiency

### Step 4: Update the route handler

**File:** `apps/api/src/adapters/http/routes/domain.rs`

Modify `list_domains` to batch-fetch auth configs. The response ordering is preserved because we iterate over `domains` in original order and use the map for lookup only:

```rust
async fn list_domains(
    State(app_state): State<AppState>,
    jar: CookieJar,
) -> AppResult<impl IntoResponse> {
    let (_, user_id) = current_user(&jar, &app_state)?;

    let domains = app_state.domain_use_cases.list_domains(user_id).await?;

    // Collect verified domain IDs for batch auth config fetch
    // list_domains already scopes domains to this owner, so authorization is handled
    let verified_domain_ids: Vec<Uuid> = domains
        .iter()
        .filter(|d| d.status == DomainStatus::Verified)
        .map(|d| d.id)
        .collect();

    // Batch fetch auth method status for all verified domains
    let auth_methods_map = app_state
        .domain_auth_use_cases
        .has_auth_methods_for_owner_domains(&verified_domain_ids)
        .await?;

    let mut response: Vec<DomainResponse> = Vec::with_capacity(domains.len());

    for d in domains {
        let dns_records = app_state.domain_use_cases.get_dns_records(&d.domain, d.id);

        // Use batch result for verified domains, default true for non-verified
        let has_auth_methods = if d.status == DomainStatus::Verified {
            auth_methods_map.get(&d.id).copied().unwrap_or(true)
        } else {
            true // Non-verified domains don't need this warning
        };

        response.push(DomainResponse {
            id: d.id,
            domain: d.domain,
            status: d.status.as_str().to_string(),
            dns_records: Some(DnsRecordsResponse {
                cname_name: dns_records.cname_name,
                cname_value: dns_records.cname_value,
                txt_name: dns_records.txt_name,
                txt_value: dns_records.txt_value,
            }),
            verified_at: d.verified_at,
            created_at: d.created_at,
            has_auth_methods,
        });
    }

    Ok(Json(response))
}
```

**Ordering preserved:** The `for d in domains` loop iterates in the original order from `list_domains`. The `HashMap` lookup does not affect ordering.

## Files to Modify

| File | Change |
|------|--------|
| `apps/api/src/application/use_cases/domain_auth.rs` | Add `get_by_domain_ids` to trait, add `has_auth_methods_for_owner_domains` method, add `use std::collections::HashMap` |
| `apps/api/src/adapters/persistence/domain_auth_config.rs` | Implement `get_by_domain_ids` (minimal SELECT) |
| `apps/api/src/adapters/http/routes/domain.rs` | Update `list_domains` to use batch method |

## Testing Approach

### Unit Tests for Use Case Logic

Add tests in `domain_auth.rs` `#[cfg(test)]` module. This requires adding an in-memory mock for `DomainAuthConfigRepo`:

```rust
#[derive(Default)]
struct InMemoryAuthConfigRepo {
    configs: Mutex<HashMap<Uuid, DomainAuthConfigProfile>>,
}

#[async_trait]
impl DomainAuthConfigRepo for InMemoryAuthConfigRepo {
    async fn get_by_domain_id(&self, domain_id: Uuid) -> AppResult<Option<DomainAuthConfigProfile>> {
        Ok(self.configs.lock().unwrap().get(&domain_id).cloned())
    }

    async fn get_by_domain_ids(&self, domain_ids: &[Uuid]) -> AppResult<Vec<DomainAuthConfigProfile>> {
        let configs = self.configs.lock().unwrap();
        Ok(domain_ids.iter()
            .filter_map(|id| configs.get(id).cloned())
            .collect())
    }

    async fn upsert(&self, domain_id: Uuid, magic_link_enabled: bool, google_oauth_enabled: bool, redirect_url: Option<&str>, whitelist_enabled: bool) -> AppResult<DomainAuthConfigProfile> {
        // ... implementation for test setup
    }

    async fn delete(&self, _domain_id: Uuid) -> AppResult<()> {
        Ok(())
    }
}
```

**Test cases:**

```rust
#[tokio::test]
async fn test_has_auth_methods_for_owner_domains_empty_input() {
    // Setup mock repos...
    let result = use_cases.has_auth_methods_for_owner_domains(&[]).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn test_has_auth_methods_for_owner_domains_with_config() {
    // Mock returns config with magic_link_enabled=true, google_oauth_enabled=false
    let result = use_cases.has_auth_methods_for_owner_domains(&[domain_id]).await.unwrap();
    assert_eq!(result.get(&domain_id), Some(&true));

    // Mock returns config with both disabled
    // ...
    assert_eq!(result.get(&domain_id), Some(&false));
}

#[tokio::test]
async fn test_has_auth_methods_for_owner_domains_missing_config_defaults_to_true() {
    // Mock returns empty vec (no config found)
    let result = use_cases.has_auth_methods_for_owner_domains(&[domain_id]).await.unwrap();
    assert_eq!(result.get(&domain_id), Some(&true)); // Defaults to true
}

#[tokio::test]
async fn test_has_auth_methods_for_owner_domains_mixed_configs() {
    // domain_a: explicit config with magic_link_enabled=true
    // domain_b: explicit config with both disabled
    // domain_c: no config (should default to true)
    let result = use_cases.has_auth_methods_for_owner_domains(&[domain_a, domain_b, domain_c]).await.unwrap();
    assert_eq!(result.get(&domain_a), Some(&true));
    assert_eq!(result.get(&domain_b), Some(&false));
    assert_eq!(result.get(&domain_c), Some(&true)); // default
}
```

### Integration-Style Test for `list_domains`

If feasible (depends on test infrastructure), add an integration test that:
1. Creates 3 test domains (verified)
2. Configures: domain_a with magic_link on, domain_b with both off, domain_c with no config
3. Calls `list_domains` and verifies:
   - domain_a: `has_auth_methods = true`
   - domain_b: `has_auth_methods = false`
   - domain_c: `has_auth_methods = true` (default)
4. Verifies response order matches creation order

### Manual Testing

1. Start local infra: `./run infra:full`
2. Seed local domain: `./run dev:seed`
3. Start API with SQL logging enabled: `RUST_LOG=sqlx=debug ./run api`
4. Create multiple test domains via UI
5. Verify the domain list endpoint:
   - Check browser dev tools Network tab for response
   - Observe SQL logs: should see single `SELECT ... WHERE domain_id = ANY($1)` instead of N separate queries
   - Verify response ordering and `has_auth_methods` values

### Pre-deploy Verification

```bash
./run api:build
```

## Edge Cases

| Case | Expected Behavior |
|------|-------------------|
| No domains | Returns empty list; batch method receives empty array (early return) |
| No verified domains | `verified_domain_ids` is empty; batch returns empty map; all domains get `has_auth_methods: true` |
| Domains without auth config | Not in DB result; defaults to `true` (matches `get_auth_config` default) |
| Mixed verified/unverified | Only verified domains are batch-fetched; unverified always return `true` |
| Response ordering | Preserved: we iterate `domains` in original order, use map for lookup only |

## Security Considerations

### Authorization Model

- **`list_domains`** already scopes results to the authenticated user's domains (`owner_end_user_id`). This is the authorization gate.
- The new batch method intentionally **skips ownership checks** because it operates on already-authorized IDs.
- The method is `pub(crate)` and named `for_owner_domains` to signal this assumption.
- **Misuse risk:** If this method is later called with user-supplied domain IDs, it would leak auth config info. The naming and visibility mitigate this, but a future refactor should consider passing `owner_end_user_id` for defense-in-depth if the method needs to become public.

### Default Behavior Risk

Defaulting to `true` for missing configs perpetuates the current behavior, which may hide missing fallback email configuration. This is intentional to avoid behavior changes, but stakeholders should be aware that `has_auth_methods: true` doesn't guarantee email will actually work — it just means magic link is assumed enabled by default.

## Performance Impact

- **Before**: N queries for N domains (1 query per domain)
- **After**: 1 query regardless of N (batch query with `ANY($1)`)
- **Expected improvement**: Linear reduction in database round trips

## Column Selection Rationale

The batch query only selects `id`, `domain_id`, `magic_link_enabled`, `google_oauth_enabled` because:
- Only `magic_link_enabled` and `google_oauth_enabled` are needed for `has_auth_methods` logic
- Unused fields: `redirect_url`, `whitelist_enabled`, `access_token_ttl_secs`, `refresh_token_ttl_days`, `created_at`, `updated_at`
- Reduces payload size on large domain lists
- If future callers need full profiles, they can add a separate `get_full_by_domain_ids` method

## Mock/Test Repository Updates

The `DomainAuthConfigRepo` trait gains a new method. If any test mocks exist that implement this trait, they must be updated to include `get_by_domain_ids`. The plan includes an in-memory mock implementation for unit tests.

## Revision History

- 2026-01-01: Initial plan (v1) created
- 2026-01-01: Revision 2 addressing feedback:
  - Documented behavior parity decision (preserve `get_auth_config` defaults, not `get_public_config`)
  - Made batch method `pub(crate)` with explicit naming (`for_owner_domains`) to signal authorization assumption
  - Added doc comment warning about ownership checks
  - Added input deduplication
  - Added unit test outline for use-case logic (not just repo)
  - Added security considerations section
- 2026-01-01: Revision 3 addressing feedback:
  - Clarified that mock/test repo implementations must be updated for new trait method
  - Added in-memory mock implementation for unit tests
  - Explained `row_to_profile` replacement with minimal struct initialization (only needed columns)
  - Added note that only verified domains trigger `has_auth_methods` lookup (matches current route logic)
  - Added integration-style test outline verifying response shape and ordering
  - Documented how to enable SQL logging (`RUST_LOG=sqlx=debug`) for manual testing
  - Explicitly confirmed response ordering is preserved (iterate domains, use map for lookup)
  - Reduced SELECT columns to only those needed, explained rationale
  - Removed unnecessary deduplication (input from `list_domains` is already unique)
  - Added section on default behavior risk (stakeholder awareness)
