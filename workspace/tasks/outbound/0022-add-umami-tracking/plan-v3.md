# Plan: Add Umami Analytics Tracking (v3)

**Task**: [0022-add-umami-tracking](./ticket.md)
**Status**: Planning
**Created**: 2026-01-01
**Revision**: 3 (addresses feedback from v2)

## Summary

Add Umami analytics tracking script to the main UI (`apps/ui`) and demo UI (`apps/demo_ui`). The script should be:
- Loaded with `defer` attribute to avoid blocking page render
- Only loaded in production builds (controlled by env var presence at build time)
- Configurable via build-time environment variables passed during deployment

## Changes from v2

Based on feedback:
1. **Confirmed layout components are server components** - Verified neither file has `"use client"`, so `process.env.NEXT_PUBLIC_*` is inlined at build time
2. **Clarified App Router head semantics** - Main UI already uses raw `<head>` alongside `export const metadata`. Both approaches are valid and compatible in App Router. Demo UI will follow the same pattern.
3. **Improved HTML validation approach** - Replaced fragile `grep -o` pattern with explicit string match and deterministic verification steps
4. **Added concrete Docker validation snippet** - Included commands to run container and verify script presence
5. **Confirmed documentation location** - `CLAUDE.md` is a symlink to `AGENTS.md`, so updating either updates both
6. **Addressed streaming behavior** - Script in `<head>` of server component appears in initial HTML before streaming starts

## Analysis

### Current State (Verified)

1. **Main UI (`apps/ui/app/layout.tsx`)**:
   - Server component (no `"use client"`)
   - Uses `export const metadata` for title/description
   - Has explicit `<head>` section with inline theme script using `dangerouslySetInnerHTML`
   - This confirms raw `<head>` works alongside `export const metadata`

2. **Demo UI (`apps/demo_ui/app/layout.tsx`)**:
   - Server component (no `"use client"`)
   - Uses `export const metadata` for title/description
   - No `<head>` section currently - will add one

3. **Documentation files**:
   - `CLAUDE.md` is a symlink → `AGENTS.md`
   - Updating either file updates both (same content)

4. **Script from ticket**:
   ```html
   <script defer src="https://cloud.umami.is/script.js" data-website-id="c8305e3a-8646-454b-a40f-2c3ab99aeb61"></script>
   ```

### Design Decisions

1. **Server component verification**: Both layout files are server components (no `"use client"` directive). This means `process.env.NEXT_PUBLIC_UMAMI_WEBSITE_ID` is evaluated at build time and inlined into the HTML. No client-side JavaScript is needed to read the env var.

2. **App Router `<head>` semantics**: In Next.js App Router, you can use both `export const metadata` (for standard meta tags) AND raw `<head>` elements (for custom scripts). They are merged at render time. This is already proven working in `apps/ui/app/layout.tsx` which uses both.

3. **Streaming compatibility**: Scripts in `<head>` of the root layout are rendered in the initial HTML shell before any streaming content. The Umami script will load correctly even with React Server Components streaming.

4. **Use raw `<script defer>` tag** instead of Next.js Script component:
   - The ticket explicitly specifies `defer` attribute behavior
   - `next/script` with `strategy="afterInteractive"` fires after hydration, which is later than `defer`
   - Matches the existing pattern in main UI

5. **Shared website ID for both apps**: Both apps track to the same Umami dashboard. Umami can filter by URL if needed later. No `data-domains` attribute needed now—keep it simple.

6. **Pass website ID via `BUILD_ARGS` at deploy time**: Dockerfile ARGs default to empty (no tracking by default). Production deploy includes the website ID.

## Implementation Steps

### Step 1: Add Umami script to Main UI

**File**: `apps/ui/app/layout.tsx`

Add a conditional script tag after the theme script in `<head>`:

```tsx
<head>
  <script
    dangerouslySetInnerHTML={{
      __html: `
        (function() {
          // ... existing theme script ...
        })();
      `,
    }}
  />
  {process.env.NEXT_PUBLIC_UMAMI_WEBSITE_ID && (
    <script
      defer
      src="https://cloud.umami.is/script.js"
      data-website-id={process.env.NEXT_PUBLIC_UMAMI_WEBSITE_ID}
    />
  )}
</head>
```

### Step 2: Add Umami script to Demo UI

**File**: `apps/demo_ui/app/layout.tsx`

Add a `<head>` section before `<body>`:

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
  <body style={{ ... }}>
    {children}
  </body>
</html>
```

### Step 3: Update Dockerfiles with empty default ARGs

**File**: `apps/ui/Dockerfile`

Add in builder stage (after existing ARGs, before `npm run build`):
```dockerfile
ARG NEXT_PUBLIC_UMAMI_WEBSITE_ID=
ENV NEXT_PUBLIC_UMAMI_WEBSITE_ID=${NEXT_PUBLIC_UMAMI_WEBSITE_ID}
```

**File**: `apps/demo_ui/Dockerfile`

Add in builder stage (after existing ARGs, before `npm run build`):
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

### Step 5: Update AGENTS.md with deploy instructions

Add to the Deployment section (note: CLAUDE.md symlinks here):

```markdown
### Analytics Tracking

To enable Umami analytics, add the website ID to BUILD_ARGS when deploying:

\`\`\`bash
BUILD_ARGS="--network=host --build-arg NEXT_PUBLIC_UMAMI_WEBSITE_ID=c8305e3a-8646-454b-a40f-2c3ab99aeb61" DEPLOY_HOST=63.178.106.82 DEPLOY_USER=ubuntu REMOTE_DIR=/opt/reauth ./infra/deploy.sh
\`\`\`

If the build arg is omitted, no analytics script is included (safe for local/staging).
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
| `AGENTS.md` | Add analytics tracking deploy instructions |

## Testing Approach

### 1. Local Development Test (without tracking)

```bash
# Start UI dev server without env var set
./run ui

# Fetch page and verify NO Umami script
curl -s http://localhost:3000 | grep -F 'cloud.umami.is'
# Expected: no output (exit code 1)
```

### 2. Local Development Test (with tracking)

```bash
# Add to apps/ui/.env.local:
# NEXT_PUBLIC_UMAMI_WEBSITE_ID=test-id-12345

# Restart dev server, then:
curl -s http://localhost:3000 | grep -F 'data-website-id="test-id-12345"'
# Expected: matches the script tag
```

### 3. Build Verification

```bash
./run ui:build    # No TypeScript/build errors
./run api:build   # No regressions in API
```

### 4. Docker Build Test (without tracking)

```bash
# Build without tracking (default)
docker build -f apps/ui/Dockerfile -t test-ui-no-track .

# Run and verify no script
docker run -d -p 3001:3000 --name test-ui-container test-ui-no-track
sleep 3  # Wait for startup
curl -s http://localhost:3001 | grep -F 'cloud.umami.is'
# Expected: no output
docker stop test-ui-container && docker rm test-ui-container
```

### 5. Docker Build Test (with tracking)

```bash
# Build with tracking
docker build --build-arg NEXT_PUBLIC_UMAMI_WEBSITE_ID=test-docker-id \
  -f apps/ui/Dockerfile -t test-ui-with-track .

# Run and verify script present
docker run -d -p 3001:3000 --name test-ui-container test-ui-with-track
sleep 3
curl -s http://localhost:3001 | grep -F 'data-website-id="test-docker-id"'
# Expected: matches the script tag with test-docker-id
docker stop test-ui-container && docker rm test-ui-container
```

### 6. Production Smoke Test (post-deploy)

1. Visit https://reauth.dev
2. Open browser DevTools → Network tab
3. Look for request to `cloud.umami.is/script.js`
4. Verify the script tag in page source contains correct website ID
5. Visit demo app (https://anypost.xyz) and verify same behavior
6. Check Umami dashboard for new page views

## Edge Cases

1. **Missing env var**: No script rendered (conditional `&&` handles this)
2. **Invalid website ID**: Umami script loads but silently fails to track
3. **CSP headers**: Current config has no CSP. If added later, add `cloud.umami.is` to `script-src`
4. **Ad blockers**: Some users will block Umami - expected, analytics will be partial
5. **Local Docker builds**: No tracking by default (empty ARG default)

## Rollback Plan

If issues arise:
1. **Quick**: Remove `--build-arg NEXT_PUBLIC_UMAMI_WEBSITE_ID=...` from deploy command and redeploy
2. **Permanent**: `git revert` the commit and redeploy

## Deploy Command (Updated)

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
- 2026-01-01: Codex review of v2 - addressed 4 findings
- 2026-01-01: Created v3 addressing v2 feedback:
  - Verified both layouts are server components (no `"use client"`)
  - Clarified App Router `<head>` semantics work alongside `export const metadata`
  - Replaced fragile HTML validation with deterministic `grep -F` commands
  - Added concrete Docker validation snippets with full commands
  - Confirmed `CLAUDE.md` symlinks to `AGENTS.md` (same file)
  - Added note about streaming compatibility
  - Improved testing section with explicit expected outputs
