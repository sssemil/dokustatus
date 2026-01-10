Use payment provider port in billing use cases
Stop directly instantiating StripeClient in DomainBillingUseCases.

Checklist
- [x] Identify direct StripeClient usage
- [x] Route operations through provider factory/port
- [x] Update tests or mocks

History
- 2026-01-01 06:52 Created from code review finding #12 Leaky abstraction in billing use cases.
- 2026-01-01 06:55 Renamed file to 0012-payment-provider-abstraction.md to use 4-digit task numbering.
- 2026-01-01 07:30 Created plan-v1.md with detailed implementation approach.
- 2026-01-01 08:15 Created plan-v2.md addressing feedback: verified all type signatures, resolved idempotency key concern, added StripeMode→PaymentMode conversion step.
- 2026-01-01 09:30 Created plan-v3.md (final revision) addressing feedback-2.md: verified validation helpers, CreateSubscriptionEventInput, PlanChangeType::as_str(), and factory constructor; confirmed existing imports; decided to remove HTTP idempotency_key extraction; documented idempotency regression risk.
- 2026-01-01 10:30 Implemented plan-v3.md changes:
  - Added From<StripeMode> for PaymentMode trait impl in payment_mode.rs
  - Injected PaymentProviderFactory into DomainBillingUseCases struct and constructor
  - Added get_active_provider() helper to select provider based on domain mode and enabled providers
  - Added plan_to_port_info() helper to convert SubscriptionPlanProfile to PlanInfo
  - Refactored preview_plan_change() to use PaymentProviderPort instead of direct StripeClient
  - Refactored change_plan() to use PaymentProviderPort, removed idempotency_key parameter
  - Updated HTTP handler to remove idempotency_key extraction
  - Updated setup.rs to create PaymentProviderFactory and inject it
  - Build passes (./run api:build), all 63 tests pass (./run api:test)
- 2026-01-01 10:35 Task complete. Ready for merge.
