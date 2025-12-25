'use client';

import { useEffect, useState } from 'react';
import { useSearchParams } from 'next/navigation';
import { Suspense } from 'react';

function WaitlistContent() {
  const searchParams = useSearchParams();
  const [position, setPosition] = useState<number | null>(null);
  const [email, setEmail] = useState<string | null>(null);

  useEffect(() => {
    const pos = searchParams.get('position');
    if (pos) {
      setPosition(parseInt(pos, 10));
    }

    // Try to get email from cookie
    const emailCookie = document.cookie
      .split('; ')
      .find(row => row.startsWith('end_user_email='));
    if (emailCookie) {
      setEmail(decodeURIComponent(emailCookie.split('=')[1]));
    }
  }, [searchParams]);

  const handleLogout = async () => {
    const hostname = window.location.hostname;
    await fetch(`/api/public/domain/${hostname}/auth/logout`, {
      method: 'POST',
      credentials: 'include',
    });
    window.location.href = '/';
  };

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
