# dokustatus

Quick usage

Prerequisites
- Docker + Docker Compose
- Rust (cargo) and sqlx-cli installed

Steps
1) Setup environment
   cp .env.example .env
   # Set RESEND_API_KEY, EMAIL_FROM, APP_ORIGIN, JWT_SECRET, DB and REDIS settings as needed

2) Start Postgres + Redis
   docker compose up -d postgres redis

3) Create and migrate database
   export DATABASE_URL=postgres://user:password@localhost:5432/clean_architecture
   sqlx database create
   sqlx migrate run

4) Prepare SQLx offline data (optional but recommended)
   cargo sqlx prepare -- --bin dokustatus

5) Run the backend
   cargo run

API
- Request magic link:
  POST http://localhost:3001/api/auth/request
  Body: { "email": "you@example.com" }

```
curl -X POST \
  http://localhost:3001/api/auth/request \
  -H 'Content-Type: application/json' \
  -d '{"email":"you@example.com"}' -i
```

- Consume magic link (token from email):
  POST http://localhost:3001/api/auth/consume
  Body: { "token": "<received_token>" }
  -> Sets HttpOnly cookies: access_token, refresh_token

```
curl -X POST \
  http://localhost:3001/api/auth/consume \
  -H 'Content-Type: application/json' \
  -d '{"token":"<received_token>"}' -i
```
