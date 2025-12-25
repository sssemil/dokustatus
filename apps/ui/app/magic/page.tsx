'use client';

import { useEffect, useState } from 'react';
import { useRouter, useSearchParams } from 'next/navigation';
import { Suspense } from 'react';

type ErrorType = 'expired' | 'suspended' | 'generic';

function MagicLinkHandler() {
  const [status, setStatus] = useState<'loading' | 'success' | 'error'>('loading');
  const [errorMessage, setErrorMessage] = useState('');
  const [errorType, setErrorType] = useState<ErrorType>('generic');
  const router = useRouter();
  const searchParams = useSearchParams();

  useEffect(() => {
    const token = searchParams.get('token');

    if (!token) {
      setStatus('error');
      setErrorMessage('Invalid or missing token.');
      return;
    }

    const consumeToken = async () => {
      const hostname = window.location.hostname;
      const isMainApp = hostname === 'reauth.dev' || hostname === 'www.reauth.dev';

      try {
        const res = await fetch(`/api/public/domain/${hostname}/auth/verify-magic-link`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ token }),
          credentials: 'include',
        });

        if (res.ok) {
          const data = await res.json();
          setStatus('success');
          setTimeout(() => {
            // Check if user is on waitlist
            if (data.waitlist_position) {
              if (isMainApp) {
                window.location.href = 'https://reauth.reauth.dev/waitlist';
              } else {
                router.push('/waitlist');
              }
            } else if (isMainApp) {
              router.push('/dashboard');
            } else if (data.redirect_url) {
              window.location.href = data.redirect_url;
            } else {
              router.push('/profile');
            }
          }, 1000);
        } else if (res.status === 401) {
          setStatus('error');
          setErrorType('expired');
          setErrorMessage('This link has expired or already been used. Please request a new one.');
        } else {
          // Try to get error message from response
          try {
            const errorData = await res.json();
            setStatus('error');
            // Check if this is a suspended account error
            if (errorData.code === 'ACCOUNT_SUSPENDED' || errorData.error_code === 'ACCOUNT_SUSPENDED' || errorData.message?.toLowerCase().includes('suspended')) {
              setErrorType('suspended');
              setErrorMessage('Your account has been suspended. Please contact support if you believe this is an error.');
            } else {
              setErrorType('generic');
              setErrorMessage(errorData.message || 'Something went wrong. Please try again.');
            }
          } catch {
            setStatus('error');
            setErrorType('generic');
            setErrorMessage('Something went wrong. Please try again.');
          }
        }
      } catch {
        setStatus('error');
        setErrorMessage('Network error. Please check your connection.');
      }
    };

    consumeToken();
  }, [searchParams, router]);

  return (
    <main className="flex items-center justify-center">
      <div className="card text-center" style={{ maxWidth: '400px', width: '100%' }}>
        {status === 'loading' && (
          <>
            <div className="spinner" style={{ margin: '0 auto', marginBottom: 'var(--spacing-lg)' }} />
            <h2 style={{ borderBottom: 'none', paddingBottom: 0 }}>Signing you in...</h2>
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
              {errorType === 'suspended' ? (
                <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="var(--accent-red)" strokeWidth="2" style={{ margin: '0 auto' }}>
                  <path d="M18.364 18.364A9 9 0 005.636 5.636m12.728 12.728A9 9 0 015.636 5.636m12.728 12.728L5.636 5.636" />
                </svg>
              ) : (
                <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="var(--accent-red)" strokeWidth="2" style={{ margin: '0 auto' }}>
                  <path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                </svg>
              )}
            </div>
            <h2 style={{ borderBottom: 'none', paddingBottom: 0 }}>
              {errorType === 'suspended' ? 'Account suspended' : errorType === 'expired' ? 'Link expired' : 'Something went wrong'}
            </h2>
            <p className="text-muted">{errorMessage}</p>
            {errorType !== 'suspended' && (
              <button
                onClick={() => router.push('/')}
                className="primary"
                style={{ marginTop: 'var(--spacing-md)' }}
              >
                Try again
              </button>
            )}
          </>
        )}
      </div>
    </main>
  );
}

export default function MagicPage() {
  return (
    <Suspense fallback={
      <main className="flex items-center justify-center">
        <div className="card text-center" style={{ maxWidth: '400px', width: '100%' }}>
          <div className="spinner" style={{ margin: '0 auto' }} />
          <h2 style={{ marginTop: 'var(--spacing-lg)', borderBottom: 'none', paddingBottom: 0 }}>Loading...</h2>
        </div>
      </main>
    }>
      <MagicLinkHandler />
    </Suspense>
  );
}
