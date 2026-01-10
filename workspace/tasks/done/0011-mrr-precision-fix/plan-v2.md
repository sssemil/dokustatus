# MRR Precision Fix - Implementation Plan v2

**Task:** 0011-mrr-precision-fix
**Created:** 2026-01-01
**Revision:** 2 (addresses feedback from plan-v1)

## Summary

The MRR calculation in `apps/api/src/application/use_cases/domain_billing.rs` (lines 1818-1825) uses integer division, causing precision loss. A yearly plan at $119/year yields `11900 / 12 = 991` cents ($9.91) instead of the correct `(11900.0 / 12.0).round() = 992` cents ($9.92).

## Changes from v1

Based on feedback:

1. **Helper function extraction is now mandatory** (was optional) - Creates a pure, testable function
2. **Verified code location** - Confirmed bug at lines 1818-1825; interval_count guard already exists at line 1815
3. **Clarified plan_stats structure** - It's `HashMap<Uuid, (String, i64, i64)>` for `(name, count, revenue_cents)`, needs f64 accumulation
4. **Division-by-zero already handled** - Line 1815: `std::cmp::max(sub.plan.interval_count as i64, 1)`
5. **Rounding consistency** - No other `.round()` in billing code; standard `.round()` is acceptable

---

## Implementation

### Phase 1: Extract Helper Function

**File:** `apps/api/src/application/use_cases/domain_billing.rs`

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
fn calculate_monthly_amount_cents(price_cents: i64, interval: &str, interval_count: i64) -> f64 {
    // Protect against division by zero (should be prevented upstream, but be safe)
    let interval_count = std::cmp::max(interval_count, 1);

    let divisor = match interval {
        "yearly" | "year" => interval_count * 12,
        "monthly" | "month" | _ => interval_count,
    };

    price_cents as f64 / divisor as f64
}
```

### Phase 2: Update get_analytics() to Use Helper

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
// Use f64 for accumulation to preserve precision; round at the end
let mut mrr_cents_f64: f64 = 0.0;
// plan_stats: (name, subscriber_count, revenue_cents_f64)
let mut plan_stats: std::collections::HashMap<Uuid, (String, i64, f64)> =
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

### Phase 3: Update plan_distribution Conversion

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

### Phase 4: Add Unit Tests

Add a `#[cfg(test)]` module at the bottom of `domain_billing.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_monthly_amount_yearly_plan() {
        // $119/year = 11900 cents / 12 months = 991.666... cents
        let result = calculate_monthly_amount_cents(11900, "yearly", 1);
        assert!((result - 991.6666666666666).abs() < 0.0001);
        assert_eq!(result.round() as i64, 992); // Rounds to 992, not truncates to 991
    }

    #[test]
    fn test_calculate_monthly_amount_monthly_plan() {
        // $9.99/month = 999 cents, interval_count=1
        let result = calculate_monthly_amount_cents(999, "monthly", 1);
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
        // Edge case: interval_count=0 should be treated as 1
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
}
```

---

## Files to Modify

| File | Change |
|------|--------|
| `apps/api/src/application/use_cases/domain_billing.rs` | Add helper function, update MRR calculation, add tests |

---

## Verification Steps

1. **Run tests:** `./run api:test`
2. **Build check:** `./run api:build`
3. **Format check:** `./run api:fmt`
4. **Lint check:** `./run api:lint`

---

## Edge Cases (All Covered)

| Case | Expected Behavior |
|------|-------------------|
| Zero subscribers | `mrr_cents: 0` |
| Free plans ($0) | Contributes 0 to MRR |
| Small prices ($1.99/year) | Rounds to 17 cents/month |
| Large interval_count (e.g., 2 years) | Divides by 24 |
| Mixed intervals | Accumulates all as f64, rounds total |
| interval_count = 0 | Treated as 1 (defensive) |

---

## Risks Assessment

| Risk Level | Description | Mitigation |
|------------|-------------|------------|
| Low | Slight change in displayed MRR values | Values become more accurate; no action needed |
| Low | No historical data affected | Analytics are calculated live; no stored values change |
| Low | f64 precision limits | Safe up to ~$90 trillion MRR; not a concern |

---

## Future Considerations (Not in Scope)

- **Millicents storage:** If sub-cent precision is ever needed, consider storing prices in millicents. Not needed now.
- **Other integer divisions:** A separate audit could check for similar bugs elsewhere in billing code.

---

## History

- 2026-01-01 v1 created
- 2026-01-01 v2 revised based on feedback:
  - Made helper function extraction mandatory
  - Verified code location at lines 1818-1825
  - Confirmed division-by-zero guard exists at line 1815
  - Clarified plan_stats structure is `HashMap<Uuid, (String, i64, i64)>`
  - Confirmed no other `.round()` usage in billing code
  - Added comprehensive tests including accumulation tests
