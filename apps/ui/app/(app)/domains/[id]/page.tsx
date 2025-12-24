'use client';

import { useState, useEffect, useCallback } from 'react';
import { useParams, useRouter } from 'next/navigation';
import { useAppContext } from '../../layout';

type Domain = {
  id: string;
  domain: string;
  status: 'pending_dns' | 'verifying' | 'verified' | 'failed';
  dns_records?: {
    cname_name: string;
    cname_value: string;
    txt_name: string;
    txt_value: string;
  };
  verified_at: string | null;
  created_at: string | null;
};

type AuthConfig = {
  magic_link_enabled: boolean;
  google_oauth_enabled: boolean;
  redirect_url: string | null;
  magic_link_config: {
    from_email: string;
    has_api_key: boolean;
  } | null;
};

export default function DomainDetailPage() {
  const params = useParams();
  const router = useRouter();
  const { isIngress } = useAppContext();
  const domainId = params.id as string;

  useEffect(() => {
    if (isIngress) {
      window.location.href = '/profile';
    }
  }, [isIngress]);

  const [domain, setDomain] = useState<Domain | null>(null);
  const [authConfig, setAuthConfig] = useState<AuthConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');
  const [copiedField, setCopiedField] = useState<string | null>(null);

  // Auth config form state
  const [magicLinkEnabled, setMagicLinkEnabled] = useState(false);
  const [resendApiKey, setResendApiKey] = useState('');
  const [fromEmail, setFromEmail] = useState('');
  const [redirectUrl, setRedirectUrl] = useState('');

  const fetchData = useCallback(async () => {
    try {
      const domainRes = await fetch(`/api/domains/${domainId}`, { credentials: 'include' });

      if (domainRes.ok) {
        const domainData = await domainRes.json();
        setDomain(domainData);

        // Only fetch auth config if domain is verified
        if (domainData.status === 'verified') {
          const configRes = await fetch(`/api/domains/${domainId}/auth-config`, { credentials: 'include' });
          if (configRes.ok) {
            const configData = await configRes.json();
            setAuthConfig(configData);
            setMagicLinkEnabled(configData.magic_link_enabled);
            setRedirectUrl(configData.redirect_url || '');
            if (configData.magic_link_config) {
              setFromEmail(configData.magic_link_config.from_email);
            }
          }
        }
      }
    } catch {
      setError('Failed to load domain');
    } finally {
      setLoading(false);
    }
  }, [domainId]);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  // Poll for verification status when domain is verifying
  useEffect(() => {
    if (!domain || (domain.status !== 'verifying' && domain.status !== 'pending_dns')) return;

    const interval = setInterval(async () => {
      try {
        const res = await fetch(`/api/domains/${domainId}/status`, { credentials: 'include' });
        if (res.ok) {
          const data = await res.json();
          if (data.status !== domain.status) {
            fetchData(); // Refetch all data when status changes
          }
        }
      } catch {
        // Continue polling
      }
    }, 5000);

    return () => clearInterval(interval);
  }, [domain, domainId, fetchData]);

  const handleStartVerification = async () => {
    try {
      const res = await fetch(`/api/domains/${domainId}/verify`, {
        method: 'POST',
        credentials: 'include',
      });

      if (res.ok) {
        fetchData();
      } else {
        setError('Failed to start verification');
      }
    } catch {
      setError('Network error. Please try again.');
    }
  };

  const handleSaveConfig = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setSuccess('');
    setSaving(true);

    try {
      const payload: Record<string, unknown> = {
        magic_link_enabled: magicLinkEnabled,
        google_oauth_enabled: false,
        redirect_url: redirectUrl || null,
      };

      if (magicLinkEnabled) {
        if (resendApiKey) {
          payload.resend_api_key = resendApiKey;
        }
        if (fromEmail) {
          payload.from_email = fromEmail;
        }
      }

      const res = await fetch(`/api/domains/${domainId}/auth-config`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
        credentials: 'include',
      });

      if (res.ok) {
        setSuccess('Configuration saved successfully');
        setResendApiKey('');
        fetchData();
      } else {
        const errData = await res.json().catch(() => ({}));
        setError(errData.message || 'Failed to save configuration');
      }
    } catch {
      setError('Network error. Please try again.');
    } finally {
      setSaving(false);
    }
  };

  const handleDeleteDomain = async () => {
    if (!confirm('Are you sure you want to delete this domain? This cannot be undone.')) return;

    try {
      const res = await fetch(`/api/domains/${domainId}`, {
        method: 'DELETE',
        credentials: 'include',
      });

      if (res.ok) {
        router.push('/domains');
      } else {
        setError('Failed to delete domain');
      }
    } catch {
      setError('Network error. Please try again.');
    }
  };

  const copyToClipboard = async (text: string, field: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedField(field);
      setTimeout(() => setCopiedField(null), 2000);
    } catch {
      const textArea = document.createElement('textarea');
      textArea.value = text;
      document.body.appendChild(textArea);
      textArea.select();
      document.execCommand('copy');
      document.body.removeChild(textArea);
      setCopiedField(field);
      setTimeout(() => setCopiedField(null), 2000);
    }
  };

  const getStatusBadge = (status: Domain['status']) => {
    const styles: Record<Domain['status'], { bg: string; color: string; label: string }> = {
      pending_dns: { bg: 'var(--accent-orange)', color: '#000', label: 'Pending DNS' },
      verifying: { bg: 'var(--accent-blue)', color: '#000', label: 'Verifying...' },
      verified: { bg: 'var(--accent-green)', color: '#000', label: 'Verified' },
      failed: { bg: 'var(--accent-red)', color: '#fff', label: 'Failed' },
    };
    const style = styles[status];
    return (
      <span
        style={{
          display: 'inline-flex',
          alignItems: 'center',
          gap: 'var(--spacing-xs)',
          padding: '4px 8px',
          borderRadius: 'var(--radius-sm)',
          backgroundColor: style.bg,
          color: style.color,
          fontSize: '12px',
          fontWeight: 500,
        }}
      >
        {status === 'verifying' && <span className="spinner" style={{ width: 12, height: 12 }} />}
        {style.label}
      </span>
    );
  };

  const formatDate = (dateString: string | null) => {
    if (!dateString) return '';
    const date = new Date(dateString);
    return date.toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    });
  };

  if (isIngress || loading) {
    return (
      <div style={{ display: 'flex', justifyContent: 'center', padding: 'var(--spacing-xl)' }}>
        <span className="spinner" />
      </div>
    );
  }

  if (!domain) {
    return (
      <div className="card">
        <p className="text-muted">Domain not found</p>
        <button onClick={() => router.push('/domains')}>Back to domains</button>
      </div>
    );
  }

  return (
    <>
      {/* Header */}
      <button
        onClick={() => router.push('/domains')}
        style={{
          background: 'none',
          border: 'none',
          color: 'var(--text-muted)',
          cursor: 'pointer',
          padding: 0,
          marginBottom: 'var(--spacing-md)',
        }}
      >
        &larr; Back to domains
      </button>

      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: 'var(--spacing-lg)' }}>
        <div>
          <h1 style={{ marginBottom: 'var(--spacing-xs)' }}>{domain.domain}</h1>
          <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--spacing-md)' }}>
            {domain.created_at && (
              <span className="text-muted" style={{ fontSize: '14px' }}>
                Created {formatDate(domain.created_at)}
              </span>
            )}
            {getStatusBadge(domain.status)}
          </div>
        </div>
        <button className="danger" onClick={handleDeleteDomain}>
          Delete
        </button>
      </div>

      {/* Verifying Banner */}
      {domain.status === 'verifying' && (
        <div
          className="card"
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 'var(--spacing-md)',
            backgroundColor: 'var(--bg-tertiary)',
            marginBottom: 'var(--spacing-lg)',
          }}
        >
          <div className="spinner" />
          <div>
            <div style={{ fontWeight: 600 }}>Looking for DNS records...</div>
            <p className="text-muted" style={{ margin: 0, fontSize: '14px' }}>
              May take a few minutes or hours depending on your DNS provider.
            </p>
          </div>
        </div>
      )}

      {/* DNS Records Section */}
      {domain.dns_records && (
        <div className="card" style={{ marginBottom: 'var(--spacing-lg)' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: 'var(--spacing-md)' }}>
            <h2 style={{ margin: 0 }}>DNS Records</h2>
            <a
              href="https://resend.com/docs/knowledge-base/godaddy"
              target="_blank"
              rel="noopener noreferrer"
              style={{ color: 'var(--accent-blue)', fontSize: '14px' }}
            >
              How to add records
            </a>
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-md)' }}>
            {/* CNAME Record */}
            <div
              style={{
                backgroundColor: 'var(--bg-tertiary)',
                borderRadius: 'var(--radius-md)',
                padding: 'var(--spacing-md)',
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 'var(--spacing-sm)' }}>
                <span style={{ fontWeight: 600, color: 'var(--text-primary)' }}>CNAME Record</span>
                {domain.status === 'verified' && (
                  <span style={{ color: 'var(--accent-green)', fontSize: '12px' }}>Verified</span>
                )}
              </div>
              <div style={{ display: 'grid', gridTemplateColumns: '80px 1fr auto', gap: 'var(--spacing-sm)', alignItems: 'center' }}>
                <span className="text-muted">Name</span>
                <code style={{ backgroundColor: 'var(--bg-secondary)', padding: '4px 8px', borderRadius: '4px', fontSize: '13px' }}>
                  {domain.dns_records.cname_name}
                </code>
                <button
                  onClick={() => copyToClipboard(domain.dns_records!.cname_name, 'cname_name')}
                  style={{ padding: '4px 8px', fontSize: '12px' }}
                >
                  {copiedField === 'cname_name' ? 'Copied!' : 'Copy'}
                </button>

                <span className="text-muted">Value</span>
                <code style={{ backgroundColor: 'var(--bg-secondary)', padding: '4px 8px', borderRadius: '4px', fontSize: '13px' }}>
                  {domain.dns_records.cname_value}
                </code>
                <button
                  onClick={() => copyToClipboard(domain.dns_records!.cname_value, 'cname_value')}
                  style={{ padding: '4px 8px', fontSize: '12px' }}
                >
                  {copiedField === 'cname_value' ? 'Copied!' : 'Copy'}
                </button>
              </div>
            </div>

            {/* TXT Record */}
            <div
              style={{
                backgroundColor: 'var(--bg-tertiary)',
                borderRadius: 'var(--radius-md)',
                padding: 'var(--spacing-md)',
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 'var(--spacing-sm)' }}>
                <span style={{ fontWeight: 600, color: 'var(--text-primary)' }}>TXT Record</span>
                {domain.status === 'verified' && (
                  <span style={{ color: 'var(--accent-green)', fontSize: '12px' }}>Verified</span>
                )}
              </div>
              <div style={{ display: 'grid', gridTemplateColumns: '80px 1fr auto', gap: 'var(--spacing-sm)', alignItems: 'center' }}>
                <span className="text-muted">Name</span>
                <code style={{ backgroundColor: 'var(--bg-secondary)', padding: '4px 8px', borderRadius: '4px', fontSize: '13px' }}>
                  {domain.dns_records.txt_name}
                </code>
                <button
                  onClick={() => copyToClipboard(domain.dns_records!.txt_name, 'txt_name')}
                  style={{ padding: '4px 8px', fontSize: '12px' }}
                >
                  {copiedField === 'txt_name' ? 'Copied!' : 'Copy'}
                </button>

                <span className="text-muted">Value</span>
                <code style={{ backgroundColor: 'var(--bg-secondary)', padding: '4px 8px', borderRadius: '4px', fontSize: '13px' }}>
                  {domain.dns_records.txt_value}
                </code>
                <button
                  onClick={() => copyToClipboard(domain.dns_records!.txt_value, 'txt_value')}
                  style={{ padding: '4px 8px', fontSize: '12px' }}
                >
                  {copiedField === 'txt_value' ? 'Copied!' : 'Copy'}
                </button>
              </div>
            </div>
          </div>

          {/* Start verification button for pending_dns status */}
          {domain.status === 'pending_dns' && (
            <button
              className="primary"
              onClick={handleStartVerification}
              style={{ marginTop: 'var(--spacing-lg)' }}
            >
              I&apos;ve added the records
            </button>
          )}

          {/* Retry button for failed status */}
          {domain.status === 'failed' && (
            <div style={{ marginTop: 'var(--spacing-lg)' }}>
              <div className="message error" style={{ marginBottom: 'var(--spacing-md)' }}>
                DNS verification failed. Please check your DNS records and try again.
              </div>
              <button className="primary" onClick={handleStartVerification}>
                Retry verification
              </button>
            </div>
          )}
        </div>
      )}

      {/* Configuration Section - Only show when verified */}
      {domain.status === 'verified' && (
        <>
          {error && (
            <div className="message error" style={{ marginBottom: 'var(--spacing-md)' }}>
              {error}
            </div>
          )}

          {success && (
            <div className="message success" style={{ marginBottom: 'var(--spacing-md)' }}>
              {success}
            </div>
          )}

          <div className="card" style={{ marginBottom: 'var(--spacing-lg)' }}>
            <h2 style={{ marginBottom: 'var(--spacing-xs)' }}>Configuration</h2>
            <p className="text-muted" style={{ marginBottom: 'var(--spacing-md)' }}>
              Login URL:{' '}
              <a
                href={`https://reauth.${domain.domain}`}
                target="_blank"
                rel="noopener noreferrer"
                style={{ color: 'var(--accent-blue)' }}
              >
                https://reauth.{domain.domain}
              </a>
            </p>
          </div>

          <form onSubmit={handleSaveConfig}>
            {/* Magic Link Section */}
            <div className="card" style={{ marginBottom: 'var(--spacing-lg)' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <div>
                  <h2 style={{ marginBottom: 'var(--spacing-xs)' }}>Magic Link</h2>
                  <p className="text-muted" style={{ margin: 0 }}>
                    Allow users to sign in with a magic link sent to their email.
                  </p>
                </div>
                <label style={{ display: 'flex', alignItems: 'center', gap: 'var(--spacing-sm)', cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={magicLinkEnabled}
                    onChange={(e) => setMagicLinkEnabled(e.target.checked)}
                    style={{ width: 18, height: 18 }}
                  />
                  <span>{magicLinkEnabled ? 'Enabled' : 'Disabled'}</span>
                </label>
              </div>

              {magicLinkEnabled && (
                <div style={{ marginTop: 'var(--spacing-lg)', borderTop: '1px solid var(--border-primary)', paddingTop: 'var(--spacing-lg)' }}>
                  <div style={{ marginBottom: 'var(--spacing-md)' }}>
                    <label htmlFor="resendApiKey">Resend API Key</label>
                    <input
                      id="resendApiKey"
                      type="password"
                      value={resendApiKey}
                      onChange={(e) => setResendApiKey(e.target.value)}
                      placeholder={authConfig?.magic_link_config?.has_api_key ? '••••••••••••••••' : 'Enter your Resend API key'}
                    />
                    <p className="text-muted" style={{ fontSize: '12px', marginTop: 'var(--spacing-xs)' }}>
                      Get your API key from{' '}
                      <a href="https://resend.com/api-keys" target="_blank" rel="noopener noreferrer" style={{ color: 'var(--accent-blue)' }}>
                        resend.com/api-keys
                      </a>
                      {authConfig?.magic_link_config?.has_api_key && ' (leave blank to keep existing key)'}
                    </p>
                  </div>

                  <div style={{ marginBottom: 'var(--spacing-md)' }}>
                    <label htmlFor="fromEmail">From Email</label>
                    <input
                      id="fromEmail"
                      type="email"
                      value={fromEmail}
                      onChange={(e) => setFromEmail(e.target.value)}
                      placeholder="noreply@yourdomain.com"
                    />
                    <p className="text-muted" style={{ fontSize: '12px', marginTop: 'var(--spacing-xs)' }}>
                      The email address magic links will be sent from. Must be verified in Resend.
                    </p>
                  </div>
                </div>
              )}
            </div>

            {/* Google OAuth Section (Placeholder) */}
            <div className="card" style={{ marginBottom: 'var(--spacing-lg)', opacity: 0.6 }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <div>
                  <h2 style={{ marginBottom: 'var(--spacing-xs)' }}>Google OAuth</h2>
                  <p className="text-muted" style={{ margin: 0 }}>
                    Allow users to sign in with their Google account.
                  </p>
                </div>
                <span
                  style={{
                    padding: '4px 8px',
                    borderRadius: 'var(--radius-sm)',
                    backgroundColor: 'var(--bg-tertiary)',
                    color: 'var(--text-muted)',
                    fontSize: '12px',
                  }}
                >
                  Coming soon
                </span>
              </div>
            </div>

            {/* Redirect URL */}
            <div className="card" style={{ marginBottom: 'var(--spacing-lg)' }}>
              <h2 style={{ marginBottom: 'var(--spacing-xs)' }}>Redirect URL</h2>
              <p className="text-muted" style={{ marginBottom: 'var(--spacing-md)' }}>
                Where to redirect users after successful login.
              </p>
              <div style={{ marginBottom: 'var(--spacing-md)' }}>
                <label htmlFor="redirectUrl">URL</label>
                <input
                  id="redirectUrl"
                  type="url"
                  value={redirectUrl}
                  onChange={(e) => setRedirectUrl(e.target.value)}
                  placeholder="https://app.yourdomain.com/callback"
                />
                <p className="text-muted" style={{ fontSize: '12px', marginTop: 'var(--spacing-xs)' }}>
                  Must be on <strong>{domain.domain}</strong> or a subdomain (e.g., app.{domain.domain}). If not set, users will see a &quot;Login successful&quot; message.
                </p>
              </div>
            </div>

            {/* Save Button */}
            <button type="submit" className="primary" disabled={saving}>
              {saving ? 'Saving...' : 'Save changes'}
            </button>
          </form>
        </>
      )}
    </>
  );
}
