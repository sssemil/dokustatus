# Plan: Add Umami Analytics Tracking

**Task**: [0022-add-umami-tracking](./ticket.md)
**Status**: Planning
**Created**: 2026-01-01

## Summary

Add Umami analytics tracking script to the main UI (`apps/ui`) and demo UI (`apps/demo_ui`). The script should be:
- Loaded asynchronously with `defer` to avoid blocking page render
- Only loaded in production to avoid polluting analytics with dev traffic
- Configurable via environment variables for the website ID

## Analysis

### Current State

1. **Main UI (`apps/ui/app/layout.tsx`)**: Root layout with `<head>` section containing a theme initialization script using `dangerouslySetInnerHTML`. Uses ThemeProvider wrapper.

2. **Demo UI (`apps/demo_ui/app/layout.tsx`)**: Minimal root layout with inline body styles, no `<head>` content.

3. **Environment Variable Pattern**:
   - Uses `NEXT_PUBLIC_*` prefix for client-side env vars
   - Env vars defined in `.env.example` files and Dockerfile ARGs
   - Production compose passes env vars to containers

4. **Script from ticket**:
   ```html
   <script defer src="https://cloud.umami.is/script.js" data-website-id="c8305e3a-8646-454b-a40f-2c3ab99aeb61"></script>
   ```

### Critical Insight: Next.js Build-Time Variables

**Important**: `NEXT_PUBLIC_*` environment variables in Next.js are **inlined at build time**, not read at runtime. This means:
- Adding env vars to `compose.yml` environment section won't work (that's runtime only)
- The Dockerfiles already use `ARG` + `ENV` pattern for build-time vars (e.g., `NEXT_PUBLIC_MAIN_DOMAIN`)
- We need to pass `--build-arg` during Docker image builds

The current `build-images.sh` accepts `BUILD_ARGS` but doesn't auto-pass env vars. The simplest approach is to **hardcode the production website ID in the Dockerfiles** since:
1. This is a single Umami instance tracking our own product
2. There's no need for different IDs per environment (dev won't have it, prod will)
3. Keeping it simple avoids complex build script changes

### Design Decisions

1. **Use Next.js `<Script>` component** vs raw `<script>` tag:
   - Next.js Script component with `strategy="afterInteractive"` is preferred as it handles defer behavior and integrates with Next.js rendering
   - However, the existing codebase uses `dangerouslySetInnerHTML` for the theme script
   - For consistency and simplicity, we'll use the `next/script` component since this is an external script

2. **Environment variable for website ID**:
   - Use `NEXT_PUBLIC_UMAMI_WEBSITE_ID` for the website ID
   - Only render the script when this env var is set (production-only control)
   - **Hardcode the production value in Dockerfiles** (simpler than modifying build scripts)
   - Local dev won't have the var set, so no tracking in dev

3. **Both apps get tracking**:
   - Main UI: primary product, needs tracking
   - Demo UI: demonstrates product capabilities, useful to track demo usage

## Implementation Steps

### Step 1: Add Umami script to Main UI

**File**: `apps/ui/app/layout.tsx`

Changes:
- Import `Script` from `next/script`
- Add Umami Script component inside `<head>` with:
  - `src="https://cloud.umami.is/script.js"`
  - `data-website-id` from env var
  - `strategy="afterInteractive"` (equivalent to defer)
- Only render when `NEXT_PUBLIC_UMAMI_WEBSITE_ID` is set

### Step 2: Add Umami script to Demo UI

**File**: `apps/demo_ui/app/layout.tsx`

Changes:
- Import `Script` from `next/script`
- Add `<head>` section to the `<html>` element
- Add Umami Script component with:
  - `src="https://cloud.umami.is/script.js"`
  - `data-website-id` from env var
  - `strategy="afterInteractive"`
- Only render when `NEXT_PUBLIC_UMAMI_WEBSITE_ID` is set

### Step 3: Update Environment Configuration

**Files to update**:

1. `apps/ui/.env.example`:
   - Add `# NEXT_PUBLIC_UMAMI_WEBSITE_ID=` (commented, empty by default for dev)

2. `apps/ui/Dockerfile`:
   - Add ARG for `NEXT_PUBLIC_UMAMI_WEBSITE_ID` with production default value
   - Add ENV to pass it through build

3. `apps/demo_ui/Dockerfile`:
   - Add ARG for `NEXT_PUBLIC_UMAMI_WEBSITE_ID` with production default value
   - Add ENV to pass it through build

**Note**: No changes needed to `compose.yml` or `infra/.env` - the website ID is baked into the Docker image at build time.

## Files to Modify

| File | Change |
|------|--------|
| `apps/ui/app/layout.tsx` | Add Script import and Umami component |
| `apps/demo_ui/app/layout.tsx` | Add Script import, head section, and Umami component |
| `apps/ui/.env.example` | Add commented `NEXT_PUBLIC_UMAMI_WEBSITE_ID` for documentation |
| `apps/ui/Dockerfile` | Add ARG/ENV for website ID with production default |
| `apps/demo_ui/Dockerfile` | Add ARG/ENV for website ID with production default |

## Testing Approach

Since there are no existing UI tests in this codebase, testing will be manual:

1. **Local Development Test**:
   - Start UI without `NEXT_PUBLIC_UMAMI_WEBSITE_ID` set
   - Verify no Umami script in page source (dev mode should not track)
   - Set `NEXT_PUBLIC_UMAMI_WEBSITE_ID=test-id` in `.env.local`
   - Restart and verify script appears in page source with correct `data-website-id`

2. **Build Verification**:
   - Run `./run ui:build` to ensure no TypeScript/build errors
   - Run `./run api:build` to ensure no regressions

3. **Production Smoke Test** (post-deploy):
   - Visit https://reauth.dev
   - Check browser DevTools Network tab for `script.js` from `cloud.umami.is`
   - Verify Umami dashboard shows page views

## Edge Cases

1. **Missing env var**: Script component should not render at all (conditional render with `&&`)
2. **Invalid website ID**: Umami script will load but silently fail to track - acceptable
3. **CSP headers**: Check if `next.config.js` headers block external scripts. Current config does not have CSP, so this is fine.
4. **Ad blockers**: Some users may block Umami - expected behavior, analytics will be partial

## Rollback Plan

If issues arise:
1. Change Dockerfile ARG defaults to empty string (e.g., `ARG NEXT_PUBLIC_UMAMI_WEBSITE_ID=`)
2. Rebuild and redeploy - script won't render without the env var
3. Alternatively, for faster rollback: `git revert` the commit and redeploy

## Code Examples

### Main UI Layout (after changes)

```tsx
import Script from 'next/script';
// ... existing imports

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={inter.variable} suppressHydrationWarning>
      <head>
        {/* Existing theme script */}
        <script dangerouslySetInnerHTML={{...}} />

        {/* Umami Analytics */}
        {process.env.NEXT_PUBLIC_UMAMI_WEBSITE_ID && (
          <Script
            src="https://cloud.umami.is/script.js"
            data-website-id={process.env.NEXT_PUBLIC_UMAMI_WEBSITE_ID}
            strategy="afterInteractive"
          />
        )}
      </head>
      <body>...</body>
    </html>
  );
}
```

### Demo UI Layout (after changes)

```tsx
import Script from 'next/script';
// ... existing imports

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <head>
        {process.env.NEXT_PUBLIC_UMAMI_WEBSITE_ID && (
          <Script
            src="https://cloud.umami.is/script.js"
            data-website-id={process.env.NEXT_PUBLIC_UMAMI_WEBSITE_ID}
            strategy="afterInteractive"
          />
        )}
      </head>
      <body style={{...}}>
        {children}
      </body>
    </html>
  );
}
```

---

## History

- 2026-01-01: Created initial plan v1
- 2026-01-01: Codex review - identified critical issue with Next.js `NEXT_PUBLIC_*` vars being build-time only. Updated plan to hardcode website ID in Dockerfiles instead of using compose.yml runtime env vars. Removed unnecessary `compose.yml` and `infra/.env.example` changes from scope.
