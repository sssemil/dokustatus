'use client';

import { useEffect } from 'react';
import { useAppContext } from '../layout';

export default function DashboardPage() {
  const { isIngress } = useAppContext();

  useEffect(() => {
    if (isIngress) {
      window.location.href = '/profile';
    }
  }, [isIngress]);

  if (isIngress) {
    return (
      <div className="flex items-center justify-center" style={{ minHeight: '200px' }}>
        <div className="spinner" />
      </div>
    );
  }

  return (
    <>
      <h1>Dashboard</h1>

      <div className="card">
        <h2>Welcome back</h2>
        <p>Your reauth.dev dashboard is ready.</p>
      </div>

      <div className="card">
        <h2>Quick Start</h2>
        <p className="text-muted">
          reauth.dev is currently in development. Once launched, you&apos;ll be able to:
        </p>
        <ul style={{ color: 'var(--text-secondary)', marginLeft: 'var(--spacing-lg)', marginBottom: 'var(--spacing-md)' }}>
          <li>Set up passwordless authentication for your SaaS</li>
          <li>Manage billing and subscriptions</li>
          <li>Send transactional emails</li>
        </ul>
        <p className="text-muted" style={{ marginBottom: 0 }}>
          All with a single DNS configuration.
        </p>
      </div>
    </>
  );
}
