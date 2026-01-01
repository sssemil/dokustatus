Split oversized public_domain_auth routes
Break `public_domain_auth.rs` into focused modules.

Checklist
- [x] Identify logical route groupings
- [x] Split into modules and update mod wiring
- [x] Verify routes compile and tests pass

History
- 2026-01-01 06:52 Created from code review finding #18 Oversized public_domain_auth.rs file.
- 2026-01-01 06:55 Renamed file to 0018-split-public-domain-auth-routes.md to use 4-digit task numbering.
- 2026-01-01 07:15 Created detailed implementation plan (plan-v1.md). Identified 11 phases to split 3,293-line file into 10 focused modules: common, config, magic_link, session, google_oauth, billing, billing_payments, billing_webhooks, billing_dummy, and mod.rs.
- 2026-01-01 07:45 Created plan-v2.md addressing feedback: added import management strategy, fixed route count (11 not 9), added clear_auth_cookies() helper, defined visibility strategy, merged billing modules (8 modules total), added lint/fmt verification steps.
- 2026-01-01 08:30 Created plan-v3.md (final revision) addressing v2 feedback: fixed import table (billing_webhooks uses StripeMode not PaymentMode), added missing imports (SubscriptionStatus, CreateSubscriptionInput), moved shared imports (StripeMode, tracing::error) to common.rs, clarified route counts and test module import strategy.
- 2026-01-01 15:10 Implemented plan-v3: Split public_domain_auth.rs (3,293 lines) into 8 focused modules in public_domain_auth/ directory. Created: common.rs (shared helpers, types, cookie utilities), config.rs (1 route), magic_link.rs (2 routes), session.rs (4 routes), google_oauth.rs (5 routes), billing.rs (9 routes), billing_webhooks.rs (2 routes), billing_dummy.rs (3 routes), mod.rs (router combiner). Added clear_auth_cookies() helper to consolidate duplicate cookie clearing logic. All 93 tests pass. Formatting applied.
