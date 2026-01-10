# Plan: Improve CSV Export Formula Injection Guard (v3)

## Summary

The current `escape_csv_field` function in `apps/api/src/application/use_cases/domain_billing.rs` (lines 2200-2223) has incomplete formula injection protection. The implementation needs refinement to:

1. Add missing dangerous character `|` (pipe) which can trigger DDE/formula execution in some spreadsheets
2. Adjust `-` (minus) handling to avoid false positives for legitimate negative numbers and hyphenated values
3. Add comprehensive unit tests for edge cases

## Changes from v2

**Addressing feedback:**

1. **Specific integration test commands**: Added explicit `curl` commands, endpoint paths, and verification steps for the billing CSV export.
2. **Unicode formula markers**: Added design decision documenting that fullwidth unicode formula characters (e.g., `＝` U+FF1D) are out of scope. Added an explicit test case.
3. **Raw strings for test readability**: Updated test assertions to use raw string literals (`r#"..."#`) where escaping is heavy.
4. **Expanded CSV search command**: Step 0 now also searches for `csv::Writer` and similar CSV library patterns.
5. **Test module placement clarified**: Added note about placing tests in a dedicated named module for clarity in the large file.

## Current Implementation Analysis

The current function checks for these formula-trigger characters at the start of a field:
- `=` (formula prefix)
- `+` (formula prefix)
- `-` (formula prefix - problematic for negative numbers)
- `@` (formula prefix)
- `\t` (tab)
- `\r` (carriage return)

**Issues identified:**
1. **Missing `|` character**: Pipe can trigger DDE attacks in Excel (e.g., `|cmd /c calc`)
2. **`-` false positives**: The current check flags `-100` (negative number) as a formula, adding unnecessary escaping
3. **No tests**: The function lacks unit tests to verify behavior

## Step-by-Step Implementation

### Step 0: Verify no other CSV export paths exist

Run a search for other CSV generation code to ensure this is the only place needing the fix:

```bash
# Search for CSV content types, file extensions, and library usage
rg -i "text/csv|content-type.*csv|\.csv" apps/api/src/
rg -i "csv::Writer|csv::writer|CsvWriter" apps/api/src/
rg "use csv" apps/api/src/
```

If other paths exist, they should use the same `escape_csv_field` function or be updated similarly.

### Step 1: Update formula detection logic

Modify the `is_formula` check to:
- Add `|` to the dangerous characters list
- Adjust `-` handling:
  - `-` followed by a digit is safe (e.g., `-100`)
  - `-` followed by `.` then a digit is safe (e.g., `-.5`)
  - `-` followed by anything else is dangerous (e.g., `-cmd`, `--`, `-`)
- Keep all other checks as-is

### Step 2: Add comprehensive unit tests

Add a `#[cfg(test)]` module with tests covering:
- Standard formula prefixes (`=`, `+`, `@`)
- Pipe character (`|`)
- Minus with formulas vs. negative numbers (including `-.5`)
- Tab and carriage return prefixes
- Standard CSV escaping (commas, quotes, newlines)
- Combined scenarios (formula + special chars)
- Empty and whitespace-only inputs
- Scientific notation (`-1e10`)
- Unicode formula markers (out of scope, documented)

### Step 3: Integration test with billing CSV export

After implementing, verify the actual billing CSV export works correctly.

**Prerequisites:**
- Local infrastructure running: `./run infra:full`
- API running: `./run api`
- Test domain seeded: `./run dev:seed`

**Test commands:**
```bash
# 1. Get an API token (adjust based on auth mechanism)
#    This may require logging in via the UI and extracting the token from browser dev tools,
#    or using the dev API key if available for testing.

# 2. Find a domain ID with billing data
#    Check the database or use the UI to identify a test domain.

# 3. Export the billing CSV (adjust endpoint path if different)
curl -H "Authorization: Bearer $API_TOKEN" \
     "https://reauth.test/api/domains/$DOMAIN_ID/billing/export" \
     -o billing-test.csv

# 4. Inspect the raw CSV for expected escaping
cat billing-test.csv | head -20
# Look for:
#   - Negative amounts should appear as: -100.00 (no quote prefix)
#   - Formula-like content should appear as: "'=SUM(...)" (with quote prefix)

# 5. Open in a spreadsheet application
libreoffice billing-test.csv
# Or: open -a "Microsoft Excel" billing-test.csv  (macOS)
# Or: upload to Google Sheets

# 6. Verify:
#   - Negative amounts display correctly (e.g., -$100.00)
#   - No formula execution warnings on open
#   - No spurious single quotes visible in cells with negative numbers
#   - If test data contains formula-like strings, they show as text (not executed)
```

## Files to Modify

| File | Change |
|------|--------|
| `apps/api/src/application/use_cases/domain_billing.rs` | Update `escape_csv_field` function (lines 2200-2223) and add test module |

## Detailed Code Changes

### In `domain_billing.rs`:

**Current code (lines 2200-2223):**
```rust
fn escape_csv_field(field: &str) -> String {
    let needs_quoting =
        field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r');

    // Check for formula injection characters at start
    let is_formula = field
        .chars()
        .next()
        .map(|c| matches!(c, '=' | '+' | '-' | '@' | '\t' | '\r'))
        .unwrap_or(false);

    let escaped = if is_formula {
        // Prefix with single quote to prevent formula execution
        format!("'{}", field)
    } else {
        field.to_string()
    };

    if needs_quoting || is_formula {
        format!("\"{}\"", escaped.replace('"', "\"\""))
    } else {
        escaped
    }
}
```

**New code:**
```rust
/// Escape a field for CSV output, including formula injection prevention.
///
/// Spreadsheet applications (Excel, Google Sheets, LibreOffice Calc) will execute
/// formulas starting with =, +, -, @, or | at the start of a cell. Tab and carriage
/// return at start can also trigger formula parsing in some applications.
///
/// We prefix such values with a single quote ('). Excel interprets a leading single
/// quote as "this is text, not a formula" and displays the value without the quote.
///
/// Special handling for `-`:
/// - `-` followed by a digit is a negative number (safe): `-100`
/// - `-` followed by `.` then digit is a negative decimal (safe): `-.5`
/// - `-` followed by anything else is potentially dangerous: `-cmd`, `--`, `-`
///
/// Note on leading whitespace: Values like ` =1+1` (space before equals) are NOT
/// escaped. Spreadsheets treat leading whitespace as literal text, not formulas.
/// This is intentional and safe.
///
/// Note on Unicode: Fullwidth formula characters (e.g., `＝` U+FF1D) are not escaped.
/// These are rarely exploited and would require extensive Unicode normalization. Out of scope.
fn escape_csv_field(field: &str) -> String {
    let needs_quoting =
        field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r');

    // Check for formula injection characters at start
    let is_formula = {
        let mut chars = field.chars();
        match chars.next() {
            Some('=' | '+' | '@' | '|' | '\t' | '\r') => true,
            Some('-') => {
                // Allow negative numbers: -100, -.5
                match chars.next() {
                    Some(c) if c.is_ascii_digit() => false, // -100, -1e10
                    Some('.') => {
                        // -.5 is safe if followed by digit, -.x is dangerous
                        !chars.next().map(|c| c.is_ascii_digit()).unwrap_or(false)
                    }
                    _ => true, // -cmd, -, -- are dangerous
                }
            }
            _ => false,
        }
    };

    let escaped = if is_formula {
        // Prefix with single quote to prevent formula execution
        format!("'{}", field)
    } else {
        field.to_string()
    };

    if needs_quoting || is_formula {
        format!("\"{}\"", escaped.replace('"', "\"\""))
    } else {
        escaped
    }
}
```

### Test module to add at end of file

Place this inside a dedicated named module for clarity in the large file. Add before the final closing brace of the file:

```rust
#[cfg(test)]
mod escape_csv_field_tests {
    use super::escape_csv_field;

    #[test]
    fn test_normal_values() {
        // Plain text should pass through unchanged
        assert_eq!(escape_csv_field("hello"), "hello");
        assert_eq!(escape_csv_field("John Doe"), "John Doe");
        assert_eq!(escape_csv_field("test@email.com"), "test@email.com"); // @ not at start
        assert_eq!(escape_csv_field("100"), "100");
    }

    #[test]
    fn test_formula_prefixes() {
        // = prefix (formula)
        assert_eq!(escape_csv_field("=1+1"), r#""'=1+1""#);
        assert_eq!(escape_csv_field("=SUM(A1:A10)"), r#""'=SUM(A1:A10)""#);

        // + prefix (formula)
        assert_eq!(escape_csv_field("+1"), r#""'+1""#);
        assert_eq!(escape_csv_field("+cmd|' /c calc'!A0"), r#""'+cmd|' /c calc'!A0""#);

        // @ prefix (formula)
        assert_eq!(escape_csv_field("@SUM(A1)"), r#""'@SUM(A1)""#);
    }

    #[test]
    fn test_pipe_dde() {
        // Pipe can trigger DDE attacks in Excel
        assert_eq!(escape_csv_field("|cmd /c calc"), r#""'|cmd /c calc""#);
        assert_eq!(escape_csv_field("|powershell"), r#""'|powershell""#);
    }

    #[test]
    fn test_minus_handling() {
        // Negative numbers should NOT be escaped (false positive prevention)
        assert_eq!(escape_csv_field("-100"), "-100");
        assert_eq!(escape_csv_field("-42.50"), "-42.50");
        assert_eq!(escape_csv_field("-0"), "-0");
        assert_eq!(escape_csv_field("-1e10"), "-1e10"); // scientific notation starts with digit

        // Negative decimals starting with dot should NOT be escaped
        assert_eq!(escape_csv_field("-.5"), "-.5");
        assert_eq!(escape_csv_field("-.123"), "-.123");
        assert_eq!(escape_csv_field("-.0"), "-.0");

        // Minus followed by non-digit/non-decimal SHOULD be escaped
        assert_eq!(escape_csv_field("-cmd"), r#""'-cmd""#);
        assert_eq!(escape_csv_field("--help"), r#""'--help""#);
        assert_eq!(escape_csv_field("-"), r#""'-""#); // lone minus
        assert_eq!(escape_csv_field("-."), r#""'-.""#); // minus-dot with no digit
        assert_eq!(escape_csv_field("-.x"), r#""'-.x""#); // minus-dot-letter
    }

    #[test]
    fn test_tab_and_cr() {
        // Tab at start
        assert_eq!(escape_csv_field("\tcmd"), "\"'\tcmd\"");

        // Carriage return at start (also needs quoting due to \r)
        assert_eq!(escape_csv_field("\rcmd"), "\"'\rcmd\"");
    }

    #[test]
    fn test_standard_csv_escaping() {
        // Commas require quoting
        assert_eq!(escape_csv_field("hello, world"), r#""hello, world""#);

        // Quotes require escaping and quoting
        assert_eq!(escape_csv_field(r#"say "hello""#), r#""say ""hello""""#);

        // Newlines require quoting
        assert_eq!(escape_csv_field("line1\nline2"), "\"line1\nline2\"");
        assert_eq!(escape_csv_field("line1\rline2"), "\"line1\rline2\"");
    }

    #[test]
    fn test_combined() {
        // Formula prefix + comma (double protection)
        assert_eq!(escape_csv_field("=SUM(A,B)"), r#""'=SUM(A,B)""#);

        // Formula prefix + quotes
        assert_eq!(escape_csv_field(r#"=EXEC("cmd")"#), r#""'=EXEC(""cmd"")""#);
    }

    #[test]
    fn test_edge_cases() {
        // Empty string
        assert_eq!(escape_csv_field(""), "");

        // Single characters
        assert_eq!(escape_csv_field("a"), "a");
        assert_eq!(escape_csv_field("="), r#""'=""#);
        assert_eq!(escape_csv_field("|"), r#""'|""#);

        // Whitespace (not trimmed, treated as literal)
        assert_eq!(escape_csv_field(" "), " ");
        assert_eq!(escape_csv_field("  "), "  ");

        // Leading whitespace before formula chars - NOT escaped (safe)
        // Spreadsheets treat leading whitespace as literal text
        assert_eq!(escape_csv_field(" =1+1"), " =1+1");
        assert_eq!(escape_csv_field("  +cmd"), "  +cmd");
    }

    #[test]
    fn test_unicode_formula_markers() {
        // Fullwidth equals (U+FF1D) - out of scope, treated as text
        // This is documented as intentional; exploiting these is extremely rare
        assert_eq!(escape_csv_field("\u{FF1D}1+1"), "\u{FF1D}1+1");

        // Fullwidth plus (U+FF0B) - out of scope
        assert_eq!(escape_csv_field("\u{FF0B}cmd"), "\u{FF0B}cmd");
    }
}
```

## Testing Approach

1. **Unit tests** (primary): The test module above covers:
   - Normal values pass through unchanged
   - Formula prefixes (`=`, `+`, `@`) are escaped
   - Pipe (`|`) DDE vectors are escaped
   - Minus handling: `-100` is safe, `-.5` is safe, `-cmd` is escaped
   - Tab and CR at start are escaped
   - Standard CSV special chars (`,`, `"`, `\n`, `\r`) are quoted
   - Combined scenarios (formula + special chars)
   - Edge cases (empty, single char, whitespace, leading-space formulas)
   - Unicode formula markers documented as out of scope

2. **Run tests** with: `./run api:test` or `cargo test escape_csv_field_tests` in the api directory

3. **Integration verification**: Follow the specific commands in Step 3 above

## Performance Note

The updated logic adds a few more character comparisons per field. This is still O(1) per field and negligible even for exports with thousands of rows. No performance concerns.

## Edge Cases Summary

| Input | Expected Output | Reason |
|-------|-----------------|--------|
| `=1+1` | `"'=1+1"` | Formula prefix |
| `+cmd` | `"'+cmd"` | Formula prefix |
| `@SUM` | `"'@SUM"` | Formula prefix |
| `\|cmd` | `"'\|cmd"` | DDE attack vector |
| `-100` | `-100` | Negative number (safe) |
| `-.5` | `-.5` | Negative decimal (safe) |
| `-1e10` | `-1e10` | Scientific notation (safe) |
| `-cmd` | `"'-cmd"` | Minus + letters (dangerous) |
| `-` | `"'-"` | Lone minus (escape for safety) |
| `-.` | `"'-."` | Minus-dot no digit (dangerous) |
| `--flag` | `"'--flag"` | Double dash (dangerous) |
| `\tcmd` | `"'\tcmd"` | Tab prefix |
| `\rcmd` | `"'\rcmd"` | CR prefix |
| ` =1+1` | ` =1+1` | Leading space - safe, no formula execution |
| `＝1+1` | `＝1+1` | Unicode fullwidth equals - out of scope |
| `hello, world` | `"hello, world"` | Standard CSV comma quoting |
| `say "hi"` | `"say ""hi"""` | Standard CSV quote escaping |
| `` | `` | Empty string unchanged |

## Design Decisions

1. **Leading whitespace not escaped**: Spreadsheets (Excel, Google Sheets, LibreOffice) treat ` =1+1` as literal text, not a formula. The leading space prevents formula parsing. This is safe and we don't need to escape it.

2. **Locale-specific decimals**: Values like `-0,50` (comma as decimal separator) are already quoted due to the comma, so formula injection is not a concern. The quoting protects them.

3. **Scientific notation**: `-1e10` starts with `-1` (digit after minus), so it's correctly identified as a negative number and not escaped.

4. **Single quote prefix**: Excel specifically interprets a leading `'` in a cell as "treat this as text". The quote itself is not displayed but prevents formula execution. This is the industry-standard mitigation.

5. **Unicode formula markers out of scope**: Fullwidth formula characters like `＝` (U+FF1D) are not escaped. These are:
   - Extremely rarely exploited in real attacks
   - Would require Unicode normalization or extensive character lists
   - Not worth the complexity; billing data is unlikely to contain fullwidth characters
   - Documented in the function docstring for future reference

## Checklist Alignment

- [x] Review formula detection logic - analyzed current implementation
- [ ] Adjust '-' handling and add '|' case - code changes defined above
- [ ] Add tests for edge cases - test module defined above
- [ ] Verify no other CSV export paths - step 0 above
- [ ] Integration test CSV export - step 3 above with specific commands

## Revision History

- 2026-01-01: Initial plan created (v1)
- 2026-01-01: v2 - Addressed feedback:
  - Added `-.5` decimal handling
  - Added step to verify no other CSV export paths exist
  - Documented leading whitespace behavior
  - Added scientific notation test
  - Added integration testing step
  - Enhanced edge case documentation
- 2026-01-01: v3 - Addressed feedback:
  - Made integration test instructions specific (endpoint, curl commands, verification steps)
  - Added Unicode formula markers as documented out-of-scope design decision
  - Updated test assertions to use raw strings for readability
  - Expanded CSV search to include library patterns (csv::Writer)
  - Clarified test module placement with named module
  - Added performance note (O(1) per field, negligible overhead)
