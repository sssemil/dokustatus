'use client';

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { Suspense } from 'react';
import { isMainApp as checkIsMainApp, getRootDomain } from '@/lib/domain-utils';

function WaitlistContent() {
  const router = useRouter();
  const [status, setStatus] = useState<'loading' | 'waitlist' | 'error'>('loading');
  const [position, setPosition] = useState<number | null>(null);
  const [email, setEmail] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState('');

  useEffect(() => {
    const checkStatus = async () => {
      const hostname = window.location.hostname;
      const isMainApp = checkIsMainApp(hostname);
      const apiDomain = getRootDomain(hostname);

      try {
        const res = await fetch(`/api/public/domain/${apiDomain}/auth/session`, {
          credentials: 'include',
        });

        if (!res.ok) {
          // Not authenticated
          router.push('/');
          return;
        }

        const data = await res.json();

        // Check for error (e.g., account suspended)
        if (data.error) {
          setStatus('error');
          setErrorMessage(data.error);
          // Log them out
          await fetch(`/api/public/domain/${apiDomain}/auth/logout`, {
            method: 'POST',
            credentials: 'include',
          });
          return;
        }

        if (!data.valid) {
          router.push('/');
          return;
        }

        // Check if user is still on waitlist
        if (data.waitlist_position) {
          setPosition(data.waitlist_position);
          setEmail(data.email || null);
          setStatus('waitlist');
        } else {
          // User is no longer on waitlist - they got approved!
          if (isMainApp) {
            router.push('/dashboard');
          } else {
            // Fetch redirect URL for custom domains
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
            router.push('/profile');
          }
        }
      } catch {
        router.push('/');
      }
    };

    checkStatus();
  }, [router]);

  const handleLogout = async () => {
    const apiDomain = getRootDomain(window.location.hostname);
    await fetch(`/api/public/domain/${apiDomain}/auth/logout`, {
      method: 'POST',
      credentials: 'include',
    });
    window.location.href = '/';
  };

  if (status === 'loading') {
    return (
      <main className="flex items-center justify-center">
        <div className="card text-center" style={{ maxWidth: '400px', width: '100%' }}>
          <div className="spinner" style={{ margin: '0 auto' }} />
          <h2 style={{ marginTop: 'var(--spacing-lg)', borderBottom: 'none', paddingBottom: 0 }}>
            Checking status...
          </h2>
        </div>
      </main>
    );
  }

  if (status === 'error') {
    return (
      <main className="flex items-center justify-center">
        <div className="card text-center" style={{ maxWidth: '450px', width: '100%' }}>
          <div style={{ marginBottom: 'var(--spacing-lg)' }}>
            <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="var(--accent-red)" strokeWidth="1.5" style={{ margin: '0 auto' }}>
              <path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
          </div>

          <h2 style={{ borderBottom: 'none', paddingBottom: 0, marginBottom: 'var(--spacing-sm)' }}>
            Account Suspended
          </h2>

          <p className="text-muted" style={{ marginBottom: 'var(--spacing-lg)', fontSize: '14px' }}>
            {errorMessage || 'Your account has been suspended.'}
          </p>

          <button onClick={() => window.location.href = '/'} className="primary" style={{ width: '100%' }}>
            Go to login
          </button>
        </div>
      </main>
    );
  }

  return (
    <main className="flex items-center justify-center">
      <div className="card text-center" style={{ maxWidth: '450px', width: '100%' }}>
        <div style={{ marginBottom: 'var(--spacing-lg)' }}>
          <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="var(--accent-blue)" strokeWidth="1.5" style={{ margin: '0 auto' }}>
            <path d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        </div>

        <h2 style={{ borderBottom: 'none', paddingBottom: 0, marginBottom: 'var(--spacing-sm)' }}>
          You&apos;re on the waitlist!
        </h2>

        {email && (
          <p className="text-muted" style={{ marginBottom: 'var(--spacing-lg)', fontSize: '14px' }}>
            {email}
          </p>
        )}

        {position && (
          <div style={{
            backgroundColor: 'var(--bg-tertiary)',
            borderRadius: 'var(--radius-md)',
            padding: 'var(--spacing-lg)',
            marginBottom: 'var(--spacing-lg)',
          }}>
            <div className="text-muted" style={{ fontSize: '13px', marginBottom: 'var(--spacing-xs)' }}>
              Your position
            </div>
            <div style={{ fontSize: '3rem', fontWeight: 700, color: 'var(--accent-blue)' }}>
              #{position}
            </div>
          </div>
        )}

        <p className="text-muted" style={{ fontSize: '14px', marginBottom: 'var(--spacing-lg)' }}>
          We&apos;ll notify you when your account is approved. Thank you for your patience!
        </p>

        <button onClick={handleLogout} style={{ width: '100%' }}>
          Sign out
        </button>
      </div>
    </main>
  );
}

export default function WaitlistPage() {
  return (
    <Suspense fallback={
      <main className="flex items-center justify-center">
        <div className="card text-center" style={{ maxWidth: '400px', width: '100%' }}>
          <div className="spinner" style={{ margin: '0 auto' }} />
          <h2 style={{ marginTop: 'var(--spacing-lg)', borderBottom: 'none', paddingBottom: 0 }}>Loading...</h2>
        </div>
      </main>
    }>
      <WaitlistContent />
    </Suspense>
  );
}
