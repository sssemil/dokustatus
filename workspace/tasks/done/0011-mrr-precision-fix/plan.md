# MRR Precision Fix - Implementation Plan v3

**Task:** 0011-mrr-precision-fix
**Created:** 2026-01-01
**Revision:** 3 (final)

## Summary

The MRR calculation in `apps/api/src/application/use_cases/domain_billing.rs` (lines 1818-1825) uses integer division, causing precision loss. A yearly plan at $119/year yields `11900 / 12 = 991` cents ($9.91) instead of the correct `(11900.0 / 12.0).round() = 992` cents ($9.92).

## Changes from v2

Based on feedback:

1. **Verified PlanDistribution struct** - Confirmed at line 290-295: `revenue_cents: i64`. Our approach of accumulating as `f64` and rounding to `i64` is compatible.
2. **No breaking tests** - Searched for `mrr_cents.*==` and `assert.*mrr` patterns; no existing exact-value assertions found.
3. **Added MONTHS_PER_YEAR constant** - Replaces magic number `12`.
4. **Added debug assertion** - `debug_assert!(price_cents >= 0)` for sanity checking.
5. **Documented wildcard match** - Explicit comment that unknown intervals are treated as monthly.
6. **Added type alias** - `PlanAccumulator` for clarity instead of raw tuple.
7. **Noted rounding mode** - Standard `.round()` (round half away from zero) is appropriate for financial calculations.
8. **Integration test out of scope** - Would require database mocking infrastructure; unit tests on the helper are sufficient for this bug fix.

---

## Implementation

### Phase 1: Add Constants and Type Alias

**File:** `apps/api/src/application/use_cases/domain_billing.rs`

Add near the top of the file (after imports, before structs):

```rust
/// Number of months in a year, used for MRR calculations
const MONTHS_PER_YEAR: i64 = 12;
```

### Phase 2: Extract Helper Function

Add a pure helper function before `get_analytics()`:

```rust
/// Calculates the monthly equivalent amount in cents for a given price and billing interval.
/// Uses floating-point arithmetic to avoid integer division precision loss.
///
/// # Arguments
/// * `price_cents` - The price in cents for the billing interval
/// * `interval` - The interval type ("yearly", "year", "monthly", "month", or other)
/// * `interval_count` - Number of intervals (e.g., 2 for "every 2 months")
///
/// # Returns
/// Monthly amount as f64 (caller should accumulate and round at the end)
///
/// # Rounding
/// Uses standard `.round()` (round half away from zero). For financial calculations
/// this is appropriate as it treats positive and negative values symmetrically.
fn calculate_monthly_amount_cents(price_cents: i64, interval: &str, interval_count: i64) -> f64 {
    debug_assert!(price_cents >= 0, "price_cents should not be negative");

    // Protect against division by zero (should be prevented upstream, but be defensive)
    let interval_count = std::cmp::max(interval_count, 1);

    let divisor = match interval {
        "yearly" | "year" => interval_count * MONTHS_PER_YEAR,
        // Unknown intervals are treated as monthly (legacy behavior preserved)
        "monthly" | "month" | _ => interval_count,
    };

    price_cents as f64 / divisor as f64
}
```

### Phase 3: Update get_analytics() to Use Helper

Modify the MRR calculation loop (lines 1807-1834):

**Before:**
```rust
let mut mrr_cents: i64 = 0;
let mut plan_stats: std::collections::HashMap<Uuid, (String, i64, i64)> =
    std::collections::HashMap::new();

for sub in &subscribers {
    if sub.subscription.status.is_active() {
        let interval_count = std::cmp::max(sub.plan.interval_count as i64, 1);
        let price = sub.plan.price_cents as i64;

        let monthly_amount = match sub.plan.interval.as_str() {
            "yearly" | "year" => price / (interval_count * 12),
            "monthly" | "month" => price / interval_count,
            _ => price / interval_count,
        };
        mrr_cents += monthly_amount;

        let entry = plan_stats
            .entry(sub.plan.id)
            .or_insert((sub.plan.name.clone(), 0, 0));
        entry.1 += 1;
        entry.2 += monthly_amount;
    }
}
```

**After:**
```rust
// Accumulator tuple: (plan_name, subscriber_count, revenue_cents_f64)
type PlanAccumulator = (String, i64, f64);

// Use f64 for accumulation to preserve precision; round at the end
let mut mrr_cents_f64: f64 = 0.0;
let mut plan_stats: std::collections::HashMap<Uuid, PlanAccumulator> =
    std::collections::HashMap::new();

for sub in &subscribers {
    if sub.subscription.status.is_active() {
        let interval_count = sub.plan.interval_count as i64;
        let price = sub.plan.price_cents as i64;

        let monthly_amount = calculate_monthly_amount_cents(
            price,
            sub.plan.interval.as_str(),
            interval_count,
        );
        mrr_cents_f64 += monthly_amount;

        let entry = plan_stats
            .entry(sub.plan.id)
            .or_insert((sub.plan.name.clone(), 0, 0.0));
        entry.1 += 1;
        entry.2 += monthly_amount;
    }
}

// Convert to i64 with proper rounding
let mrr_cents = mrr_cents_f64.round() as i64;
```

### Phase 4: Update plan_distribution Conversion

Modify the final conversion (lines 1836-1844):

**Before:**
```rust
let plan_distribution = plan_stats
    .into_iter()
    .map(|(id, (name, count, revenue))| PlanDistribution {
        plan_id: id,
        plan_name: name,
        subscriber_count: count,
        revenue_cents: revenue,
    })
    .collect();
```

**After:**
```rust
let plan_distribution = plan_stats
    .into_iter()
    .map(|(id, (name, count, revenue_f64))| PlanDistribution {
        plan_id: id,
        plan_name: name,
        subscriber_count: count,
        revenue_cents: revenue_f64.round() as i64,
    })
    .collect();
```

### Phase 5: Add Unit Tests

Add a `#[cfg(test)]` module at the bottom of `domain_billing.rs`:

```rust
#[cfg(test)]
mod mrr_calculation_tests {
    use super::*;

    #[test]
    fn test_calculate_monthly_amount_yearly_plan() {
        // $119/year = 11900 cents / 12 months = 991.666... cents
        let result = calculate_monthly_amount_cents(11900, "yearly", 1);
        assert!((result - 991.6666666666666).abs() < 0.0001);
        assert_eq!(result.round() as i64, 992); // Rounds to 992, not truncates to 991
    }

    #[test]
    fn test_calculate_monthly_amount_year_alias() {
        // "year" should behave same as "yearly"
        let result = calculate_monthly_amount_cents(11900, "year", 1);
        assert_eq!(result.round() as i64, 992);
    }

    #[test]
    fn test_calculate_monthly_amount_monthly_plan() {
        // $9.99/month = 999 cents, interval_count=1
        let result = calculate_monthly_amount_cents(999, "monthly", 1);
        assert_eq!(result, 999.0);
    }

    #[test]
    fn test_calculate_monthly_amount_month_alias() {
        // "month" should behave same as "monthly"
        let result = calculate_monthly_amount_cents(999, "month", 1);
        assert_eq!(result, 999.0);
    }

    #[test]
    fn test_calculate_monthly_amount_biannual() {
        // $200 every 2 years = 20000 cents / 24 months = 833.333... cents
        let result = calculate_monthly_amount_cents(20000, "yearly", 2);
        assert!((result - 833.3333333333334).abs() < 0.0001);
        assert_eq!(result.round() as i64, 833);
    }

    #[test]
    fn test_calculate_monthly_amount_quarterly() {
        // $30 every 3 months = 3000 cents / 3 = 1000 cents
        let result = calculate_monthly_amount_cents(3000, "monthly", 3);
        assert_eq!(result, 1000.0);
    }

    #[test]
    fn test_calculate_monthly_amount_zero_price() {
        // Free plan
        let result = calculate_monthly_amount_cents(0, "yearly", 1);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_calculate_monthly_amount_zero_interval_count() {
        // Edge case: interval_count=0 should be treated as 1 (defensive)
        let result = calculate_monthly_amount_cents(1200, "monthly", 0);
        assert_eq!(result, 1200.0);
    }

    #[test]
    fn test_calculate_monthly_amount_very_small_yearly() {
        // $1.99/year = 199 cents / 12 = 16.583... cents
        let result = calculate_monthly_amount_cents(199, "yearly", 1);
        assert_eq!(result.round() as i64, 17); // Rounds up, not truncates to 16
    }

    #[test]
    fn test_calculate_monthly_amount_unknown_interval() {
        // Unknown intervals default to monthly behavior (legacy)
        let result = calculate_monthly_amount_cents(1500, "weekly", 1);
        assert_eq!(result, 1500.0); // Treated as monthly
    }

    #[test]
    fn test_mrr_accumulation_precision() {
        // 10 subscribers at $119/year each
        // Each: 11900/12 = 991.666...
        // Sum: 9916.666...
        // Rounded: 9917 cents
        let mut total = 0.0;
        for _ in 0..10 {
            total += calculate_monthly_amount_cents(11900, "yearly", 1);
        }
        assert_eq!(total.round() as i64, 9917);

        // Old integer math would give: 10 * (11900 / 12) = 10 * 991 = 9910
        // We gain 7 cents of accuracy
    }

    #[test]
    fn test_mrr_accumulation_mixed_intervals() {
        // 5 yearly at $119 + 3 monthly at $9.99
        let mut total = 0.0;
        for _ in 0..5 {
            total += calculate_monthly_amount_cents(11900, "yearly", 1);
        }
        for _ in 0..3 {
            total += calculate_monthly_amount_cents(999, "monthly", 1);
        }
        // 5 * 991.666... + 3 * 999 = 4958.333... + 2997 = 7955.333...
        assert_eq!(total.round() as i64, 7955);
    }

    #[test]
    fn test_mrr_large_subscriber_count() {
        // 1000 subscribers at $119/year
        // Each: 991.666...
        // Total: 991666.666...
        // Rounded: 991667 cents = $9916.67 MRR
        let mut total = 0.0;
        for _ in 0..1000 {
            total += calculate_monthly_amount_cents(11900, "yearly", 1);
        }
        assert_eq!(total.round() as i64, 991667);

        // Old integer math: 1000 * 991 = 991000 (loses $6.67)
    }
}
```

---

## Files to Modify

| File | Change |
|------|--------|
| `apps/api/src/application/use_cases/domain_billing.rs` | Add constant, helper function, update MRR calculation, add tests |

---

## Verification Steps

1. **Run tests:** `./run api:test` - All new tests should pass
2. **Build check:** `./run api:build` - Verify offline build succeeds
3. **Format check:** `./run api:fmt` - Ensure code is formatted
4. **Lint check:** `./run api:lint` - Verify no clippy warnings

---

## Pre-Implementation Checklist

- [x] Verified `PlanDistribution` struct has `revenue_cents: i64` (line 290-295)
- [x] Confirmed no existing tests assert exact MRR values
- [x] Confirmed no other `.round()` usage in billing code (no inconsistency introduced)
- [x] Division-by-zero guard exists at line 1815; helper adds redundant guard defensively

---

## Edge Cases (All Covered)

| Case | Expected Behavior | Test |
|------|-------------------|------|
| Zero subscribers | `mrr_cents: 0` | N/A (no accumulation) |
| Free plans ($0) | Contributes 0 to MRR | `test_calculate_monthly_amount_zero_price` |
| Small prices ($1.99/year) | Rounds to 17 cents/month | `test_calculate_monthly_amount_very_small_yearly` |
| Large interval_count (2 years) | Divides by 24 | `test_calculate_monthly_amount_biannual` |
| Mixed intervals | Accumulates all as f64, rounds total | `test_mrr_accumulation_mixed_intervals` |
| interval_count = 0 | Treated as 1 (defensive) | `test_calculate_monthly_amount_zero_interval_count` |
| Unknown interval | Treated as monthly (legacy) | `test_calculate_monthly_amount_unknown_interval` |
| Large subscriber count | Precision maintained | `test_mrr_large_subscriber_count` |

---

## Risks Assessment

| Risk Level | Description | Mitigation |
|------------|-------------|------------|
| Low | Slight change in displayed MRR values | Values become more accurate; verified no tests break |
| Low | No historical data affected | Analytics are calculated live; no stored values change |
| Low | f64 precision limits | Safe up to ~$90 trillion MRR; not a concern |
| None | Clippy warnings | Standard `as f64`/`as i64` casts are idiomatic; will verify with `./run api:lint` |

---

## Out of Scope

- **Integration test:** Would require database mocking infrastructure not currently in place. Unit tests on the pure helper function are sufficient.
- **Millicents storage:** If sub-cent precision is ever needed, consider storing prices in millicents. Not needed now.
- **Audit other integer divisions:** A separate task could audit for similar bugs elsewhere.

---

## History

- 2026-01-01 v1: Initial plan created
- 2026-01-01 v2: Made helper function mandatory, verified code location, added comprehensive tests
- 2026-01-01 v3: Final revision addressing feedback:
  - Verified `PlanDistribution` struct definition (line 290-295)
  - Confirmed no breaking test assertions
  - Added `MONTHS_PER_YEAR` constant
  - Added `debug_assert!` for negative price sanity check
  - Documented wildcard match behavior
  - Added `PlanAccumulator` type alias
  - Added tests for interval aliases ("year", "month") and unknown intervals
  - Added large subscriber count test
  - Documented rounding mode (round half away from zero)
