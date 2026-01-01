# Plan v2 - Fix token hash collision risk

Summary
Make the domain-bound token hash unambiguous by hashing a length-prefixed byte layout, then add regression coverage for collision pairs and (if needed) a short-lived legacy fallback to honor in-flight magic links.

Decisions to confirm
- Legacy compatibility: keep a fallback path for the magic-link TTL window, or accept invalidating existing links on deploy.

Step-by-step implementation approach
1. Re-locate `hash_domain_token` in `apps/api/src/application/use_cases/domain_auth.rs` and review how it is used for both magic link generation and consumption (including the Redis key format).
2. Replace the ambiguous concatenation with a length-prefixed byte layout (e.g., `[u32 len][token bytes][u32 len][domain bytes]`) before hashing so no token/domain pair can collide.
3. If legacy compatibility is desired, add `hash_domain_token_legacy` that preserves the old concatenation and update `consume_magic_link` to try the new hash first, then fall back to the legacy hash for existing entries.
4. Add or update unit tests in the existing `#[cfg(test)]` module to cover:
   - A collision pair that used to match under concatenation but no longer does under the new layout.
   - (If legacy fallback is added) consuming a magic link stored with the old hash.
5. Re-check any other callers of `hash_domain_token` (currently magic link generation/consumption) so both sides use the new format consistently.

Files to modify
- `apps/api/src/application/use_cases/domain_auth.rs` (hash helper, magic link consumption, tests)

Testing approach
- Run `./run api:test` or a focused `cargo test domain_auth::tests` to cover the new hash helper and collision regression.
- If legacy fallback is added, include a test that exercises the fallback path.

Edge cases to handle
- In-flight magic links created before deploy: decide whether to support legacy hash for the TTL window or accept invalidation.
- Tokens/domains containing delimiters or unusual characters: length-prefix layout avoids delimiter collisions.
- Ensure the hash input uses byte lengths (not character count) to avoid UTF-8 ambiguity.
