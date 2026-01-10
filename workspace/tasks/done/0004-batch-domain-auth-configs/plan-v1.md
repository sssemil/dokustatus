# Implementation Plan: Batch Domain Auth Configs (N+1 Fix)

## Summary

The `list_domains` endpoint in `apps/api/src/adapters/http/routes/domain.rs` (line 211-257) has an N+1 query issue. For each domain returned, it calls `get_auth_config()` individually to check whether auth methods are enabled (`has_auth_methods` field). This results in N additional database queries for N domains.

The fix involves adding a batch method to fetch auth configs for multiple domain IDs in a single query, then using that in the route handler.

## Current Problem Location

**File:** `apps/api/src/adapters/http/routes/domain.rs:221-238`

```rust
for d in domains {
    // ...
    // Check if domain has any auth methods enabled (only matters for verified domains)
    let has_auth_methods = if d.status == DomainStatus::Verified {
        if let Ok(Some(config)) = app_state
            .domain_auth_use_cases
            .get_auth_config(user_id, d.id)  // <-- N+1: called per domain
            .await
            .map(|(cfg, _)| Some(cfg))
        {
            config.magic_link_enabled || config.google_oauth_enabled
        } else {
            false
        }
    } else {
        true // Non-verified domains don't need this warning
    };
    // ...
}
```

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
        "SELECT id, domain_id, magic_link_enabled, google_oauth_enabled, redirect_url, whitelist_enabled, access_token_ttl_secs, refresh_token_ttl_days, created_at, updated_at FROM domain_auth_config WHERE domain_id = ANY($1)",
    )
    .bind(domain_ids)
    .fetch_all(&self.pool)
    .await
    .map_err(AppError::from)?;
    Ok(rows.into_iter().map(row_to_profile).collect())
}
```

### Step 3: Add batch helper method to `DomainAuthUseCases`

**File:** `apps/api/src/application/use_cases/domain_auth.rs`

Add a public method that returns a HashMap for O(1) lookup:

```rust
use std::collections::HashMap;

/// Batch fetch auth configs for multiple domain IDs.
/// Returns a map from domain_id to whether auth methods are enabled.
/// Domains without explicit config get default based on fallback availability.
pub async fn get_auth_methods_enabled_batch(
    &self,
    domain_ids: &[Uuid],
) -> AppResult<HashMap<Uuid, bool>> {
    // Fetch all configs in one query
    let configs = self.auth_config_repo.get_by_domain_ids(domain_ids).await?;

    // Build lookup map: domain_id -> has_auth_methods
    let mut result: HashMap<Uuid, bool> = HashMap::new();

    for config in configs {
        let has_methods = config.magic_link_enabled || config.google_oauth_enabled;
        result.insert(config.domain_id, has_methods);
    }

    // For domains not in result, check if fallbacks are available
    let has_magic_link_fallback = self.fallback_resend_api_key.is_some()
        && self.fallback_email_domain.is_some();
    let has_google_fallback = self.has_google_oauth_fallback();
    let default_has_methods = has_magic_link_fallback || has_google_fallback;

    for domain_id in domain_ids {
        result.entry(*domain_id).or_insert(default_has_methods);
    }

    Ok(result)
}
```

### Step 4: Update the route handler

**File:** `apps/api/src/adapters/http/routes/domain.rs`

Modify `list_domains` to batch-fetch auth configs:

```rust
async fn list_domains(
    State(app_state): State<AppState>,
    jar: CookieJar,
) -> AppResult<impl IntoResponse> {
    let (_, user_id) = current_user(&jar, &app_state)?;

    let domains = app_state.domain_use_cases.list_domains(user_id).await?;

    // Collect verified domain IDs for batch auth config fetch
    let verified_domain_ids: Vec<Uuid> = domains
        .iter()
        .filter(|d| d.status == DomainStatus::Verified)
        .map(|d| d.id)
        .collect();

    // Batch fetch auth method status for all verified domains
    let auth_methods_map = app_state
        .domain_auth_use_cases
        .get_auth_methods_enabled_batch(&verified_domain_ids)
        .await?;

    let mut response: Vec<DomainResponse> = Vec::with_capacity(domains.len());

    for d in domains {
        let dns_records = app_state.domain_use_cases.get_dns_records(&d.domain, d.id);

        // Use batch result for verified domains, default true for non-verified
        let has_auth_methods = if d.status == DomainStatus::Verified {
            auth_methods_map.get(&d.id).copied().unwrap_or(false)
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

### Step 5: Add import for HashMap

**File:** `apps/api/src/adapters/http/routes/domain.rs`

Add to imports at top of file:

```rust
use std::collections::HashMap;  // May not be needed if not used in route file itself
```

Note: The HashMap is used inside the use case, not in the route file directly, so this import is not needed in domain.rs.

## Files to Modify

| File | Change |
|------|--------|
| `apps/api/src/application/use_cases/domain_auth.rs` | Add `get_by_domain_ids` to trait, add `get_auth_methods_enabled_batch` method |
| `apps/api/src/adapters/persistence/domain_auth_config.rs` | Implement `get_by_domain_ids` |
| `apps/api/src/adapters/http/routes/domain.rs` | Update `list_domains` to use batch method |

## Testing Approach

### Unit Testing

The repository method can be tested with the existing test infrastructure if any DB integration tests exist. The key behavior to verify:

1. **Empty input returns empty result**
2. **Multiple domain IDs returns correct configs**
3. **Unknown domain IDs are simply missing from result (no error)**

### Manual Testing

1. Start local infra: `./run infra:full`
2. Seed local domain: `./run dev:seed`
3. Start API: `./run api`
4. Create multiple test domains via UI
5. Verify the domain list endpoint:
   - Check browser dev tools Network tab
   - Confirm only one query for auth configs (via SQL logging or observing response time)

### Pre-deploy Verification

```bash
./run api:build
```

## Edge Cases

1. **No domains**: Returns empty list, batch method receives empty array (handled with early return)
2. **No verified domains**: `verified_domain_ids` is empty, batch method returns empty map, all domains get `has_auth_methods: true`
3. **Domains without auth config**: Not in the map; defaults to checking fallback availability
4. **Mixed verified/unverified**: Only verified domains are batch-fetched; unverified always return true
5. **Fallback config available**: If no explicit config exists but fallback is available, method should return true

## Performance Impact

- **Before**: N queries for N domains (1 query per domain)
- **After**: 1 query regardless of N (batch query with `ANY($1)`)
- **Expected improvement**: Linear reduction in database round trips

## Revision History

- 2026-01-01: Initial plan created
