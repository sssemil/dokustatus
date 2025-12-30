'use client';

import { useEffect, useState } from 'react';
import { useSearchParams } from 'next/navigation';
import { Suspense } from 'react';
import { isMainApp as checkIsMainApp, getRootDomain } from '@/lib/domain-utils';

function GoogleAuthHandler() {
  const [status, setStatus] = useState<'loading' | 'error'>('loading');
  const [errorMessage, setErrorMessage] = useState('');
  const searchParams = useSearchParams();

  useEffect(() => {
    const startOAuth = async () => {
      const hostname = window.location.hostname;
      const isMainApp = checkIsMainApp(hostname);

      // Get domain from query param (if coming from main app) or from hostname
      let domain = searchParams.get('domain');
      if (!domain) {
        if (isMainApp) {
          setStatus('error');
          setErrorMessage('Missing domain parameter.');
          return;
        }
        domain = getRootDomain(hostname);
      }

      try {
        const res = await fetch(`/api/public/domain/${domain}/auth/google/start`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          credentials: 'include',
        });

        if (res.ok) {
          const data = await res.json();
          // Redirect to Google OAuth
          window.location.href = data.auth_url;
        } else {
          const errData = await res.json().catch(() => ({}));
          setStatus('error');
          if (errData.message?.includes('not enabled')) {
            setErrorMessage('Google OAuth is not enabled for this domain.');
          } else {
            setErrorMessage(errData.message || 'Failed to start Google sign-in.');
          }
        }
      } catch {
        setStatus('error');
        setErrorMessage('Network error. Please check your connection.');
      }
    };

    startOAuth();
  }, [searchParams]);

  return (
    <main className="flex items-center justify-center">
      <div className="card text-center" style={{ maxWidth: '400px', width: '100%' }}>
        {status === 'loading' && (
          <>
            <div className="spinner" style={{ margin: '0 auto', marginBottom: 'var(--spacing-lg)' }} />
            <h2 style={{ borderBottom: 'none', paddingBottom: 0 }}>Connecting to Google...</h2>
            <p className="text-muted">Please wait a moment.</p>
          </>
        )}

        {status === 'error' && (
          <>
            <div style={{ marginBottom: 'var(--spacing-lg)' }}>
              <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="var(--accent-red)" strokeWidth="2" style={{ margin: '0 auto' }}>
                <path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
            </div>
            <h2 style={{ borderBottom: 'none', paddingBottom: 0 }}>Something went wrong</h2>
            <p className="text-muted">{errorMessage}</p>
            <button
              onClick={() => window.history.back()}
              className="primary"
              style={{ marginTop: 'var(--spacing-md)' }}
            >
              Go back
            </button>
          </>
        )}
      </div>
    </main>
  );
}

export default function GoogleAuthPage() {
  return (
    <Suspense fallback={
      <main className="flex items-center justify-center">
        <div className="card text-center" style={{ maxWidth: '400px', width: '100%' }}>
          <div className="spinner" style={{ margin: '0 auto' }} />
          <h2 style={{ marginTop: 'var(--spacing-lg)', borderBottom: 'none', paddingBottom: 0 }}>Loading...</h2>
        </div>
      </main>
    }>
      <GoogleAuthHandler />
    </Suspense>
  );
}
