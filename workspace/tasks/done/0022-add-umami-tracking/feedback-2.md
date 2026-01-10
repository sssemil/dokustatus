# Feedback: Plan v2 for 0022-add-umami-tracking

## What's good
- Clear scope: covers both `apps/ui` and `apps/demo_ui` with identical treatment and calls out shared website ID rationale.
- Sensible gating: conditional render on `NEXT_PUBLIC_UMAMI_WEBSITE_ID` prevents accidental tracking in non-prod builds.
- Build-time config: Dockerfile ARG + ENV pattern aligns with Next.js inlining and avoids hardcoding secrets.
- Deployment guidance: explicit `BUILD_ARGS` example reduces ambiguity for production rollout.
- Edge cases and rollback: captures CSP and ad-blocker realities plus easy rollback path.

## What's missing or unclear
- **Next.js `<head>` semantics**: In App Router, raw `<head>` usage is supported but the plan doesn’t confirm if the demo UI layout already uses the new `head.tsx` pattern or expects metadata in `export const metadata`. Clarify whether adding `<head>` directly is consistent with current conventions in `apps/demo_ui`.
- **Type of env evaluation**: The plan assumes `process.env.NEXT_PUBLIC_UMAMI_WEBSITE_ID` is available in server components at build time. It is for Next, but if the layout is a client component in either app, `process.env` usage differs. Confirm `layout.tsx` is server (no `"use client"`).
- **HTML validation step practicality**: The proposed `curl | grep -o 'script.*umami'` could fail due to minification or `<script>` formatting across lines. Needs a more robust check (or note the limitations).
- **Docker build verification**: No instructions on how to inspect built HTML inside the image (Next.js output is not trivial to check without running container). The plan says “verify script appears” but not how.
- **CLAUDE.md ownership**: Plan assumes CLAUDE.md is the right place, but the repo also has AGENTS.md with instructions; not referenced. Confirm docs location and whether the team prefers updating AGENTS.md instead of CLAUDE.md.

## Suggested improvements
- Add a short note confirming both `layout.tsx` files are **server components** (no `"use client"`), or add a small check step.
- Update the HTML validation approach to something deterministic, e.g. render a page and `grep` for `data-website-id="test-id"` using `curl -s` and `rg` or `grep -F` with a clear expected substring.
- Provide a concrete Docker validation snippet (e.g., `docker run -p 3000:3000 test-ui` then `curl http://localhost:3000`), or explicitly state it’s optional.
- Mention whether to add `data-domains` now or leave it out; if shared ID is used, consider documenting filtering in Umami dashboard settings.
- Clarify the exact doc file to update for deploy notes (CLAUDE.md vs AGENTS.md) based on current repo norms.

## Risks or concerns
- **App Router semantics**: If `apps/demo_ui` or `apps/ui` use metadata/`head.tsx`, mixing raw `<head>` tags could be inconsistent or trigger lint/style guidance. Low risk but worth confirming.
- **Tracking reliability**: If the script is inserted in a server component but the app uses streaming, confirm the script still ends up in the initial HTML head (likely yes, but note it).
- **Testing fragility**: The HTML validation step as written may be flaky and could lead to false negatives; adjust to avoid wasted time during verification.
- **Documentation drift**: If deployment instructions live in multiple docs, future readers might miss the build arg requirement.
