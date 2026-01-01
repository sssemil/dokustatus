# Remove Legacy Functions

**Plan:** [plan-v3.md](./plan-v3.md)

## Description

Find and remove/refactor out all legacy functions, such as `hash_domain_token_legacy`.

## Checklist

- [x] Remove `hash_domain_token_legacy` function from Rust API
- [x] Simplify `consume_magic_link_from_store` (remove legacy fallback)
- [x] Update collision avoidance test (remove legacy hash assertions)
- [x] Delete legacy fallback test
- [x] Verify tests pass
- [x] Remove unused `StripeConfig` TypeScript interface (optional)

## History

- 2026-01-01 Created task to clean up legacy code from previous fixes.
- 2026-01-01 Created plan-v1.md with detailed implementation approach. Identified primary target (hash_domain_token_legacy in domain_auth.rs) and secondary target (unused StripeConfig in billing.ts). Reviewed by Codex and incorporated feedback about TTL verification, call site validation, and maintaining collision test coverage.
- 2026-01-01 Added plan review feedback in feedback-1.md.
- 2026-01-01 Created plan-v2.md addressing feedback: added TTL verification evidence (15-min default, no prod override), confirmed Redis `set_ex` usage for all magic links, widened StripeConfig search to repo-wide, added pre-flight deployment timing check, included `./run api:build` in verification, and noted doc comment on line 1496 for removal.
- 2026-01-01 12:04 Added plan review feedback in feedback-2.md with gaps, risks, and improvements.
- 2026-01-01 12:15 Created plan-v3.md (revision 3/3) addressing all feedback: added discovery step with full `rg "legacy"` audit, verified no infra/.env TTL override, confirmed no index re-exports for StripeConfig, added `./run api:fmt` step, documented intentionally-kept items (StripeMode), and acknowledged deployment timing gap with explicit risk acceptance.
- 2026-01-01 12:07 Added plan review feedback in feedback-3.md with clarifications, gaps, and risk notes.

- 2026-01-01 12:08 Removed legacy hash fallback and cleaned tests; dropped unused StripeConfig interface.

- 2026-01-01 12:12 Added DomainProfile import for tests; ran api:fmt/test/build and cargo check (warnings), ui:build failed (next missing).

- 2026-01-01 12:14 Removed unused base64 import in domain_auth tests; re-ran api:fmt/test/build and cargo check (warnings persist).

- 2026-01-01 12:16 Checked off checklist after completing removals and verification steps.

- 2026-01-01 12:16 Completed legacy function removal task; ready to move to outbound.

- 2026-01-01 12:18 Moved task files to outbound per completion protocol.
