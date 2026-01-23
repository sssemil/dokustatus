'use client';

import { useEffect, useState } from 'react';
import { useRouter, useSearchParams } from 'next/navigation';
import { Suspense } from 'react';
import { isMainApp as checkIsMainApp, getRootDomain } from '@/lib/domain-utils';
import { Button } from '@/components/ui';

type Status = 'loading' | 'needs_link' | 'success' | 'error';

interface LinkConfirmationData {
  // Token containing server-derived data (single-use, 5 min TTL)
  linkToken: string;
  // Email for UI display only
  email: string;
}

function GoogleCallbackHandler() {
  const [status, setStatus] = useState<Status>('loading');
  const [errorMessage, setErrorMessage] = useState('');
  const [linkData, setLinkData] = useState<LinkConfirmationData | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [retryExpired, setRetryExpired] = useState(false);
  const router = useRouter();
  const searchParams = useSearchParams();

  useEffect(() => {
    const handleCallback = async () => {
      setRetryExpired(false);
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
              linkToken: data.link_token,
              email: data.email,
            });
          }
        } else {
          const errData = await res.json().catch(() => ({}));
          setStatus('error');
          if (res.status === 410 || errData.code === 'OAUTH_RETRY_EXPIRED') {
            sessionStorage.removeItem('oauth_state');
            setRetryExpired(true);
            setErrorMessage('Your sign-in session expired. Please restart the login process.');
          } else if (errData.message?.includes('not enabled')) {
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
          link_token: linkData.linkToken,
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
    <main className="flex items-center justify-center min-h-screen">
      <div className="bg-zinc-900 rounded-lg p-8 border border-zinc-800 text-center max-w-[450px] w-full">
        {status === 'loading' && (
          <>
            <div className="spinner mx-auto mb-6" />
            <h2 className="text-xl font-semibold text-white">Signing you in...</h2>
            <p className="text-zinc-400 mt-2">Please wait a moment.</p>
          </>
        )}

        {status === 'success' && (
          <>
            <div className="mb-6 text-emerald-400">
              <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="mx-auto">
                <path d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            </div>
            <h2 className="text-xl font-semibold text-white">You&apos;re in!</h2>
            <p className="text-zinc-400 mt-2">Redirecting...</p>
          </>
        )}

        {status === 'needs_link' && linkData && (
          <>
            <div className="mb-6 text-blue-400">
              <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="mx-auto">
                <path d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" />
              </svg>
            </div>
            <h2 className="text-xl font-semibold text-white">Link your account?</h2>
            <p className="text-zinc-400 mt-2 mb-6">
              An account with the email <strong>{linkData.email}</strong> already exists.
              Would you like to link your Google account to it?
            </p>
            <div className="flex gap-2 justify-center">
              <Button
                onClick={handleCancelLink}
                disabled={confirming}
                variant="default"
              >
                Cancel
              </Button>
              <Button
                onClick={handleConfirmLink}
                disabled={confirming}
                variant="primary"
                loading={confirming}
              >
                Link Account
              </Button>
            </div>
          </>
        )}

        {status === 'error' && (
          <>
            <div className="mb-6 text-red-400">
              <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="mx-auto">
                <path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
            </div>
            <h2 className="text-xl font-semibold text-white">Something went wrong</h2>
            <p className="text-zinc-400 mt-2">{errorMessage}</p>
            <Button
              onClick={() => router.push('/')}
              variant="primary"
              className="mt-4"
            >
              {retryExpired ? 'Restart sign-in' : 'Try again'}
            </Button>
          </>
        )}
      </div>
    </main>
  );
}

export default function GoogleCallbackPage() {
  return (
    <Suspense fallback={
      <main className="flex items-center justify-center min-h-screen">
        <div className="bg-zinc-900 rounded-lg p-8 border border-zinc-800 text-center max-w-[400px] w-full">
          <div className="spinner mx-auto" />
          <h2 className="text-xl font-semibold text-white mt-6">Loading...</h2>
        </div>
      </main>
    }>
      <GoogleCallbackHandler />
    </Suspense>
  );
}
