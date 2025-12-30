'use client';

import { useEffect, useState } from 'react';
import { useRouter, useSearchParams } from 'next/navigation';
import { Suspense } from 'react';
import { isMainApp as checkIsMainApp, getRootDomain } from '@/lib/domain-utils';

type Status = 'loading' | 'needs_link' | 'success' | 'error';

interface LinkConfirmationData {
  existingUserId: string;
  email: string;
  googleId: string;
  domain: string;
}

function GoogleCallbackHandler() {
  const [status, setStatus] = useState<Status>('loading');
  const [errorMessage, setErrorMessage] = useState('');
  const [linkData, setLinkData] = useState<LinkConfirmationData | null>(null);
  const [confirming, setConfirming] = useState(false);
  const router = useRouter();
  const searchParams = useSearchParams();

  useEffect(() => {
    const handleCallback = async () => {
      const code = searchParams.get('code');
      const state = searchParams.get('state');
      const error = searchParams.get('error');

      if (error) {
        setStatus('error');
        setErrorMessage(error === 'access_denied'
          ? 'Google sign-in was cancelled.'
          : `Google sign-in failed: ${error}`);
        return;
      }

      if (!code || !state) {
        setStatus('error');
        setErrorMessage('Invalid callback parameters.');
        return;
      }

      const hostname = window.location.hostname;
      const isMainApp = checkIsMainApp(hostname);
      const apiDomain = isMainApp ? getRootDomain(hostname) : getRootDomain(hostname);

      try {
        // Exchange code with backend
        const res = await fetch(`/api/public/domain/${apiDomain}/auth/google/exchange`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ code, state }),
          credentials: 'include',
        });

        if (res.ok) {
          const data = await res.json();

          if (data.status === 'logged_in') {
            // Redirect to completion URL to set cookies on the correct domain
            // The completion URL points to reauth.{domain}/google-complete?token=...
            setStatus('success');
            if (data.completion_url) {
              window.location.href = data.completion_url;
            } else {
              // Fallback (shouldn't happen with new API)
              setErrorMessage('Missing completion URL from server.');
              setStatus('error');
            }
          } else if (data.status === 'needs_link_confirmation') {
            setStatus('needs_link');
            setLinkData({
              existingUserId: data.existing_user_id,
              email: data.email,
              googleId: data.google_id,
              domain: data.domain,
            });
          }
        } else {
          const errData = await res.json().catch(() => ({}));
          setStatus('error');
          if (errData.message?.includes('not enabled')) {
            setErrorMessage('Google OAuth is not enabled for this domain.');
          } else if (errData.message?.includes('expired')) {
            setErrorMessage('The sign-in session has expired. Please try again.');
          } else {
            setErrorMessage(errData.message || 'Failed to complete Google sign-in.');
          }
        }
      } catch {
        setStatus('error');
        setErrorMessage('Network error. Please check your connection.');
      }
    };

    handleCallback();
  }, [searchParams, router]);

  const handleConfirmLink = async () => {
    if (!linkData) return;
    setConfirming(true);

    const hostname = window.location.hostname;
    const apiDomain = getRootDomain(hostname);

    try {
      const res = await fetch(`/api/public/domain/${apiDomain}/auth/google/confirm-link`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          existing_user_id: linkData.existingUserId,
          google_id: linkData.googleId,
          domain: linkData.domain, // Include the original domain for redirect
        }),
        credentials: 'include',
      });

      if (res.ok) {
        const data = await res.json();
        setStatus('success');
        // Redirect to completion URL to set cookies on the correct domain
        if (data.completion_url) {
          window.location.href = data.completion_url;
        } else {
          setErrorMessage('Missing completion URL from server.');
          setStatus('error');
        }
      } else {
        const errData = await res.json().catch(() => ({}));
        setStatus('error');
        setErrorMessage(errData.message || 'Failed to link Google account.');
      }
    } catch {
      setStatus('error');
      setErrorMessage('Network error. Please check your connection.');
    } finally {
      setConfirming(false);
    }
  };

  const handleCancelLink = () => {
    setStatus('error');
    setErrorMessage('Google account linking was cancelled. You can try a different sign-in method.');
  };

  return (
    <main className="flex items-center justify-center">
      <div className="card text-center" style={{ maxWidth: '450px', width: '100%' }}>
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

        {status === 'needs_link' && linkData && (
          <>
            <div style={{ marginBottom: 'var(--spacing-lg)' }}>
              <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="var(--accent-blue)" strokeWidth="2" style={{ margin: '0 auto' }}>
                <path d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" />
              </svg>
            </div>
            <h2 style={{ borderBottom: 'none', paddingBottom: 0 }}>Link your account?</h2>
            <p className="text-muted" style={{ marginBottom: 'var(--spacing-lg)' }}>
              An account with the email <strong>{linkData.email}</strong> already exists.
              Would you like to link your Google account to it?
            </p>
            <div style={{ display: 'flex', gap: 'var(--spacing-sm)', justifyContent: 'center' }}>
              <button
                onClick={handleCancelLink}
                disabled={confirming}
                style={{ minWidth: '100px' }}
              >
                Cancel
              </button>
              <button
                onClick={handleConfirmLink}
                disabled={confirming}
                className="primary"
                style={{ minWidth: '100px' }}
              >
                {confirming ? 'Linking...' : 'Link Account'}
              </button>
            </div>
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
              onClick={() => router.push('/')}
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

export default function GoogleCallbackPage() {
  return (
    <Suspense fallback={
      <main className="flex items-center justify-center">
        <div className="card text-center" style={{ maxWidth: '400px', width: '100%' }}>
          <div className="spinner" style={{ margin: '0 auto' }} />
          <h2 style={{ marginTop: 'var(--spacing-lg)', borderBottom: 'none', paddingBottom: 0 }}>Loading...</h2>
        </div>
      </main>
    }>
      <GoogleCallbackHandler />
    </Suspense>
  );
}
