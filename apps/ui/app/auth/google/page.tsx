'use client';

import { useEffect, useState } from 'react';
import { useSearchParams } from 'next/navigation';
import { Suspense } from 'react';
import { isMainApp as checkIsMainApp, getRootDomain } from '@/lib/domain-utils';
import { Button } from '@/components/ui';

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
    <main className="flex items-center justify-center min-h-screen">
      <div className="bg-zinc-900 rounded-lg p-8 border border-zinc-800 text-center max-w-[400px] w-full">
        {status === 'loading' && (
          <>
            <div className="spinner mx-auto mb-6" />
            <h2 className="text-xl font-semibold text-white">Connecting to Google...</h2>
            <p className="text-zinc-400 mt-2">Please wait a moment.</p>
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
              onClick={() => window.history.back()}
              variant="primary"
              className="mt-4"
            >
              Go back
            </Button>
          </>
        )}
      </div>
    </main>
  );
}

export default function GoogleAuthPage() {
  return (
    <Suspense fallback={
      <main className="flex items-center justify-center min-h-screen">
        <div className="bg-zinc-900 rounded-lg p-8 border border-zinc-800 text-center max-w-[400px] w-full">
          <div className="spinner mx-auto" />
          <h2 className="text-xl font-semibold text-white mt-6">Loading...</h2>
        </div>
      </main>
    }>
      <GoogleAuthHandler />
    </Suspense>
  );
}
