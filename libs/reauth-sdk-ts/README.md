# @reauth/sdk

TypeScript SDK for [reauth.dev](https://reauth.dev) - passwordless authentication for your apps.

## Installation

```bash
npm install @reauth/sdk
# or
pnpm add @reauth/sdk
# or
yarn add @reauth/sdk
```

## Quick Start

### React (Next.js)

```typescript
// app/layout.tsx
import { AuthProvider } from '@reauth/sdk/react';

export default function RootLayout({ children }) {
  return (
    <AuthProvider config={{ domain: 'yourdomain.com' }}>
      {children}
    </AuthProvider>
  );
}

// app/dashboard/page.tsx
import { ProtectedRoute, useAuthContext } from '@reauth/sdk/react';

export default function Dashboard() {
  return (
    <ProtectedRoute>
      <DashboardContent />
    </ProtectedRoute>
  );
}

function DashboardContent() {
  const { user, logout } = useAuthContext();

  return (
    <div>
      <h1>Welcome, {user?.email}</h1>
      <button onClick={logout}>Sign out</button>
    </div>
  );
}
```

### Vanilla JavaScript

```typescript
import { createReauthClient } from '@reauth/sdk';

const reauth = createReauthClient({ domain: 'yourdomain.com' });

// Check if authenticated
const session = await reauth.getSession();
if (!session.valid) {
  reauth.login(); // Redirect to login
}

// Log out
await reauth.logout();
```

### Server-Side (API Routes)

```typescript
// Next.js API Route
import { createServerClient } from '@reauth/sdk/server';
import { NextRequest, NextResponse } from 'next/server';

const reauth = createServerClient({ domain: 'yourdomain.com' });

export async function GET(request: NextRequest) {
  const user = await reauth.getUser(request.headers.get('cookie') || '');

  if (!user) {
    return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
  }

  return NextResponse.json({ user });
}
```

## How It Works

reauth.dev handles the entire login flow. Your app just needs to:

1. **Redirect to login** - Send users to `https://reauth.yourdomain.com`
2. **Check session** - Verify authentication via the session endpoint
3. **Handle logout** - Clear session cookies

```
User clicks "Sign in" → Redirect to reauth.yourdomain.com
→ reauth handles email + magic link → User authenticates
→ Redirect back to your app with cookies set
→ Your app checks /auth/session → User info returned
```

## API Reference

### Browser Client

```typescript
import { createReauthClient } from '@reauth/sdk';

const reauth = createReauthClient({ domain: 'yourdomain.com' });

// Redirect to login page
reauth.login();

// Check authentication status
const session = await reauth.getSession();
// Returns: { valid, end_user_id, email, roles, waitlist_position, error, error_code }

// Refresh access token (when session.valid === false)
const success = await reauth.refresh();

// Log out
await reauth.logout();

// Delete account (self-service)
const deleted = await reauth.deleteAccount();
```

### Server Client

```typescript
import { createServerClient } from '@reauth/sdk/server';

const reauth = createServerClient({ domain: 'yourdomain.com' });

// Get raw session data
const session = await reauth.getSession(cookieHeader);

// Get user object (or null if not authenticated)
const user = await reauth.getUser(cookieHeader);
// Returns: { id, email, roles } | null
```

### React Components

```typescript
import { AuthProvider, useAuthContext, useAuth, ProtectedRoute } from '@reauth/sdk/react';

// AuthProvider - Wrap your app
<AuthProvider config={{ domain: 'yourdomain.com' }}>
  {children}
</AuthProvider>

// useAuthContext - Access auth state
const { user, loading, error, isOnWaitlist, waitlistPosition, login, logout, refetch } = useAuthContext();

// useAuth - Standalone hook (without provider)
const auth = useAuth({ domain: 'yourdomain.com' });

// ProtectedRoute - Protect pages
<ProtectedRoute fallback={<Loading />} onWaitlist={() => router.push('/waitlist')}>
  <ProtectedContent />
</ProtectedRoute>
```

## Types

```typescript
type ReauthSession = {
  valid: boolean;
  end_user_id: string | null;
  email: string | null;
  roles: string[] | null;
  waitlist_position: number | null;
  error: string | null;
  error_code: 'ACCOUNT_SUSPENDED' | null;
};

type User = {
  id: string;
  email: string;
  roles: string[];
};

type ReauthConfig = {
  domain: string;
};
```

## Handling Edge Cases

### Token Refresh

```typescript
const session = await reauth.getSession();

if (!session.valid && !session.error_code) {
  // Access token expired, try refresh
  const refreshed = await reauth.refresh();
  if (refreshed) {
    const newSession = await reauth.getSession();
    // Continue with newSession
  } else {
    // Refresh token also expired, redirect to login
    reauth.login();
  }
}
```

### Waitlist

```typescript
if (session.valid && session.waitlist_position) {
  // User authenticated but on waitlist
  // Show waitlist page with position
  console.log(`Position: #${session.waitlist_position}`);
}
```

### Account Suspended

```typescript
if (session.error_code === 'ACCOUNT_SUSPENDED') {
  // Show suspended message
  // User cannot access app until admin unfreezes
}
```

## Prerequisites

1. **Verify your domain** on [reauth.dev](https://reauth.dev)
2. **Set redirect URL** (where users go after login)
3. **Enable magic link auth** in domain settings

## Cookies

reauth.dev sets these cookies on `.yourdomain.com`:

| Cookie | Purpose | Expiry |
|--------|---------|--------|
| `end_user_access_token` | Auth token (HttpOnly) | 24 hours |
| `end_user_refresh_token` | Token renewal (HttpOnly) | 30 days |
| `end_user_email` | Display email | 30 days |

All cookies require HTTPS.

## License

MIT
