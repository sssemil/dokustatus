'use client';

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';

type WaitlistData = {
  email: string;
  position: number;
  total: number;
};

export default function WaitlistPage() {
  const [data, setData] = useState<WaitlistData | null>(null);
  const [status, setStatus] = useState<'loading' | 'authenticated' | 'unauthenticated'>('loading');
  const router = useRouter();

  useEffect(() => {
    const fetchData = async () => {
      try {
        const [meRes, waitlistRes] = await Promise.all([
          fetch('/api/user/me', { credentials: 'include' }),
          fetch('/api/user/waitlist', { credentials: 'include' }),
        ]);

        if (meRes.status === 401 || waitlistRes.status === 401) {
          setStatus('unauthenticated');
          router.push('/');
          return;
        }

        if (meRes.ok && waitlistRes.ok) {
          const me = await meRes.json();
          const waitlist = await waitlistRes.json();

          if (!me.on_waitlist) {
            router.push('/dashboard');
            return;
          }

          setData({
            email: me.email,
            position: waitlist.position,
            total: waitlist.total,
          });
          setStatus('authenticated');
        }
      } catch {
        setStatus('unauthenticated');
        router.push('/');
      }
    };

    fetchData();
  }, [router]);

  const handleLogout = async () => {
    await fetch('/api/auth/logout', {
      method: 'POST',
      credentials: 'include',
    });
    router.push('/');
  };

  if (status === 'loading' || !data) {
    return (
      <main className="flex items-center justify-center">
        <div className="card text-center" style={{ maxWidth: '440px', width: '100%' }}>
          <div className="spinner" style={{ margin: '0 auto' }} />
          <p className="text-muted mt-md">Loading...</p>
        </div>
      </main>
    );
  }

  const percentile = Math.round((1 - (data.position / data.total)) * 100);

  return (
    <main className="flex items-center justify-center">
      <div className="card text-center" style={{ maxWidth: '440px', width: '100%' }}>
        <h2 style={{ borderBottom: 'none', paddingBottom: 0, marginBottom: 'var(--spacing-xs)' }}>
          You&apos;re on the list!
        </h2>
        <p className="text-muted" style={{ fontSize: '13px' }}>{data.email}</p>

        <div style={{ margin: 'var(--spacing-xl) 0' }}>
          <div style={{ fontSize: '4rem', fontWeight: 800, color: 'var(--accent-blue)', lineHeight: 1 }}>
            #{data.position}
          </div>
          <p className="text-muted" style={{ marginBottom: 0 }}>in line</p>
        </div>

        <div className="flex justify-center gap-lg" style={{
          padding: 'var(--spacing-md)',
          backgroundColor: 'var(--bg-tertiary)',
          borderRadius: 'var(--radius-md)',
          marginBottom: 'var(--spacing-lg)'
        }}>
          <div className="text-center">
            <div style={{ fontSize: '1.25rem', fontWeight: 700 }}>{data.total}</div>
            <div className="text-muted" style={{ fontSize: '11px', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
              total signups
            </div>
          </div>
          <div style={{ width: '1px', backgroundColor: 'var(--border-primary)' }} />
          <div className="text-center">
            <div style={{ fontSize: '1.25rem', fontWeight: 700 }}>top {100 - percentile}%</div>
            <div className="text-muted" style={{ fontSize: '11px', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
              of waitlist
            </div>
          </div>
        </div>

        <div className="message info" style={{ textAlign: 'left', marginBottom: 'var(--spacing-lg)' }}>
          <p style={{ margin: 0, fontSize: '13px' }}>
            We&apos;re building something special. You&apos;ll be among the first to know when we launch.
          </p>
        </div>

        <button onClick={handleLogout}>
          Sign out
        </button>
      </div>

      <p className="text-muted text-center" style={{
        position: 'fixed',
        bottom: 'var(--spacing-lg)',
        left: 0,
        right: 0,
        fontSize: '12px'
      }}>
        reauth.dev — Auth, billing, email. One DNS setup.
      </p>
    </main>
  );
}
