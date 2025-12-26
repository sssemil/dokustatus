'use client';

import { useState, useEffect } from 'react';
import Link from 'next/link';

type UsageStats = {
  domains_count: number;
  total_users: number;
};

type Domain = {
  id: string;
  domain: string;
  status: string;
  has_auth_methods: boolean;
};

export default function DashboardPage() {
  const [stats, setStats] = useState<UsageStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [domainsWithoutAuth, setDomainsWithoutAuth] = useState<Domain[]>([]);

  useEffect(() => {
    const fetchData = async () => {
      try {
        // Fetch stats
        const statsRes = await fetch('/api/domains/stats', { credentials: 'include' });
        if (statsRes.ok) {
          const data = await statsRes.json();
          setStats(data);
        }

        // Fetch domains to check for auth methods
        const domainsRes = await fetch('/api/domains', { credentials: 'include' });
        if (domainsRes.ok) {
          const domains: Domain[] = await domainsRes.json();
          const withoutAuth = domains.filter(d => d.status === 'verified' && !d.has_auth_methods);
          setDomainsWithoutAuth(withoutAuth);
        }
      } catch {
        // Ignore
      } finally {
        setLoading(false);
      }
    };
    fetchData();
  }, []);

  return (
    <>
      <h1>Dashboard</h1>

      {/* No Auth Methods Warning */}
      {domainsWithoutAuth.length > 0 && (
        <div className="message warning" style={{ marginBottom: 'var(--spacing-md)' }}>
          {domainsWithoutAuth.length === 1 ? (
            <>
              Domain <strong>{domainsWithoutAuth[0].domain}</strong> has no login methods enabled.{' '}
              <Link href={`/domains/${domainsWithoutAuth[0].id}`} style={{ color: 'inherit', fontWeight: 500 }}>
                Configure now
              </Link>
            </>
          ) : (
            <>
              {domainsWithoutAuth.length} domains have no login methods enabled.{' '}
              <Link href="/domains" style={{ color: 'inherit', fontWeight: 500 }}>
                Configure now
              </Link>
            </>
          )}
        </div>
      )}

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
