# Plan v3 - Fix token hash collision risk

Summary
Harden the domain-bound token hash by hashing an unambiguous, length-prefixed byte layout, add regression coverage for former collision pairs, and optionally keep a short-lived legacy fallback to honor in-flight magic links.

Decisions to confirm
- Legacy compatibility: keep a fallback path for the magic-link TTL window, or accept invalidating existing links on deploy.

Step-by-step implementation approach
1. Locate `hash_domain_token` in `apps/api/src/application/use_cases/domain_auth.rs` and trace all call sites (magic-link generation + consumption, plus any Redis key usage).
2. Replace the ambiguous concatenation with a length-prefixed byte layout (e.g., `[u32 len][token bytes][u32 len][domain bytes]`) and ensure a fixed endianness is used for the length encoding.
3. Update magic-link generation to use the new hash so newly issued links are stored with the safe format.
4. If legacy compatibility is desired, add `hash_domain_token_legacy` (old concatenation) and have `consume_magic_link` try the new hash first, then fall back to legacy for the TTL window.
5. Add or update unit tests in the existing `#[cfg(test)]` module to cover:
   - A collision pair that used to match under concatenation but no longer does under the new layout.
   - (If legacy fallback is added) consuming a magic link stored with the old hash.
6. Re-check any other callers of `hash_domain_token` so both sides use the new format consistently.

Files to modify
- `apps/api/src/application/use_cases/domain_auth.rs` (hash helper, magic link generation/consumption, tests)

Testing approach
- Run `./run api:test` or a focused `cargo test domain_auth::tests` to cover the new hash helper and collision regression.
- If legacy fallback is added, include a test that exercises the fallback path.

Edge cases to handle
- In-flight magic links created before deploy: decide whether to support legacy hash for the TTL window or accept invalidation.
- Tokens/domains containing delimiters or unusual characters: length-prefix layout avoids delimiter collisions.
- Ensure the hash input uses byte lengths (not character count) to avoid UTF-8 ambiguity.
