'use client';

import { useState, useEffect, useCallback, useRef } from 'react';
import { useRouter } from 'next/navigation';
import ConfirmModal from '@/components/ConfirmModal';

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

type WizardStep = 'closed' | 'input' | 'records';

export default function DomainsPage() {
  const router = useRouter();
  const [domains, setDomains] = useState<Domain[]>([]);
  const [loading, setLoading] = useState(true);
  const [wizardStep, setWizardStep] = useState<WizardStep>('closed');
  const [newDomain, setNewDomain] = useState('');
  const [createdDomain, setCreatedDomain] = useState<Domain | null>(null);
  const [error, setError] = useState('');
  const [copiedField, setCopiedField] = useState<string | null>(null);
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  // Close menu when clicking outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setOpenMenuId(null);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

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


  const validateDomain = (domain: string): string | null => {
    const trimmed = domain.trim().toLowerCase();

    if (!trimmed) {
      return 'Please enter a domain name';
    }

    // Check for protocol prefix
    if (trimmed.startsWith('http://') || trimmed.startsWith('https://')) {
      return 'Enter just the domain name without http:// or https://';
    }

    // Check for path
    if (trimmed.includes('/')) {
      return 'Enter just the domain name without any path';
    }

    // Basic domain format check
    const parts = trimmed.split('.');
    if (parts.length < 2) {
      return 'Please enter a valid domain (e.g., example.com)';
    }

    // Check for empty parts (e.g., "example..com")
    if (parts.some(p => p.length === 0)) {
      return 'Invalid domain format';
    }

    // Multi-part TLDs
    const multiPartTlds = ['co.uk', 'com.au', 'co.nz', 'com.br', 'co.jp', 'org.uk', 'net.au'];
    for (const tld of multiPartTlds) {
      if (trimmed.endsWith(tld)) {
        if (parts.length > 3) {
          return `Please enter your root domain (e.g., example.${tld}), not a subdomain`;
        }
        if (parts.length < 3) {
          return 'Please enter a valid domain';
        }
        return null; // Valid multi-part TLD domain
      }
    }

    // Standard TLDs - should have exactly 2 parts
    if (parts.length > 2) {
      return `Please enter your root domain (e.g., ${parts.slice(-2).join('.')}), not a subdomain`;
    }

    return null; // Valid
  };

  const handleCreateDomain = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    const validationError = validateDomain(newDomain);
    if (validationError) {
      setError(validationError);
      return;
    }

    try {
      const res = await fetch('/api/domains', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ domain: newDomain.trim().toLowerCase() }),
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
        // Navigate to domain detail page
        router.push(`/domains/${createdDomain.id}`);
      } else {
        setError('Failed to start verification');
      }
    } catch {
      setError('Network error. Please try again.');
    }
  };

  const handleDeleteDomain = async (domainId: string) => {
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
    } finally {
      setDeleteConfirmId(null);
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
                Enter your root domain. Your login page will be at <code>reauth.yourdomain.com</code>
              </p>
              <form onSubmit={handleCreateDomain}>
                <label htmlFor="domain">Root domain</label>
                <input
                  id="domain"
                  type="text"
                  value={newDomain}
                  onChange={(e) => setNewDomain(e.target.value)}
                  placeholder="example.com"
                  style={{ marginBottom: 'var(--spacing-md)' }}
                />
                <p className="text-muted" style={{ fontSize: '12px', marginTop: '-12px', marginBottom: 'var(--spacing-md)' }}>
                  Enter your root domain (e.g., example.com), not a subdomain
                </p>
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
            onClick={() => router.push(`/domains/${domain.id}`)}
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              cursor: 'pointer',
              transition: 'background-color 0.15s ease',
            }}
            onMouseEnter={(e) => (e.currentTarget.style.backgroundColor = 'var(--bg-tertiary)')}
            onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = '')}
          >
            <div>
              <div style={{ fontWeight: 600, marginBottom: 'var(--spacing-xs)' }}>{domain.domain}</div>
              {domain.status === 'verified' && (
                <div style={{ fontSize: '12px', marginBottom: 'var(--spacing-xs)' }}>
                  <span className="text-muted">Login: </span>
                  <span style={{ color: 'var(--accent-blue)' }}>
                    reauth.{domain.domain}
                  </span>
                </div>
              )}
              {getStatusBadge(domain.status)}
            </div>
            <div style={{ display: 'flex', gap: 'var(--spacing-sm)', alignItems: 'center' }}>
              {domain.status === 'failed' && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    handleRetryVerification(domain);
                  }}
                >
                  Retry
                </button>
              )}
              <div ref={openMenuId === domain.id ? menuRef : null} style={{ position: 'relative' }}>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    setOpenMenuId(openMenuId === domain.id ? null : domain.id);
                  }}
                  style={{
                    padding: 'var(--spacing-xs)',
                    backgroundColor: 'transparent',
                    border: 'none',
                    cursor: 'pointer',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                  }}
                >
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                    <circle cx="12" cy="5" r="2" />
                    <circle cx="12" cy="12" r="2" />
                    <circle cx="12" cy="19" r="2" />
                  </svg>
                </button>
                {openMenuId === domain.id && (
                  <div style={{
                    position: 'absolute',
                    top: '100%',
                    right: 0,
                    marginTop: 'var(--spacing-xs)',
                    backgroundColor: 'var(--bg-secondary)',
                    border: '1px solid var(--border-primary)',
                    borderRadius: 'var(--radius-sm)',
                    boxShadow: 'var(--shadow-md)',
                    zIndex: 100,
                    minWidth: '120px',
                  }}>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        setOpenMenuId(null);
                        setDeleteConfirmId(domain.id);
                      }}
                      style={{
                        width: '100%',
                        padding: 'var(--spacing-sm) var(--spacing-md)',
                        backgroundColor: 'transparent',
                        border: 'none',
                        color: 'var(--accent-red)',
                        fontSize: '13px',
                        textAlign: 'left',
                        cursor: 'pointer',
                      }}
                    >
                      Delete
                    </button>
                  </div>
                )}
              </div>
              <span className="text-muted" style={{ fontSize: '18px' }}>&rarr;</span>
            </div>
          </div>
        ))
      )}

      <ConfirmModal
        isOpen={deleteConfirmId !== null}
        title="Delete Domain"
        message="Are you sure you want to delete this domain? This cannot be undone."
        confirmLabel="Delete"
        cancelLabel="Cancel"
        variant="danger"
        onConfirm={() => deleteConfirmId && handleDeleteDomain(deleteConfirmId)}
        onCancel={() => setDeleteConfirmId(null)}
      />
    </>
  );
}
