'use client';

import { useEffect, useState } from 'react';
import { useRouter, useSearchParams } from 'next/navigation';
import { Suspense } from 'react';
import { isMainApp as checkIsMainApp, getRootDomain, URLS } from '@/lib/domain-utils';
import { Button } from '@/components/ui';

type ErrorType = 'expired' | 'suspended' | 'session_mismatch' | 'generic';

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
      const isMainApp = checkIsMainApp(hostname);
      const apiDomain = getRootDomain(hostname);

      try {
        const res = await fetch(`/api/public/domain/${apiDomain}/auth/verify-magic-link`, {
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
                window.location.href = URLS.waitlist;
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
          // Check if this is a session mismatch error
          try {
            const errorData = await res.json();
            setStatus('error');
            if (errorData.code === 'SESSION_MISMATCH') {
              setErrorType('session_mismatch');
              setErrorMessage('Please use the same browser or device where you requested the login link.');
            } else {
              setErrorType('expired');
              setErrorMessage('This link has expired or already been used. Please request a new one.');
            }
          } catch {
            setStatus('error');
            setErrorType('expired');
            setErrorMessage('This link has expired or already been used. Please request a new one.');
          }
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
    <main className="flex items-center justify-center min-h-screen">
      <div className="bg-zinc-900 rounded-lg p-8 border border-zinc-800 text-center max-w-[400px] w-full">
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

        {status === 'error' && (
          <>
            <div className="mb-6 text-red-400">
              {errorType === 'suspended' ? (
                <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="mx-auto">
                  <path d="M18.364 18.364A9 9 0 005.636 5.636m12.728 12.728A9 9 0 015.636 5.636m12.728 12.728L5.636 5.636" />
                </svg>
              ) : (
                <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="mx-auto">
                  <path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                </svg>
              )}
            </div>
            <h2 className="text-xl font-semibold text-white">
              {errorType === 'suspended' ? 'Account suspended' : errorType === 'expired' ? 'Link expired' : errorType === 'session_mismatch' ? 'Wrong browser' : 'Something went wrong'}
            </h2>
            <p className="text-zinc-400 mt-2">{errorMessage}</p>
            {errorType !== 'suspended' && (
              <Button
                onClick={() => router.push('/')}
                variant="primary"
                className="mt-4"
              >
                Try again
              </Button>
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
      <main className="flex items-center justify-center min-h-screen">
        <div className="bg-zinc-900 rounded-lg p-8 border border-zinc-800 text-center max-w-[400px] w-full">
          <div className="spinner mx-auto" />
          <h2 className="text-xl font-semibold text-white mt-6">Loading...</h2>
        </div>
      </main>
    }>
      <MagicLinkHandler />
    </Suspense>
  );
}
