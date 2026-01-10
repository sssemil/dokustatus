# Plan v1 - Fix token hash collision risk

Summary
Update the domain-bound token hashing to be unambiguous (length-prefix or delimiter-based) so raw token + domain inputs cannot collide, and add regression tests. Optionally support a short-lived legacy hash during deploy to avoid breaking in-flight magic links.

Step-by-step implementation approach
1. Locate `hash_domain_token` in `apps/api/src/application/use_cases/domain_auth.rs` and change the hash input format to be unambiguous (preferred: length-prefix each part before hashing).
2. If backward compatibility is desired, add a `hash_domain_token_legacy` helper and update `consume_magic_link` to attempt the new hash first, then fall back to legacy hash for existing Redis entries.
3. Add unit tests in the existing `#[cfg(test)]` module to verify no collision between ambiguous input pairs and (if added) that the legacy hash path still works during transition.
4. Double-check any other callers of `hash_domain_token` (currently only magic link generation/consumption) to ensure they stay consistent with the new hashing format.

Files to modify
- `apps/api/src/application/use_cases/domain_auth.rs` (hash helper, magic link consumption, tests)

Testing approach
- Run `./run api:test` or a focused `cargo test domain_auth::tests` to cover the new hash helper and collision regression.
- If legacy fallback is added, include a test that simulates old-hash Redis lookup and confirms it still consumes correctly.

Edge cases to handle
- In-flight magic links created before deploy: decide whether to support legacy hash for the TTL window or accept invalidation.
- Ambiguous concatenation pairs (e.g., raw="ab", domain="c" vs raw="a", domain="bc").
- Ensure chosen delimiter or length-prefix approach cannot be confused by token/domain contents (length-prefix avoids delimiter collisions).
