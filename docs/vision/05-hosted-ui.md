# Hosted UI

## Overview

All auth pages are hosted at `auth.{customer-domain}`. Fully branded, no code required.

---

## Phase 1 Pages

### Login (`/login`)

Single page for both login and signup. Account created automatically if email doesn't exist.

```
┌─────────────────────────────────────────────────────┐
│                                                     │
│                   [App Logo]                        │
│                                                     │
│              Sign in to {App Name}                  │
│                                                     │
│     ┌─────────────────────────────────────────┐     │
│     │                                         │     │
│     │      Continue with Google            G  │     │
│     │                                         │     │
│     └─────────────────────────────────────────┘     │
│                                                     │
│                    ── or ──                         │
│                                                     │
│     ┌─────────────────────────────────────────┐     │
│     │  email@example.com                      │     │
│     └─────────────────────────────────────────┘     │
│                                                     │
│     ┌─────────────────────────────────────────┐     │
│     │          Send magic link                │     │
│     └─────────────────────────────────────────┘     │
│                                                     │
│                                                     │
│         By continuing, you agree to the             │
│         Terms of Service and Privacy Policy         │
│                                                     │
└─────────────────────────────────────────────────────┘
```

**Query Parameters:**

| Param | Description |
|-------|-------------|
| `redirect` | URL to redirect after login |
| `error` | Error message to display |

**Example URLs:**
- `https://auth.myapp.com/login`
- `https://auth.myapp.com/login?redirect=https://myapp.com/dashboard`
- `https://auth.myapp.com/login?error=session_expired`

---

### Magic Link Sent (`/login/sent`)

Confirmation page after magic link email is sent.

```
┌─────────────────────────────────────────────────────┐
│                                                     │
│                   [App Logo]                        │
│                                                     │
│                   ✓ Check your email                │
│                                                     │
│     We sent a sign-in link to:                      │
│     user@example.com                                │
│                                                     │
│     Click the link in your email to sign in.        │
│     The link expires in 15 minutes.                 │
│                                                     │
│                                                     │
│     ┌─────────────────────────────────────────┐     │
│     │          Resend link                    │     │
│     └─────────────────────────────────────────┘     │
│                                                     │
│              ← Back to sign in                      │
│                                                     │
└─────────────────────────────────────────────────────┘
```

---

### Verify (`/verify`)

Magic link landing page. Shows briefly while session is created.

```
┌─────────────────────────────────────────────────────┐
│                                                     │
│                   [App Logo]                        │
│                                                     │
│                                                     │
│                     [Spinner]                       │
│                                                     │
│                 Signing you in...                   │
│                                                     │
│                                                     │
└─────────────────────────────────────────────────────┘
```

**Query Parameters:**

| Param | Description |
|-------|-------------|
| `token` | Magic link token |
| `redirect` | URL to redirect after verification |

**Error States:**
- Token invalid → redirect to `/login?error=invalid_link`
- Token expired → redirect to `/login?error=link_expired`
- Token used → redirect to `/login?error=link_used`

---

### Billing (`/billing`)

Stripe customer portal. Redirects to Stripe-hosted portal.

```
Flow:
1. User visits auth.myapp.com/billing
2. reauth creates Stripe portal session
3. Redirect to Stripe portal
4. User manages subscription
5. Stripe redirects back to app
```

**What users can do:**
- View current plan
- Change plan (upgrade/downgrade)
- Update payment method
- View invoices
- Cancel subscription

---

### Settings (`/settings`)

User profile management.

```
┌─────────────────────────────────────────────────────┐
│                                                     │
│  [App Logo]                          [Sign Out]     │
│                                                     │
├─────────────────────────────────────────────────────┤
│                                                     │
│  Account Settings                                   │
│                                                     │
│  ┌─────────────────────────────────────────────┐   │
│  │  Profile                                    │   │
│  │                                             │   │
│  │  [Avatar]  Change photo                     │   │
│  │                                             │   │
│  │  Name                                       │   │
│  │  ┌─────────────────────────────────────┐   │   │
│  │  │  Jane Doe                           │   │   │
│  │  └─────────────────────────────────────┘   │   │
│  │                                             │   │
│  │  Email                                      │   │
│  │  user@example.com (verified ✓)              │   │
│  │                                             │   │
│  └─────────────────────────────────────────────┘   │
│                                                     │
│  ┌─────────────────────────────────────────────┐   │
│  │  Connected Accounts                         │   │
│  │                                             │   │
│  │  Google    user@gmail.com    [Connected]    │   │
│  │                                             │   │
│  └─────────────────────────────────────────────┘   │
│                                                     │
│  ┌─────────────────────────────────────────────┐   │
│  │  Subscription                               │   │
│  │                                             │   │
│  │  Current Plan: Pro                          │   │
│  │  Status: Active                             │   │
│  │  Renews: January 15, 2025                   │   │
│  │                                             │   │
│  │  [Manage Subscription]                      │   │
│  │                                             │   │
│  └─────────────────────────────────────────────┘   │
│                                                     │
│                                                     │
│  ┌─────────────────────────────────────────────┐   │
│  │  Danger Zone                                │   │
│  │                                             │   │
│  │  [Delete Account]                           │   │
│  │                                             │   │
│  └─────────────────────────────────────────────┘   │
│                                                     │
└─────────────────────────────────────────────────────┘
```

---

### Logout (`/logout`)

Clears session, redirects.

**Query Parameters:**

| Param | Description |
|-------|-------------|
| `redirect` | URL to redirect after logout (default: `/login`) |

---

## Callback Routes (Internal)

These routes handle OAuth callbacks. Not user-facing.

| Route | Purpose |
|-------|---------|
| `/callback/google` | Google OAuth callback |

---

## Branding

Customers configure branding in the reauth dashboard:

```typescript
interface Branding {
    appName: string        // "My App"
    logoUrl: string        // "https://myapp.com/logo.png"
    primaryColor: string   // "#6366f1"
    // Future: custom CSS, fonts, etc.
}
```

**Applied to:**
- Logo in header
- Button colors
- Link colors
- Email templates

---

## Error States

### Error Messages

| Error Code | Message |
|------------|---------|
| `session_expired` | Your session has expired. Please sign in again. |
| `invalid_link` | This link is invalid. Please request a new one. |
| `link_expired` | This link has expired. Please request a new one. |
| `link_used` | This link has already been used. Please request a new one. |
| `oauth_error` | Something went wrong with Google sign-in. Please try again. |
| `account_deleted` | This account has been deleted. |

### Display

```
┌─────────────────────────────────────────────────────┐
│                                                     │
│  ┌─────────────────────────────────────────────┐   │
│  │  ⚠ Your session has expired.                │   │
│  │     Please sign in again.                   │   │
│  └─────────────────────────────────────────────┘   │
│                                                     │
│  ... rest of login form ...                         │
│                                                     │
└─────────────────────────────────────────────────────┘
```

---

## Mobile Responsiveness

All pages are mobile-responsive. Same layout, adjusted sizing.

```
Mobile (<640px):
- Full-width inputs
- Larger tap targets (min 44px)
- Logo scales down
- Reduced padding

Desktop (≥640px):
- Centered card (max-width 400px)
- Standard sizing
```

---

## Security

### CSRF Protection
- State parameter in OAuth flow
- Referer checking on form submissions

### Rate Limiting
- Magic link: 5 per email per hour
- Login attempts: 10 per IP per minute
- OAuth: 20 per IP per minute

### Cookie Settings
```
Name: reauth_session
Domain: .myapp.com (includes subdomains)
Path: /
HttpOnly: true
Secure: true
SameSite: Lax
MaxAge: 30 days (configurable)
```

---

## Phase 2 Pages

These are added in Phase 2:

| Page | Purpose |
|------|---------|
| `/keys` | API key management |
| `/team` | Team settings, members |
| `/team/invite` | Accept team invitation |
| `/credits` | Credit balance, top-up |
| `/referrals` | Referral code, stats |
| `/waitlist` | Pre-launch signup |
