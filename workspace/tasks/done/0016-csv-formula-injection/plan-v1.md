# Plan: Improve CSV Export Formula Injection Guard

## Summary

The current `escape_csv_field` function in `apps/api/src/application/use_cases/domain_billing.rs` (lines 2200-2223) has incomplete formula injection protection. The implementation needs refinement to:

1. Add missing dangerous character `|` (pipe) which can trigger DDE/formula execution in some spreadsheets
2. Adjust `-` (minus) handling to avoid false positives for legitimate negative numbers and hyphenated values
3. Add comprehensive unit tests for edge cases

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

### Step 1: Update formula detection logic

Modify the `is_formula` check to:
- Add `|` to the dangerous characters list
- Adjust `-` handling: only treat as formula trigger if followed by non-digit character (e.g., `-cmd` is dangerous, `-100` is safe)
- Keep all other checks as-is

### Step 2: Add comprehensive unit tests

Add a `#[cfg(test)]` module with tests covering:
- Standard formula prefixes (`=`, `+`, `@`)
- Pipe character (`|`)
- Minus with formulas vs. negative numbers
- Tab and carriage return prefixes
- Standard CSV escaping (commas, quotes, newlines)
- Combined scenarios (formula + special chars)
- Empty and whitespace-only inputs
- Unicode and non-ASCII content

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
/// Spreadsheet applications (Excel, Google Sheets, etc.) will execute formulas
/// starting with =, +, -, @, |, tab, or carriage return. We prefix such values
/// with a single quote to prevent formula injection attacks.
///
/// Special handling for `-`:
/// - `-` followed by a digit is treated as a negative number (safe)
/// - `-` followed by non-digit is treated as potential formula (escaped)
fn escape_csv_field(field: &str) -> String {
    let needs_quoting =
        field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r');

    // Check for formula injection characters at start
    let is_formula = {
        let mut chars = field.chars();
        match chars.next() {
            Some('=' | '+' | '@' | '|' | '\t' | '\r') => true,
            Some('-') => {
                // Only treat as formula if next char is not a digit (negative numbers are safe)
                !chars.next().map(|c| c.is_ascii_digit()).unwrap_or(false)
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
    }

    #[test]
    fn test_escape_csv_field_formula_prefixes() {
        // = prefix
        assert_eq!(escape_csv_field("=1+1"), "\"'=1+1\"");
        assert_eq!(escape_csv_field("=SUM(A1:A10)"), "\"'=SUM(A1:A10)\"");

        // + prefix
        assert_eq!(escape_csv_field("+1"), "\"'+1\"");
        assert_eq!(escape_csv_field("+cmd|' /c calc'!A0"), "\"'+cmd|' /c calc'!A0\"");

        // @ prefix
        assert_eq!(escape_csv_field("@SUM(A1)"), "\"'@SUM(A1)\"");
    }

    #[test]
    fn test_escape_csv_field_pipe_dde() {
        // Pipe can trigger DDE attacks
        assert_eq!(escape_csv_field("|cmd /c calc"), "\"'|cmd /c calc\"");
        assert_eq!(escape_csv_field("|powershell"), "\"'|powershell\"");
    }

    #[test]
    fn test_escape_csv_field_minus_handling() {
        // Negative numbers should NOT be escaped (false positive prevention)
        assert_eq!(escape_csv_field("-100"), "-100");
        assert_eq!(escape_csv_field("-42.50"), "-42.50");
        assert_eq!(escape_csv_field("-0"), "-0");

        // Minus followed by non-digit SHOULD be escaped
        assert_eq!(escape_csv_field("-cmd"), "\"'-cmd\"");
        assert_eq!(escape_csv_field("--help"), "\"'--help\"");
        assert_eq!(escape_csv_field("-"), "\"'-\""); // lone minus
    }

    #[test]
    fn test_escape_csv_field_tab_and_cr() {
        // Tab at start
        assert_eq!(escape_csv_field("\tcmd"), "\"'\tcmd\"");

        // Carriage return at start
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

        // Whitespace
        assert_eq!(escape_csv_field(" "), " ");
        assert_eq!(escape_csv_field("  "), "  ");
    }
}
```

## Testing Approach

1. **Unit tests** (primary): Add the test module above which covers:
   - Normal values pass through unchanged
   - Formula prefixes (`=`, `+`, `@`) are escaped
   - Pipe (`|`) DDE vectors are escaped
   - Minus handling: `-100` is safe, `-cmd` is escaped
   - Tab and CR at start are escaped
   - Standard CSV special chars (`,`, `"`, `\n`, `\r`) are quoted
   - Combined scenarios (formula + special chars)
   - Edge cases (empty, single char, etc.)

2. **Run tests** with: `./run api:test` or `cargo test escape_csv` in the api directory

## Edge Cases to Handle

| Input | Expected Output | Reason |
|-------|-----------------|--------|
| `=1+1` | `"'=1+1"` | Formula prefix |
| `+cmd` | `"'+cmd"` | Formula prefix |
| `@SUM` | `"'@SUM"` | Formula prefix |
| `\|cmd` | `"'\|cmd"` | DDE attack vector (NEW) |
| `-100` | `-100` | Negative number (safe, FIX) |
| `-cmd` | `"'-cmd"` | Minus + letters (dangerous) |
| `-` | `"'-"` | Lone minus (escape for safety) |
| `--flag` | `"'--flag"` | Double dash (dangerous) |
| `\tcmd` | `"'\tcmd"` | Tab prefix |
| `\rcmd` | `"'\rcmd"` | CR prefix |
| `hello, world` | `"hello, world"` | Standard CSV comma quoting |
| `say "hi"` | `"say ""hi"""` | Standard CSV quote escaping |
| `` | `` | Empty string unchanged |

## Checklist Alignment

- [x] Review formula detection logic - analyzed current implementation
- [ ] Adjust '-' handling and add '|' case - code changes defined above
- [ ] Add tests for edge cases - test module defined above

## Revision History

- 2026-01-01: Initial plan created
