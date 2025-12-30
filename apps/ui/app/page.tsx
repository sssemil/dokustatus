'use client';

import { useState, useEffect } from 'react';
import { useRouter } from 'next/navigation';
import { isMainApp as checkIsMainApp, getRootDomain, URLS } from '@/lib/domain-utils';

interface AuthMethods {
  magic_link: boolean;
  google_oauth: boolean;
}

export default function Home() {
  const [email, setEmail] = useState('');
  const [status, setStatus] = useState<'checking' | 'idle' | 'loading' | 'sent' | 'error'>('checking');
  const [errorMessage, setErrorMessage] = useState('');
  const [displayDomain, setDisplayDomain] = useState('');
  const [authMethods, setAuthMethods] = useState<AuthMethods>({ magic_link: true, google_oauth: false });
  const [googleLoading, setGoogleLoading] = useState(false);
  const router = useRouter();

  useEffect(() => {
    const hostname = window.location.hostname;
    const isMainApp = checkIsMainApp(hostname);

    // For API calls, use the root domain (strip reauth. prefix)
    const apiDomain = getRootDomain(hostname);
    setDisplayDomain(apiDomain);

    // Fetch auth methods configuration
    const fetchAuthMethods = async () => {
      try {
        const res = await fetch(`/api/public/domain/${apiDomain}/config`);
        if (res.ok) {
          const config = await res.json();
          setAuthMethods({
            magic_link: config.auth_methods?.magic_link ?? true,
            google_oauth: config.auth_methods?.google_oauth ?? false,
          });
        }
      } catch {
        // Use defaults on error
      }
    };
    fetchAuthMethods();

    const checkAuth = async () => {
      try {
        const res = await fetch(`/api/public/domain/${apiDomain}/auth/session`, { credentials: 'include' });
        if (res.ok) {
          const data = await res.json();

          // Check for error (e.g., account suspended)
          if (data.error) {
            // Logout and show login
            await fetch(`/api/public/domain/${apiDomain}/auth/logout`, { method: 'POST', credentials: 'include' });
            if (isMainApp) {
              window.location.href = URLS.authIngress;
            } else {
              setStatus('idle');
            }
            return;
          }

          if (data.valid) {
            // Check if user is on waitlist
            if (data.waitlist_position) {
              if (isMainApp) {
                window.location.href = URLS.waitlist;
              } else {
                router.push('/waitlist');
              }
              return;
            }

            if (isMainApp) {
              // On main app, go to dashboard
              router.push('/dashboard');
            } else {
              // On ingress, fetch redirect URL and redirect there
              try {
                const configRes = await fetch(`/api/public/domain/${apiDomain}/config`);
                if (configRes.ok) {
                  const config = await configRes.json();
                  if (config.redirect_url) {
                    window.location.href = config.redirect_url;
                    return;
                  }
                }
              } catch {}
              // Fallback: show profile link
              router.push('/profile');
            }
            return;
          }
        }
      } catch {
        // Not authenticated
      }

      if (isMainApp) {
        // On main app, redirect to auth ingress for login
        window.location.href = URLS.authIngress;
      } else {
        // On auth ingress, show login form
        setStatus('idle');
      }
    };
    checkAuth();
  }, [router]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setStatus('loading');
    setErrorMessage('');

    const apiDomain = getRootDomain(window.location.hostname);

    try {
      const res = await fetch(`/api/public/domain/${apiDomain}/auth/request-magic-link`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email }),
        credentials: 'include',
      });

      if (res.ok) {
        setStatus('sent');
      } else if (res.status === 429) {
        setStatus('error');
        setErrorMessage('Too many requests. Please wait a moment and try again.');
      } else {
        setStatus('error');
        setErrorMessage('Something went wrong. Please try again.');
      }
    } catch {
      setStatus('error');
      setErrorMessage('Network error. Please check your connection.');
    }
  };

  const handleGoogleSignIn = async () => {
    setGoogleLoading(true);
    setErrorMessage('');

    const apiDomain = getRootDomain(window.location.hostname);

    try {
      const res = await fetch(`/api/public/domain/${apiDomain}/auth/google/start`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
      });

      if (res.ok) {
        const data = await res.json();
        window.location.href = data.auth_url;
      } else {
        const errData = await res.json().catch(() => ({}));
        setStatus('error');
        setErrorMessage(errData.message || 'Failed to start Google sign-in.');
        setGoogleLoading(false);
      }
    } catch {
      setStatus('error');
      setErrorMessage('Network error. Please check your connection.');
      setGoogleLoading(false);
    }
  };

  if (status === 'checking') {
    return (
      <main className="flex items-center justify-center">
        <div className="card text-center" style={{ maxWidth: '400px', width: '100%' }}>
          <div className="spinner" style={{ margin: '0 auto' }} />
        </div>
      </main>
    );
  }

  if (status === 'sent') {
    return (
      <main className="flex items-center justify-center">
        <div className="card text-center" style={{ maxWidth: '400px', width: '100%' }}>
          <div style={{ marginBottom: 'var(--spacing-lg)' }}>
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="var(--accent-blue)" strokeWidth="2" style={{ margin: '0 auto' }}>
              <path d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" />
            </svg>
          </div>
          <h2>Check your email</h2>
          <p>
            We sent a sign-in link to <code>{email}</code>
          </p>
          <p className="text-muted" style={{ fontSize: '13px' }}>
            Click the link in the email to sign in. The link expires in 15 minutes.
          </p>
          <button onClick={() => setStatus('idle')} style={{ marginTop: 'var(--spacing-md)' }}>
            Use a different email
          </button>
        </div>
      </main>
    );
  }

  // Check if no auth methods are available
  const noAuthMethods = !authMethods.magic_link && !authMethods.google_oauth;

  return (
    <main className="flex items-center justify-center">
      <div className="card" style={{ maxWidth: '400px', width: '100%' }}>
        <div className="text-center" style={{ marginBottom: 'var(--spacing-lg)' }}>
          <h2 style={{ marginBottom: 'var(--spacing-xs)', borderBottom: 'none', paddingBottom: 0 }}>
            {displayDomain || 'Sign In'}
          </h2>
          <p className="text-muted" style={{ fontSize: '13px', marginBottom: 0 }}>Sign in to your account</p>
        </div>

        {noAuthMethods && (
          <div className="message error">
            No login methods are configured for this domain.
          </div>
        )}

        {status === 'error' && (
          <div className="message error" style={{ marginBottom: 'var(--spacing-md)' }}>{errorMessage}</div>
        )}

        {/* Google Sign In */}
        {authMethods.google_oauth && (
          <button
            type="button"
            onClick={handleGoogleSignIn}
            disabled={googleLoading}
            style={{
              width: '100%',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              gap: 'var(--spacing-sm)',
              backgroundColor: 'var(--bg-tertiary)',
              border: '1px solid var(--border-primary)',
              marginBottom: authMethods.magic_link ? 'var(--spacing-md)' : 0,
            }}
          >
            {googleLoading ? (
              <span className="spinner" style={{ width: 18, height: 18 }} />
            ) : (
              <svg width="18" height="18" viewBox="0 0 24 24">
                <path
                  d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"
                  fill="#4285F4"
                />
                <path
                  d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
                  fill="#34A853"
                />
                <path
                  d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"
                  fill="#FBBC05"
                />
                <path
                  d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"
                  fill="#EA4335"
                />
              </svg>
            )}
            {googleLoading ? 'Connecting...' : 'Continue with Google'}
          </button>
        )}

        {/* Separator when both methods are available */}
        {authMethods.magic_link && authMethods.google_oauth && (
          <div style={{
            display: 'flex',
            alignItems: 'center',
            gap: 'var(--spacing-md)',
            marginBottom: 'var(--spacing-md)',
          }}>
            <div style={{ flex: 1, height: '1px', backgroundColor: 'var(--border-primary)' }} />
            <span className="text-muted" style={{ fontSize: '12px' }}>or</span>
            <div style={{ flex: 1, height: '1px', backgroundColor: 'var(--border-primary)' }} />
          </div>
        )}

        {/* Magic Link Form */}
        {authMethods.magic_link && (
          <form onSubmit={handleSubmit}>
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="you@example.com"
              required
              disabled={status === 'loading'}
            />

            <button
              type="submit"
              className="primary"
              disabled={status === 'loading' || !email}
              style={{ width: '100%' }}
            >
              {status === 'loading' ? 'Sending...' : 'Send magic link'}
            </button>
          </form>
        )}

        {authMethods.magic_link && (
          <p className="text-muted text-center" style={{ fontSize: '12px', marginTop: 'var(--spacing-lg)', marginBottom: 0 }}>
            No password needed. We&apos;ll email you a secure sign-in link.
          </p>
        )}
      </div>
    </main>
  );
}
