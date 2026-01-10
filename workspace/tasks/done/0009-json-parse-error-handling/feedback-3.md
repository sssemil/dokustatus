# Plan Feedback v3: JSON Parse Error Handling

## What's good

- **Comprehensive NULL handling:** The explicit `Value::Null` check addresses the SQLx behavior concern from feedback-2. The verification note confirms SQL NULL maps to `Value::Null` for `serde_json::Value` type.
- **Serialization error propagation:** Excellent change from v2. Now returns `AppError::Internal` instead of silently writing `[]`, which surfaces bugs instead of masking data loss.
- **Well-structured semantic table:** The table clearly shows behavior for each scenario (SQL NULL, JSON null, valid arrays, type mismatches) with logging decisions.
- **Fail-closed safety analysis:** Good justification that empty roles/features are safe defaults for authorization and billing logic.
- **PII consideration documented:** Confirms roles/features don't contain PII, addressing feedback-2's concern about logging raw JSON.
- **Complete code examples:** The plan includes concrete implementation snippets that are ready to copy with minimal adjustments.

## What's missing or unclear

1. **Clone timing clarification:** The helper always calls `json.clone()` inside `from_value()` for non-null values (line 114). The comment says "clone happens only in helper when needed" but it happens for _every_ successful parse, not just on failure. Consider clarifying this trade-off or explaining why it's acceptable.

2. **Missing `use` statements:** The plan mentions adding `parse_json_with_fallback` to `mod.rs` but doesn't show the necessary imports (`serde_json`, `tracing`, `serde::de::DeserializeOwned`). Confirm these are already in scope or add them.

3. **Incomplete `row_to_profile` for subscription_plan.rs:** The code snippet shows `// ... rest unchanged` (line 226) but doesn't show the full function. For implementation, the actual current function body is needed to know what fields to preserve.

4. **`AppError` type import:** The serialization error handling uses `AppError::Internal` but doesn't confirm the error type signature or that this variant exists. Verify the exact error construction.

5. **Test for serialization failure:** The testing section only covers deserialization. Consider adding a test that verifies serialization error propagation works correctly (even if it requires mocking since `Vec<String>` serialization cannot actually fail).

## Suggested improvements

1. **Avoid clone on success path:** Consider using `serde_json::from_str` on the string representation instead, or document why the clone overhead is acceptable:
   ```rust
   // Alternative: parse from string to avoid clone
   match serde_json::from_str::<T>(&json.to_string()) {
       Ok(v) => v,
       Err(err) => { /* log and return default */ }
   }
   ```
   However, this adds a serialize-then-deserialize overhead. The current approach may be fine given these are small arrays. Document the trade-off.

2. **Add `#[inline]` hint:** Since this helper is called frequently in hot paths (every DB row), consider `#[inline]` to let the compiler decide on inlining.

3. **Consider `tracing::instrument`:** For better observability, the helper could use `#[instrument(skip(json), fields(field_name, entity_type, entity_id))]` to capture span context automatically.

4. **Document visibility:** The helper should be `pub(super)` or `pub(crate)` since it's only used within the persistence module. The plan shows it as a bare function without visibility modifier.

5. **Add integration test note:** While unit tests cover the helper, note that manual verification should include checking logs appear in the expected format when running `./run api` with seeded corrupt data.

## Risks or concerns

1. **Behavior change for serialization errors:** Previously, serialization failures silently wrote `[]`. Now they return a 500 error. If there are any edge cases where serialization could fail (malformed strings, encoding issues), this changes the API behavior from "succeed with empty data" to "fail the request". Verify this is the desired behavior for all call sites.

2. **Visibility of the helper function:** If `parse_json_with_fallback` is public without restrictions, it could be used incorrectly elsewhere. Consider documenting its intended scope or restricting visibility.

3. **Missing domain_id in logs:** While entity_id is sufficient to trace back to specific records, for multi-tenant log filtering, having domain_id would allow filtering all issues for a specific tenant. The plan explicitly decided against this—verify this is acceptable for ops workflows.

4. **No migration for NOT NULL:** The follow-up section mentions adding `NOT NULL` constraints but doesn't note the risk: if any existing rows have NULL values, the migration would fail. A data migration to set NULL to `[]` would be needed first.

## Summary

Plan v3 is ready for implementation with minor clarifications. The main changes from v2 (explicit NULL check, serialization error propagation) are well-reasoned. Address the visibility modifier, confirm `AppError::Internal` exists, and verify the clone behavior is documented. The serialization behavior change from silent fallback to error propagation is the biggest user-facing change—confirm this is acceptable.

**Recommendation:** Approve with the following pre-implementation steps:
1. Verify `AppError::Internal` variant exists and check its signature
2. Add visibility modifier (`pub(super)`) to the helper
3. Note the clone-on-every-parse trade-off in comments
