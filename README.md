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

## Local Development

### One-time setup

```bash
# 1. Install mkcert
brew install mkcert    # macOS
# or: sudo apt install mkcert  # Linux

# 2. Generate local TLS certificates
./run certs

# 3. Add hosts entries
echo "127.0.0.1 reauth.test reauth.reauth.test ingress.reauth.test" | sudo tee -a /etc/hosts

# 4. Copy example env files
cp apps/api/.env.example apps/api/.env
# Edit apps/api/.env to add your RESEND_API_KEY and EMAIL_FROM
```

### Fresh start

```bash
# 1. Nuke and restart containers
./run infra:stop
./run infra:full

# 2. Apply migrations and seed
./run db:migrate
./run dev:seed

# 3. Start API (terminal 1)
./run api

# 4. Start UI (terminal 2)
./run ui

# 5. Access https://reauth.test/
```

### What the seed does

`./run dev:seed` renames the `reauth.dev` domain to `reauth.test` in the database.
This keeps the owner and all config from migrations, just changes the domain name for local dev.

### Available Commands

Run `./run help` to see all commands:

| Command | Description |
|---------|-------------|
| `infra:full` | Start postgres, redis, coredns, caddy |
| `infra:stop` | Stop all containers |
| `db:migrate` | Run database migrations |
| `dev:seed` | Rename reauth.dev → reauth.test for local dev |
| `api` | Run the API locally |
| `ui` | Run the UI dev server |
| `certs` | Generate local TLS certificates |

## License

MIT
