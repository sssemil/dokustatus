# Feedback on MRR Precision Fix Plan v1

**Reviewer:** Claude Opus 4.5
**Date:** 2026-01-01

---

## What's Good

1. **Clear problem identification**: The plan correctly identifies the integer division issue at lines 1818-1824 and explains the precision loss with a concrete example ($119/year → $9.92 vs $9.91).

2. **Minimal scope**: Only one file to modify, and the API contract (field name + type) stays the same. Low blast radius.

3. **Concrete fix approach**: Using `f64` for intermediate calculations and `.round()` at the end is the standard solution for this class of bug.

4. **Good edge case list**: Covers zero subscribers, zero price plans, small prices, large interval_count, and mixed intervals.

5. **Test cases are specific**: Actual input/output values with expected cents calculations.

---

## What's Missing or Unclear

### 1. No code verification step

The plan assumes the bug is at lines 1818-1824 but doesn't include actually reading the current code to verify. Line numbers may have drifted. **Action:** Before implementing, read `domain_billing.rs` to confirm the exact location and context.

### 2. Accumulation strategy for per-plan revenue is vague

Phase 1, step 3 says "Store accumulated revenue as f64" for `plan_stats`, but doesn't specify:
- What is the current type/structure of `plan_stats`?
- Does `revenue_cents` get set per-subscriber or summed across all subscribers of a plan?
- When exactly to convert (per-plan or at final PlanDistribution build)?

**Action:** Read the `plan_stats` HashMap usage to understand the current structure before changing it.

### 3. Test location unclear

Phase 2 says add tests to `domain_billing.rs`, but:
- The function `get_analytics()` likely requires database context (subscriptions, plans). Pure unit tests may not be straightforward.
- The plan mentions "Extract the MRR calculation logic into a testable helper function (optional but cleaner)" but doesn't commit to this approach.

**Action:** Decide whether to:
- (a) Extract a pure `calculate_mrr(price: i64, interval: &str, interval_count: i64) -> f64` helper that's trivially testable, OR
- (b) Write integration tests that set up mock/test subscriptions

Option (a) is simpler and more robust. Make it a firm plan, not optional.

### 4. No mention of `interval_count` validation

What happens if `interval_count` is 0? Division by zero would panic. The plan doesn't address this.

**Action:** Either:
- Confirm that `interval_count` is validated elsewhere (DB constraint, deserialization)
- Add a guard: `if interval_count == 0 { interval_count = 1 }` or skip that subscription with a warning

### 5. Rounding strategy not fully specified

The plan uses `.round()` which is "round half away from zero" in Rust. For financial calculations, "banker's rounding" (round half to even) is sometimes preferred. Should this match any existing rounding conventions in the codebase?

**Action:** Search for other `.round()` usages in the billing code to ensure consistency.

---

## Suggested Improvements

1. **Extract helper function (make this mandatory, not optional):**
   ```rust
   fn calculate_monthly_amount_cents(price_cents: i64, interval: &str, interval_count: i64) -> f64 {
       let divisor = match interval {
           "yearly" | "year" => interval_count * 12,
           _ => interval_count,
       };
       if divisor == 0 { return 0.0; }
       price_cents as f64 / divisor as f64
   }
   ```
   This makes testing trivial and the main function cleaner.

2. **Add rounding consistency check to Phase 3:** After fixing, grep for other integer divisions in billing code that might have the same bug.

3. **Consider cents_100 (millicents) for sub-cent precision:** If future plans might have prices like $0.001/month, storing in millicents (1/1000 of a cent) avoids floating point entirely. Probably overkill for now, but worth noting.

4. **Document the fix in code comments:** A brief comment explaining why f64 is used would help future maintainers.

---

## Risks and Concerns

### Low Risk

- **Semantic change to `mrr_cents` values:** Existing analytics data (if stored historically) won't retroactively update. If dashboards compare month-over-month, there may be a visible bump. This is actually correct behavior, but worth noting in release notes.

### Medium Risk

- **Untested code path:** The plan acknowledges there are no existing tests for `get_analytics()`. If the implementation touches adjacent logic, regressions could go unnoticed. Mitigate by extracting the helper and testing it thoroughly.

### Low Risk

- **f64 precision for very large amounts:** At $1 billion/month MRR, f64 can represent values exactly up to ~2^53 cents (~$90 trillion). Not a real concern.

---

## Verdict

**Plan is solid and ready for implementation** with one mandatory change:

> **Make helper function extraction required, not optional.** This is the cleanest path to testable, maintainable code.

Minor additions:
- Verify current code location before implementing
- Add division-by-zero guard
- Check for rounding consistency with existing code
