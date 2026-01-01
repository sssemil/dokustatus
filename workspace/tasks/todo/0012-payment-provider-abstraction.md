Use payment provider port in billing use cases
Stop directly instantiating StripeClient in DomainBillingUseCases.

Checklist
- [ ] Identify direct StripeClient usage
- [ ] Route operations through provider factory/port
- [ ] Update tests or mocks

History
- 2026-01-01 06:52 Created from code review finding #12 Leaky abstraction in billing use cases.
- 2026-01-01 06:55 Renamed file to 0012-payment-provider-abstraction.md to use 4-digit task numbering.
