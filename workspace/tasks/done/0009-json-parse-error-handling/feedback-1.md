# Feedback: Plan v1 (0009-json-parse-error-handling)

## What's good
- Clear inventory of affected sites with file references, which makes implementation scoping straightforward.
- Balanced strategy (log + fallback) preserves backward compatibility while improving observability.
- Proposed helper reduces duplication and standardizes error handling.
- Test ideas cover both valid and invalid JSON inputs.

## What's missing or unclear
- Logging fields use `raw_json = %json`, but `Value` formatting can be large; log volume impact and truncation strategy are not addressed.
- The helper signature takes `json: Value` by value and clones it; it is unclear if the call sites already own the `Value` or could pass by reference to avoid extra clones.
- The plan says to add a helper in `persistence/mod.rs`, but does not confirm whether `tracing` is already in scope for that module or if imports are needed.
- The strategy prefers logging but does not mention when (if ever) we should propagate errors for critical fields or fail fast (e.g., if features/roles are required for access control).
- Test plan assumes `serde_json::Value::Null` should fall back, but does not state whether NULL should be treated differently (e.g., `None` vs empty list) in domain logic.

## Suggested improvements
- Consider passing `&serde_json::Value` to the helper to avoid cloning; if ownership is required, document why and keep clones local to the helper.
- Include a simple rate-limiting or sampling note for warn logs if this could be triggered frequently (e.g., a noisy tenant), or include a TODO for follow-up.
- Expand the helper to accept an `entity_id` field instead of a formatted `context` string to keep logs structured and filterable.
- Decide and document the intended semantic for NULL vs empty array for each field; if they differ, return `Option<Vec<String>>` or make the fallback explicit per call site.
- Add a small unit test to assert logging happens on error (if using `tracing-test` or a log capture helper already present in the repo). If log capture is not standard, note it as a follow-up.

## Risks or concerns
- Using a warning log on every parse failure could flood logs in the presence of bad data, masking other signals.
- Falling back to empty lists might hide authorization/feature gating problems if roles or features are used to enforce access or billing logic.
- The serialization error handling proposal uses `error!` but still swallows; if this ever happens, it may indicate a deeper invariant break that should be surfaced.
