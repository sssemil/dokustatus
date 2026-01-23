# Repository Guidelines

## Org

You work inside a text-only workspace. Everything is Markdown.

Workspace layout:

./workspace
    /plans
       High-level plans. One file per plan. You may edit and expand these files over time.
    /tasks
        /todo
            New tasks. One Markdown file per task.
        /in-progress
            Tasks you are currently working on.
        /done
            Finished tasks. Read-only unless you need to append a short post-mortem.

All plans and tasks must live inside the /workspace directory. Do not create files or folders outside it.

Task file format:

- Title on the first line
- Short description
- Checklist or steps (if relevant)
- "History" section at the bottom where you append dated notes as you work

Rules for working:

- Only .md files are allowed.
- You update documents by appending or editing text. Do not delete history.
- Every change must be recorded in the task’s "History" section with a timestamp and a short note.
- When you start a task, move its file from /workspace/tasks/todo to /workspace/tasks/in-progress and append a history entry.
- As you make progress, append notes instead of rewriting earlier content.
- When a task is complete, move it to /workspace/tasks/done and append a final history entry.

Plans:

- Plans describe direction, scope, and next steps.
- When tasks come from a plan, link the plan file at the top of the task.
- If a plan changes, append a dated "Revision" section instead of replacing earlier text.

General behavior:

- Prefer small, incremental edits.
- Keep filenames stable; if you must rename, record it in the file history.
- Never change the folder structure under /workspace.
- Treat the workspace as the source of truth. If something is unclear, add a note to the relevant plan or task.

Your goal is to move work forward by editing, appending, creating tasks, and moving them through todo → in-progress → done while keeping a clear written trail of changes — all inside /workspace.

Use 4-digit numbers to number tasks.

## Intro

This project pairs a Rust backend (Axum + SQLx) with a Next.js UI, a TypeScript SDK, and demo apps. Local dev uses Docker (Postgres, Redis, CoreDNS, Caddy) plus mkcert for TLS. Prefer the `./run` helper for common tasks.

## Project Structure & Module Organization
- `apps/api`: backend code split by clean architecture layers (`domain`, `application`, `adapters`, `infra`); entrypoints in `main.rs` and `lib.rs`.
- `apps/api/migrations`: SQLx migrations; keep new migrations ordered and idempotent.
- `apps/ui`: Next.js App Router frontend (`app/` pages, `globals.css`, config).
- `apps/demo_api`: demo API (Node/TS) used for the demo domain.
- `apps/demo_ui`: demo UI (Next.js) for the demo domain.
- `libs/reauth-sdk-ts`: TypeScript SDK package.
- `local_infra`: local CoreDNS + Caddy configs and mkcert-managed TLS certs.
- `infra`: deploy compose, Caddy config, secrets, and deploy scripts.
- `docker-compose.yml`: local dev services (postgres, redis, coredns, caddy).
- `run`: local command runner (see `./run help`).

## Build, Test, and Development Commands
Prefer `./run` (see `./run help`). Common flows:
- `./run infra` or `./run infra:full`: start local services (`infra:full` includes CoreDNS + Caddy).
- `./run certs`: generate mkcert certs for local TLS; update `/etc/hosts` for `reauth.test`, `reauth.reauth.test`, `ingress.reauth.test`.
- `./run db:migrate` / `./run db:prepare` / `./run dev:seed`: migrations, SQLx offline data, seed local domain.
- `./run api` / `./run api:fmt` / `./run api:lint` / `./run api:test`: backend dev, formatting, linting, tests.
- `./run ui` / `./run ui:install` / `./run ui:build`: UI dev, install, build.
- `./run sdk` / `./run sdk:build`: SDK dev/build.
- `./run demo` / `./run demo:api` / `./run demo:ui` / `./run demo:install` / `./run demo:setup`: demo apps and demo domain.

### Pre-Deploy Verification
Before deploying, always verify the API builds successfully:
```bash
./run api:build
```
This runs `SQLX_OFFLINE=true cargo build --release` because the local database may not be running.

## Coding Style & Naming Conventions
- Rust 2024 edition; always run `cargo fmt` before committing. Prefer small modules aligned to `domain/application/adapters/infra`.
- Naming: Rust modules `snake_case`; types and traits `PascalCase`; functions `snake_case`; constants `SCREAMING_SNAKE_CASE`.
- Error handling uses `anyhow` for main and typed errors in `application`; propagate via `?` and map to HTTP errors in adapters.
- Prefer enums for error codes and variant propagation; avoid free-form strings for error types.
- Frontend: functional React components in `apps/ui/app` and `apps/demo_ui/app`, `PascalCase` component names, co-locate styles in `globals.css` or module styles.
- Comments should help future readers understand the code, not document what changed. Avoid transient comments like "(now always true)" or "(was optional before)" - these become confusing historical artifacts. Write comments as if the code has always been this way.

## Testing Guidelines
- Add `#[cfg(test)]` modules near the logic they verify; prefer unit tests for use cases and lightweight integration tests for adapters.
- For DB-dependent tests, spin up a dedicated schema via `./run infra` and isolate data per test.
- UI/SDK tests are not yet set up; if adding, prefer React Testing Library and keep fixtures under `apps/ui/__tests__/`.

## Commit & Pull Request Guidelines
- Commit history favors short, imperative summaries (e.g., `polish up ui a bit and new endpoints`, `fix env default val`); follow that style.
- PRs should include: brief description, linked issue (if any), list of commands/tests run, and screenshots for UI changes. Note schema changes and required env updates explicitly.

## Deployment
- **Deploy command**: `BUILD_ARGS="--network=host" DEPLOY_HOST=63.178.106.82 DEPLOY_USER=ubuntu REMOTE_DIR=/opt/reauth ./infra/deploy.sh`
- The deploy script builds Docker images, syncs them to the server, and runs `docker compose -f infra/compose.yml --env-file infra/.env up -d`.
- Production uses Caddy for TLS termination and routes API/UI/demo traffic.

## Secrets Management
Secrets are stored in `infra/secrets/` as individual files (one secret per file). These are mounted into containers via Docker secrets and read at runtime.

**Current secrets (infra/compose.yml):**
- `jwt_secret` - JWT signing key for user sessions.
- `postgres_password` - Database password.
- `redis_password` - Redis password.
- `process_number_key` - AES-256 key (base64) for encrypting sensitive data.
- `fallback_resend_api_key` - Fallback Resend API key for domains without custom email config.
- `reauth_dev_api_key` - Developer API key for server-to-server authentication.
- `anypost_api_key` - Demo app API key (Anypost).

**Adding a new secret:**
1. Create the file in `infra/secrets/` (e.g., `infra/secrets/my_secret`)
2. Add to `infra/compose.yml` under the `secrets:` section
3. Add to the service's `secrets:` list
4. Export in the service's entrypoint: `export MY_SECRET="$$(cat /run/secrets/my_secret)"`

## Security & Configuration Tips
- Never commit secrets; load them via `infra/secrets/`. Keep JWT/DB/email keys private.
- When changing request/response shapes, update backend routes (`apps/api/src/adapters/http/routes/`), the UI consumers under `apps/ui/app/`, and SDK/demo callers under `libs/reauth-sdk-ts/` and `apps/demo_*`.

## General Tips
- Never use alerts in browsers, use our internal modal for dialogs, maintain a common style/component for it.
- Use long press for destructive actions; if an action is very destructive, require a dialog where a user has to type in a text of acknowledgement (like in case of removing a domain name type in the domain name which will enable the delete button in the modal and then hold the delete button for 3 seconds using our common hold to delete pattern).

DO NOT BE LAZY! Be proactive, and action biased. Commit frequently.

## Co-op with Codex Guideline

During planning, once you have a ready plan, before asking me to review it, always first ask codex for an opinion on your plan like more or less like this:

```
codex exec """
    review my plan: my very long dummy plan placeholder

    phase 1

    phase 2

    bla bla
    bla ; just say OKAY I reviewed im just testing cli integration
    """

    OpenAI Codex v0.77.0 (research preview)
    --------
    workdir: /home/user/Projects/TQDM/reauth
    model: gpt-5.2-codex
    provider: openai
    approval: never
    sandbox: workspace-write [workdir, /tmp, $TMPDIR]
    reasoning effort: high
    reasoning summaries: auto
    session id: 019b66b7-b1ab-7760-a21f-b8bebb6e1829
    --------
    user

    review my plany: my very long dummy plan placeholder

    phase 1

    phase 2

    bla bla
    bla ; just say OKAY I reviewed im just testing cli integration

    mcp startup: no servers

    thinking
    **Acknowledging review request**
    codex
    OKAY I reviewed.
    tokens used
    2,349
    OKAY I reviewed.
```

And then validate and integrate the suggested changes into your plan.

Additionally, use this pattern once you are ready to commit changes; ask codex to review the uncommited changes, and then again, review suggestions, and apply them if they are good and valid.
