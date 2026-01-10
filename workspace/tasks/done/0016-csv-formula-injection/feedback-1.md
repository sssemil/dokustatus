# Feedback on plan-v1.md

## What's Good

1. **Clear problem analysis**: The plan correctly identifies the three issues (missing `|`, false positives on `-`, no tests).

2. **Detailed code changes**: Providing the exact before/after code makes implementation straightforward and reduces ambiguity.

3. **Comprehensive test coverage**: The test suite covers normal values, all formula prefixes, the new pipe case, minus edge cases, tab/CR, standard CSV escaping, and combined scenarios.

4. **Edge case table**: The summary table at the end is excellent for quick reference during implementation and review.

5. **Smart minus handling**: The logic to treat `-` followed by a digit as safe (negative number) while escaping `-` followed by non-digit is a good balance between security and usability.

## What's Missing or Unclear

1. **No handling for whitespace-prefixed formulas**: Some spreadsheet apps will still execute `" =1+1"` (space before equals). The plan doesn't address whether leading whitespace should be trimmed or whether ` =` should also be escaped. This is a potential bypass.

2. **Missing `0x` check for `-`**: The check `chars.next().map(|c| c.is_ascii_digit())` will correctly handle `-100` but what about `-1e10` (scientific notation) or `-0x10` (hex, if somehow relevant)? Clarify if these are in scope.

3. **No mention of existing callers**: The plan should confirm that `escape_csv_field` is only called in `domain_billing.rs` and that no other CSV export paths exist. If there are other CSV exports, they may need the same fix.

4. **Test for `.` after minus**: `-42.50` is tested but `-42.50` starts with `-4`, so it passes. What about `-.5`? This is a valid negative decimal (0.5 negative) but starts with `-.` which is non-digit. The current logic would escape it as a formula. Is this intended?

5. **No mention of regression testing the actual CSV export**: The plan focuses on unit tests for `escape_csv_field` but doesn't mention integration/manual testing of the actual billing CSV export to ensure the changes work end-to-end.

## Suggested Improvements

1. **Handle `-.` for decimals like `-.5`**:
   ```rust
   Some('-') => {
       match chars.next() {
           Some(c) if c.is_ascii_digit() => false, // -100
           Some('.') => chars.next().map(|c| c.is_ascii_digit()).unwrap_or(false) == false, // -.5 is safe
           _ => true, // -cmd, -, -- are dangerous
       }
   }
   ```
   Alternatively, document that `-.5` will be escaped and accept it as a minor false positive.

2. **Add a test for `-.5`**: Either confirm it's escaped (and document why) or fix the logic and test it passes through.

3. **Verify no other CSV export paths exist**: Add a step to grep for other CSV generation code (`text/csv`, `content-type.*csv`, `\.csv`) to ensure this is the only place needing the fix.

4. **Consider leading whitespace**: Add a note about whether ` =1+1` (space-prefixed formula) should be considered. If the answer is "no, we don't trim, and spreadsheets will treat it as text", document that reasoning.

5. **Add a comment about why the single quote prefix works**: The code already has a comment, but it could note that Excel specifically interprets a leading `'` as "text" and won't execute formulas.

## Risks and Concerns

1. **False negative on `-.5`**: As noted, `-.5` starts with `-.` (non-digit after minus) so it would be escaped. This is a minor false positive, not a security risk, but could confuse users seeing `"'-.5"` in their CSV.

2. **Locale-specific decimal separators**: Some locales use `,` as decimal separator. If the billing data ever contains `-0,50` this would already be quoted due to the comma, but verify there's no locale issue.

3. **Breaking change for existing exports**: Adding escaping to values that weren't escaped before could surprise downstream consumers. The plan correctly handles negative numbers (no change) and the new escapes are for security, so this is acceptable.

4. **Test coverage of inline `\|` escaping in markdown**: In the edge case table, `\|cmd` appears with a backslash. Verify the actual test uses `|cmd` (raw pipe) not escaped.

## Summary

The plan is solid and ready for implementation with minor clarifications:
- Decide on `-.5` handling (escape or allow)
- Confirm no other CSV export paths exist
- Consider adding a note about leading-whitespace formulas

Overall quality: **Good** - proceed with implementation after addressing the `-.5` case.

---
Reviewed: 2026-01-01
