# Derive JWT Secret from Developer API Key for Self-Verification

Enable customer backends to verify user session JWTs locally without calling the reauth backend, by deriving the JWT signing secret deterministically from the developer API key.

## Problem

Currently, when a customer's backend receives a user session JWT, it must call the reauth API to verify the token. This adds latency and creates a dependency on reauth availability for every authenticated request.

## Solution

Derive the JWT signing secret deterministically from the developer API key using a KDF (e.g., HKDF). This allows:
- Customer backend has the API key
- Customer backend can derive the same JWT secret
- Customer backend can verify JWTs locally without network calls

## Checklist

- [ ] Design the key derivation scheme (HKDF with domain context)
- [ ] Update JWT signing to use derived secret per-domain
- [ ] Document the derivation algorithm for SDK implementers
- [ ] Update TypeScript SDK with local JWT verification
- [ ] Add tests for key derivation consistency
- [ ] Update documentation for self-verification flow

## Security Considerations

- The API key must remain secret (already required)
- KDF must use a fixed, documented context string to prevent key confusion
- Consider key rotation implications

## History

- 2026-01-25 Created task for JWT secret derivation from developer API key.
