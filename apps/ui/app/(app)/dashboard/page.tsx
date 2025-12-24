'use client';

import { useState, useEffect } from 'react';
import Link from 'next/link';

type UsageStats = {
  domains_count: number;
  total_users: number;
};

export default function DashboardPage() {
  const [stats, setStats] = useState<UsageStats | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const fetchStats = async () => {
      try {
        const res = await fetch('/api/domains/stats', { credentials: 'include' });
        if (res.ok) {
          const data = await res.json();
          setStats(data);
        }
      } catch {
        // Ignore
      } finally {
        setLoading(false);
      }
    };
    fetchStats();
  }, []);

  return (
    <>
      <h1>Dashboard</h1>

      {/* Usage Stats */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: 'var(--spacing-md)' }}>
        <div className="card" style={{ textAlign: 'center' }}>
          <div className="text-muted" style={{ fontSize: '13px', marginBottom: 'var(--spacing-xs)' }}>Domains</div>
          <div style={{ fontSize: '2rem', fontWeight: 700, color: 'var(--text-primary)' }}>
            {loading ? '—' : stats?.domains_count ?? 0}
          </div>
        </div>
        <div className="card" style={{ textAlign: 'center' }}>
          <div className="text-muted" style={{ fontSize: '13px', marginBottom: 'var(--spacing-xs)' }}>Total Users</div>
          <div style={{ fontSize: '2rem', fontWeight: 700, color: 'var(--text-primary)' }}>
            {loading ? '—' : stats?.total_users ?? 0}
          </div>
        </div>
      </div>

      {/* Quick Actions */}
      {stats && stats.domains_count === 0 && (
        <div className="card">
          <h2>Get Started</h2>
          <p className="text-muted" style={{ marginBottom: 'var(--spacing-md)' }}>
            Add your first domain to start using reauth.dev authentication.
          </p>
          <Link href="/domains">
            <button className="primary">+ Add Domain</button>
          </Link>
        </div>
      )}

      <div className="card">
        <h2>Quick Start</h2>
        <p className="text-muted">
          reauth.dev provides passwordless authentication for your SaaS:
        </p>
        <ul style={{ color: 'var(--text-secondary)', marginLeft: 'var(--spacing-lg)', marginBottom: 'var(--spacing-md)' }}>
          <li>Magic link email authentication</li>
          <li>User management dashboard</li>
          <li>Whitelist mode for controlled access</li>
        </ul>
        <p className="text-muted" style={{ marginBottom: 0 }}>
          All with a single DNS configuration.
        </p>
      </div>
    </>
  );
}
