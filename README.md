# reauth.dev

> **Auth, billing, email. One DNS setup.**

Unified user infrastructure for indie SaaS developers. Instead of wiring together Clerk + Stripe + Resend, developers add a few DNS records and get everything working together out of the box.

## Quick Start (The Vision)

```javascript
import { getUser } from 'reauth'

const user = await getUser(request)
// {
//   id: 'usr_abc123',
//   email: 'user@example.com',
//   plan: 'pro',
//   subscriptionStatus: 'active',
//   ...
// }
```

## Tech Stack

- **Language:** Rust
- **Framework:** Axum
- **Database:** PostgreSQL
- **Cache:** Redis
- **Email:** Resend (customer's API key)

## Project Structure

```
reauth/
├── apps/
│   ├── api/          # Rust backend (Axum + SQLx)
│   └── ui/           # Next.js frontend
├── docs/
│   └── vision/       # Product vision & architecture docs
└── infra/            # Docker deployment infrastructure
```

## Documentation

See `/docs/vision/` for detailed product documentation including:
- Product overview and target market
- Architecture and data model
- SDK integration patterns
- Hosted UI specifications
- Development phases and timeline

## Development

```bash
# Start infrastructure
docker compose up -d

# Run API (from apps/api)
cargo run

# Run UI (from apps/ui)
npm run dev
```

## License

MIT
