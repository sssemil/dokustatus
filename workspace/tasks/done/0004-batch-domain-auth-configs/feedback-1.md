# Feedback on plan-v1 (0004-batch-domain-auth-configs)

## What's good
- Pinpoints the exact N+1 path in `apps/api/src/adapters/http/routes/domain.rs` and explains why it happens.
- Clear, incremental steps that touch the right layers (repo, use case, route) and keep the change scoped.
- Batch query approach (`ANY($1)`) matches existing patterns in the codebase and includes an empty-input guard.
- Covers main edge cases and gives a lightweight verification checklist plus pre-deploy build.

## What's missing or unclear
- **Behavior parity for default configs:** `get_auth_config` currently defaults `magic_link_enabled: true` even when no config exists (see `apps/api/src/application/use_cases/domain_auth.rs`). The plan’s fallback-based default (`fallback_resend_api_key`/`fallback_email_domain`/`has_google_oauth_fallback`) changes `has_auth_methods` semantics for domains without a config. This needs an explicit decision.
- **Ownership/authorization assumptions:** `get_auth_config` enforces domain ownership via `domain_repo.get_by_id`. The new batch method skips ownership checks; the plan should explicitly note that `list_domains` already scopes domains to the owner and that this method must only be used with authorized IDs.
- **Test coverage for new logic:** The plan mentions repo tests but not the use-case logic for defaults/fallbacks. That’s where the behavioral change (if any) will live.

## Suggested improvements
- Decide and document the intended default for `has_auth_methods` when no config exists. If you want to preserve current behavior, default to `true` (or mirror `get_auth_config`’s default). If you want to align with `get_public_config`, update the plan to call out the intentional change and update any UI expectations.
- Consider keeping the batch helper private to the use case and naming it to reflect its intended context (e.g., `get_auth_methods_enabled_for_owner_domains`) or pass `owner_end_user_id` and validate ownership in one query if you want defense-in-depth without N+1.
- Add a focused unit test for `get_auth_methods_enabled_batch` covering: empty input, explicit config enabling/disabling, and missing config defaults. That will lock in the chosen behavior and prevent regressions.
- Optionally de-duplicate `domain_ids` before querying to keep result sizes predictable if the input can contain duplicates.

## Risks or concerns
- **Silent behavior change:** Switching default logic to fallback availability could flip `has_auth_methods` from `true` to `false` for verified domains without explicit config, which might surface new warnings in the UI or affect downstream logic.
- **Security assumption drift:** A generic batch method without ownership checks may be reused later in contexts that don’t guarantee authorization. Clarify its intended usage or enforce ownership in the method signature.

