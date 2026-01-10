# Plan Review Feedback (v3)

## What's good

- Clear identification of the N+1 source in `list_domains` and a focused batch fix that keeps response ordering intact.
- Explicit decision to preserve `get_auth_config` default semantics avoids unexpected UI changes.
- Scope is well-contained: repo batch method + use case helper + route update.
- Security notes and naming (`for_owner_domains`) document the ownership assumption.
- Testing plan is concrete (unit, integration-style, manual) and covers key behaviors and ordering.

## What's missing or unclear

- Error-handling behavior change: current `list_domains` swallows `get_auth_config` errors and returns `has_auth_methods = false`; the plan now uses `?` and would 500 on any batch query error. This is a behavior change not called out.
- The batch repo method returns `DomainAuthConfigProfile` but fills non-selected fields with placeholders (0/None). This is not aligned with real config values or the synthetic defaults (e.g., TTLs 86400/30), and the plan does not justify the inconsistency.
- If a future caller reuses `get_by_domain_ids` for anything beyond `has_auth_methods`, the partial data can silently cause bugs. This risk is not addressed in the design.

## Suggested improvements

- Avoid partial `DomainAuthConfigProfile` altogether: have the repo return a minimal DTO (e.g., `Vec<(Uuid, bool, bool)>`) or a `HashMap<Uuid, AuthMethodFlags>` so only the needed fields are exposed.
- If you keep `DomainAuthConfigProfile`, either select all columns and reuse `row_to_profile`, or set defaults that match the synthetic defaults used in `get_auth_config` (86400/30) to avoid inconsistent values.
- Decide and document the intended error behavior for `list_domains`: either preserve the old “best-effort” behavior (defaulting on error) or explicitly change to fail fast, and update tests accordingly.
- Consider making `has_auth_methods_for_owner_domains` the only public-facing helper and keep the repo method private if possible, to reduce future misuse.

## Risks or concerns

- Partial struct data: returning `DomainAuthConfigProfile` with fake TTLs/fields could lead to subtle regressions if reused later.
- Behavior drift on failures: a transient DB error could now fail the entire `list_domains` request instead of returning a response with `has_auth_methods = false`.
- Ownership bypass remains a footgun if the helper is reused outside `list_domains`; the doc comment helps, but a narrower interface would be safer.
