'use client';

import { useState, useEffect, useCallback } from 'react';
import { useParams, useRouter } from 'next/navigation';

type Domain = {
  id: string;
  domain: string;
  status: 'pending_dns' | 'verifying' | 'verified' | 'failed';
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

export default function DomainAuthConfigPage() {
  const params = useParams();
  const router = useRouter();
  const domainId = params.id as string;

  const [domain, setDomain] = useState<Domain | null>(null);
  const [authConfig, setAuthConfig] = useState<AuthConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');

  // Form state
  const [magicLinkEnabled, setMagicLinkEnabled] = useState(false);
  const [resendApiKey, setResendApiKey] = useState('');
  const [fromEmail, setFromEmail] = useState('');
  const [redirectUrl, setRedirectUrl] = useState('');

  const fetchData = useCallback(async () => {
    try {
      const [domainRes, configRes] = await Promise.all([
        fetch(`/api/domains/${domainId}`, { credentials: 'include' }),
        fetch(`/api/domains/${domainId}/auth-config`, { credentials: 'include' }),
      ]);

      if (domainRes.ok) {
        const domainData = await domainRes.json();
        setDomain(domainData);
      }

      if (configRes.ok) {
        const configData = await configRes.json();
        setAuthConfig(configData);
        setMagicLinkEnabled(configData.magic_link_enabled);
        setRedirectUrl(configData.redirect_url || '');
        if (configData.magic_link_config) {
          setFromEmail(configData.magic_link_config.from_email);
        }
      }
    } catch {
      setError('Failed to load configuration');
    } finally {
      setLoading(false);
    }
  }, [domainId]);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  const handleSave = async (e: React.FormEvent) => {
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

      // Only include magic link config if enabling or updating
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
        setResendApiKey(''); // Clear the API key field
        fetchData(); // Refresh the data
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

  if (loading) {
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

  if (domain.status !== 'verified') {
    return (
      <>
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
        <div className="card">
          <h2>Authentication Settings</h2>
          <p className="text-muted">{domain.domain}</p>
          <div
            style={{
              padding: 'var(--spacing-xl)',
              backgroundColor: 'var(--bg-tertiary)',
              borderRadius: 'var(--radius-md)',
              textAlign: 'center',
              marginTop: 'var(--spacing-md)',
            }}
          >
            <p className="text-muted" style={{ margin: 0 }}>
              Domain must be verified before configuring authentication.
            </p>
          </div>
        </div>
      </>
    );
  }

  return (
    <>
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

      <h1>Authentication Settings</h1>
      <p className="text-muted" style={{ marginBottom: 'var(--spacing-lg)' }}>
        Configure login methods for <strong>{domain.domain}</strong>
      </p>

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

      <form onSubmit={handleSave}>
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
          <input
            type="url"
            value={redirectUrl}
            onChange={(e) => setRedirectUrl(e.target.value)}
            placeholder="https://app.yourdomain.com/callback"
          />
          <p className="text-muted" style={{ fontSize: '12px', marginTop: 'var(--spacing-xs)' }}>
            If not set, users will see a &quot;Login successful&quot; message.
          </p>
        </div>

        {/* Save Button */}
        <div style={{ display: 'flex', gap: 'var(--spacing-sm)' }}>
          <button type="submit" className="primary" disabled={saving}>
            {saving ? 'Saving...' : 'Save changes'}
          </button>
          <button type="button" onClick={() => router.push('/domains')}>
            Cancel
          </button>
        </div>
      </form>
    </>
  );
}
