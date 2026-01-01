# Plan v1: Transactional Domain Delete

## Summary

The current domain deletion implementation in `apps/api/src/adapters/persistence/domain.rs:155-162` performs a single `DELETE FROM domains WHERE id = $1` query without wrapping it in a transaction. While most related tables have `ON DELETE CASCADE` constraints, this creates potential issues:

1. **Race conditions**: Another request could insert related data between the cascade deletes
2. **Partial failures**: If the server crashes during cascade operations, we could have orphaned data
3. **Missing tables**: Some tables use `ON DELETE SET NULL` or `ON DELETE RESTRICT` which won't cascade

The goal is to ensure domain deletion removes all related data atomically within a transaction.

## Current State Analysis

### Tables with `ON DELETE CASCADE` (already handled automatically):
| Table | Foreign Key |
|-------|-------------|
| domain_end_users | domain_id -> domains(id) |
| domain_auth_config | domain_id -> domains(id) |
| domain_auth_magic_link | domain_id -> domains(id) |
| domain_auth_google_oauth | domain_id -> domains(id) |
| domain_api_keys | domain_id -> domains(id) |
| domain_roles | domain_id -> domains(id) |
| domain_billing_stripe_config | domain_id -> domains(id) |
| subscription_plans | domain_id -> domains(id) |
| user_subscriptions | domain_id -> domains(id) |
| billing_payments | domain_id -> domains(id) |
| domain_enabled_payment_providers | domain_id -> domains(id) |
| subscription_events | subscription_id -> user_subscriptions(id) (cascades through user_subscriptions) |

### Tables with Non-Cascade Constraints:
| Table | Foreign Key | On Delete |
|-------|-------------|-----------|
| domains | owner_end_user_id -> domain_end_users(id) | SET NULL |
| domain_api_keys | created_by_end_user_id -> domain_end_users(id) | SET NULL |
| user_subscriptions | plan_id -> subscription_plans(id) | RESTRICT |
| user_subscriptions | granted_by -> domain_end_users(id) | (no action specified) |
| billing_payments | subscription_id -> user_subscriptions(id) | SET NULL |
| billing_payments | plan_id -> subscription_plans(id) | SET NULL |
| subscription_events | created_by -> domain_end_users(id) | (no action specified) |

### Current Delete Flow:
1. Use case (`domain.rs:158-169`) checks ownership
2. Persistence layer (`domain.rs:155-162`) executes simple DELETE
3. PostgreSQL cascades to related tables

### Problem:
The `ON DELETE RESTRICT` on `user_subscriptions.plan_id` means deleting a domain with active subscriptions will fail because:
- Domain delete cascades to subscription_plans
- subscription_plans delete is blocked by user_subscriptions referencing them

However, since `user_subscriptions.domain_id` also has `ON DELETE CASCADE`, PostgreSQL should handle this correctly by deleting the subscriptions first. Let me verify the delete order is correct.

After analyzing PostgreSQL's cascade behavior: when deleting a domain, PostgreSQL will:
1. Delete domain_end_users (CASCADE from domains)
2. Delete user_subscriptions (CASCADE from domains AND domain_end_users)
3. Delete subscription_plans (CASCADE from domains)

The order is determined by PostgreSQL's internal dependency tracking, which should handle this correctly since user_subscriptions has CASCADE on domain_id.

## Implementation Approach

### Option A: Explicit Transaction (Recommended)
Wrap the deletion in an explicit transaction for:
1. Atomicity guarantees even if PostgreSQL behavior changes
2. Ability to add pre-delete validation/cleanup
3. Consistent error handling
4. Future-proofing for additional cleanup steps

### Option B: Trust CASCADE
Keep current implementation since all domain_id FKs have CASCADE. The RESTRICT on plan_id is mitigated by user_subscriptions also cascading on domain_id.

**Decision: Option A** - Explicit transaction provides better guarantees and allows for future extensibility.

## Step-by-Step Implementation

### Step 1: Add Transaction Support to PostgresPersistence

**File:** `apps/api/src/adapters/persistence/mod.rs`

Add a method to begin a transaction:

```rust
impl PostgresPersistence {
    pub async fn begin_transaction(&self) -> AppResult<sqlx::Transaction<'_, sqlx::Postgres>> {
        self.pool.begin().await.map_err(AppError::from)
    }
}
```

### Step 2: Modify DomainRepo Trait

**File:** `apps/api/src/application/use_cases/domain.rs`

Update the `delete` method signature to support transactional deletion, or add a new `delete_with_related` method:

```rust
#[async_trait]
pub trait DomainRepo: Send + Sync {
    // ... existing methods ...

    /// Delete a domain and all related data atomically
    async fn delete(&self, domain_id: Uuid) -> AppResult<()>;
}
```

The signature stays the same, but the implementation changes.

### Step 3: Implement Transactional Delete

**File:** `apps/api/src/adapters/persistence/domain.rs`

Replace the simple delete with a transactional version:

```rust
async fn delete(&self, domain_id: Uuid) -> AppResult<()> {
    let mut tx = self.pool.begin().await.map_err(AppError::from)?;

    // Delete in order to avoid FK constraint issues
    // Even though CASCADE handles most cases, explicit ordering provides safety

    // 1. Delete subscription events (via user_subscriptions cascade, but be explicit)
    sqlx::query("DELETE FROM subscription_events WHERE subscription_id IN (SELECT id FROM user_subscriptions WHERE domain_id = $1)")
        .bind(domain_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    // 2. Delete billing payments
    sqlx::query("DELETE FROM billing_payments WHERE domain_id = $1")
        .bind(domain_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    // 3. Delete user subscriptions
    sqlx::query("DELETE FROM user_subscriptions WHERE domain_id = $1")
        .bind(domain_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    // 4. Delete subscription plans
    sqlx::query("DELETE FROM subscription_plans WHERE domain_id = $1")
        .bind(domain_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    // 5. Delete enabled payment providers
    sqlx::query("DELETE FROM domain_enabled_payment_providers WHERE domain_id = $1")
        .bind(domain_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    // 6. Delete billing stripe config
    sqlx::query("DELETE FROM domain_billing_stripe_config WHERE domain_id = $1")
        .bind(domain_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    // 7. Delete roles
    sqlx::query("DELETE FROM domain_roles WHERE domain_id = $1")
        .bind(domain_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    // 8. Delete API keys
    sqlx::query("DELETE FROM domain_api_keys WHERE domain_id = $1")
        .bind(domain_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    // 9. Delete auth configs
    sqlx::query("DELETE FROM domain_auth_google_oauth WHERE domain_id = $1")
        .bind(domain_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    sqlx::query("DELETE FROM domain_auth_magic_link WHERE domain_id = $1")
        .bind(domain_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    sqlx::query("DELETE FROM domain_auth_config WHERE domain_id = $1")
        .bind(domain_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    // 10. Delete end users (this will SET NULL owner_end_user_id on domains)
    sqlx::query("DELETE FROM domain_end_users WHERE domain_id = $1")
        .bind(domain_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    // 11. Finally delete the domain itself
    sqlx::query("DELETE FROM domains WHERE id = $1")
        .bind(domain_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    tx.commit().await.map_err(AppError::from)?;

    Ok(())
}
```

### Alternative: Simpler Approach

Since all `domain_id` FKs have CASCADE, we could simplify to:

```rust
async fn delete(&self, domain_id: Uuid) -> AppResult<()> {
    let mut tx = self.pool.begin().await.map_err(AppError::from)?;

    sqlx::query("DELETE FROM domains WHERE id = $1")
        .bind(domain_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    tx.commit().await.map_err(AppError::from)?;

    Ok(())
}
```

This still provides:
- Atomicity (all-or-nothing)
- Isolation (other transactions see consistent state)

The CASCADE constraints handle the actual related data deletion.

**Recommendation:** Use the simpler approach since CASCADE constraints are already properly configured. The transaction wrapper provides the atomicity guarantee requested in the task.

### Step 4: Add Logging/Tracing

Add tracing to track the deletion:

```rust
use tracing::instrument;

#[instrument(skip(self), fields(domain_id = %domain_id))]
async fn delete(&self, domain_id: Uuid) -> AppResult<()> {
    tracing::info!("Starting transactional domain deletion");

    let mut tx = self.pool.begin().await.map_err(AppError::from)?;

    let result = sqlx::query("DELETE FROM domains WHERE id = $1")
        .bind(domain_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    if result.rows_affected() == 0 {
        tracing::warn!("Domain not found for deletion");
        // Transaction auto-rolls back on drop
        return Err(AppError::NotFound);
    }

    tx.commit().await.map_err(AppError::from)?;

    tracing::info!("Domain deletion completed successfully");
    Ok(())
}
```

## Files to Modify

1. **`apps/api/src/adapters/persistence/domain.rs`** (lines 155-162)
   - Wrap delete in transaction
   - Add rows_affected check
   - Add tracing

## Testing Approach

### Unit Test (Mock-based)
Add a test to `apps/api/src/application/use_cases/domain.rs` in the `mod tests` section:

```rust
#[tokio::test]
async fn test_delete_domain_calls_repo() {
    // Existing pattern using mock repos
}
```

### Integration Test Approach
Since the codebase doesn't have DB integration tests set up, document the manual test approach:

1. Start local infra: `./run infra`
2. Seed test data: `./run dev:seed`
3. Create a test domain via API
4. Add related data (end users, auth config, billing)
5. Delete the domain
6. Verify all related tables are empty

### Database Verification Query

After deletion, run:
```sql
SELECT
    (SELECT COUNT(*) FROM domain_end_users WHERE domain_id = '<domain_id>') as end_users,
    (SELECT COUNT(*) FROM domain_auth_config WHERE domain_id = '<domain_id>') as auth_config,
    (SELECT COUNT(*) FROM domain_auth_magic_link WHERE domain_id = '<domain_id>') as magic_link,
    (SELECT COUNT(*) FROM domain_auth_google_oauth WHERE domain_id = '<domain_id>') as google_oauth,
    (SELECT COUNT(*) FROM domain_api_keys WHERE domain_id = '<domain_id>') as api_keys,
    (SELECT COUNT(*) FROM domain_roles WHERE domain_id = '<domain_id>') as roles,
    (SELECT COUNT(*) FROM domain_billing_stripe_config WHERE domain_id = '<domain_id>') as billing_config,
    (SELECT COUNT(*) FROM subscription_plans WHERE domain_id = '<domain_id>') as plans,
    (SELECT COUNT(*) FROM user_subscriptions WHERE domain_id = '<domain_id>') as subscriptions,
    (SELECT COUNT(*) FROM billing_payments WHERE domain_id = '<domain_id>') as payments,
    (SELECT COUNT(*) FROM domain_enabled_payment_providers WHERE domain_id = '<domain_id>') as providers;
```

All counts should be 0.

## Edge Cases to Handle

1. **Domain doesn't exist**: Return NotFound error (already handled in use case layer)
2. **System domain (no owner)**: Already blocked in use case layer (`domain.rs:162-166`)
3. **Concurrent deletion**: Transaction isolation prevents double-delete issues
4. **Related data modified during delete**: Transaction provides snapshot isolation
5. **Large number of related records**: CASCADE is efficient, but very large domains may take time

## Rollback Strategy

If the transaction fails at any point, PostgreSQL automatically rolls back all changes. No partial state is possible.

## Migration Consideration

No migration needed - the schema already has correct CASCADE constraints. We're just adding a transaction wrapper to the application code.

## Pre-Implementation Verification

Before implementing, verify the build succeeds:
```bash
./run api:build
```

## Estimated Changes

- ~20 lines modified in `apps/api/src/adapters/persistence/domain.rs`
- Optionally ~10-20 lines for a unit test

## Open Questions

1. Should we log the count of deleted related records for auditing?
2. Should we emit an event/webhook when a domain is deleted?
3. Should we add a soft-delete option for recovery purposes?

These are out of scope for this task but worth considering for future work.

---

## Codex Review Feedback (2026-01-01)

Codex provided the following feedback on the initial plan:

### Findings (by severity)

1. **Medium**: Wrapping a single `DELETE ... CASCADE` in an explicit transaction doesn't add atomicity - the statement is already atomic and cascades are part of the same statement. The change may not achieve the stated goal as-is.

2. **Medium**: On `rows_affected() == 0`, the transaction is left uncommitted. While dropping tx should rollback, it's implicit. Consider explicit `tx.rollback().await` for clarity and to release the connection promptly.

3. **Low**: The transaction wrapper adds overhead without material benefit unless we add more statements (e.g., audit logging, outbox pattern).

### Answers to Review Questions

1. **Is CASCADE + transaction sufficient?** Yes, cascades + single delete is already sufficient for atomicity. The transaction wrapper doesn't change behavior unless we add more statements.

2. **Edge cases missed?** Implicit rollback on early return; idempotency semantics; future side effects outside DB won't be covered.

3. **Should we add explicit deletes?** No need if cascades are correct. Keep relying on FKs unless custom ordering or partial cleanup is needed.

### Plan Revision Based on Feedback

Given that:
- Single DELETE with CASCADE is already atomic
- Transaction wrapper adds minimal value without additional operations
- The task explicitly requests "transactional" deletion

**Revised Decision**: Keep the transaction wrapper but:
1. Add explicit rollback on NotFound for connection hygiene
2. Add `DELETE ... RETURNING id` pattern for clarity
3. Future-proof for audit logging / outbox pattern if needed

### Updated Implementation

```rust
async fn delete(&self, domain_id: Uuid) -> AppResult<()> {
    let mut tx = self.pool.begin().await.map_err(AppError::from)?;

    // Use RETURNING to verify deletion in one round-trip
    let row: Option<(Uuid,)> = sqlx::query_as("DELETE FROM domains WHERE id = $1 RETURNING id")
        .bind(domain_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(AppError::from)?;

    if row.is_none() {
        // Explicit rollback for connection hygiene
        tx.rollback().await.map_err(AppError::from)?;
        return Err(AppError::NotFound);
    }

    tx.commit().await.map_err(AppError::from)?;
    Ok(())
}
```

This approach:
- Uses `RETURNING` to check existence without separate query
- Explicitly rolls back on NotFound
- Provides transaction wrapper for future extensibility (audit, outbox)
- Is cleaner than rows_affected() check
