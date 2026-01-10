# Feedback on Plan v2 (Task 0021)

## What's good about the plan

- Scope is well-defined with clear primary/secondary targets and explicit file/line references.
- Pre-flight checks for TTL and deployment timing show good risk awareness for magic link behavior.
- Call-site verification and repo-wide `StripeConfig` search are concrete and reproducible.
- Step-by-step edits are specific enough to execute with low ambiguity.
- Verification checklist is thorough and includes `./run api:build` as required.

## What's missing or unclear

- The task asks to remove "all legacy functions," but the plan only targets `hash_domain_token_legacy` and `StripeConfig` without a discovery step to confirm no other legacy functions remain.
- Deployment timing verification relies on task completion date; it does not confirm actual production deploy time or the current runtime TTL value.
- TTL verification is based on defaults and `.env.example`; it does not confirm `MAGIC_LINK_TTL_MINUTES` in `infra/.env` or runtime env for production.
- `rg --type rust` limits search to Rust files; docs/tests in other languages may still reference `hash_domain_token_legacy` or legacy behavior.
- The plan does not mention checking for re-exports (e.g., `apps/ui/types/index.ts`) that might surface `StripeConfig` to other modules.

## Suggested improvements

- Add a discovery step: `rg "legacy" --type rust --type ts --type tsx --type md apps/ libs/` and document any legacy items intentionally left in place.
- Add a production config check: inspect `infra/.env` and `infra/compose.yml` for `MAGIC_LINK_TTL_MINUTES`, or confirm runtime value via logs/config output if available.
- Widen the hash search: `rg "hash_domain_token_legacy"` (no type filter) and `rg "legacy hash" -g'*.md'` to catch doc references.
- Note any re-export checks for `StripeConfig` (e.g., `rg "StripeConfig" apps/ui/types`), and remove or update index exports if present.
- Add a formatting step before commit (`./run api:fmt`) since Rust changes are planned and this is a repo guideline.

## Risks or concerns

- If production overrides the magic link TTL to a higher value, removing fallback could break older in-flight links beyond 15 minutes.
- If the deployment timestamp is unknown, fallback removal could impact users immediately after deploy even with a short TTL.
- A hidden re-export of `StripeConfig` could cause downstream type errors in the UI if removed without updating index files.
- Narrow search scope risks leaving stale doc references that confuse future maintainers.
