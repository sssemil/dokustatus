'use client';

import { useState, useEffect } from 'react';
import { useRouter } from 'next/navigation';

export default function Home() {
  const [email, setEmail] = useState('');
  const [status, setStatus] = useState<'checking' | 'idle' | 'loading' | 'sent' | 'error'>('checking');
  const [errorMessage, setErrorMessage] = useState('');
  const [displayDomain, setDisplayDomain] = useState('');
  const router = useRouter();

  useEffect(() => {
    const hostname = window.location.hostname;

    // Determine if we're on an auth ingress (reauth.* subdomain) or main app (reauth.dev)
    const isMainApp = hostname === 'reauth.dev' || hostname === 'www.reauth.dev';
    const isLocalhost = hostname === 'localhost';

    // For API calls, use the current hostname (ingress handles its own domain)
    const apiDomain = hostname;

    // Display domain: strip "reauth." prefix for ingress subdomains
    const rootDomain = hostname.startsWith('reauth.') && hostname !== 'reauth.dev'
      ? hostname.slice(7)
      : hostname;
    setDisplayDomain(rootDomain);

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
              window.location.href = 'https://reauth.reauth.dev/';
            } else {
              setStatus('idle');
            }
            return;
          }

          if (data.valid) {
            // Check if user is on waitlist
            if (data.waitlist_position) {
              router.push('/waitlist');
              return;
            }

            if (isMainApp) {
              // On reauth.dev, go to dashboard
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
        // On reauth.dev, redirect to auth ingress for login
        window.location.href = 'https://reauth.reauth.dev/';
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

    const hostname = window.location.hostname;

    try {
      const res = await fetch(`/api/public/domain/${hostname}/auth/request-magic-link`, {
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

  return (
    <main className="flex items-center justify-center">
      <div className="card" style={{ maxWidth: '400px', width: '100%' }}>
        <div className="text-center" style={{ marginBottom: 'var(--spacing-lg)' }}>
          <h2 style={{ marginBottom: 'var(--spacing-xs)', borderBottom: 'none', paddingBottom: 0 }}>
            {displayDomain || 'Sign In'}
          </h2>
          <p className="text-muted" style={{ fontSize: '13px', marginBottom: 0 }}>Sign in to your account</p>
        </div>

        <form onSubmit={handleSubmit}>
          <input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="you@example.com"
            required
            disabled={status === 'loading'}
          />

          {status === 'error' && (
            <div className="message error">{errorMessage}</div>
          )}

          <button
            type="submit"
            className="primary"
            disabled={status === 'loading' || !email}
            style={{ width: '100%' }}
          >
            {status === 'loading' ? 'Sending...' : 'Send magic link'}
          </button>
        </form>

        <p className="text-muted text-center" style={{ fontSize: '12px', marginTop: 'var(--spacing-lg)', marginBottom: 0 }}>
          No password needed. We&apos;ll email you a secure sign-in link.
        </p>
      </div>
    </main>
  );
}
