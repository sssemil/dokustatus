# Feedback on plan-v2.md

## What's Good

1. **Addressed all v1 feedback**: The plan now correctly handles `-.5` decimals, includes a step to verify no other CSV export paths exist, documents leading whitespace behavior, and adds integration testing.

2. **Thorough documentation in code**: The docstring on `escape_csv_field` explains the "why" comprehensively - which spreadsheet apps are affected, why single-quote prefix works, and the special minus handling logic.

3. **Complete edge case table**: The summary table is excellent. It covers normal cases, all attack vectors (`=`, `+`, `@`, `|`, `\t`, `\r`), minus edge cases (`-100`, `-.5`, `-cmd`, `-`, `-.`, `--`), and standard CSV escaping.

4. **Design decisions section**: Explicitly documenting decisions about leading whitespace, locale-specific decimals, and scientific notation prevents future confusion.

5. **Test coverage is comprehensive**: 8 test functions covering all scenarios. The tests are well-organized by category (normal, formula prefixes, pipe, minus, tab/CR, CSV escaping, combined, edge cases).

6. **Checklist alignment**: Mapping plan steps back to the ticket checklist ensures traceability.

## What's Missing or Unclear

1. **`-.` logic inverted**: The code in the plan has a subtle bug. Look at:
   ```rust
   Some('.') => {
       // -.5 is safe if followed by digit, -.x is dangerous
       !chars.next().map(|c| c.is_ascii_digit()).unwrap_or(false)
   }
   ```
   This returns `true` (is_formula) when the char is NOT a digit. But `unwrap_or(false)` means `-.` (no third char) returns `false`, then `!false = true` means it's treated as a formula. That's correct for `-.` alone.

   However: `-.5` → `chars.next()` = `Some('5')` → `is_ascii_digit()` = `true` → `!true` = `false` → NOT a formula. That's correct!

   Actually, on closer review the logic is correct. Never mind this point.

2. **No handling for uppercase/unicode formula markers**: Some spreadsheets may treat `＝` (fullwidth equals U+FF1D) as a formula. This is an edge case and probably out of scope, but worth mentioning in design decisions.

3. **Integration test is vague**: Step 3 says "manually test the actual billing CSV export" but doesn't specify:
   - Which endpoint to hit
   - What test data to seed
   - Expected values to verify
   Consider adding specific commands: `curl ... > test.csv && libreoffice test.csv`

4. **No mention of performance**: The new logic adds a few more character checks. This is negligible, but if this function is called in a hot path (thousands of rows), worth noting it's still O(1) per field.

5. **Test assertions use string literals**: The tests check for escaped output like `"\"'=1+1\""`. This is hard to read. Consider using raw strings (`r#""'=1+1""#`) for clarity, or adding comments showing the expected output visually.

## Suggested Improvements

1. **Add specific integration test commands**:
   ```bash
   # Seed test domain with negative amounts
   ./run dev:seed
   # Export billing CSV
   curl -H "Authorization: Bearer $TOKEN" https://reauth.test/api/domains/$DOMAIN_ID/billing/export > test.csv
   # Verify in spreadsheet
   libreoffice test.csv
   ```

2. **Add a test for Unicode formula markers** (optional, document decision):
   ```rust
   #[test]
   fn test_escape_csv_field_unicode() {
       // Fullwidth equals (U+FF1D) - out of scope, treated as text
       assert_eq!(escape_csv_field("\u{FF1D}1+1"), "\u{FF1D}1+1");
   }
   ```
   Or add to design decisions: "Unicode formula markers (fullwidth `＝`) are out of scope as they're rarely exploited and would require extensive Unicode handling."

3. **Consider raw strings for test readability**:
   ```rust
   assert_eq!(escape_csv_field("=1+1"), r#""'=1+1""#);
   ```

4. **Add a note about the Bash search command in Step 0**: The command `rg -i "text/csv|content-type.*csv|\.csv" apps/api/src/` should also search for `csv::Writer` or similar CSV library usage if the project uses one.

## Risks and Concerns

1. **Low risk**: The single-quote prefix is a well-established mitigation. The implementation is sound.

2. **Leading whitespace decision is correct but may surprise**: If someone reports that ` =SUM()` isn't being escaped, the documented behavior will help explain why. Consider adding this to user-facing documentation if the export is customer-accessible.

3. **Pipe character is less common**: Adding `|` is correct, but ensure it doesn't break any legitimate data. Billing data unlikely to start with `|`, so this is safe.

4. **Test module placement**: The plan says "add at end of file (before closing brace)". In a 2200+ line file, ensure the tests are actually in the correct module scope. May need `#[cfg(test)] mod escape_csv_tests { ... }` if the file has multiple modules.

## Summary

**Plan v2 is ready for implementation.** All feedback from v1 has been addressed. The only substantive suggestions are:

1. Make integration test instructions more specific (which endpoint, what data)
2. Add a note about Unicode formula markers being out of scope
3. Consider raw strings for test readability (optional)

Overall quality: **Excellent** - proceed with implementation.

---
Reviewed: 2026-01-01
