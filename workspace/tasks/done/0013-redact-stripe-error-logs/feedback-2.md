# Feedback on Plan v2 - Redact Stripe Error Logs

**Reviewer**: Claude
**Date**: 2026-01-01

---

## What's Good

1. **Thorough scope verification**: Confirming only two logging locations exist via codebase search prevents scope creep and missed spots.

2. **Sensible fallback behavior**: The `<body redacted, N bytes>` pattern for non-parseable responses maintains debuggability without leaking data.

3. **Edge case table**: Clear matrix showing how different body formats are handled.

4. **Risk assessment**: The risk table with severities and mitigations shows mature planning.

5. **Doc comment**: Adding security-focused documentation helps future maintainers understand why this exists.

6. **Deferred Request-Id**: Good judgment to keep this out of scope rather than expanding the PR unnecessarily.

7. **Test coverage**: Five tests covering happy path, minimal error, invalid JSON, empty, and non-error JSON structures.

---

## What's Missing or Unclear

1. **StripeErrorResponse struct location not specified**: The plan assumes `StripeErrorResponse` already exists and is in scope. Should verify this struct exists in the file and confirm its field structure matches what the helper expects (i.e., has `error.error_type`, `error.code`, `error.message`).

2. **No mention of existing tests to run**: Before adding new tests, the plan should confirm existing tests still pass after the changes (not just that new tests work).

3. **Line numbers may drift**: The plan references specific lines (606, 619-622, 626) but doesn't show surrounding context. If the file has been modified since the plan was written, these may be off.

4. **Formatting verification**: No mention of running `./run api:fmt` after changes.

---

## Suggested Improvements

1. **Verify StripeErrorResponse struct**: Before implementation, read lines ~100-150 of `stripe_client.rs` to confirm the struct exists and has the expected nested structure (`error.error_type`, `error.code`, `error.message`). If the field names differ (e.g., `r#type` instead of `error_type`), the plan needs adjustment.

2. **Add fmt step to testing approach**:
   ```
   0. Format: Run `./run api:fmt` before commit
   ```

3. **Consider inline format string for clarity**: The format string `type={}, code={:?}, message={:?}` uses `{:?}` for `Option` types. This will produce output like `code=Some("card_declined")`. If cleaner output is preferred (e.g., `code=card_declined`), consider:
   ```rust
   error.error.code.as_deref().unwrap_or("<none>")
   ```
   However, the current approach is fine for debugging purposes.

4. **Add a comment explaining the byte count fallback**: Developers seeing `<body redacted, 847 bytes>` might wonder why. A brief inline comment explaining this is for non-Stripe-error responses would help.

---

## Risks or Concerns

1. **Struct field name mismatch risk**: If `StripeErrorResponse` uses `r#type` (due to `type` being a Rust keyword), the plan's code will fail to compile. This is the highest implementation risk.

2. **Optional field handling**: The plan assumes `code` and `message` are `Option<String>`. If they're different types (e.g., `Option<&str>` or a custom enum), the format string may not work as expected.

3. **Test visibility**: The `redact_stripe_body` function is private. The tests are in the same module so they can access it, but this should be verified against the existing test module structure.

---

## Verdict

**Ready to implement with minor verification**: The plan is well-structured and addresses the core issue. Before coding, verify the `StripeErrorResponse` struct's exact field names and types. The risk of struct mismatch is low but would cause a compile error that's easy to fix.

Recommended pre-implementation step:
```bash
grep -A 20 "struct StripeErrorResponse" apps/api/src/infra/stripe_client.rs
```
