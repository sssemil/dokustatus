# Plan: Add Umami Analytics Tracking (v2)

**Task**: [0022-add-umami-tracking](./ticket.md)
**Status**: Planning
**Created**: 2026-01-01
**Revision**: 2 (addresses feedback from v1)

## Summary

Add Umami analytics tracking script to the main UI (`apps/ui`) and demo UI (`apps/demo_ui`). The script should be:
- Loaded with `defer` attribute to avoid blocking page render
- Only loaded in production builds (controlled by env var presence at build time)
- Configurable via build-time environment variables passed during deployment

## Changes from v1

Based on feedback:
1. **No hardcoded website IDs in Dockerfiles** - Dockerfile ARGs default to empty; the website ID is passed via `BUILD_ARGS` at deploy time
2. **Added demo UI `.env.example`** - Created for documentation parity
3. **Use raw `<script defer>` instead of `next/script`** - The ticket specifies `defer` behavior; `strategy="afterInteractive"` loads later than defer. Using raw script tag in `<head>` matches the ticket exactly.
4. **Added HTML validation step** - Verify script appears exactly once in rendered output
5. **Both apps share the same website ID** - Documented as intentional (single Umami dashboard tracks all reauth traffic)

## Analysis

### Current State

1. **Main UI (`apps/ui/app/layout.tsx`)**: Root layout with `<head>` section containing a theme initialization script using `dangerouslySetInnerHTML`.

2. **Demo UI (`apps/demo_ui/app/layout.tsx`)**: Minimal root layout with no `<head>` content.

3. **Build-time env pattern in Dockerfiles**:
   - `ARG NEXT_PUBLIC_*` defines build arg with default
   - `ENV NEXT_PUBLIC_*=${...}` passes it to the build
   - `NEXT_PUBLIC_*` vars are inlined by Next.js at build time
   - `BUILD_ARGS` passed to `build-images.sh` applies to all Docker builds

4. **Script from ticket**:
   ```html
   <script defer src="https://cloud.umami.is/script.js" data-website-id="c8305e3a-8646-454b-a40f-2c3ab99aeb61"></script>
   ```

### Design Decisions

1. **Use raw `<script defer>` tag** instead of Next.js Script component:
   - The ticket explicitly specifies `defer` attribute behavior
   - `next/script` with `strategy="afterInteractive"` fires after hydration, which is later than `defer`
   - For analytics, true defer (loads during parse, executes after DOM ready) is more appropriate
   - Matches the existing pattern in main UI (uses `dangerouslySetInnerHTML` for theme script)

2. **Shared website ID for both apps**:
   - Both main UI and demo UI track to the same Umami website
   - Simpler deployment (one `BUILD_ARGS` value covers both)
   - Umami can filter by URL path if separation is needed later
   - If separate tracking is needed in future, a second env var can be added

3. **Pass website ID via `BUILD_ARGS` at deploy time**:
   - Dockerfile ARGs default to empty (no tracking by default)
   - Production deploy command includes the website ID in `BUILD_ARGS`
   - Avoids hardcoding production secrets in code
   - Staging/dev builds won't accidentally pollute production analytics

4. **Conditional rendering based on env var presence**:
   - If `NEXT_PUBLIC_UMAMI_WEBSITE_ID` is empty/unset, no script is rendered
   - This is a build-time decision (var is inlined during `npm run build`)
   - Local dev and Docker builds without the arg won't track

5. **Script placement in App Router**:
   - Main UI already has `<head>` in layout.tsx with a working inline script (theme script)
   - This confirms Next.js App Router preserves `<head>` content from root layout
   - Demo UI will add a `<head>` element following the same pattern
   - The script will be placed inside `<head>` where `defer` is honored

## Codex Review Notes (v2)

Codex raised these points which are addressed in the plan:

1. **ARG placement in Dockerfile** - Plan already specifies adding ARG/ENV in builder stage "after existing ARGs" - this is the correct location (before `npm run build`)

2. **Script placement in App Router** - Verified: main UI already has `<head>` with working inline script, confirming the pattern works. The script is added inside the explicit `<head>` element.

3. **Shared website ID across domains** - Intentional: we want unified analytics. If needed later, Umami supports `data-domains` attribute to filter. Not adding now to keep implementation simple.

4. **Documentation location** - CLAUDE.md is correct per the existing `## Deployment` section. No AGENTS.md exists in this repo.

## Implementation Steps

### Step 1: Add Umami script to Main UI

**File**: `apps/ui/app/layout.tsx`

Add a conditional script tag after the theme script in `<head>`:

```tsx
{process.env.NEXT_PUBLIC_UMAMI_WEBSITE_ID && (
  <script
    defer
    src="https://cloud.umami.is/script.js"
    data-website-id={process.env.NEXT_PUBLIC_UMAMI_WEBSITE_ID}
  />
)}
```

### Step 2: Add Umami script to Demo UI

**File**: `apps/demo_ui/app/layout.tsx`

Add a `<head>` section with the conditional script:

```tsx
<html lang="en">
  <head>
    {process.env.NEXT_PUBLIC_UMAMI_WEBSITE_ID && (
      <script
        defer
        src="https://cloud.umami.is/script.js"
        data-website-id={process.env.NEXT_PUBLIC_UMAMI_WEBSITE_ID}
      />
    )}
  </head>
  <body ...>
```

### Step 3: Update Dockerfiles with empty default ARGs

**File**: `apps/ui/Dockerfile`

Add in builder stage (after existing ARGs):
```dockerfile
ARG NEXT_PUBLIC_UMAMI_WEBSITE_ID=
ENV NEXT_PUBLIC_UMAMI_WEBSITE_ID=${NEXT_PUBLIC_UMAMI_WEBSITE_ID}
```

**File**: `apps/demo_ui/Dockerfile`

Add in builder stage (after existing ARGs):
```dockerfile
ARG NEXT_PUBLIC_UMAMI_WEBSITE_ID=
ENV NEXT_PUBLIC_UMAMI_WEBSITE_ID=${NEXT_PUBLIC_UMAMI_WEBSITE_ID}
```

### Step 4: Update .env.example files

**File**: `apps/ui/.env.example`

Add:
```
# Umami analytics website ID (only set in production builds)
# NEXT_PUBLIC_UMAMI_WEBSITE_ID=
```

**File**: `apps/demo_ui/.env.example` (create new)

```
# Production defaults for demo UI
NEXT_PUBLIC_DOMAIN=anypost.xyz

# Umami analytics website ID (only set in production builds)
# NEXT_PUBLIC_UMAMI_WEBSITE_ID=
```

### Step 5: Update CLAUDE.md with deploy instructions

Add to the Deployment section:

```markdown
To enable Umami tracking, add the website ID to BUILD_ARGS:
BUILD_ARGS="--network=host --build-arg NEXT_PUBLIC_UMAMI_WEBSITE_ID=c8305e3a-8646-454b-a40f-2c3ab99aeb61" ...
```

## Files to Modify

| File | Change |
|------|--------|
| `apps/ui/app/layout.tsx` | Add conditional Umami script in `<head>` |
| `apps/demo_ui/app/layout.tsx` | Add `<head>` section with conditional Umami script |
| `apps/ui/Dockerfile` | Add ARG/ENV for `NEXT_PUBLIC_UMAMI_WEBSITE_ID` (empty default) |
| `apps/demo_ui/Dockerfile` | Add ARG/ENV for `NEXT_PUBLIC_UMAMI_WEBSITE_ID` (empty default) |
| `apps/ui/.env.example` | Add commented `NEXT_PUBLIC_UMAMI_WEBSITE_ID` |
| `apps/demo_ui/.env.example` | Create with domain var and commented Umami ID |
| `CLAUDE.md` | Add deploy-time Umami BUILD_ARGS note |

## Testing Approach

### Local Development Test
1. Start UI without `NEXT_PUBLIC_UMAMI_WEBSITE_ID` set
2. View page source and verify no Umami script appears
3. Set `NEXT_PUBLIC_UMAMI_WEBSITE_ID=test-id` in `.env.local`
4. Restart dev server and verify script appears exactly once with correct `data-website-id`

### Build Verification
```bash
./run ui:build    # Ensure no TypeScript/build errors
./run api:build   # Ensure no regressions
```

### Docker Build Test
```bash
# Build without tracking (default)
docker build -f apps/ui/Dockerfile -t test-ui .
# Verify no script in built output

# Build with tracking
docker build --build-arg NEXT_PUBLIC_UMAMI_WEBSITE_ID=test-id -f apps/ui/Dockerfile -t test-ui .
# Verify script appears in built output
```

### HTML Validation Step
For each app, after building with the website ID set:
1. Run the production build: `npm run build && npm start`
2. Fetch the HTML: `curl http://localhost:3000 | grep -o 'script.*umami' | wc -l`
3. Verify count is exactly 1

### Production Smoke Test (post-deploy)
1. Visit https://reauth.dev
2. Check browser DevTools Network tab for `script.js` from `cloud.umami.is`
3. Verify Umami dashboard shows page views
4. Visit demo app and verify same behavior

## Edge Cases

1. **Missing env var**: No script rendered (conditional `&&` handles this)
2. **Invalid website ID**: Umami script loads but silently fails to track
3. **CSP headers**: Current config has no CSP. If added later, `cloud.umami.is` must be allowlisted. Consider adding a note in the layout file.
4. **Ad blockers**: Some users will block Umami - expected, analytics will be partial
5. **Local Docker builds**: No tracking by default (empty ARG default prevents pollution)

## Rollback Plan

If issues arise:
1. **Quick**: Remove `--build-arg NEXT_PUBLIC_UMAMI_WEBSITE_ID=...` from deploy command and redeploy
2. **Permanent**: `git revert` the commit and redeploy

## Deploy Command Update

Current:
```bash
BUILD_ARGS="--network=host" DEPLOY_HOST=63.178.106.82 DEPLOY_USER=ubuntu REMOTE_DIR=/opt/reauth ./infra/deploy.sh
```

New (with Umami tracking):
```bash
BUILD_ARGS="--network=host --build-arg NEXT_PUBLIC_UMAMI_WEBSITE_ID=c8305e3a-8646-454b-a40f-2c3ab99aeb61" DEPLOY_HOST=63.178.106.82 DEPLOY_USER=ubuntu REMOTE_DIR=/opt/reauth ./infra/deploy.sh
```

---

## History

- 2026-01-01: Created v1 plan
- 2026-01-01: Created v2 addressing feedback:
  - Removed hardcoded website ID from Dockerfiles
  - Added `apps/demo_ui/.env.example`
  - Changed from `next/script` to raw `<script defer>` per ticket requirement
  - Added HTML validation step
  - Documented shared website ID as intentional design choice
  - Added deploy command examples
- 2026-01-01: Codex review of v2 - addressed 4 findings:
  - Confirmed ARG placement is correct (builder stage, before npm run build)
  - Verified `<head>` script pattern works in App Router (main UI already uses it)
  - Documented shared website ID as intentional (Umami data-domains available if needed)
  - Confirmed CLAUDE.md is correct location for deploy docs
