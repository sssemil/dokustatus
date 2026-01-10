# MRR Precision Fix - Implementation Plan v1

**Task:** 0011-mrr-precision-fix
**Created:** 2026-01-01

## Summary

The MRR (Monthly Recurring Revenue) calculation in `apps/api/src/application/use_cases/domain_billing.rs` uses integer division which causes precision loss. For example, a yearly plan at $119/year should yield $9.92/month MRR, but integer division `119 / 12 = 9` loses ~$0.92/month per subscriber.

The problem is in the `get_analytics` function around lines 1818-1824:

```rust
let monthly_amount = match sub.plan.interval.as_str() {
    "yearly" | "year" => price / (interval_count * 12),  // Integer division!
    "monthly" | "month" => price / interval_count,        // Integer division!
    _ => price / interval_count,
};
```

## Impact Analysis

- **Affected Values**: `mrr_cents` in `BillingAnalytics`, per-plan `revenue_cents` in `PlanDistribution`
- **Scope**: Backend calculation only - frontend just displays what backend returns
- **API Contract**: Field name `mrr_cents` and type `i64` remain unchanged; only the calculation accuracy improves

## Step-by-Step Implementation

### Phase 1: Fix MRR Calculation

**File:** `apps/api/src/application/use_cases/domain_billing.rs`

1. Modify the MRR calculation loop in `get_analytics()` (lines ~1802-1834) to use floating-point arithmetic for intermediate calculations, then round to nearest integer at the end:

   ```rust
   // Current (broken):
   let monthly_amount = match sub.plan.interval.as_str() {
       "yearly" | "year" => price / (interval_count * 12),
       "monthly" | "month" => price / interval_count,
       _ => price / interval_count,
   };

   // Fixed approach - use f64 for calculation, round at end:
   let monthly_amount = match sub.plan.interval.as_str() {
       "yearly" | "year" => (price as f64) / ((interval_count * 12) as f64),
       "monthly" | "month" => (price as f64) / (interval_count as f64),
       _ => (price as f64) / (interval_count as f64),
   };
   ```

2. Accumulate MRR as `f64` then convert to `i64` with proper rounding at the end:
   - Change `mrr_cents: i64 = 0` to `mrr_cents_f64: f64 = 0.0`
   - Accumulate: `mrr_cents_f64 += monthly_amount;`
   - At the end: `mrr_cents: mrr_cents_f64.round() as i64`

3. Same approach for per-plan `revenue_cents` in `plan_stats` HashMap:
   - Store accumulated revenue as `f64`
   - Convert to `i64` with rounding when building `PlanDistribution`

### Phase 2: Add Unit Tests

**File:** `apps/api/src/application/use_cases/domain_billing.rs`

Add a `#[cfg(test)]` module with tests for the MRR calculation logic:

1. **Test: Yearly plan precision**
   - Input: 1 subscriber, $119.00/year (11900 cents)
   - Expected MRR: `(11900 / 12).round() = 992 cents` ($9.92)
   - Current broken result: `11900 / 12 = 991 cents` ($9.91 - loses $0.01)

2. **Test: Multiple yearly subscribers**
   - Input: 10 subscribers at $119/year
   - Expected: `(119 * 10 / 12 * 100).round() = 9917 cents` ($99.17)
   - Demonstrates cumulative error

3. **Test: Edge case - very small monthly**
   - Input: $1.99/year plan
   - Expected: `(199 / 12).round() = 17 cents`
   - Not 16 cents (floor)

4. **Test: Mixed intervals**
   - Combine monthly and yearly plans
   - Verify total rounds correctly

### Phase 3: Verify Existing Tests Pass

Run existing tests to ensure no regressions:

```bash
./run api:test
```

## Files to Modify

| File | Change |
|------|--------|
| `apps/api/src/application/use_cases/domain_billing.rs` | Fix MRR calculation, add tests |

## Edge Cases to Handle

1. **Zero subscribers**: Should return `mrr_cents: 0`
2. **Zero price plans**: Free plans should contribute 0 to MRR
3. **Very small prices**: Sub-dollar amounts like $0.99/year should round properly
4. **Large interval_count**: e.g., `interval_count=2` for "every 2 years" = divide by 24
5. **Mix of intervals**: Yearly + monthly subscribers in same calculation
6. **Single subscriber rounding**: Even one subscriber should round, not truncate

## Testing Approach

Since there are no existing tests for `get_analytics()`, we add unit tests that:
- Extract the MRR calculation logic into a testable helper function (optional but cleaner)
- Test edge cases directly with known inputs/outputs
- Follow the project's existing test pattern (simple `#[test]` functions in a `#[cfg(test)] mod tests` block)

## Verification Steps

1. Run `./run api:test` - all tests pass
2. Run `./run api:build` - builds successfully
3. Manual verification (optional): Query analytics endpoint with known subscription data

---

## History

- 2026-01-01 Created implementation plan from code review finding #11
