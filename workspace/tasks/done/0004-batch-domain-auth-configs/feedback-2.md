# Plan Review Feedback (v2)

## What's good
- Clear problem statement and scope (N+1 in `list_domains`), with explicit decision to preserve `get_auth_config` semantics.
- Batch query approach is concrete and consistent with existing patterns (`ANY($1)`), plus an early-return for empty input.
- Ownership/authorization assumption is called out with naming and doc comments, reducing misuse risk.
- Edge cases and performance impact are explicitly documented.
- Includes unit-test intent for the new helper method, not just repo methods.

## What's missing or unclear
- The plan does not specify whether the batch repo method should be added to any mock/test repository implementations (if present) or how to update them.
- It is unclear how `DomainAuthConfigProfile` is constructed in the repo (mentions `row_to_profile`, but not whether it handles `NULL` safely or if `redirect_url`/TTL fields are optional).
- The route update assumes `DomainStatus::Verified` as the only state requiring `has_auth_methods` lookup, but does not reference existing behavior to confirm this matches current UI expectations.
- Testing section does not mention verifying that the response shape and ordering are unchanged (if ordering is important to clients).
- Manual testing relies on observing query count via logs but doesn't mention enabling SQLx logging or where to find it.

## Suggested improvements
- Add a small note to update any in-memory/mock repo structs used in tests to include `get_by_domain_ids`, to avoid compile-time breakage.
- Consider returning a `HashMap<Uuid, DomainAuthConfigProfile>` directly from the repo or use-case to avoid recomputing `has_auth_methods` in the route if other callers might reuse it later.
- Add a single integration-style test (if feasible) for `list_domains` verifying `has_auth_methods` values for: verified domains with config, verified domains without config, and unverified domains.
- Clarify whether `redirect_url`, TTL fields, and `whitelist_enabled` are needed in the batch query; if they are unused for this logic, select only required columns to reduce payload.
- Mention whether to keep domain list ordering intact and ensure the new map lookup does not reorder results.

## Risks or concerns
- If the batch method is later used with untrusted domain IDs, it could leak auth configuration presence; the plan’s guardrails help but are not foolproof.
- Defaulting to `true` for missing configs perpetuates the current behavior, which may hide missing fallback email configuration; ensure stakeholders agree with this choice.
- Deduplication uses a `HashSet`, which is fine for correctness but changes iteration order; make sure no downstream logic depends on input order.
