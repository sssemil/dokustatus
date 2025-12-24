'use client';

import { useState, useEffect, useCallback } from 'react';
import { useRouter } from 'next/navigation';

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

type WizardStep = 'closed' | 'input' | 'records' | 'verifying';

export default function DomainsPage() {
  const router = useRouter();
  const [domains, setDomains] = useState<Domain[]>([]);
  const [loading, setLoading] = useState(true);
  const [wizardStep, setWizardStep] = useState<WizardStep>('closed');
  const [newDomain, setNewDomain] = useState('');
  const [createdDomain, setCreatedDomain] = useState<Domain | null>(null);
  const [error, setError] = useState('');
  const [copiedField, setCopiedField] = useState<string | null>(null);

  const fetchDomains = useCallback(async () => {
    try {
      const res = await fetch('/api/domains', { credentials: 'include' });
      if (res.ok) {
        const data = await res.json();
        setDomains(data);
      }
    } catch {
      // Ignore
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchDomains();
  }, [fetchDomains]);

  useEffect(() => {
    if (wizardStep !== 'verifying' || !createdDomain) return;

    const interval = setInterval(async () => {
      try {
        const res = await fetch(`/api/domains/${createdDomain.id}/status`, {
          credentials: 'include',
        });
        if (res.ok) {
          const data = await res.json();
          if (data.status === 'verified') {
            clearInterval(interval);
            setWizardStep('closed');
            setCreatedDomain(null);
            fetchDomains();
          } else if (data.status === 'failed') {
            clearInterval(interval);
            setWizardStep('closed');
            setCreatedDomain(null);
            fetchDomains();
          }
        }
      } catch {
        // Continue polling
      }
    }, 5000);

    return () => clearInterval(interval);
  }, [wizardStep, createdDomain, fetchDomains]);

  const handleCreateDomain = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    if (!newDomain.trim()) {
      setError('Please enter a domain name');
      return;
    }

    try {
      const res = await fetch('/api/domains', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ domain: newDomain.trim() }),
        credentials: 'include',
      });

      if (res.ok) {
        const data = await res.json();
        setCreatedDomain(data);
        setWizardStep('records');
        setNewDomain('');
      } else {
        const errData = await res.json().catch(() => ({}));
        setError(errData.message || 'Failed to add domain');
      }
    } catch {
      setError('Network error. Please try again.');
    }
  };

  const handleStartVerification = async () => {
    if (!createdDomain) return;

    try {
      const res = await fetch(`/api/domains/${createdDomain.id}/verify`, {
        method: 'POST',
        credentials: 'include',
      });

      if (res.ok) {
        setWizardStep('verifying');
        fetchDomains();
      } else {
        setError('Failed to start verification');
      }
    } catch {
      setError('Network error. Please try again.');
    }
  };

  const handleDeleteDomain = async (domainId: string) => {
    if (!confirm('Are you sure you want to delete this domain?')) return;

    try {
      const res = await fetch(`/api/domains/${domainId}`, {
        method: 'DELETE',
        credentials: 'include',
      });

      if (res.ok) {
        fetchDomains();
      }
    } catch {
      // Ignore
    }
  };

  const handleRetryVerification = async (domain: Domain) => {
    try {
      const res = await fetch(`/api/domains/${domain.id}/verify`, {
        method: 'POST',
        credentials: 'include',
      });

      if (res.ok) {
        fetchDomains();
      }
    } catch {
      // Ignore
    }
  };

  const copyToClipboard = async (text: string, field: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedField(field);
      setTimeout(() => setCopiedField(null), 2000);
    } catch {
      // Fallback for older browsers
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

  const closeWizard = () => {
    setWizardStep('closed');
    setNewDomain('');
    setCreatedDomain(null);
    setError('');
  };

  return (
    <>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <h1>Domains</h1>
        {wizardStep === 'closed' && (
          <button className="primary" onClick={() => setWizardStep('input')}>
            + Add domain
          </button>
        )}
      </div>

      {/* Wizard */}
      {wizardStep !== 'closed' && (
        <div className="card">
          {wizardStep === 'input' && (
            <>
              <h2>Add a domain</h2>
              <p className="text-muted" style={{ marginBottom: 'var(--spacing-md)' }}>
                Enter the domain you want to use for authentication.
              </p>
              <form onSubmit={handleCreateDomain}>
                <label htmlFor="domain">Domain name</label>
                <input
                  id="domain"
                  type="text"
                  value={newDomain}
                  onChange={(e) => setNewDomain(e.target.value)}
                  placeholder="login.example.com"
                  style={{ marginBottom: 'var(--spacing-md)' }}
                />
                {error && (
                  <div className="message error" style={{ marginBottom: 'var(--spacing-md)' }}>
                    {error}
                  </div>
                )}
                <div style={{ display: 'flex', gap: 'var(--spacing-sm)' }}>
                  <button type="submit" className="primary">
                    + Add Domain
                  </button>
                  <button type="button" onClick={closeWizard}>
                    Cancel
                  </button>
                </div>
              </form>
            </>
          )}

          {wizardStep === 'records' && createdDomain?.dns_records && (
            <>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
                <div>
                  <h2>DNS Records</h2>
                  <p className="text-muted" style={{ marginBottom: 'var(--spacing-md)' }}>
                    Add the following DNS records in your domain provider.
                  </p>
                </div>
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
                  </div>
                  <div style={{ display: 'grid', gridTemplateColumns: '80px 1fr auto', gap: 'var(--spacing-sm)', alignItems: 'center' }}>
                    <span className="text-muted">Name</span>
                    <code style={{ backgroundColor: 'var(--bg-secondary)', padding: '4px 8px', borderRadius: '4px', fontSize: '13px' }}>
                      {createdDomain.dns_records.cname_name}
                    </code>
                    <button
                      onClick={() => copyToClipboard(createdDomain.dns_records!.cname_name, 'cname_name')}
                      style={{ padding: '4px 8px', fontSize: '12px' }}
                    >
                      {copiedField === 'cname_name' ? 'Copied!' : 'Copy'}
                    </button>

                    <span className="text-muted">Value</span>
                    <code style={{ backgroundColor: 'var(--bg-secondary)', padding: '4px 8px', borderRadius: '4px', fontSize: '13px' }}>
                      {createdDomain.dns_records.cname_value}
                    </code>
                    <button
                      onClick={() => copyToClipboard(createdDomain.dns_records!.cname_value, 'cname_value')}
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
                  </div>
                  <div style={{ display: 'grid', gridTemplateColumns: '80px 1fr auto', gap: 'var(--spacing-sm)', alignItems: 'center' }}>
                    <span className="text-muted">Name</span>
                    <code style={{ backgroundColor: 'var(--bg-secondary)', padding: '4px 8px', borderRadius: '4px', fontSize: '13px' }}>
                      {createdDomain.dns_records.txt_name}
                    </code>
                    <button
                      onClick={() => copyToClipboard(createdDomain.dns_records!.txt_name, 'txt_name')}
                      style={{ padding: '4px 8px', fontSize: '12px' }}
                    >
                      {copiedField === 'txt_name' ? 'Copied!' : 'Copy'}
                    </button>

                    <span className="text-muted">Value</span>
                    <code style={{ backgroundColor: 'var(--bg-secondary)', padding: '4px 8px', borderRadius: '4px', fontSize: '13px' }}>
                      {createdDomain.dns_records.txt_value}
                    </code>
                    <button
                      onClick={() => copyToClipboard(createdDomain.dns_records!.txt_value, 'txt_value')}
                      style={{ padding: '4px 8px', fontSize: '12px' }}
                    >
                      {copiedField === 'txt_value' ? 'Copied!' : 'Copy'}
                    </button>
                  </div>
                </div>
              </div>

              {error && (
                <div className="message error" style={{ marginTop: 'var(--spacing-md)' }}>
                  {error}
                </div>
              )}

              <div style={{ display: 'flex', gap: 'var(--spacing-sm)', marginTop: 'var(--spacing-lg)' }}>
                <button className="primary" onClick={handleStartVerification}>
                  I&apos;ve added the records
                </button>
                <button onClick={closeWizard}>Cancel</button>
              </div>
            </>
          )}

          {wizardStep === 'verifying' && (
            <>
              <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--spacing-md)' }}>
                <div className="spinner" />
                <div>
                  <h2 style={{ margin: 0 }}>Looking for DNS records...</h2>
                  <p className="text-muted" style={{ margin: 'var(--spacing-xs) 0 0' }}>
                    It may take a few minutes or hours, depending on your DNS provider&apos;s propagation time.
                  </p>
                </div>
              </div>
              <p className="text-muted" style={{ marginTop: 'var(--spacing-md)' }}>
                You can close this wizard and we&apos;ll keep checking in the background. We&apos;ll email you when verification is complete or if it fails.
              </p>
              <button onClick={closeWizard} style={{ marginTop: 'var(--spacing-md)' }}>
                Close
              </button>
            </>
          )}
        </div>
      )}

      {/* Domains List */}
      {loading ? (
        <div style={{ display: 'flex', justifyContent: 'center', padding: 'var(--spacing-xl)' }}>
          <div className="spinner" />
        </div>
      ) : domains.length === 0 && wizardStep === 'closed' ? (
        <div className="card" style={{ textAlign: 'center' }}>
          <p className="text-muted">No domains added yet. Click &quot;+ Add domain&quot; to get started.</p>
        </div>
      ) : (
        domains.map((domain) => (
          <div
            key={domain.id}
            className="card"
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
            }}
          >
            <div>
              <div style={{ fontWeight: 600, marginBottom: 'var(--spacing-xs)' }}>{domain.domain}</div>
              {getStatusBadge(domain.status)}
            </div>
            <div style={{ display: 'flex', gap: 'var(--spacing-sm)' }}>
              {domain.status === 'verified' && (
                <button onClick={() => router.push(`/domains/${domain.id}/auth`)}>
                  Auth Settings
                </button>
              )}
              {domain.status === 'failed' && (
                <button onClick={() => handleRetryVerification(domain)}>Retry</button>
              )}
              <button className="danger" onClick={() => handleDeleteDomain(domain.id)}>
                Delete
              </button>
            </div>
          </div>
        ))
      )}
    </>
  );
}
