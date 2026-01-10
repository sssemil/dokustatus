# Plan: Improve CSV Export Formula Injection Guard (v2)

## Summary

The current `escape_csv_field` function in `apps/api/src/application/use_cases/domain_billing.rs` (lines 2200-2223) has incomplete formula injection protection. The implementation needs refinement to:

1. Add missing dangerous character `|` (pipe) which can trigger DDE/formula execution in some spreadsheets
2. Adjust `-` (minus) handling to avoid false positives for legitimate negative numbers and hyphenated values
3. Add comprehensive unit tests for edge cases

## Changes from v1

**Addressing feedback:**

1. **Handle `-.5` decimals**: Updated minus logic to allow `-.` followed by digit (e.g., `-.5`, `-.123`) as safe negative decimals.
2. **Verify no other CSV exports**: Added step to search codebase for other CSV generation paths.
3. **Leading whitespace clarification**: Documented that leading-whitespace formulas (e.g., ` =1+1`) are not escaped because spreadsheets treat them as text, not formulas. This is safe.
4. **Scientific notation**: Documented that `-1e10` will be escaped (starts with `-1` which is digit, so actually safe). Added test.
5. **Integration testing step**: Added manual verification of actual CSV export.
6. **Test for `-.5`**: Added explicit test case.

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
# Search for CSV content types and file extensions
rg -i "text/csv|content-type.*csv|\.csv" apps/api/src/
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

### Step 3: Verify with integration test

After implementing, manually test the actual billing CSV export to ensure:
- Negative amounts display correctly (no spurious escaping)
- Formula-like content is properly escaped
- File opens correctly in Excel/Google Sheets

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

### Test module to add at end of file (before closing brace):

```rust
#[cfg(test)]
mod tests {
    use super::escape_csv_field;

    #[test]
    fn test_escape_csv_field_normal_values() {
        // Plain text should pass through unchanged
        assert_eq!(escape_csv_field("hello"), "hello");
        assert_eq!(escape_csv_field("John Doe"), "John Doe");
        assert_eq!(escape_csv_field("test@email.com"), "test@email.com"); // @ not at start
        assert_eq!(escape_csv_field("100"), "100");
    }

    #[test]
    fn test_escape_csv_field_formula_prefixes() {
        // = prefix (formula)
        assert_eq!(escape_csv_field("=1+1"), "\"'=1+1\"");
        assert_eq!(escape_csv_field("=SUM(A1:A10)"), "\"'=SUM(A1:A10)\"");

        // + prefix (formula)
        assert_eq!(escape_csv_field("+1"), "\"'+1\"");
        assert_eq!(escape_csv_field("+cmd|' /c calc'!A0"), "\"'+cmd|' /c calc'!A0\"");

        // @ prefix (formula)
        assert_eq!(escape_csv_field("@SUM(A1)"), "\"'@SUM(A1)\"");
    }

    #[test]
    fn test_escape_csv_field_pipe_dde() {
        // Pipe can trigger DDE attacks in Excel
        assert_eq!(escape_csv_field("|cmd /c calc"), "\"'|cmd /c calc\"");
        assert_eq!(escape_csv_field("|powershell"), "\"'|powershell\"");
    }

    #[test]
    fn test_escape_csv_field_minus_handling() {
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
        assert_eq!(escape_csv_field("-cmd"), "\"'-cmd\"");
        assert_eq!(escape_csv_field("--help"), "\"'--help\"");
        assert_eq!(escape_csv_field("-"), "\"'-\""); // lone minus
        assert_eq!(escape_csv_field("-."), "\"'-.\""); // minus-dot with no digit
        assert_eq!(escape_csv_field("-.x"), "\"'-.x\""); // minus-dot-letter
    }

    #[test]
    fn test_escape_csv_field_tab_and_cr() {
        // Tab at start
        assert_eq!(escape_csv_field("\tcmd"), "\"'\tcmd\"");

        // Carriage return at start (also needs quoting due to \r)
        assert_eq!(escape_csv_field("\rcmd"), "\"'\rcmd\"");
    }

    #[test]
    fn test_escape_csv_field_standard_csv_escaping() {
        // Commas require quoting
        assert_eq!(escape_csv_field("hello, world"), "\"hello, world\"");

        // Quotes require escaping and quoting
        assert_eq!(escape_csv_field("say \"hello\""), "\"say \"\"hello\"\"\"");

        // Newlines require quoting
        assert_eq!(escape_csv_field("line1\nline2"), "\"line1\nline2\"");
        assert_eq!(escape_csv_field("line1\rline2"), "\"line1\rline2\"");
    }

    #[test]
    fn test_escape_csv_field_combined() {
        // Formula prefix + comma (double protection)
        assert_eq!(escape_csv_field("=SUM(A,B)"), "\"'=SUM(A,B)\"");

        // Formula prefix + quotes
        assert_eq!(escape_csv_field("=EXEC(\"cmd\")"), "\"'=EXEC(\"\"cmd\"\")\"");
    }

    #[test]
    fn test_escape_csv_field_edge_cases() {
        // Empty string
        assert_eq!(escape_csv_field(""), "");

        // Single characters
        assert_eq!(escape_csv_field("a"), "a");
        assert_eq!(escape_csv_field("="), "\"'=\"");
        assert_eq!(escape_csv_field("|"), "\"'|\"");

        // Whitespace (not trimmed, treated as literal)
        assert_eq!(escape_csv_field(" "), " ");
        assert_eq!(escape_csv_field("  "), "  ");

        // Leading whitespace before formula chars - NOT escaped (safe)
        // Spreadsheets treat leading whitespace as literal text
        assert_eq!(escape_csv_field(" =1+1"), " =1+1");
        assert_eq!(escape_csv_field("  +cmd"), "  +cmd");
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

2. **Run tests** with: `./run api:test` or `cargo test escape_csv` in the api directory

3. **Integration verification**: After implementing, manually export a billing CSV and verify:
   - Open in Excel/Google Sheets
   - Negative amounts display correctly (e.g., `-$100.00`)
   - No formula execution warnings
   - Data integrity maintained

## Edge Cases Summary

| Input | Expected Output | Reason |
|-------|-----------------|--------|
| `=1+1` | `"'=1+1"` | Formula prefix |
| `+cmd` | `"'+cmd"` | Formula prefix |
| `@SUM` | `"'@SUM"` | Formula prefix |
| `\|cmd` | `"'\|cmd"` | DDE attack vector (NEW) |
| `-100` | `-100` | Negative number (safe) |
| `-.5` | `-.5` | Negative decimal (safe, FIXED in v2) |
| `-1e10` | `-1e10` | Scientific notation (safe) |
| `-cmd` | `"'-cmd"` | Minus + letters (dangerous) |
| `-` | `"'-"` | Lone minus (escape for safety) |
| `-.` | `"'-."` | Minus-dot no digit (dangerous) |
| `--flag` | `"'--flag"` | Double dash (dangerous) |
| `\tcmd` | `"'\tcmd"` | Tab prefix |
| `\rcmd` | `"'\rcmd"` | CR prefix |
| ` =1+1` | ` =1+1` | Leading space - safe, no formula execution |
| `hello, world` | `"hello, world"` | Standard CSV comma quoting |
| `say "hi"` | `"say ""hi"""` | Standard CSV quote escaping |
| `` | `` | Empty string unchanged |

## Design Decisions

1. **Leading whitespace not escaped**: Spreadsheets (Excel, Google Sheets, LibreOffice) treat ` =1+1` as literal text, not a formula. The leading space prevents formula parsing. This is safe and we don't need to escape it.

2. **Locale-specific decimals**: Values like `-0,50` (comma as decimal separator) are already quoted due to the comma, so formula injection is not a concern. The quoting protects them.

3. **Scientific notation**: `-1e10` starts with `-1` (digit after minus), so it's correctly identified as a negative number and not escaped.

4. **Single quote prefix**: Excel specifically interprets a leading `'` in a cell as "treat this as text". The quote itself is not displayed but prevents formula execution. This is the industry-standard mitigation.

## Checklist Alignment

- [x] Review formula detection logic - analyzed current implementation
- [ ] Adjust '-' handling and add '|' case - code changes defined above
- [ ] Add tests for edge cases - test module defined above
- [ ] Verify no other CSV export paths - step 0 above
- [ ] Integration test CSV export - step 3 above

## Revision History

- 2026-01-01: Initial plan created (v1)
- 2026-01-01: v2 - Addressed feedback:
  - Added `-.5` decimal handling
  - Added step to verify no other CSV export paths exist
  - Documented leading whitespace behavior
  - Added scientific notation test
  - Added integration testing step
  - Enhanced edge case documentation
