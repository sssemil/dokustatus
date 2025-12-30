'use client';

import { useEffect, useState } from 'react';
import { useSearchParams } from 'next/navigation';
import { Suspense } from 'react';
import { getRootDomain } from '@/lib/domain-utils';

type Status = 'loading' | 'success' | 'error';

function GoogleCompleteHandler() {
  const [status, setStatus] = useState<Status>('loading');
  const [errorMessage, setErrorMessage] = useState('');
  const searchParams = useSearchParams();

  useEffect(() => {
    const handleComplete = async () => {
      const token = searchParams.get('token');

      if (!token) {
        setStatus('error');
        setErrorMessage('Missing completion token.');
        return;
      }

      const hostname = window.location.hostname;
      const apiDomain = getRootDomain(hostname);

      try {
        // Call the complete endpoint to set cookies on this domain
        const res = await fetch(`/api/public/domain/${apiDomain}/auth/google/complete`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ token }),
          credentials: 'include',
        });

        if (res.ok) {
          const data = await res.json();
          setStatus('success');
          // Redirect to the configured redirect URL
          setTimeout(() => {
            if (data.redirect_url) {
              window.location.href = data.redirect_url;
            } else {
              // Fallback to domain root
              window.location.href = `https://${apiDomain}`;
            }
          }, 1000);
        } else {
          const errData = await res.json().catch(() => ({}));
          setStatus('error');
          if (errData.message?.includes('expired')) {
            setErrorMessage('The sign-in session has expired. Please try again.');
          } else if (errData.message?.includes('mismatch')) {
            setErrorMessage('Security error: domain mismatch. Please try again.');
          } else {
            setErrorMessage(errData.message || 'Failed to complete sign-in.');
          }
        }
      } catch {
        setStatus('error');
        setErrorMessage('Network error. Please check your connection.');
      }
    };

    handleComplete();
  }, [searchParams]);

  return (
    <main className="flex items-center justify-center">
      <div className="card text-center" style={{ maxWidth: '450px', width: '100%' }}>
        {status === 'loading' && (
          <>
            <div className="spinner" style={{ margin: '0 auto', marginBottom: 'var(--spacing-lg)' }} />
            <h2 style={{ borderBottom: 'none', paddingBottom: 0 }}>Completing sign-in...</h2>
            <p className="text-muted">Please wait a moment.</p>
          </>
        )}

        {status === 'success' && (
          <>
            <div style={{ marginBottom: 'var(--spacing-lg)' }}>
              <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="var(--accent-green)" strokeWidth="2" style={{ margin: '0 auto' }}>
                <path d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            </div>
            <h2 style={{ borderBottom: 'none', paddingBottom: 0 }}>You&apos;re in!</h2>
            <p className="text-muted">Redirecting...</p>
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
              onClick={() => window.location.href = '/'}
              className="primary"
              style={{ marginTop: 'var(--spacing-md)' }}
            >
              Try again
            </button>
          </>
        )}
      </div>
    </main>
  );
}

export default function GoogleCompletePage() {
  return (
    <Suspense fallback={
      <main className="flex items-center justify-center">
        <div className="card text-center" style={{ maxWidth: '400px', width: '100%' }}>
          <div className="spinner" style={{ margin: '0 auto' }} />
          <h2 style={{ marginTop: 'var(--spacing-lg)', borderBottom: 'none', paddingBottom: 0 }}>Loading...</h2>
        </div>
      </main>
    }>
      <GoogleCompleteHandler />
    </Suspense>
  );
}
