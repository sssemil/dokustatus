# Plan: Remove Legacy Functions

**Related Ticket:** [ticket.md](./ticket.md)

## Summary

Remove legacy functions and types that were kept for backwards compatibility but are no longer needed. The primary target is `hash_domain_token_legacy` in the Rust API, which was a transitional fallback for magic links during the token hash collision fix (task 0002). Additionally, there are legacy type aliases in the TypeScript UI (`StripeMode`, `StripeConfig`) that duplicate newer types.

## Identified Legacy Items

### 1. Rust API: `hash_domain_token_legacy` (High Priority)

**Location:** `apps/api/src/application/use_cases/domain_auth.rs:1496-1503`

**What it does:** Implements the old (vulnerable) token hashing that concatenated raw token + domain without length-prefixing. This was kept as a fallback in `consume_magic_link_from_store` to honor magic links created before the fix was deployed.

**Why it can be removed:** Magic links have a configurable TTL (`MAGIC_LINK_TTL_MINUTES`, default 15 minutes, see `apps/api/src/infra/config.rs:15`). Since task 0002 was completed on 2026-01-01, any in-flight magic links using the old hash format have long expired. The legacy fallback is no longer needed. Additionally, Redis automatically expires magic link entries after their TTL, so there are no lingering legacy-hash entries to clean up.

**Affected code:**
- `hash_domain_token_legacy` function (lines 1496-1503): DELETE
- `consume_magic_link_from_store` function (lines 1505-1523): SIMPLIFY - remove legacy fallback logic
- Test: `test_hash_domain_token_avoids_collisions` (lines 2028-2042): UPDATE - remove legacy hash assertions
- Test: `test_consume_magic_link_falls_back_to_legacy_hash` (lines 2044-2066): DELETE entirely

### 2. TypeScript UI: Legacy Type Aliases (Low Priority)

**Location:** `apps/ui/types/billing.ts`

**Items:**
- `StripeMode` (line 35): Duplicate of `PaymentMode` (line 8) - both are `'test' | 'live'`
- `StripeConfig` interface (lines 49-54): Unused legacy interface

**Assessment:**
- `StripeMode` is actively used in the codebase (12 references in billing.ts, 4 in domains page). While it's a duplicate of `PaymentMode`, replacing it would require updating multiple files and the interface names are semantically appropriate in their contexts (Stripe-specific config vs generic payment mode).
- `StripeConfig` appears to be defined but never imported/used anywhere.

**Recommendation:**
- Keep `StripeMode` for now - it's used and semantically correct in Stripe-specific contexts
- Remove `StripeConfig` interface if truly unused (verify with grep)

## Step-by-Step Implementation

### Phase 1: Remove Rust Legacy Hash Function

1. **Remove `hash_domain_token_legacy` function**
   - File: `apps/api/src/application/use_cases/domain_auth.rs`
   - Delete lines 1496-1503 (the function and its doc comment)

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
   - Remove references to `hash_domain_token_legacy`
   - Retain the test to verify the length-prefixed hash avoids collisions (this ensures the bug fix from task 0002 remains covered):
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
   - Delete the entire test (lines 2044-2066)
   - This test specifically verified legacy fallback behavior which is being removed

### Phase 2: Remove Unused TypeScript Legacy Interface (Optional)

1. **Verify `StripeConfig` is unused**
   - Run: `grep -r "StripeConfig" apps/ui --include="*.ts" --include="*.tsx" | grep -v "StripeConfigStatus" | grep -v "types/billing.ts"`
   - If no results, the interface is safe to remove

2. **Remove `StripeConfig` interface if unused**
   - File: `apps/ui/types/billing.ts`
   - Delete lines 48-54 (the comment and interface)

3. **Keep `StripeMode` type alias**
   - It's actively used and semantically appropriate
   - No action needed

## Testing Approach

### Rust API Tests

1. **Run existing tests to verify no regressions:**
   ```bash
   cd apps/api && cargo test domain_auth::tests
   ```

2. **Run full test suite:**
   ```bash
   cd apps/api && cargo test
   ```

3. **Build verification:**
   ```bash
   ./run api:build
   ```

### Manual Verification

1. Magic link flow still works end-to-end:
   - Start local dev environment
   - Trigger magic link send
   - Click magic link
   - Verify authentication succeeds

## Edge Cases & Considerations

1. **In-flight magic links**: Not a concern since task 0002 was deployed long ago (same day as today's date shows 2026-01-01, but the TTL for magic links is minutes, not days).

2. **Hash format stability**: The new `hash_domain_token` function uses length-prefixed hashing which is stable. No migration needed.

3. **Backwards compatibility**: None needed. All magic links in Redis are already using the new hash format.

4. **Database migrations**: None required. The hash is stored in Redis with TTL expiration. Redis automatically removes expired entries.

5. **Call site verification**: Confirmed via grep that `hash_domain_token_legacy` is only called from:
   - `consume_magic_link_from_store` (the fallback logic being removed)
   - Test functions (being updated/deleted)

   And `consume_magic_link_from_store` is only called from one place in production code (line 478).

## Files to Modify

| File | Action |
|------|--------|
| `apps/api/src/application/use_cases/domain_auth.rs` | Remove legacy hash function, simplify consume function, update/delete tests |
| `apps/ui/types/billing.ts` | Remove unused `StripeConfig` interface (optional) |

## Verification Checklist

- [ ] `hash_domain_token_legacy` function removed
- [ ] `consume_magic_link_from_store` simplified (no legacy fallback)
- [ ] Legacy collision test updated to remove legacy assertions
- [ ] Legacy fallback test deleted
- [ ] `cargo test` passes
- [ ] `cargo build` succeeds
- [ ] No compiler warnings about unused code
