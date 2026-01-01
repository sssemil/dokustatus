# Feedback: Plan v1 Review

## What's good

- Clear scope and prioritization, with the Rust legacy hash removal called out as primary.
- Specific file/line references and before/after snippets make the edits straightforward to apply.
- Good rationale for removing the legacy path, tied to TTL and Redis expiry.
- Test impact is explicitly called out, with collision coverage retained.
- Optional TS cleanup is isolated and low-risk, which keeps the main change focused.

## What's missing or unclear

- TTL assumptions: the plan cites `MAGIC_LINK_TTL_MINUTES` default 15, but does not confirm the runtime/prod value or whether it has ever been configured longer. If TTL was increased (hours/days), the legacy path might still be needed for a period after deployment.
- Release timing: the rationale relies on the fix having been deployed long enough ago. There is no check that the legacy-hash removal is occurring well past the maximum TTL in production.
- Redis TTL enforcement: the plan assumes all magic links are stored with an expiry. It does not confirm the store uses `EXPIRE`/`SETEX` in all code paths (or that there are no non-expiring keys).
- Search scope for `StripeConfig`: the plan only greps `apps/ui`, but `StripeConfig` could be referenced via imports from other packages (e.g., demo UI, shared libs) if path aliases are used.
- Call-site verification is asserted but not backed by a concrete command or reference; a `rg` or `rg --type rust` check should be recorded.

## Suggested improvements

- Add a step to verify runtime TTL and deployment timing (e.g., check env defaults or config for production) before removing the legacy path.
- Add a quick check to confirm the magic link store sets expirations (e.g., locate the Redis `set`/`set_ex` call in the store implementation).
- Replace the `grep` command with `rg` and widen the search to the whole repo for `StripeConfig` (and for `hash_domain_token_legacy`).
- Note if any docs/comments mention the legacy hash; remove or update those references if present.
- Include `./run api:build` in the verification checklist (it is called out in repo guidelines and pre-deploy verification).

## Risks / concerns

- If production TTLs are longer than assumed, removing the legacy fallback could break in-flight magic links for users who received links before this change.
- If any magic link keys are not expiring due to a bug in the store, users with older links could fail to authenticate after this change.
- Removing `StripeConfig` without a repo-wide search could break a build if another package imports it via path aliasing.
