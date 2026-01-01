# Plan v2: Transactional Domain Delete

## Summary

This plan addresses feedback on plan-v1 by clarifying the call chain, idempotency semantics, and SQLx offline requirements.

### Goal
Wrap domain deletion in an explicit transaction to:
1. Provide an atomic boundary for future extensibility (audit logging, outbox events)
2. Satisfy the task requirement for "transactional" deletion
3. Maintain current idempotent behavior (delete of non-existent domain already fails at use-case layer)

## Feedback Analysis

| Feedback Item | Resolution |
|---------------|------------|
| Use-case layer already checks existence | Confirmed: `delete_domain` calls `get_domain()` first, which returns `NotFound` if domain doesn't exist. Repo-level `NotFound` won't change behavior. |
| Call chain unclear | Documented below: route → use case → repo |
| SQLx offline data / `cargo sqlx prepare` | No impact: we're using `sqlx::query()` (unchecked), not `sqlx::query!()`. No offline data regeneration needed. |
| Logging consistency | Persistence layer has no `#[instrument]` macros currently. The use-case layer already has `#[instrument(skip(self))]` on `delete_domain`. Will not add tracing to persistence layer. |
| NotFound from repo could change idempotency | Won't happen: use-case layer handles NotFound before calling repo. Current repo ignores rows_affected, which is idempotent. We'll keep this behavior. |

## Call Chain Analysis

```
DELETE /api/domains/{domain_id}
    │
    ▼
apps/api/src/adapters/http/routes/domain.rs:376 delete_domain()
    │ calls
    ▼
apps/api/src/application/use_cases/domain.rs:158 DomainUseCases::delete_domain()
    │ 1. calls get_domain() → returns NotFound if domain doesn't exist
    │ 2. checks owner_end_user_id is Some (blocks system domain deletion)
    │ 3. calls repo.delete()
    ▼
apps/api/src/adapters/persistence/domain.rs:155 impl DomainRepo::delete()
    │ executes DELETE FROM domains WHERE id = $1
    ▼
PostgreSQL cascades to related tables (all have ON DELETE CASCADE on domain_id)
```

**Key insight**: The use-case layer already validates existence and ownership. The repo layer performs a "fire and forget" delete that succeeds even if 0 rows are affected. This is the desired idempotent behavior and should be preserved.

## Current vs Proposed Behavior

| Scenario | Current Behavior | Proposed Behavior |
|----------|------------------|-------------------|
| Domain exists | Delete succeeds, cascades, returns 204 | Same (wrapped in transaction) |
| Domain doesn't exist | Use-case returns NotFound (404) | Same (no change) |
| Domain is system domain | Use-case returns InvalidInput (400) | Same (no change) |
| Concurrent deletion | First delete wins, second returns 404 | Same (no change) |
| Delete with many related records | Cascades atomically | Same (already atomic) |

## Implementation

### Minimal Change (Recommended)

**File:** `apps/api/src/adapters/persistence/domain.rs` lines 155-162

Replace:
```rust
async fn delete(&self, domain_id: Uuid) -> AppResult<()> {
    sqlx::query("DELETE FROM domains WHERE id = $1")
        .bind(domain_id)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;
    Ok(())
}
```

With:
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

### Why Not Use `DELETE ... RETURNING`?

The v1 plan suggested `DELETE ... RETURNING id` to check if deletion occurred. However:
1. The use-case layer already checks existence before calling delete
2. Changing to return `NotFound` on 0 rows would break the current idempotent semantics (delete of already-deleted domain silently succeeds at repo level)
3. Adding `RETURNING` provides no benefit since we don't use the returned value

### Why Not Add Explicit Deletes?

The v1 plan listed explicit deletes for all related tables. However:
1. All `domain_id` foreign keys have `ON DELETE CASCADE`
2. Explicit deletes are redundant and create maintenance burden (new tables could be missed)
3. PostgreSQL handles cascade ordering internally

### Why Keep the Transaction Wrapper?

While a single `DELETE` statement with CASCADE is already atomic, the transaction wrapper:
1. Satisfies the explicit task requirement for "transactional" deletion
2. Provides a future-proof structure for adding audit logging or outbox events
3. Has negligible performance overhead (single statement within transaction)

## Files to Modify

1. `apps/api/src/adapters/persistence/domain.rs` (lines 155-162) - Wrap delete in transaction

## Verification Steps

### Pre-Implementation
```bash
./run api:build  # Verify current build succeeds
```

### Post-Implementation
```bash
./run api:fmt    # Format code
./run api:lint   # Check lints
./run api:build  # Verify build succeeds
./run api:test   # Run existing tests
```

### Manual Integration Test (Optional)

1. Start local infra: `./run infra`
2. Seed data: `./run dev:seed`
3. Create a test domain via UI or API
4. Add related data (auth config, end users, etc.)
5. Delete the domain
6. Verify all related data is removed:

```sql
-- Replace <domain_id> with the deleted domain's UUID
SELECT
    (SELECT COUNT(*) FROM domains WHERE id = '<domain_id>') as domain,
    (SELECT COUNT(*) FROM domain_end_users WHERE domain_id = '<domain_id>') as end_users,
    (SELECT COUNT(*) FROM domain_auth_config WHERE domain_id = '<domain_id>') as auth_config,
    (SELECT COUNT(*) FROM domain_api_keys WHERE domain_id = '<domain_id>') as api_keys;
-- All counts should be 0
```

## Risk Assessment

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Transaction overhead | Low | Negligible for single statement |
| Breaking existing behavior | Very Low | No semantic changes; use-case layer unchanged |
| Missing new tables in future | N/A | Relying on CASCADE, not explicit deletes |
| Check/delete race condition | Very Low | See analysis below |

### Race Condition Analysis (from Codex review)

Codex noted that the existence check (`get_domain()`) happens outside the transaction, leaving a theoretical race where another request could delete the domain between check and delete.

**Assessment**: This is an acceptable tradeoff because:
1. The race window is extremely short (milliseconds)
2. The outcome is benign: delete of 0 rows succeeds silently (idempotent)
3. The use-case layer already checks ownership/existence for user feedback
4. Moving the check into the transaction would require changing `DomainRepo` trait and all implementations

**Alternative considered**: Using `DELETE ... RETURNING id` would make check+delete atomic, but would change error semantics (repo returns NotFound vs silent success). This is a behavioral change beyond the scope of "wrap in transaction."

**Recommendation**: Keep the current approach. The race is theoretical and the outcome is safe. If stricter atomicity is needed in the future, it can be addressed as a separate task.

## Out of Scope

These items were mentioned in v1 but are explicitly out of scope:
- Audit logging for domain deletion (future task)
- Soft-delete / recovery (future task)
- Deletion webhooks / events (future task)
- Adding `#[instrument]` to persistence layer (not consistent with current patterns)

## Estimated Changes

- ~8 lines modified in `apps/api/src/adapters/persistence/domain.rs`
- No new files
- No SQLx offline regeneration needed
- No migration needed

---

## Codex Review (2026-01-01)

**Reviewer**: gpt-5.2-codex

**Summary**: Plan is clean and minimal. Identified check/delete race condition as the main consideration.

**Findings**:
1. Transaction around single DELETE is mainly scaffolding for future work
2. Existence check outside transaction leaves small race window
3. `DELETE ... RETURNING` would make check+delete atomic
4. Error handling (begin/commit propagation) is sufficient

**Recommendations considered**:
- Move existence check into transaction: Rejected (requires trait changes, over-engineering)
- Use `DELETE ... RETURNING`: Rejected (changes error semantics)

**Resolution**: Documented race condition as acceptable risk. Current approach is safe due to idempotent delete semantics.
