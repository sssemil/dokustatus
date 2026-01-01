# Plan: Remove Legacy Functions (v2)

**Related Ticket:** [ticket.md](./ticket.md)
**Previous Plan:** [plan-v1.md](./plan-v1.md)
**Feedback Addressed:** [feedback-1.md](./feedback-1.md)

## Summary

Remove legacy functions and types that were kept for backwards compatibility but are no longer needed. The primary target is `hash_domain_token_legacy` in the Rust API, which was a transitional fallback for magic links during the token hash collision fix (task 0002). Additionally, there is an unused legacy interface (`StripeConfig`) in the TypeScript UI.

## Feedback Addressed in This Revision

| Feedback Item | How Addressed |
|---------------|---------------|
| TTL assumptions not verified | Added verification step with evidence from code and config analysis |
| Release timing not confirmed | Added explicit timing verification pre-step |
| Redis TTL enforcement not confirmed | Verified `set_ex` usage in `domain_magic_links.rs:58` |
| Search scope too narrow for StripeConfig | Widened to repo-wide search using `rg` |
| Call-site verification not backed by command | Added concrete `rg` commands with results |
| No check for docs/comments mentioning legacy hash | Added search and found comment on line 1496 to remove |
| Missing `./run api:build` in verification | Added to verification checklist |

## Pre-Removal Verification (NEW)

Before proceeding with removal, verify the following conditions are met:

### 1. TTL Verification

**Evidence gathered:**
- `MAGIC_LINK_TTL_MINUTES` default is 15 minutes (`apps/api/src/infra/config.rs:52`)
- `.env.example` shows `MAGIC_LINK_TTL_MINUTES=15` as the expected value
- No override found in `infra/` deployment configs (grep returned no matches)
- Conclusion: Production uses default 15-minute TTL

**Redis TTL enforcement verified:**
- `apps/api/src/infra/domain_magic_links.rs:58` uses `set_ex(key, json, ttl_secs)`
- This is the only code path for saving magic links, confirming all entries have TTL
- Redis automatically evicts expired keys; no non-expiring magic link keys exist

### 2. Deployment Timing

**Timeline:**
- Task 0002 (collision fix with legacy fallback) was completed on 2026-01-01
- Current date is 2026-01-01 (same day)
- **Issue:** If this is truly the same day, 15 minutes may not have elapsed since deployment

**Required verification before execution:**
- [ ] Confirm task 0002 was deployed to production
- [ ] Confirm at least 15 minutes (or configured TTL) have elapsed since deployment
- If not enough time has elapsed, delay execution or accept that a small number of in-flight magic links may fail

### 3. Call-Site Verification (Repo-Wide)

**Command run:** `rg 'hash_domain_token_legacy' --type rust`

**Results (source code only):**
```
apps/api/src/application/use_cases/domain_auth.rs:1497:fn hash_domain_token_legacy(raw: &str, domain: &str) -> String {
apps/api/src/application/use_cases/domain_auth.rs:1517:    let legacy_hash = hash_domain_token_legacy(raw_token, domain_name);
apps/api/src/application/use_cases/domain_auth.rs:2035:        let legacy_a = hash_domain_token_legacy(raw_a, domain_a);
apps/api/src/application/use_cases/domain_auth.rs:2036:        let legacy_b = hash_domain_token_legacy(raw_b, domain_b);
apps/api/src/application/use_cases/domain_auth.rs:2053:        let legacy_hash = hash_domain_token_legacy(raw_token, domain);
```

**Conclusion:** Function is only used in:
1. `consume_magic_link_from_store` (line 1517) - being simplified
2. Test functions (lines 2035, 2036, 2053) - being updated/deleted

### 4. StripeConfig Verification (Repo-Wide)

**Command run:** `rg 'StripeConfig' --type ts --type tsx` (and checked all matches)

**Analysis of matches:**
- `StripeConfig` (the exact interface, lines 49-54 in billing.ts) - defined but never imported
- `StripeConfigStatus` - actively used (different interface, keep it)
- `UpdateStripeConfigInput` - actively used (different interface, keep it)
- `DeleteStripeConfigInput` - actively used (different interface, keep it)
- `BillingStripeConfig` (Rust) - completely separate entity, not TypeScript

**Conclusion:** `StripeConfig` interface at `apps/ui/types/billing.ts:49-54` is safe to remove. No imports found.

## Identified Legacy Items

### 1. Rust API: `hash_domain_token_legacy` (High Priority)

**Location:** `apps/api/src/application/use_cases/domain_auth.rs`

**Items to remove/modify:**
| Line(s) | Item | Action |
|---------|------|--------|
| 1496 | Doc comment `/// Legacy hash format...` | DELETE |
| 1497-1503 | `hash_domain_token_legacy` function | DELETE |
| 1505-1523 | `consume_magic_link_from_store` function | SIMPLIFY |
| 2028-2042 | `test_hash_domain_token_avoids_collisions` | UPDATE (remove legacy refs) |
| 2044-2066 | `test_consume_magic_link_falls_back_to_legacy_hash` | DELETE |

### 2. TypeScript UI: `StripeConfig` Interface (Low Priority)

**Location:** `apps/ui/types/billing.ts:48-54`

**Items to remove:**
| Line(s) | Item | Action |
|---------|------|--------|
| 48 | Comment `// Legacy interface...` | DELETE |
| 49-54 | `StripeConfig` interface | DELETE |

**Note:** Keep `StripeMode` (line 35) - it's actively used in 16+ locations and is semantically appropriate for Stripe-specific contexts.

## Step-by-Step Implementation

### Phase 0: Pre-Flight Checks

1. **Verify deployment timing**
   ```bash
   # Check when task 0002 was completed
   cat workspace/tasks/done/0002-fix-token-hash-collision/ticket.md | grep -i "history" -A 10
   ```
   - Confirm legacy fallback has been in production for > 15 minutes
   - If unclear, wait or accept minor disruption to in-flight links

### Phase 1: Remove Rust Legacy Hash Function

1. **Remove `hash_domain_token_legacy` function and its doc comment**
   - File: `apps/api/src/application/use_cases/domain_auth.rs`
   - Delete lines 1496-1503

2. **Simplify `consume_magic_link_from_store` function**
   - File: `apps/api/src/application/use_cases/domain_auth.rs`
   - Before (lines 1505-1523):
     ```rust
     async fn consume_magic_link_from_store(
         magic_link_store: &dyn DomainMagicLinkStore,
         raw_token: &str,
         domain_name: &str,
         session_id: &str,
     ) -> AppResult<Option<DomainMagicLinkData>> {
         let token_hash = hash_domain_token(raw_token, domain_name);
         let data = magic_link_store.consume(&token_hash, session_id).await?;
         if data.is_some() {
             return Ok(data);
         }

         let legacy_hash = hash_domain_token_legacy(raw_token, domain_name);
         if legacy_hash == token_hash {
             return Ok(None);
         }

         magic_link_store.consume(&legacy_hash, session_id).await
     }
     ```
   - After:
     ```rust
     async fn consume_magic_link_from_store(
         magic_link_store: &dyn DomainMagicLinkStore,
         raw_token: &str,
         domain_name: &str,
         session_id: &str,
     ) -> AppResult<Option<DomainMagicLinkData>> {
         let token_hash = hash_domain_token(raw_token, domain_name);
         magic_link_store.consume(&token_hash, session_id).await
     }
     ```

3. **Update `test_hash_domain_token_avoids_collisions` test**
   - File: `apps/api/src/application/use_cases/domain_auth.rs`
   - Remove legacy hash assertions, keep collision coverage:
     ```rust
     #[test]
     fn test_hash_domain_token_avoids_collisions() {
         // Test case: inputs that would collide with naive concatenation
         // "ab" + "c" vs "a" + "bc" both produce "abc" if naively concatenated
         let raw_a = "ab";
         let domain_a = "c";
         let raw_b = "a";
         let domain_b = "bc";

         // Length-prefixed hashing ensures no collision
         let scoped_a = hash_domain_token(raw_a, domain_a);
         let scoped_b = hash_domain_token(raw_b, domain_b);
         assert_ne!(scoped_a, scoped_b);
     }
     ```

4. **Delete `test_consume_magic_link_falls_back_to_legacy_hash` test**
   - File: `apps/api/src/application/use_cases/domain_auth.rs`
   - Delete the entire test (lines 2044-2066 approximately)

### Phase 2: Remove Unused TypeScript Interface

1. **Remove `StripeConfig` interface**
   - File: `apps/ui/types/billing.ts`
   - Delete lines 48-54:
     ```typescript
     // Legacy interface for backwards compatibility
     export interface StripeConfig {
       publishable_key: string | null;
       has_secret_key: boolean;
       is_connected: boolean;
       // NOTE: No using_fallback field - each domain must configure their own Stripe account.
     }
     ```

### Phase 3: Verification

1. **Run Rust tests:**
   ```bash
   ./run api:test
   ```

2. **Build API (pre-deploy verification):**
   ```bash
   ./run api:build
   ```

3. **Check for unused code warnings:**
   ```bash
   cargo check 2>&1 | grep -i "unused\|dead_code"
   ```

4. **Verify TypeScript builds:**
   ```bash
   ./run ui:build
   ```

## Verification Checklist

- [ ] Pre-flight: Confirmed task 0002 deployed > 15 min ago (or accepted risk)
- [ ] `hash_domain_token_legacy` function removed
- [ ] Doc comment `/// Legacy hash format...` removed
- [ ] `consume_magic_link_from_store` simplified (no legacy fallback)
- [ ] `test_hash_domain_token_avoids_collisions` updated (legacy refs removed)
- [ ] `test_consume_magic_link_falls_back_to_legacy_hash` deleted
- [ ] `StripeConfig` interface removed from `billing.ts`
- [ ] `./run api:test` passes
- [ ] `./run api:build` succeeds
- [ ] `./run ui:build` succeeds
- [ ] No compiler warnings about unused code

## Risks and Mitigations

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| In-flight magic links break if removed too soon | Low (if TTL elapsed) | Verify 15+ min since deploy; fallback: users re-request link |
| Non-expiring Redis keys exist | Very Low | Verified all saves use `set_ex`; Redis evicts expired keys |
| StripeConfig used elsewhere | Very Low | Repo-wide search found no imports; distinct from `StripeConfigStatus` |
| Collision test coverage lost | None | Test updated to retain collision assertions without legacy refs |

## Files Modified

| File | Changes |
|------|---------|
| `apps/api/src/application/use_cases/domain_auth.rs` | Remove legacy hash function + comment, simplify consume function, update/delete tests |
| `apps/ui/types/billing.ts` | Remove unused `StripeConfig` interface + comment |

## Revision History

- v2 (2026-01-01): Addressed feedback - added TTL verification, Redis TTL enforcement check, repo-wide searches, doc comment removal, and `./run api:build` step.
