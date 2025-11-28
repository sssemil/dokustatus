# Repository Guidelines

This project pairs a Rust backend (Axum + SQLx) with a Next.js UI. Follow the conventions below to stay consistent and productive.

## Project Structure & Module Organization
- `src/`: backend code split by clean architecture layers (`domain`, `application`, `adapters`, `infra`); entrypoints in `main.rs` and `lib.rs`.
- `migrations/`: SQLx migrations; keep new migrations ordered and idempotent.
- `ui/`: Next.js App Router frontend (`app/` pages, `globals.css`, config).
- `docker-compose.yml`: local Postgres. `.env.example` documents required settings.

## Build, Test, and Development Commands
- `docker compose up -d postgres`: start local DB.
- `sqlx migrate run`: apply migrations (uses `DATABASE_URL`).
- `cargo sqlx prepare -- --bin dokustatus`: refresh offline SQLx data when queries change.
- `cargo run`: start the API (bind addr from config).
- `cargo fmt` / `cargo clippy --all-targets --all-features`: format and lint Rust.
- `cargo test`: run backend tests (add `DATABASE_URL` pointing to a test DB if hitting the database).
- `npm install && npm run dev --prefix ui`: start the UI on port 3000; `npm run build --prefix ui` for production build.

## Coding Style & Naming Conventions
- Rust 2024 edition; always run `cargo fmt` before committing. Prefer small modules aligned to `domain/application/adapters/infra`.
- Naming: Rust modules `snake_case`; types and traits `PascalCase`; functions `snake_case`; constants `SCREAMING_SNAKE_CASE`.
- Error handling uses `anyhow` for main and typed errors in `application`; propagate via `?` and map to HTTP errors in adapters.
- Frontend: functional React components in `ui/app`, `PascalCase` component names, co-locate styles in `globals.css` or module styles.

## Testing Guidelines
- Add `#[cfg(test)]` modules near the logic they verify; prefer unit tests for use cases and lightweight integration tests for adapters.
- For DB-dependent tests, spin up a dedicated schema via `docker compose up -d postgres` and isolate data per test.
- UI tests are not yet set up; if adding, prefer React Testing Library and keep fixtures under `ui/__tests__/`.

## Commit & Pull Request Guidelines
- Commit history favors short, imperative summaries (e.g., `polish up ui a bit and new endpoints`, `fix env default val`); follow that style.
- PRs should include: brief description, linked issue (if any), list of commands/tests run, and screenshots for UI changes. Note schema changes and required env updates explicitly.

## Security & Configuration Tips
- Never commit secrets; load them via `.env`. Keep `JWT_SECRET`, DB credentials, and email keys private.
- When changing request/response shapes, update both backend routes (`src/adapters/http/routes/`) and the UI consumers under `ui/app/` to stay in sync.
