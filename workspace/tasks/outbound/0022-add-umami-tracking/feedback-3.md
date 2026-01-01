# Plan Review Feedback (v3)

## What is good about the plan
- Verifies both layouts are server components and that raw `<head>` usage is already established in `apps/ui/app/layout.tsx`, which reduces risk of App Router surprises.
- Uses a conditional script tag keyed off a build-time env var, so local/staging builds stay clean by default.
- Keeps the script tag matching the ticket (`defer` + Umami URL), avoiding `next/script` timing differences.
- Calls out streaming behavior and includes a clear rollback path.
- Provides concrete test steps (curl and Docker) plus production smoke checks.

## What is missing or unclear
- How `BUILD_ARGS` are wired into `infra/deploy.sh`/`infra/compose.yml` for both `apps/ui` and `apps/demo_ui` images. If the deploy path does not propagate build args to each service, the script will never be included in production.
- Whether `apps/demo_ui` already has `app/head.tsx` or any `<head>` customizations. Adding `<head>` in the layout could duplicate or override existing head content.
- The statement “only loaded in production builds” conflicts with the local dev test that explicitly enables it via `.env.local`. Clarify if production-only is required (e.g., guard on `NODE_ENV`).
- Whether a single Umami website ID for both apps is desired. If not, the plan should include separate IDs or a `data-domains` strategy.
- Demo UI coverage: there is no explicit local/docker test for `apps/demo_ui` to confirm the script tag renders there too.
- SPA navigation tracking expectations: confirm Umami auto-tracks Next.js route changes as desired; if not, route-change hooks may be needed.

## Suggested improvements
- Add an explicit check and update path for `apps/demo_ui/app/head.tsx` (if it exists) instead of introducing a new `<head>` in the layout.
- Specify exact changes needed in `infra/compose.yml` or `infra/deploy.sh` to pass `NEXT_PUBLIC_UMAMI_WEBSITE_ID` to both UI builds, or confirm `BUILD_ARGS` already fan out correctly.
- Add a demo UI verification step (dev + Docker) mirroring the main UI tests to avoid missing the second app.
- If production-only is a hard requirement, gate on `process.env.NODE_ENV === "production"` in both layouts.
- Decide on analytics segmentation (one ID vs two IDs) and capture that choice explicitly in the plan.

## Risks or concerns
- Build args not actually applied to UI/demo UI images would yield a silent failure in production (no script tag even though the deploy command includes the arg).
- Using a single website ID merges data between `reauth.dev` and `anypost.xyz`, making it harder to separate metrics later.
- Adding `<head>` to the demo UI layout could conflict with any existing head logic, causing duplicate tags or unexpected overrides.
- If the ID is set in dev, analytics data may be polluted unless the environment is gated.
- If Umami does not auto-track SPA route changes in Next.js, pageview counts may be under-reported.
