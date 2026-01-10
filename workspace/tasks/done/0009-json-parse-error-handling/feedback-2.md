# Plan Feedback v2: JSON Parse Error Handling

## What's good

- Clear inventory of affected sites and explicit mapping to file/line locations.
- Solid semantic analysis of NULL vs empty array with a fail-closed rationale.
- Helper function design centralizes logging and fallback behavior; truncation detail prevents log bloat.
- Testing section includes concrete unit cases and acknowledges current log-test limitations.
- Risks/mitigations and rollback are outlined and aligned with backward-compatibility goals.

## What's missing or unclear

- The plan doesn’t confirm how `serde_json::Value::Null` will be surfaced from SQLx for JSONB NULLs. SQLx may map SQL NULL to `Option<Value>` rather than `Value::Null` depending on query; current callers may need `Option<Value>` handling.
- It’s unclear whether `roles_json`/`features_json` are already `Value` or optional in all three sites. If any are `Option<Value>`, the helper signature needs to accept `Option<&Value>` or the callers must coalesce.
- The logging includes `raw_json` and `entity_id`, but no `tenant_id`/`domain_id` context. If multi-tenant diagnostics rely on domain scoping, consider adding it.
- The plan assumes `to_value` serialization errors are impossible but doesn’t specify how to surface them in tests or how to avoid silently writing `[]` in truly exceptional cases.

## Suggested improvements

- Decide on a consistent handling strategy for SQL NULL vs JSON null at the query level. Example: query `COALESCE(jsonb_column, '[]'::jsonb) AS roles` to avoid `Option<Value>` and treat SQL NULL as empty array without warning.
- If SQL NULL should be logged differently than parse errors, add a dedicated branch (e.g., `parse_json_with_fallback_opt`) that logs a distinct message for `None` vs invalid JSON types.
- Expand the helper to accept an optional `context` map (or `domain_id`) for multi-tenant filtering if that is a common diagnostic need.
- Consider using `tracing::warn!(?json, ...)` with `serde_json::to_string` only on failure, but also cap the count (e.g., `roles_len`) to avoid overly verbose logs even with truncation.
- Add a small unit test that verifies valid JSON doesn’t log or allocate unnecessarily by using a custom type and `#[cfg(test)]` instrumentation (if feasible), or at least document that the helper clones only on failure to clarify intent.

## Risks or concerns

- If the SQLx mapping is `Option<Value>` for NULL JSONB, the helper as written could force unwrapping and either panic or treat NULL incorrectly; this can introduce subtle behavior differences.
- Logging raw JSON (even truncated) could expose sensitive data if any of these fields can contain PII. Confirm that roles/features are safe to log or redact further.
- Changing serialization error handling to log and return `[]` could mask systemic issues; if this ever occurs, it may be better to surface an error up the stack for a 500 response instead of silently clearing data.
