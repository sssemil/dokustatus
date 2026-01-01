# Plan: ES256 Per-Domain JWT Signing with JWKS Endpoint

## Goal
Switch from HS256 (shared secret) to ES256 (asymmetric) JWT signing with per-domain keypairs, enabling developer backends to verify user tokens locally without calling reauth.

## Key Decisions
- **Algorithm**: ES256 (ECDSA P-256)
- **Scope**: Per-domain keypairs (not global)
- **Backward compatibility**: None needed - clean switch
- **Endpoints**: Replace `verify-token` with JWKS + new `user-status` endpoint

## Scope Clarification
- **CHANGES**: Domain end-user JWTs (`DomainEndUserClaims`) - used for authenticating users on customer domains
- **UNCHANGED**: Workspace user JWTs (`Claims`) - used for reauth.dev dashboard authentication, stays HS256 with global secret

---

## Implementation Phases

### Phase 1: Database Schema

**New file**: `apps/api/migrations/00008_domain_jwt_keypairs.sql`

```sql
CREATE TABLE domain_jwt_keypairs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    kid VARCHAR(64) NOT NULL UNIQUE,             -- Key ID for JWKS (globally unique)
    private_key_encrypted TEXT NOT NULL,         -- PKCS#8 PEM, encrypted
    public_key_pem TEXT NOT NULL,                -- SPKI PEM, plaintext
    curve VARCHAR(20) NOT NULL DEFAULT 'P-256',
    algorithm VARCHAR(10) NOT NULL DEFAULT 'ES256',
    is_current BOOLEAN NOT NULL DEFAULT TRUE,    -- Only one current per domain (for signing)
    revoked_at TIMESTAMP NULL,                   -- NULL = active, set = revoked
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Index for fast lookup of current signing key
CREATE INDEX idx_domain_jwt_keypairs_current ON domain_jwt_keypairs(domain_id) WHERE is_current = TRUE AND revoked_at IS NULL;

-- Index for JWKS (all non-revoked keys for a domain)
CREATE INDEX idx_domain_jwt_keypairs_active ON domain_jwt_keypairs(domain_id) WHERE revoked_at IS NULL;
```

---

### Phase 2: Rust Infrastructure

**New files**:
1. `apps/api/src/domain/entities/domain_jwt_keypair.rs` - Entity struct
2. `apps/api/src/adapters/persistence/domain_jwt_keypair.rs` - Repository
3. `apps/api/src/infra/ec_keypair.rs` - Key generation + PEM-to-JWK conversion

**Cargo.toml addition**:
```toml
p256 = { version = "0.13", features = ["ecdsa", "pem", "pkcs8"] }
```

**Key functions in `ec_keypair.rs`**:
- `generate_es256_keypair() -> (private_pem, public_pem)`
- `pem_to_jwk_coordinates(public_pem) -> (x, y)` for JWKS response

---

### Phase 3: JWT Module Changes

**Modify**: `apps/api/src/application/jwt.rs`

Add ES256 signing/verification:
```rust
pub fn issue_domain_end_user_es256(
    end_user_id, domain_id, domain, roles, subscription,
    private_key_pem: &str,  // Decrypted PEM
    kid: &str,              // Key ID for header
    ttl: Duration,
) -> AppResult<String>

pub fn verify_domain_end_user_es256(
    token: &str,
    public_key_pem: &str,
) -> AppResult<DomainEndUserClaims>
```

Remove old HS256 `issue_domain_end_user()` and `verify_domain_end_user()`.

---

### Phase 4: Domain Creation - Generate Keypair

**Modify**: `apps/api/src/application/use_cases/domain.rs`

Add new dependencies to `DomainUseCases`:
```rust
pub struct DomainUseCases {
    repo: Arc<dyn DomainRepo>,
    keypair_repo: Arc<dyn DomainJwtKeypairRepo>,  // NEW
    dns_verifier: Arc<dyn DnsVerifier>,
    cipher: ProcessCipher,  // NEW - for encrypting private keys
    ingress_domain: String,
}
```

In `add_domain()`:
1. Create domain (existing)
2. Generate ES256 keypair using `ec_keypair::generate_es256_keypair()`
3. Encrypt private key with `cipher.encrypt(&private_pem)`
4. Store keypair via `keypair_repo.create(...)`

**Also modify**: `apps/api/src/infra/setup.rs`
- Wire `keypair_repo` and `cipher` into `DomainUseCases`

---

### Phase 5: Update Token Issuance & Verification

**Modify**: `apps/api/src/application/use_cases/domain_auth.rs`

Add method to access keypairs:
```rust
pub async fn get_keypair_for_domain(&self, domain: &str) -> AppResult<KeypairForSigning> {
    // Returns decrypted private key + kid for signing
}
```

**Modify**: `apps/api/src/adapters/http/routes/public_domain_auth.rs`

**Token Issuance** (`complete_login()`, `refresh_token()`):
1. Fetch domain's keypair via `domain_auth_use_cases.get_keypair_for_domain()`
2. Decrypt private key (done inside use case)
3. Call `jwt::issue_domain_end_user_es256()` with private key + kid

**Token Verification** (`check_session()`, `get_current_user()`):
1. Fetch domain's public key via `domain_auth_use_cases.get_public_key_for_domain()`
2. Call `jwt::verify_domain_end_user_es256()` with public key
3. **Note**: This adds a DB lookup per session check - consider in-memory caching for hot paths

---

### Phase 6: JWKS Endpoint

**Add route**: `GET /api/public/domain/{domain}/.well-known/jwks.json`

**Location**: `apps/api/src/adapters/http/routes/public_domain_auth.rs`

Response format:
```json
{
  "keys": [{
    "kty": "EC",
    "crv": "P-256",
    "alg": "ES256",
    "use": "sig",
    "kid": "uuid-here",
    "x": "base64url-x-coordinate",
    "y": "base64url-y-coordinate"
  }]
}
```

---

### Phase 7: New User Status Endpoint

**Replace** `POST /api/developer/{domain}/auth/verify-token`
**With** `GET /api/developer/{domain}/users/{user_id}/status`

**Location**: `apps/api/src/adapters/http/routes/developer.rs`

Response:
```json
{
  "id": "uuid",
  "is_frozen": false,
  "is_whitelisted": true,
  "roles": ["user"],
  "subscription_status": "active"
}
```

---

### Phase 8: SDK Changes

**Modify**: `libs/reauth-sdk-ts/src/server.ts`

Add `jose` dependency for local JWT verification:
```typescript
async verifyToken(token: string): Promise<JwtClaims | null> {
  const jwks = await getJwks();  // Cached
  const JWKS = jose.createLocalJWKSet({ keys: jwks });
  const { payload } = await jose.jwtVerify(token, JWKS, {
    algorithms: ['ES256'],
  });
  return payload as JwtClaims;
}

async getUserStatus(userId: string): Promise<UserStatus | null> {
  // Calls new endpoint, requires API key
}
```

**package.json**: Add `jose` ^5.0.0

---

### Phase 9: Lazy Keypair Generation for Existing Domains

**No separate migration script needed.** Use lazy generation with atomic upsert:

In `get_keypair_for_domain()`:
```rust
pub async fn get_keypair_for_domain(&self, domain_id: Uuid) -> AppResult<KeypairForSigning> {
    // Try to get existing current keypair first
    if let Some(keypair) = self.keypair_repo.get_current_for_domain(domain_id).await? {
        return Ok(decrypt_and_return(keypair));
    }

    // Lazy generation with race-safe transaction
    let (private_pem, public_pem) = ec_keypair::generate_es256_keypair()?;
    let kid = Uuid::new_v4().to_string();
    let encrypted = self.cipher.encrypt(&private_pem)?;

    let keypair = self.keypair_repo.create_if_no_current(domain_id, &kid, &encrypted, &public_pem).await?;
    Ok(decrypt_and_return(keypair))
}
```

**Repository implementation:**
```rust
// Get current signing key for a domain
pub async fn get_current_for_domain(&self, domain_id: Uuid) -> AppResult<Option<DomainJwtKeypair>> {
    sqlx::query_as!(DomainJwtKeypair,
        r#"SELECT * FROM domain_jwt_keypairs
           WHERE domain_id = $1 AND is_current = TRUE AND revoked_at IS NULL"#,
        domain_id
    ).fetch_optional(&self.pool).await.map_err(...)
}

// Get all active keys for JWKS
pub async fn get_all_active_for_domain(&self, domain_id: Uuid) -> AppResult<Vec<DomainJwtKeypair>> {
    sqlx::query_as!(DomainJwtKeypair,
        r#"SELECT * FROM domain_jwt_keypairs
           WHERE domain_id = $1 AND revoked_at IS NULL
           ORDER BY created_at DESC"#,
        domain_id
    ).fetch_all(&self.pool).await.map_err(...)
}

// Create new key (race-safe with transaction)
pub async fn create_if_no_current(&self, domain_id: Uuid, kid: &str, private_encrypted: &str, public_pem: &str) -> AppResult<DomainJwtKeypair> {
    // Transaction: check if current exists, if not insert
    let mut tx = self.pool.begin().await?;

    let existing = sqlx::query_scalar!(
        "SELECT id FROM domain_jwt_keypairs WHERE domain_id = $1 AND is_current = TRUE AND revoked_at IS NULL FOR UPDATE",
        domain_id
    ).fetch_optional(&mut *tx).await?;

    if existing.is_some() {
        tx.rollback().await?;
        return self.get_current_for_domain(domain_id).await?.ok_or(AppError::Internal("...".into()));
    }

    let keypair = sqlx::query_as!(DomainJwtKeypair,
        r#"INSERT INTO domain_jwt_keypairs (domain_id, kid, private_key_encrypted, public_key_pem, is_current)
           VALUES ($1, $2, $3, $4, TRUE) RETURNING *"#,
        domain_id, kid, private_encrypted, public_pem
    ).fetch_one(&mut *tx).await?;

    tx.commit().await?;
    Ok(keypair)
}
```

**Benefits**:
- Race-safe: Transaction with `SELECT ... FOR UPDATE` handles concurrent requests
- SQL migration just creates empty table
- Keys generated on-demand when first needed
- Zero-downtime deploy
- Key rotation supported from day 1

---

## Files Summary

### New Files
| File | Purpose |
|------|---------|
| `apps/api/migrations/00008_domain_jwt_keypairs.sql` | Schema |
| `apps/api/src/domain/entities/domain_jwt_keypair.rs` | Entity |
| `apps/api/src/adapters/persistence/domain_jwt_keypair.rs` | Repository |
| `apps/api/src/infra/ec_keypair.rs` | Key generation |

### Modified Files
| File | Changes |
|------|---------|
| `apps/api/Cargo.toml` | Add `p256` crate |
| `apps/api/src/domain/entities/mod.rs` | Export `domain_jwt_keypair` entity |
| `apps/api/src/adapters/persistence/mod.rs` | Export keypair repo |
| `apps/api/src/infra/mod.rs` | Export `ec_keypair` module |
| `apps/api/src/application/jwt.rs` | Add ES256 functions, keep HS256 for workspace only |
| `apps/api/src/application/use_cases/domain.rs` | Add keypair_repo + cipher, gen keypair on domain create |
| `apps/api/src/application/use_cases/domain_auth.rs` | Add `get_keypair_for_domain()`, `get_public_key_for_domain()` |
| `apps/api/src/infra/setup.rs` | Wire keypair_repo + cipher into DomainUseCases |
| `apps/api/src/adapters/http/routes/public_domain_auth.rs` | JWKS endpoint, ES256 signing/verification, TTL 86400→900 |
| `apps/api/src/adapters/http/routes/developer.rs` | Add user-status endpoint, remove verify-token |
| `libs/reauth-sdk-ts/src/server.ts` | Local JWKS verification, add getUserStatus() |
| `libs/reauth-sdk-ts/src/types.ts` | Add JwtClaims, UserStatus types |
| `libs/reauth-sdk-ts/package.json` | Add `jose` ^5.0.0 |

---

## Security Notes
- Private keys encrypted with ProcessCipher (AES-256-GCM with unique nonces per encryption)
- Public keys in JWKS are... public (safe to expose)
- `kid` in JWT header allows future key rotation
- API key still required for user-status endpoint (real-time checks)
- **Strict algorithm enforcement**: Reject `alg=none` and `alg=HS256` in both server and SDK
- **Require kid**: Reject tokens without `kid` or with unknown `kid`
- **JWS signature format**: `jsonwebtoken` crate uses JWS-compliant R||S format for ES256 (not DER)
- **Both access AND refresh tokens**: Both use ES256 with same per-domain keypair

---

## Additional Considerations (from Codex review)

### Key Rotation Strategy (Built-in from Day 1)

**Schema supports rotation out of the box:**
- Multiple keys per domain (no UNIQUE on domain_id)
- `is_current` flag marks signing key
- `revoked_at` marks revoked keys (excluded from JWKS)

**Rotation flow:**
```rust
pub async fn rotate_key(&self, domain_id: Uuid) -> AppResult<DomainJwtKeypair> {
    let mut tx = self.pool.begin().await?;

    // 1. Mark old key as not current (but keep active for verification)
    sqlx::query!(
        "UPDATE domain_jwt_keypairs SET is_current = FALSE WHERE domain_id = $1 AND is_current = TRUE",
        domain_id
    ).execute(&mut *tx).await?;

    // 2. Create new current key
    let (private_pem, public_pem) = ec_keypair::generate_es256_keypair()?;
    let kid = Uuid::new_v4().to_string();
    let encrypted = self.cipher.encrypt(&private_pem)?;

    let new_key = sqlx::query_as!(DomainJwtKeypair,
        r#"INSERT INTO domain_jwt_keypairs (domain_id, kid, private_key_encrypted, public_key_pem, is_current)
           VALUES ($1, $2, $3, $4, TRUE) RETURNING *"#,
        domain_id, &kid, &encrypted, &public_pem
    ).fetch_one(&mut *tx).await?;

    tx.commit().await?;
    Ok(new_key)
}
```

**Key lifecycle:**
1. `is_current = TRUE, revoked_at = NULL` → Used for signing, in JWKS
2. `is_current = FALSE, revoked_at = NULL` → Not for signing, still in JWKS (old tokens valid)
3. `revoked_at = <timestamp>` → Removed from JWKS, tokens fail verification

**Cleanup old keys** (run periodically):
```sql
-- Remove keys that have been non-current for > 30 days (max refresh token TTL)
DELETE FROM domain_jwt_keypairs
WHERE is_current = FALSE
  AND revoked_at IS NULL
  AND created_at < NOW() - INTERVAL '31 days';
```

**Emergency revocation:**
```rust
pub async fn revoke_key(&self, kid: &str) -> AppResult<()> {
    sqlx::query!(
        "UPDATE domain_jwt_keypairs SET revoked_at = NOW() WHERE kid = $1",
        kid
    ).execute(&self.pool).await?;
    Ok(())
}
```

**JWKS returns all active keys:**
```rust
pub async fn get_jwks(&self, domain_id: Uuid) -> AppResult<Vec<JwkPublicKey>> {
    let keys = self.keypair_repo.get_all_active_for_domain(domain_id).await?;
    keys.into_iter().map(|k| to_jwk(&k)).collect()
}
```

### Migration Strategy
- **Don't block startup** with key generation
- Run migration as explicit CLI command or background job
- Add `has_jwt_keypair` check before signing; fail gracefully if missing
- Log domains without keys for monitoring

### JWKS Response Headers
```
Cache-Control: public, max-age=3600
```
- 1 hour cache is reasonable; SDK can cache longer internally
- Consider `ETag` for conditional requests

### Domain Deletion
- `ON DELETE CASCADE` handles key cleanup automatically
- Keys immediately stop working (removed from JWKS)

### Algorithm Downgrade Protection
Server verification:
```rust
let mut validation = Validation::new(Algorithm::ES256);
validation.algorithms = vec![Algorithm::ES256]; // Only ES256
validation.leeway = 60; // 60 seconds clock skew tolerance
```

SDK verification:
```typescript
await jose.jwtVerify(token, JWKS, {
  algorithms: ['ES256'],  // Reject all others
  clockTolerance: 60,     // 60 seconds clock skew
});
```

### JWKS Edge Cases

**Empty JWKS (domain has no keypair yet):**
- Return empty `{ "keys": [] }` with `Cache-Control: no-store`
- SDK should retry on next request (lazy generation will create key)
- Don't return 404 (confusing for clients)

**JWKS caching strategy:**
```
# When keys exist
Cache-Control: public, max-age=3600

# When no keys (empty JWKS)
Cache-Control: no-store
```

### Migration Strategy for Existing HS256 Tokens

**User decision**: No backward compatibility needed (clean switch).

**Implication**: All existing sessions will be invalidated on deploy.
- Users will need to re-login after deploy
- This is acceptable per user's decision
- Access tokens (24h max) will fail immediately
- Refresh tokens (30 days) will fail on next refresh attempt

**Alternative (if needed later)**: Dual-verify window not implemented initially.

---

## Implementation Order

Execute in this order due to dependencies:

1. **Migration + Entity + Repo** (Phase 1, 2) - foundation
2. **EC Keypair module** (Phase 2) - key generation utilities
3. **JWT module changes** (Phase 3) - ES256 signing/verification
4. **DomainUseCases changes** (Phase 4) - keypair generation on domain create
5. **DomainAuthUseCases + public_domain_auth.rs** (Phase 5) - token issuance/verification
6. **JWKS endpoint** (Phase 6) - public key distribution
7. **Developer routes** (Phase 7) - user-status endpoint
8. **SDK changes** (Phase 8) - local verification
9. **Migration script** (Phase 9) - backfill existing domains

**Deploy strategy**: Just deploy - keypairs generated lazily on first login per domain.

---

## Config Change

**Reduce default access token TTL to 15 minutes** (from 24 hours):

**File**: `apps/api/src/infra/config.rs` or domain auth config defaults

```rust
// Before
let access_ttl_secs = config.access_token_ttl_secs.unwrap_or(86400); // 24h

// After
let access_ttl_secs = config.access_token_ttl_secs.unwrap_or(900); // 15 min
```

**Rationale**: With local JWT verification, frozen status isn't checked in real-time. Shorter TTL means:
- Frozen users lose access within 15 minutes
- Subscription changes reflected within 15 minutes
- More frequent refresh token usage (still 30 days)

---

## Open Questions (Resolved)

| Question | Decision |
|----------|----------|
| Per-domain or global keypair? | Per-domain |
| Algorithm? | ES256 |
| Replace or coexist with verify-token? | Replace, add user-status |
| Backward compatibility? | None needed |
| Migration for existing domains? | Lazy generation on first login |
| Access token TTL? | 15 minutes (down from 24h) |
