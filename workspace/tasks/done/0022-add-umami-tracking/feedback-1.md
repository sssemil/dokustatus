# Feedback on plan-v1.md

## What's good
- Clear summary of scope and target files.
- Calls out Next.js build-time env behavior and avoids runtime-only config.
- Conditional rendering avoids analytics noise in dev by default.
- Includes testing and rollback notes, which helps reduce deploy risk.

## What's missing or unclear
- Hardcoding the production website ID in Dockerfiles risks tracking local Docker builds or staging unless every build overrides it.
- No mention of `apps/demo_ui/.env.example` (if it exists) or whether demo should use a separate Umami website ID.
- `strategy="afterInteractive"` is not the same as `defer`; it loads later than a deferred script in the head.
- Production-only behavior is described, but the plan only gates on env var presence, not `NODE_ENV`.
- Placement of `next/script` inside `<head>` in App Router should be validated; the plan assumes it behaves like a plain script tag.

## Suggested improvements
- Keep Dockerfiles default empty and pass the ID via build args at deploy time (use `BUILD_ARGS="--build-arg NEXT_PUBLIC_UMAMI_WEBSITE_ID=..."` with `./infra/deploy.sh`). This avoids baking prod IDs into every image.
- Add the env var to `apps/demo_ui/.env.example` as well, or document why it is omitted.
- Decide whether demo traffic should be tracked separately; if yes, add a second env var or a note that both apps share the same ID intentionally.
- If the ticket expects `defer`, consider using a raw `<script defer ...>` in `<head>` or confirm that `afterInteractive` is acceptable.
- Add a quick validation step in the plan to check the script appears exactly once in rendered HTML for both apps.

## Risks or concerns
- Hardcoding makes ID rotation or staging separation require code changes and rebuilds, increasing operational friction.
- Local Docker builds could unintentionally pollute production analytics.
- If CSP headers are added later, the script may stop loading without obvious errors unless added to the CSP allowlist.
