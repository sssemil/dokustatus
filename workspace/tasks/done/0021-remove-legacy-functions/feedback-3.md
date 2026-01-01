# Feedback for plan-v3

## What's good about the plan
- Clear scope tied to explicit targets (`hash_domain_token_legacy`, legacy fallback, and `StripeConfig`).
- Strong discovery step with documented `rg` outputs and a final itemized removal list.
- Pre-removal verification is thoughtful (TTL checks, Redis `set_ex` usage).
- Implementation steps are concrete with before/after snippets for the Rust change.
- Verification checklist is comprehensive and matches the task checklist.

## What's missing or unclear
- `StripeConfig` usage is only checked within `apps/ui/`; there is no repo-wide search to confirm it is unused across `libs/` or `apps/demo_*`.
- The `cargo check` command in Phase 3 might fail without `SQLX_OFFLINE=true` or a running DB; the plan does not specify how to avoid false failures.
- The plan notes `StripeMode` is kept but leaves a “Legacy” comment in place; it is unclear whether that comment should be removed to avoid confusion.

## Suggested improvements
- Add a repo-wide search for `StripeConfig` (e.g., `rg 'StripeConfig'`) to conclusively prove it is unused outside `apps/ui/`.
- Replace `cargo check` with `SQLX_OFFLINE=true cargo check` or omit it in favor of `./run api:build` to avoid SQLx offline issues.
- Decide explicitly whether to keep or remove the “Legacy type alias” comment on `StripeMode` to reduce ambiguity.

## Risks or concerns
- If prod env overrides `MAGIC_LINK_TTL_MINUTES` outside the repo (secrets or host env), the TTL assumptions could be wrong; consider verifying with deploy config or ops notes before removing the legacy fallback.
- If in-flight magic links were generated with the legacy hash within the TTL window, some users will need to re-request links; low impact but worth coordinating with deploy timing.
