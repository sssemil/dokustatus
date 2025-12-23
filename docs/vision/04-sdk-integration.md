# SDK & Integration

## Integration Tiers

### Tier 0: No Code

Redirect-based flow. No SDK needed.

```
1. User visits your app
2. You redirect to auth.yourapp.com/login?redirect=https://yourapp.com/dashboard
3. User logs in
4. Redirected back with session cookie set
5. Your server reads cookie, calls reauth API to verify
```

### Tier 1: One Function

The core use case. One function that returns everything.

```typescript
import { getUser } from 'reauth'

export async function handler(request: Request) {
    const user = await getUser(request)
    
    if (!user) {
        return Response.redirect('/login')
    }
    
    // user is fully typed, includes billing status
    console.log(user.plan) // 'pro'
    console.log(user.subscriptionStatus) // 'active'
}
```

### Tier 2: Middleware

Framework-specific middleware for cleaner code.

```typescript
import { withAuth } from 'reauth/next'

export const GET = withAuth(async (request, { user }) => {
    // user is guaranteed to exist
    return Response.json({ message: `Hello ${user.name}` })
})
```

---

## SDK Reference

### Installation

```bash
npm install reauth
# or
pnpm add reauth
# or
yarn add reauth
```

### Configuration

```typescript
// Option 1: Environment variable (recommended)
// Set REAUTH_API_KEY in your environment

// Option 2: Explicit configuration
import { configure } from 'reauth'

configure({
    apiKey: 'sk_live_...',
    // Optional: custom API URL for self-hosted
    apiUrl: 'https://api.reauth.dev',
})
```

### getUser()

The primary function. Extracts session from request, returns user data or null.

```typescript
import { getUser } from 'reauth'

const user = await getUser(request)
```

**Parameters:**
- `request` — Standard Request object, or Node.js IncomingMessage, or object with `cookies`/`headers`

**Returns:** `User | null`

```typescript
interface User {
    id: string                          // 'usr_abc123'
    email: string                       // 'user@example.com'
    emailVerified: boolean              // true
    name: string | null                 // 'Jane Doe'
    avatarUrl: string | null            // 'https://...'
    
    // Billing
    plan: string | null                 // 'pro' (plan slug)
    planFeatures: Record<string, any>   // { maxProjects: 10 }
    subscriptionStatus: SubscriptionStatus | null
    subscriptionEndsAt: string | null   // ISO date
    
    // Metadata
    createdAt: string                   // ISO date
    lastSeenAt: string | null           // ISO date
}

type SubscriptionStatus = 
    | 'active' 
    | 'past_due' 
    | 'cancelled' 
    | 'trialing'
```

**Example:**

```typescript
const user = await getUser(request)

if (!user) {
    return Response.redirect('https://auth.myapp.com/login')
}

if (user.subscriptionStatus !== 'active') {
    return Response.redirect('https://auth.myapp.com/billing')
}

if (!user.planFeatures.apiAccess) {
    return Response.json({ error: 'Upgrade required' }, { status: 403 })
}
```

### getSession()

Lower-level function. Returns raw session data.

```typescript
import { getSession } from 'reauth'

const session = await getSession(request)
```

**Returns:** `Session | null`

```typescript
interface Session {
    id: string
    userId: string
    expiresAt: string
    createdAt: string
}
```

### verifySession()

Verify a session token directly (useful for custom auth flows).

```typescript
import { verifySession } from 'reauth'

const session = await verifySession(token)
```

---

## Middleware

### Next.js (App Router)

```typescript
import { withAuth } from 'reauth/next'

// Basic protection
export const GET = withAuth(async (request, { user }) => {
    return Response.json({ user })
})

// With plan requirement
export const GET = withAuth(
    { plan: 'pro' },
    async (request, { user }) => {
        // Only pro users reach here
        return Response.json({ data: 'premium content' })
    }
)

// With custom unauthorized handler
export const GET = withAuth(
    { 
        onUnauthorized: () => Response.json(
            { error: 'Please log in' }, 
            { status: 401 }
        )
    },
    async (request, { user }) => {
        return Response.json({ user })
    }
)
```

### Next.js (Middleware)

```typescript
// middleware.ts
import { authMiddleware } from 'reauth/next'

export default authMiddleware({
    publicPaths: ['/', '/about', '/pricing'],
    loginUrl: 'https://auth.myapp.com/login',
})

export const config = {
    matcher: ['/((?!_next/static|_next/image|favicon.ico).*)'],
}
```

### Express

```typescript
import express from 'express'
import { withAuth } from 'reauth/express'

const app = express()

// Protect a route
app.get('/dashboard', withAuth(), (req, res) => {
    // req.user is available
    res.json({ user: req.user })
})

// With plan requirement
app.get('/api/premium', withAuth({ plan: 'pro' }), (req, res) => {
    res.json({ data: 'premium' })
})
```

### Hono

```typescript
import { Hono } from 'hono'
import { withAuth } from 'reauth/hono'

const app = new Hono()

app.get('/dashboard', withAuth(), (c) => {
    const user = c.get('user')
    return c.json({ user })
})
```

---

## Auth URLs

Your hosted auth pages:

| URL | Purpose |
|-----|---------|
| `https://auth.yourapp.com/login` | Login page |
| `https://auth.yourapp.com/login?redirect=URL` | Login with redirect |
| `https://auth.yourapp.com/logout` | Logout (clears session) |
| `https://auth.yourapp.com/logout?redirect=URL` | Logout with redirect |
| `https://auth.yourapp.com/billing` | Billing portal |
| `https://auth.yourapp.com/settings` | User settings |

### Linking to Auth Pages

```typescript
// Login link
<a href="https://auth.myapp.com/login?redirect=https://myapp.com/dashboard">
    Sign In
</a>

// Logout link
<a href="https://auth.myapp.com/logout?redirect=https://myapp.com">
    Sign Out
</a>

// Billing link
<a href="https://auth.myapp.com/billing">
    Manage Subscription
</a>
```

---

## TypeScript Types

Full TypeScript support included.

```typescript
import type { User, Session, SubscriptionStatus } from 'reauth'

function isProUser(user: User): boolean {
    return user.plan === 'pro' && user.subscriptionStatus === 'active'
}
```

---

## Error Handling

```typescript
import { getUser, ReauthError } from 'reauth'

try {
    const user = await getUser(request)
} catch (error) {
    if (error instanceof ReauthError) {
        console.error('Reauth error:', error.code, error.message)
        // error.code: 'invalid_token' | 'expired_token' | 'network_error' | ...
    }
    throw error
}
```

---

## Common Patterns

### Protect All Routes Except Public Ones

```typescript
// Next.js middleware.ts
import { authMiddleware } from 'reauth/next'

export default authMiddleware({
    publicPaths: [
        '/',
        '/about',
        '/pricing',
        '/blog/(.*)',
    ],
})
```

### Check Plan Features

```typescript
const user = await getUser(request)

if (!user) {
    return unauthorized()
}

// Check specific feature
if (!user.planFeatures.apiAccess) {
    return Response.json(
        { error: 'API access requires Pro plan' },
        { status: 403 }
    )
}

// Check numeric limit
if (projectCount >= user.planFeatures.maxProjects) {
    return Response.json(
        { error: 'Project limit reached' },
        { status: 403 }
    )
}
```

### Handle Subscription States

```typescript
const user = await getUser(request)

if (!user) {
    return redirect('/login')
}

switch (user.subscriptionStatus) {
    case 'active':
    case 'trialing':
        // Full access
        break
    
    case 'past_due':
        // Maybe show warning banner but allow access
        break
    
    case 'cancelled':
        // Check if still in grace period
        if (new Date(user.subscriptionEndsAt) > new Date()) {
            // Still has access until period ends
        } else {
            return redirect('/billing')
        }
        break
    
    default:
        // No subscription
        return redirect('/billing')
}
```

### Server Component (Next.js)

```typescript
// app/dashboard/page.tsx
import { getUser } from 'reauth'
import { cookies } from 'next/headers'
import { redirect } from 'next/navigation'

export default async function DashboardPage() {
    const cookieStore = cookies()
    const user = await getUser({ cookies: cookieStore })
    
    if (!user) {
        redirect('https://auth.myapp.com/login')
    }
    
    return (
        <div>
            <h1>Welcome, {user.name}</h1>
            <p>Plan: {user.plan}</p>
        </div>
    )
}
```

### API Route (Next.js)

```typescript
// app/api/data/route.ts
import { getUser } from 'reauth'

export async function GET(request: Request) {
    const user = await getUser(request)
    
    if (!user) {
        return Response.json(
            { error: 'Unauthorized' },
            { status: 401 }
        )
    }
    
    const data = await fetchDataForUser(user.id)
    
    return Response.json({ data })
}
```

---

## Phase 2 Additions

These are added in Phase 2 (not in MVP):

### API Key Verification

```typescript
import { api } from 'reauth'

// Verify API key from request
const key = await api.verify(request)

// With permission check
const key = await api.verify(request, {
    require: ['read', 'write'],
})

// With credit deduction
const key = await api.verify(request, {
    cost: 10,  // deduct 10 credits
})
```

### Team Context

```typescript
const user = await getUser(request)

// User's teams
user.teams  // [{ id, name, role }]

// Current team (if team context in session)
user.currentTeam  // { id, name, role }
```

### Feature Flags

```typescript
import { flags } from 'reauth'

const enabled = await flags.isEnabled('new-dashboard', user)

// With default
const variant = await flags.getVariant('checkout-flow', user, 'control')
```

### Analytics

```typescript
import { analytics } from 'reauth'

// Track event
analytics.track('feature_used', {
    feature: 'export',
    format: 'csv',
})
```
