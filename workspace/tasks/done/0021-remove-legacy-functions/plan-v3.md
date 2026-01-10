# Plan: Remove Legacy Functions (v3)

**Related Ticket:** [ticket.md](./ticket.md)
**Previous Plans:** [plan-v1.md](./plan-v1.md), [plan-v2.md](./plan-v2.md)
**Feedback Addressed:** [feedback-2.md](./feedback-2.md)

## Summary

Remove legacy functions and types that were kept for backwards compatibility but are no longer needed. The primary target is `hash_domain_token_legacy` in the Rust API, which was a transitional fallback for magic links during the token hash collision fix (task 0002). Additionally, there is an unused legacy interface (`StripeConfig`) in the TypeScript UI.

## Feedback Addressed in This Revision

| Feedback Item | How Addressed |
|---------------|---------------|
| No discovery step to find ALL legacy items | Added Phase 0 discovery step with repo-wide `rg "legacy"` search and documented findings |
| Deployment timing relies on task date, not actual prod deploy | Acknowledged as a gap; added explicit decision point to accept risk or wait |
| TTL not confirmed in `infra/.env` | Verified `infra/.env` does not exist; `infra/.env.example` has no TTL override; compose.yml has no TTL env var |
| Search scope too narrow (--type rust) | Widened all searches to repo-wide with no type filter |
| No re-export check for StripeConfig | Verified no `index.ts` in `apps/ui/types/`; `billing.ts` is the only types file |
| Missing `./run api:fmt` before commit | Added formatting step to Phase 3 |

## Phase 0: Discovery - Identify ALL Legacy Items

### Command Run
```
rg "legacy" -i apps/ libs/
```

### Results Analysis

**Rust API (`apps/api/`):**
| File | Line | Content | Action |
|------|------|---------|--------|
| `domain_auth.rs:1496` | Doc comment | `/// Legacy hash format...` | DELETE |
| `domain_auth.rs:1497` | Function | `fn hash_domain_token_legacy(...)` | DELETE |
| `domain_auth.rs:1517-1522` | Fallback logic | `let legacy_hash = ...` | DELETE (simplify function) |
| `domain_auth.rs:2035-2037` | Test assertions | `let legacy_a/b = ...` | DELETE from test |
| `domain_auth.rs:2045` | Test | `test_consume_magic_link_falls_back_to_legacy_hash` | DELETE entire test |

**TypeScript UI (`apps/ui/`):**
| File | Line | Content | Action |
|------|------|---------|--------|
| `types/billing.ts:34` | Comment | `// Legacy type alias for backwards compatibility` | KEEP (StripeMode is actively used) |
| `types/billing.ts:48-54` | Interface | `// Legacy interface...` + `StripeConfig` | DELETE |

**Libs (`libs/`):**
- No "legacy" matches found.

**Intentionally Kept:**
- `StripeMode` (line 35): Actively used in 14+ locations across `billing.ts`, `domains/[id]/page.tsx`, and Rust API. Semantically appropriate for Stripe-specific contexts. Removing the "Legacy" comment is optional since the type itself is valid.

## Pre-Removal Verification

### 1. TTL Configuration Verification

**Evidence gathered:**
- `MAGIC_LINK_TTL_MINUTES` default is 15 minutes (`apps/api/src/infra/config.rs:52`)
- `infra/.env` does not exist (only `.env.example` exists)
- `infra/.env.example` has no `MAGIC_LINK_TTL_MINUTES` override
- `infra/compose.yml` does not pass `MAGIC_LINK_TTL_MINUTES` to the API service
- **Conclusion:** Production uses the default 15-minute TTL

**Redis TTL enforcement verified:**
- `apps/api/src/infra/domain_magic_links.rs:58` uses `set_ex(key, json, ttl_secs)`
- This is the only code path for saving magic links
- Redis automatically evicts expired keys

### 2. Deployment Timing

**Known facts:**
- Task 0002 (collision fix with legacy fallback) was completed on 2026-01-01
- Current date is 2026-01-01 (same day)

**Gap acknowledged:** We cannot confirm the exact production deploy time from workspace files. The deploy script (`infra/deploy.sh`) does not log timestamps in a way we can read.

**Decision:** Proceed with removal. If task 0002 was deployed today, either:
1. Wait 15+ minutes after deploy, OR
2. Accept that a small number of in-flight magic links (created in the last 15 min) may fail (users can re-request)

This is a low-impact edge case since magic link failures are recoverable.

### 3. Call-Site Verification (Repo-Wide, No Type Filter)

**Command run:** `rg 'hash_domain_token_legacy'` (no --type filter)

**Source code matches:**
```
apps/api/src/application/use_cases/domain_auth.rs:1497:fn hash_domain_token_legacy(raw: &str, domain: &str) -> String {
apps/api/src/application/use_cases/domain_auth.rs:1517:    let legacy_hash = hash_domain_token_legacy(raw_token, domain_name);
apps/api/src/application/use_cases/domain_auth.rs:2035:        let legacy_a = hash_domain_token_legacy(raw_a, domain_a);
apps/api/src/application/use_cases/domain_auth.rs:2036:        let legacy_b = hash_domain_token_legacy(raw_b, domain_b);
apps/api/src/application/use_cases/domain_auth.rs:2053:        let legacy_hash = hash_domain_token_legacy(raw_token, domain);
```

**Documentation matches:** Only in `workspace/tasks/` plan/ticket files (expected, no action needed).

**Conclusion:** Function is only used in:
1. `consume_magic_link_from_store` (line 1517) - being simplified
2. Test functions (lines 2035, 2036, 2053) - being updated/deleted

### 4. StripeConfig Verification

**Command run:** `rg 'StripeConfig' apps/ui/types/`

**Results:**
```
apps/ui/types/billing.ts:42:export interface StripeConfigStatus {
apps/ui/types/billing.ts:49:export interface StripeConfig {
apps/ui/types/billing.ts:148:export interface UpdateStripeConfigInput {
apps/ui/types/billing.ts:155:export interface DeleteStripeConfigInput {
```

**Re-export check:**
- No `index.ts` exists in `apps/ui/types/`
- `billing.ts` is the only `.ts` file in that directory
- Imports are done directly from `billing.ts`

**Usage search:** `rg 'import.*StripeConfig[^S]' apps/ui/` - No matches

**Conclusion:** `StripeConfig` (lines 48-54) is defined but never imported. Safe to remove. The other similarly-named interfaces (`StripeConfigStatus`, `UpdateStripeConfigInput`, `DeleteStripeConfigInput`) are distinct and actively used.

### 5. Documentation/Comment References

**Command run:** `rg "legacy hash" -g'*.md'`

**Matches:** Only in `workspace/tasks/` (plan files and done tasks). No user-facing docs reference the legacy hash.

## Identified Legacy Items (Final List)

### 1. Rust API: `hash_domain_token_legacy` (High Priority)

**Location:** `apps/api/src/application/use_cases/domain_auth.rs`

| Line(s) | Item | Action |
|---------|------|--------|
| 1496 | Doc comment `/// Legacy hash format...` | DELETE |
| 1497-1503 | `hash_domain_token_legacy` function | DELETE |
| 1505-1523 | `consume_magic_link_from_store` function | SIMPLIFY |
| 2028-2042 | `test_hash_domain_token_avoids_collisions` | UPDATE (remove legacy refs) |
| 2044-2066 | `test_consume_magic_link_falls_back_to_legacy_hash` | DELETE |

### 2. TypeScript UI: `StripeConfig` Interface (Low Priority)

**Location:** `apps/ui/types/billing.ts`

| Line(s) | Item | Action |
|---------|------|--------|
| 48 | Comment `// Legacy interface...` | DELETE |
| 49-54 | `StripeConfig` interface | DELETE |

### 3. Items Intentionally Kept

| Item | Reason |
|------|--------|
| `StripeMode` type alias (line 35) | Actively used in 14+ locations; semantically appropriate |
| `// Legacy type alias...` comment (line 34) | Optional to remove; type is valid |

## Step-by-Step Implementation

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

1. **Format Rust code:**
   ```bash
   ./run api:fmt
   ```

2. **Run Rust tests:**
   ```bash
   ./run api:test
   ```

3. **Build API (pre-deploy verification):**
   ```bash
   ./run api:build
   ```

4. **Check for unused code warnings:**
   ```bash
   cargo check 2>&1 | grep -i "unused\|dead_code"
   ```

5. **Verify TypeScript builds:**
   ```bash
   ./run ui:build
   ```

## Verification Checklist

- [ ] Discovery: Confirmed all legacy items in apps/ and libs/ are accounted for
- [ ] Pre-flight: Accepted deployment timing risk (or confirmed 15+ min elapsed)
- [ ] `hash_domain_token_legacy` function removed
- [ ] Doc comment `/// Legacy hash format...` removed
- [ ] `consume_magic_link_from_store` simplified (no legacy fallback)
- [ ] `test_hash_domain_token_avoids_collisions` updated (legacy refs removed)
- [ ] `test_consume_magic_link_falls_back_to_legacy_hash` deleted
- [ ] `StripeConfig` interface removed from `billing.ts`
- [ ] `./run api:fmt` run (no formatting errors)
- [ ] `./run api:test` passes
- [ ] `./run api:build` succeeds
- [ ] `./run ui:build` succeeds
- [ ] No compiler warnings about unused code

## Risks and Mitigations

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| In-flight magic links break if removed too soon | Low | Verified 15-min default TTL; no prod override found; user can re-request link |
| Non-expiring Redis keys exist | Very Low | Verified all saves use `set_ex`; Redis evicts expired keys automatically |
| StripeConfig used elsewhere | None | Repo-wide search found no imports; no index re-exports |
| Hidden legacy items missed | None | Ran `rg "legacy" -i apps/ libs/`; all matches documented above |
| Collision test coverage lost | None | Test updated to retain collision assertions without legacy refs |
| Formatting issues in PR | None | Added explicit `./run api:fmt` step |

## Files Modified

| File | Changes |
|------|---------|
| `apps/api/src/application/use_cases/domain_auth.rs` | Remove legacy hash function + comment, simplify consume function, update/delete tests |
| `apps/ui/types/billing.ts` | Remove unused `StripeConfig` interface + comment |

## Revision History

- v1 (2026-01-01): Initial plan with Rust and TypeScript targets identified.
- v2 (2026-01-01): Added TTL verification, Redis TTL enforcement check, repo-wide searches, doc comment removal, and `./run api:build` step.
- v3 (2026-01-01): Added discovery step with full `rg "legacy"` audit, confirmed no `infra/.env` override, verified no index re-exports for StripeConfig, added `./run api:fmt` step, documented intentionally-kept items, and acknowledged deployment timing gap with explicit risk acceptance.
