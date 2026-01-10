# Implementation Plan: Batch Domain Auth Configs (N+1 Fix) — v2

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

    // Deduplicate domain IDs to avoid redundant processing
    let unique_ids: Vec<Uuid> = {
        let mut seen = std::collections::HashSet::new();
        domain_ids.iter().copied().filter(|id| seen.insert(*id)).collect()
    };

    // Fetch all configs in one query
    let configs = self.auth_config_repo.get_by_domain_ids(&unique_ids).await?;

    // Build lookup map: domain_id -> has_auth_methods
    let mut result: HashMap<Uuid, bool> = HashMap::new();

    for config in configs {
        let has_methods = config.magic_link_enabled || config.google_oauth_enabled;
        result.insert(config.domain_id, has_methods);
    }

    // Domains without explicit config default to true (magic_link_enabled: true)
    // This matches get_auth_config's synthetic default behavior
    for domain_id in unique_ids {
        result.entry(domain_id).or_insert(true);
    }

    Ok(result)
}
```

**Key design decisions:**
- Method is `pub(crate)` (not public) to discourage misuse outside the application layer
- Name includes `for_owner_domains` to signal authorization assumption
- Doc comment explicitly warns that ownership is not verified
- Deduplicates input IDs to keep result sizes predictable
- Defaults to `true` matching existing `get_auth_config` behavior

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

## Files to Modify

| File | Change |
|------|--------|
| `apps/api/src/application/use_cases/domain_auth.rs` | Add `get_by_domain_ids` to trait, add `has_auth_methods_for_owner_domains` method, add `use std::collections::{HashMap, HashSet}` |
| `apps/api/src/adapters/persistence/domain_auth_config.rs` | Implement `get_by_domain_ids` |
| `apps/api/src/adapters/http/routes/domain.rs` | Update `list_domains` to use batch method |

## Testing Approach

### Unit Tests for Use Case Logic

Add tests in `domain_auth.rs` `#[cfg(test)]` module:

```rust
#[tokio::test]
async fn test_has_auth_methods_for_owner_domains_empty_input() {
    // Setup mock repos...
    let result = use_cases.has_auth_methods_for_owner_domains(&[]).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn test_has_auth_methods_for_owner_domains_explicit_config() {
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
async fn test_has_auth_methods_for_owner_domains_deduplicates_input() {
    // Pass [id1, id1, id2]
    // Verify repo receives deduplicated list or result handles it correctly
}
```

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

| Case | Expected Behavior |
|------|-------------------|
| No domains | Returns empty list; batch method receives empty array (early return) |
| No verified domains | `verified_domain_ids` is empty; batch returns empty map; all domains get `has_auth_methods: true` |
| Domains without auth config | Not in DB result; defaults to `true` (matches `get_auth_config` default) |
| Mixed verified/unverified | Only verified domains are batch-fetched; unverified always return `true` |
| Duplicate domain IDs in input | Deduplicated before query; each ID appears once in result |

## Security Considerations

### Authorization Model

- **`list_domains`** already scopes results to the authenticated user's domains (`owner_end_user_id`). This is the authorization gate.
- The new batch method intentionally **skips ownership checks** because it operates on already-authorized IDs.
- The method is `pub(crate)` and named `for_owner_domains` to signal this assumption.
- **Misuse risk:** If this method is later called with user-supplied domain IDs, it would leak auth config info. The naming and visibility mitigate this, but a future refactor should consider passing `owner_end_user_id` for defense-in-depth if the method needs to become public.

## Performance Impact

- **Before**: N queries for N domains (1 query per domain)
- **After**: 1 query regardless of N (batch query with `ANY($1)`)
- **Expected improvement**: Linear reduction in database round trips

## Revision History

- 2026-01-01: Initial plan (v1) created
- 2026-01-01: Revision 2 addressing feedback:
  - Documented behavior parity decision (preserve `get_auth_config` defaults, not `get_public_config`)
  - Made batch method `pub(crate)` with explicit naming (`for_owner_domains`) to signal authorization assumption
  - Added doc comment warning about ownership checks
  - Added input deduplication
  - Added unit test outline for use-case logic (not just repo)
  - Added security considerations section
