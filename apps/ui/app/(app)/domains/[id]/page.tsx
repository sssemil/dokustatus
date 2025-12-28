'use client';

import { useState, useEffect, useCallback, useRef } from 'react';
import { useParams, useRouter, useSearchParams } from 'next/navigation';
import ConfirmModal from '@/components/ConfirmModal';
import HoldToConfirmButton from '@/components/HoldToConfirmButton';

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
  whitelist_enabled: boolean;
  magic_link_config: {
    from_email: string;
    has_api_key: boolean;
  } | null;
  using_fallback: boolean;
  fallback_from_email: string | null;
};

type EndUser = {
  id: string;
  email: string;
  roles: string[];
  email_verified_at: string | null;
  last_login_at: string | null;
  is_frozen: boolean;
  is_whitelisted: boolean;
  created_at: string | null;
};

type Tab = 'dns' | 'configuration' | 'roles' | 'users' | 'api-keys';

type Role = {
  id: string;
  name: string;
  user_count: number;
  created_at: string | null;
};

type ApiKey = {
  id: string;
  key_prefix: string;
  name: string;
  last_used_at: string | null;
  created_at: string | null;
};

const VALID_TABS: Tab[] = ['dns', 'configuration', 'roles', 'users', 'api-keys'];

export default function DomainDetailPage() {
  const params = useParams();
  const router = useRouter();
  const searchParams = useSearchParams();
  const domainId = params.id as string;

  // Get initial tab from URL or default to 'dns'
  const tabFromUrl = searchParams.get('tab') as Tab | null;
  const initialTab = tabFromUrl && VALID_TABS.includes(tabFromUrl) ? tabFromUrl : 'dns';

  const [domain, setDomain] = useState<Domain | null>(null);
  const [authConfig, setAuthConfig] = useState<AuthConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');
  const [copiedField, setCopiedField] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<Tab>(initialTab);

  // Update URL when tab changes
  const handleTabChange = useCallback((tab: Tab) => {
    setActiveTab(tab);
    const newParams = new URLSearchParams(searchParams.toString());
    newParams.set('tab', tab);
    router.replace(`/domains/${domainId}?${newParams.toString()}`, { scroll: false });
  }, [domainId, router, searchParams]);
  const [endUsers, setEndUsers] = useState<EndUser[]>([]);
  const [loadingUsers, setLoadingUsers] = useState(false);
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [deleteUserConfirmId, setDeleteUserConfirmId] = useState<string | null>(null);
  const [showWhitelistModal, setShowWhitelistModal] = useState(false);
  const [showInviteModal, setShowInviteModal] = useState(false);
  const [inviteEmail, setInviteEmail] = useState('');
  const [invitePreWhitelist, setInvitePreWhitelist] = useState(false);
  const [inviting, setInviting] = useState(false);
  const [inviteError, setInviteError] = useState('');
  const menuRef = useRef<HTMLDivElement>(null);

  // API Keys state
  const [apiKeys, setApiKeys] = useState<ApiKey[]>([]);
  const [loadingApiKeys, setLoadingApiKeys] = useState(false);
  const [showCreateKeyModal, setShowCreateKeyModal] = useState(false);
  const [newKeyName, setNewKeyName] = useState('');
  const [creatingKey, setCreatingKey] = useState(false);
  const [newlyCreatedKey, setNewlyCreatedKey] = useState<string | null>(null);

  // Roles state
  const [roles, setRoles] = useState<Role[]>([]);
  const [loadingRoles, setLoadingRoles] = useState(false);
  const [newRoleName, setNewRoleName] = useState('');
  const [creatingRole, setCreatingRole] = useState(false);
  const [roleError, setRoleError] = useState('');

  // DNS record verification status (for verifying domains)
  const [cnameVerified, setCnameVerified] = useState(false);
  const [txtVerified, setTxtVerified] = useState(false);

  // Auth config form state
  const [magicLinkEnabled, setMagicLinkEnabled] = useState(false);
  const [resendApiKey, setResendApiKey] = useState('');
  const [fromEmail, setFromEmail] = useState('');
  const [redirectUrl, setRedirectUrl] = useState('');
  const [whitelistEnabled, setWhitelistEnabled] = useState(false);
  const [userSearch, setUserSearch] = useState('');

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
            setWhitelistEnabled(configData.whitelist_enabled);
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

  const fetchEndUsers = useCallback(async () => {
    if (!domain || domain.status !== 'verified') return;
    setLoadingUsers(true);
    try {
      const res = await fetch(`/api/domains/${domainId}/end-users`, { credentials: 'include' });
      if (res.ok) {
        const data = await res.json();
        setEndUsers(data);
      }
    } catch {
      // Ignore
    } finally {
      setLoadingUsers(false);
    }
  }, [domainId, domain]);

  useEffect(() => {
    if (activeTab === 'users' && domain?.status === 'verified') {
      fetchEndUsers();
    }
  }, [activeTab, domain, fetchEndUsers]);

  const fetchApiKeys = useCallback(async () => {
    if (!domain || domain.status !== 'verified') return;
    setLoadingApiKeys(true);
    try {
      const res = await fetch(`/api/domains/${domainId}/api-keys`, { credentials: 'include' });
      if (res.ok) {
        const data = await res.json();
        setApiKeys(data);
      }
    } catch {
      // Ignore
    } finally {
      setLoadingApiKeys(false);
    }
  }, [domainId, domain]);

  useEffect(() => {
    if (activeTab === 'api-keys' && domain?.status === 'verified') {
      fetchApiKeys();
    }
  }, [activeTab, domain, fetchApiKeys]);

  const fetchRoles = useCallback(async () => {
    if (!domain || domain.status !== 'verified') return;
    setLoadingRoles(true);
    try {
      const res = await fetch(`/api/domains/${domainId}/roles`, { credentials: 'include' });
      if (res.ok) {
        const data = await res.json();
        setRoles(data);
      }
    } catch {
      // Ignore
    } finally {
      setLoadingRoles(false);
    }
  }, [domainId, domain]);

  useEffect(() => {
    if (activeTab === 'roles' && domain?.status === 'verified') {
      fetchRoles();
    }
  }, [activeTab, domain, fetchRoles]);

  // Poll for verification status when domain is verifying
  useEffect(() => {
    if (!domain || (domain.status !== 'verifying' && domain.status !== 'pending_dns')) return;

    const checkStatus = async () => {
      try {
        const res = await fetch(`/api/domains/${domainId}/status`, { credentials: 'include' });
        if (res.ok) {
          const data = await res.json();
          // Update individual record status
          setCnameVerified(data.cname_verified);
          setTxtVerified(data.txt_verified);
          if (data.status !== domain.status) {
            fetchData(); // Refetch all data when status changes
          }
        }
      } catch {
        // Continue polling
      }
    };

    // Check immediately on mount
    checkStatus();

    const interval = setInterval(checkStatus, 5000);

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

  const handleSaveConfig = async (e: React.FormEvent, whitelistAllExisting = false) => {
    e.preventDefault();
    setError('');
    setSuccess('');
    setSaving(true);

    try {
      const payload: Record<string, unknown> = {
        magic_link_enabled: magicLinkEnabled,
        google_oauth_enabled: false,
        redirect_url: redirectUrl || null,
        whitelist_enabled: whitelistEnabled,
        whitelist_all_existing: whitelistAllExisting,
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

  const handleRemoveCustomConfig = async () => {
    if (!confirm('Remove custom email configuration? The domain will use reauth\'s shared email service.')) {
      return;
    }

    setError('');
    setSuccess('');

    try {
      const res = await fetch(`/api/domains/${domainId}/auth-config/magic-link`, {
        method: 'DELETE',
        credentials: 'include',
      });

      if (res.ok) {
        setSuccess('Custom email configuration removed');
        setResendApiKey('');
        setFromEmail('');
        fetchData();
      } else {
        const errData = await res.json().catch(() => ({}));
        setError(errData.message || 'Failed to remove configuration');
      }
    } catch {
      setError('Network error. Please try again.');
    }
  };

  const handleWhitelistToggle = (enabled: boolean) => {
    if (enabled && !authConfig?.whitelist_enabled) {
      // Show whitelist modal to ask about existing users
      setShowWhitelistModal(true);
    } else {
      setWhitelistEnabled(enabled);
    }
  };

  const handleWhitelistConfirm = async (whitelistAllExisting: boolean) => {
    setShowWhitelistModal(false);
    setWhitelistEnabled(true);

    if (whitelistAllExisting) {
      setSaving(true);
      try {
        const res = await fetch(`/api/domains/${domainId}/auth-config`, {
          method: 'PATCH',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            magic_link_enabled: magicLinkEnabled,
            google_oauth_enabled: false,
            redirect_url: redirectUrl || null,
            whitelist_enabled: true,
            whitelist_all_existing: true,
          }),
          credentials: 'include',
        });
        if (res.ok) {
          setSuccess('Whitelist enabled and all existing users have been whitelisted');
          fetchData();
        } else {
          setError('Failed to enable whitelist');
        }
      } catch {
        setError('Network error');
      } finally {
        setSaving(false);
      }
    }
  };

  const handleDeleteDomain = async () => {
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

  const handleUserAction = async (userId: string, action: 'freeze' | 'unfreeze' | 'whitelist' | 'unwhitelist' | 'delete') => {
    if (action === 'delete') {
      setDeleteUserConfirmId(null);
    }

    const methodMap = {
      freeze: 'POST',
      unfreeze: 'DELETE',
      whitelist: 'POST',
      unwhitelist: 'DELETE',
      delete: 'DELETE',
    };

    const urlMap = {
      freeze: `/api/domains/${domainId}/end-users/${userId}/freeze`,
      unfreeze: `/api/domains/${domainId}/end-users/${userId}/freeze`,
      whitelist: `/api/domains/${domainId}/end-users/${userId}/whitelist`,
      unwhitelist: `/api/domains/${domainId}/end-users/${userId}/whitelist`,
      delete: `/api/domains/${domainId}/end-users/${userId}`,
    };

    try {
      const res = await fetch(urlMap[action], {
        method: methodMap[action],
        credentials: 'include',
      });

      if (res.ok) {
        setOpenMenuId(null);
        fetchEndUsers();
      } else {
        setError(`Failed to ${action} user`);
      }
    } catch {
      setError('Network error. Please try again.');
    }
  };

  const handleInviteUser = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!inviteEmail.trim()) return;

    setInviting(true);
    setInviteError('');

    try {
      const res = await fetch(`/api/domains/${domainId}/end-users/invite`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          email: inviteEmail.trim(),
          pre_whitelist: invitePreWhitelist,
        }),
        credentials: 'include',
      });

      if (res.ok) {
        setShowInviteModal(false);
        setInviteEmail('');
        setInvitePreWhitelist(false);
        setSuccess('Invitation sent successfully');
        fetchEndUsers();
      } else {
        const errData = await res.json().catch(() => ({}));
        setInviteError(errData.message || 'Failed to invite user');
      }
    } catch {
      setInviteError('Network error. Please try again.');
    } finally {
      setInviting(false);
    }
  };

  const handleCreateApiKey = async (e: React.FormEvent) => {
    e.preventDefault();
    setCreatingKey(true);
    setError('');

    try {
      const res = await fetch(`/api/domains/${domainId}/api-keys`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: newKeyName.trim() || 'Default' }),
        credentials: 'include',
      });

      if (res.ok) {
        const data = await res.json();
        setNewlyCreatedKey(data.key);
        setNewKeyName('');
        fetchApiKeys();
      } else {
        const errData = await res.json().catch(() => ({}));
        setError(errData.message || 'Failed to create API key');
        setShowCreateKeyModal(false);
      }
    } catch {
      setError('Network error. Please try again.');
      setShowCreateKeyModal(false);
    } finally {
      setCreatingKey(false);
    }
  };

  const handleRevokeApiKeyDirect = async (keyId: string) => {
    try {
      const res = await fetch(`/api/domains/${domainId}/api-keys/${keyId}`, {
        method: 'DELETE',
        credentials: 'include',
      });

      if (res.ok) {
        setSuccess('API key revoked successfully');
        fetchApiKeys();
      } else {
        setError('Failed to revoke API key');
      }
    } catch {
      setError('Network error. Please try again.');
    }
  };

  const handleCreateRole = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newRoleName.trim()) return;

    setCreatingRole(true);
    setRoleError('');

    try {
      const res = await fetch(`/api/domains/${domainId}/roles`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: newRoleName.trim().toLowerCase() }),
        credentials: 'include',
      });

      if (res.ok) {
        setNewRoleName('');
        fetchRoles();
      } else {
        const errData = await res.json().catch(() => ({}));
        setRoleError(errData.message || 'Failed to create role');
      }
    } catch {
      setRoleError('Network error. Please try again.');
    } finally {
      setCreatingRole(false);
    }
  };

  const handleDeleteRole = async (roleName: string) => {
    try {
      const res = await fetch(`/api/domains/${domainId}/roles/${encodeURIComponent(roleName)}`, {
        method: 'DELETE',
        credentials: 'include',
      });

      if (res.ok) {
        setSuccess(`Role "${roleName}" deleted`);
        fetchRoles();
        fetchEndUsers(); // Refresh users to update their roles
      } else {
        setError('Failed to delete role');
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
        <HoldToConfirmButton
          label="Delete Domain"
          holdingLabel="Hold to delete..."
          onConfirm={handleDeleteDomain}
          variant="danger"
          duration={3000}
        />
      </div>

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

      {/* No Auth Methods Warning */}
      {domain.status === 'verified' && !magicLinkEnabled && !authConfig?.google_oauth_enabled && (
        <div className="message warning" style={{ marginBottom: 'var(--spacing-md)' }}>
          No login methods are enabled. Go to the Configuration tab to enable Magic Link or Google OAuth.
        </div>
      )}

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

      {/* Tabs */}
      <div style={{
        display: 'flex',
        gap: 'var(--spacing-xs)',
        marginBottom: 'var(--spacing-lg)',
        borderBottom: '1px solid var(--border-primary)',
      }}>
        {[
          { id: 'dns' as Tab, label: 'DNS Records' },
          ...(domain.status === 'verified' ? [
            { id: 'configuration' as Tab, label: 'Configuration' },
            { id: 'roles' as Tab, label: 'Roles' },
            { id: 'users' as Tab, label: 'Users' },
            { id: 'api-keys' as Tab, label: 'API Keys' },
          ] : []),
        ].map((tab) => (
          <button
            key={tab.id}
            onClick={() => handleTabChange(tab.id)}
            style={{
              padding: 'var(--spacing-sm) var(--spacing-md)',
              backgroundColor: activeTab === tab.id ? 'var(--bg-tertiary)' : 'transparent',
              border: activeTab === tab.id ? '1px solid var(--border-primary)' : '1px solid transparent',
              borderBottom: activeTab === tab.id ? '1px solid var(--bg-tertiary)' : '1px solid transparent',
              borderRadius: 'var(--radius-sm) var(--radius-sm) 0 0',
              color: activeTab === tab.id ? 'var(--text-primary)' : 'var(--text-muted)',
              cursor: 'pointer',
              fontSize: '14px',
              fontWeight: activeTab === tab.id ? 600 : 400,
              marginBottom: '-1px',
            }}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* DNS Records Tab */}
      {activeTab === 'dns' && domain.dns_records && (
        <>
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
                  {(domain.status === 'verified' || cnameVerified) ? (
                    <span style={{
                      padding: '2px 8px',
                      borderRadius: 'var(--radius-sm)',
                      backgroundColor: 'var(--accent-green)',
                      color: '#000',
                      fontSize: '11px',
                      fontWeight: 500,
                    }}>Verified</span>
                  ) : domain.status === 'verifying' ? (
                    <span style={{
                      display: 'inline-flex',
                      alignItems: 'center',
                      gap: '4px',
                      padding: '2px 8px',
                      borderRadius: 'var(--radius-sm)',
                      backgroundColor: 'var(--accent-orange)',
                      color: '#000',
                      fontSize: '11px',
                      fontWeight: 500,
                    }}>
                      <span className="spinner" style={{ width: 10, height: 10 }} />
                      Verifying
                    </span>
                  ) : null}
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
                  {(domain.status === 'verified' || txtVerified) ? (
                    <span style={{
                      padding: '2px 8px',
                      borderRadius: 'var(--radius-sm)',
                      backgroundColor: 'var(--accent-green)',
                      color: '#000',
                      fontSize: '11px',
                      fontWeight: 500,
                    }}>Verified</span>
                  ) : domain.status === 'verifying' ? (
                    <span style={{
                      display: 'inline-flex',
                      alignItems: 'center',
                      gap: '4px',
                      padding: '2px 8px',
                      borderRadius: 'var(--radius-sm)',
                      backgroundColor: 'var(--accent-orange)',
                      color: '#000',
                      fontSize: '11px',
                      fontWeight: 500,
                    }}>
                      <span className="spinner" style={{ width: 10, height: 10 }} />
                      Verifying
                    </span>
                  ) : null}
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
        </>
      )}

      {/* Configuration Tab */}
      {activeTab === 'configuration' && domain.status === 'verified' && (
        <>
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
                  {/* Show indicator for custom vs fallback config */}
                  {authConfig?.magic_link_config?.has_api_key && (
                    <div
                      style={{
                        marginBottom: 'var(--spacing-md)',
                        padding: 'var(--spacing-sm) var(--spacing-md)',
                        backgroundColor: 'rgba(34, 197, 94, 0.1)',
                        border: '1px solid rgba(34, 197, 94, 0.3)',
                        borderRadius: 'var(--radius-sm)',
                      }}
                    >
                      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                        <div>
                          <span style={{ color: 'rgb(34, 197, 94)' }}>Using your custom email configuration</span>
                          <span className="text-muted" style={{ marginLeft: 'var(--spacing-sm)' }}>
                            ({authConfig.magic_link_config.from_email})
                          </span>
                        </div>
                        {authConfig?.fallback_from_email && (
                          <button
                            type="button"
                            onClick={handleRemoveCustomConfig}
                            style={{
                              background: 'none',
                              border: 'none',
                              color: 'var(--text-muted)',
                              cursor: 'pointer',
                              fontSize: '12px',
                              textDecoration: 'underline',
                            }}
                          >
                            Use shared service instead
                          </button>
                        )}
                      </div>
                    </div>
                  )}
                  {!authConfig?.magic_link_config?.has_api_key && authConfig?.using_fallback && (
                    <div
                      style={{
                        marginBottom: 'var(--spacing-md)',
                        padding: 'var(--spacing-sm) var(--spacing-md)',
                        backgroundColor: 'rgba(59, 130, 246, 0.1)',
                        border: '1px solid rgba(59, 130, 246, 0.3)',
                        borderRadius: 'var(--radius-sm)',
                      }}
                    >
                      <div style={{ fontWeight: 500, marginBottom: 4 }}>Using reauth&apos;s shared email service</div>
                      <div className="text-muted" style={{ fontSize: '13px' }}>
                        Magic links will be sent from <code style={{ backgroundColor: 'var(--bg-tertiary)', padding: '2px 6px', borderRadius: 4 }}>{authConfig.fallback_from_email}</code>
                      </div>
                      <div className="text-muted" style={{ fontSize: '12px', marginTop: 4 }}>
                        Add your own Resend API key below for custom branding.
                      </div>
                    </div>
                  )}

                  <div style={{ marginBottom: 'var(--spacing-md)' }}>
                    <label htmlFor="resendApiKey">
                      Resend API Key
                      {authConfig?.using_fallback && !authConfig?.magic_link_config?.has_api_key && (
                        <span className="text-muted" style={{ fontWeight: 'normal', marginLeft: 'var(--spacing-sm)' }}>(optional)</span>
                      )}
                    </label>
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
                    <label htmlFor="fromEmail">
                      From Email
                      {authConfig?.using_fallback && !authConfig?.magic_link_config?.has_api_key && (
                        <span className="text-muted" style={{ fontWeight: 'normal', marginLeft: 'var(--spacing-sm)' }}>(optional)</span>
                      )}
                    </label>
                    <input
                      id="fromEmail"
                      type="email"
                      value={fromEmail}
                      onChange={(e) => setFromEmail(e.target.value)}
                      placeholder={authConfig?.using_fallback ? authConfig.fallback_from_email || 'noreply@yourdomain.com' : 'noreply@yourdomain.com'}
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

            {/* Whitelist Mode */}
            <div className="card" style={{ marginBottom: 'var(--spacing-lg)' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <div>
                  <h2 style={{ marginBottom: 'var(--spacing-xs)' }}>Whitelist Mode</h2>
                  <p className="text-muted" style={{ margin: 0 }}>
                    When enabled, only whitelisted users can sign in.
                  </p>
                </div>
                <label style={{ display: 'flex', alignItems: 'center', gap: 'var(--spacing-sm)', cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={whitelistEnabled}
                    onChange={(e) => handleWhitelistToggle(e.target.checked)}
                    style={{ width: 18, height: 18 }}
                  />
                  <span>{whitelistEnabled ? 'Enabled' : 'Disabled'}</span>
                </label>
              </div>
              {whitelistEnabled && (
                <p className="text-muted" style={{ marginTop: 'var(--spacing-md)', fontSize: '13px' }}>
                  Go to the Users tab to manage which users are whitelisted.
                </p>
              )}
            </div>

            {/* Save Button */}
            <button type="submit" className="primary" disabled={saving}>
              {saving ? 'Saving...' : 'Save changes'}
            </button>
              </form>
            </>
          )}

      {/* Roles Tab */}
      {activeTab === 'roles' && domain.status === 'verified' && (
        <>
          <div className="card" style={{ marginBottom: 'var(--spacing-lg)' }}>
            <h2 style={{ marginBottom: 'var(--spacing-xs)' }}>Create Role</h2>
            <p className="text-muted" style={{ marginBottom: 'var(--spacing-md)' }}>
              Roles can be assigned to users and accessed via the SDK.
            </p>
            <form onSubmit={handleCreateRole} style={{ display: 'flex', gap: 'var(--spacing-sm)', alignItems: 'flex-start' }}>
              <div style={{ flex: 1 }}>
                <input
                  type="text"
                  value={newRoleName}
                  onChange={(e) => setNewRoleName(e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, ''))}
                  placeholder="e.g., admin, editor, viewer"
                  style={{ width: '100%' }}
                />
                <p className="text-muted" style={{ fontSize: '12px', marginTop: 'var(--spacing-xs)' }}>
                  Lowercase letters, numbers, and hyphens only.
                </p>
              </div>
              <button type="submit" className="primary" disabled={creatingRole || !newRoleName.trim()}>
                {creatingRole ? 'Creating...' : 'Create'}
              </button>
            </form>
            {roleError && (
              <div className="message error" style={{ marginTop: 'var(--spacing-sm)' }}>
                {roleError}
              </div>
            )}
          </div>

          {loadingRoles ? (
            <div style={{ display: 'flex', justifyContent: 'center', padding: 'var(--spacing-xl)' }}>
              <span className="spinner" />
            </div>
          ) : roles.length === 0 ? (
            <div className="card" style={{ textAlign: 'center' }}>
              <p className="text-muted">No roles created yet. Create a role above to get started.</p>
            </div>
          ) : (
            roles.map((role) => (
              <div
                key={role.id}
                className="card"
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  marginBottom: 'var(--spacing-sm)',
                }}
              >
                <div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--spacing-sm)', marginBottom: 'var(--spacing-xs)' }}>
                    <code style={{
                      backgroundColor: 'var(--bg-tertiary)',
                      padding: '4px 8px',
                      borderRadius: '4px',
                      fontSize: '14px',
                      fontWeight: 600,
                    }}>
                      {role.name}
                    </code>
                  </div>
                  <span className="text-muted" style={{ fontSize: '12px' }}>
                    {role.user_count} {role.user_count === 1 ? 'user' : 'users'}
                  </span>
                </div>
                <HoldToConfirmButton
                  label="Delete"
                  holdingLabel={`Deleting... (${role.user_count} users)`}
                  onConfirm={() => handleDeleteRole(role.name)}
                  variant="danger"
                  duration={3000}
                  style={{ fontSize: '13px', padding: '6px 12px' }}
                />
              </div>
            ))
          )}
        </>
      )}

      {/* Users Tab */}
      {activeTab === 'users' && domain.status === 'verified' && (
            <>
              {/* Search and Invite */}
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 'var(--spacing-md)', gap: 'var(--spacing-md)' }}>
                <input
                  type="text"
                  value={userSearch}
                  onChange={(e) => setUserSearch(e.target.value)}
                  placeholder="Search by email..."
                  style={{ flex: 1, maxWidth: '400px' }}
                />
                <button className="primary" onClick={() => setShowInviteModal(true)}>
                  Invite User
                </button>
              </div>

              {loadingUsers ? (
                <div style={{ display: 'flex', justifyContent: 'center', padding: 'var(--spacing-xl)' }}>
                  <span className="spinner" />
                </div>
              ) : endUsers.length === 0 ? (
                <div className="card" style={{ textAlign: 'center' }}>
                  <p className="text-muted">No users have signed up yet.</p>
                </div>
              ) : endUsers.filter((user) => user.email.toLowerCase().includes(userSearch.toLowerCase())).length === 0 ? (
                <div className="card" style={{ textAlign: 'center' }}>
                  <p className="text-muted">No users match your search.</p>
                </div>
              ) : (
                endUsers
                  .filter((user) => user.email.toLowerCase().includes(userSearch.toLowerCase()))
                  .map((user) => (
                  <div
                    key={user.id}
                    className="card"
                    onClick={() => router.push(`/domains/${domainId}/users/${user.id}`)}
                    style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      alignItems: 'center',
                      cursor: 'pointer',
                      transition: 'background-color 0.15s ease',
                      marginBottom: 'var(--spacing-sm)',
                    }}
                    onMouseEnter={(e) => (e.currentTarget.style.backgroundColor = 'var(--bg-tertiary)')}
                    onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = '')}
                  >
                    <div>
                      <div style={{ fontWeight: 600, marginBottom: 'var(--spacing-xs)' }}>{user.email}</div>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--spacing-sm)' }}>
                        {user.last_login_at && (
                          <span className="text-muted" style={{ fontSize: '12px' }}>
                            Last login: {formatDate(user.last_login_at)}
                          </span>
                        )}
                        {user.is_frozen && (
                          <span style={{
                            padding: '2px 6px',
                            borderRadius: 'var(--radius-sm)',
                            backgroundColor: 'var(--accent-red)',
                            color: '#fff',
                            fontSize: '11px',
                            fontWeight: 500,
                          }}>
                            Frozen
                          </span>
                        )}
                        {user.is_whitelisted && (
                          <span style={{
                            padding: '2px 6px',
                            borderRadius: 'var(--radius-sm)',
                            backgroundColor: 'var(--accent-green)',
                            color: '#000',
                            fontSize: '11px',
                            fontWeight: 500,
                          }}>
                            Whitelisted
                          </span>
                        )}
                        {user.roles && user.roles.map((role) => (
                          <span
                            key={role}
                            style={{
                              padding: '2px 6px',
                              borderRadius: 'var(--radius-sm)',
                              backgroundColor: 'var(--bg-tertiary)',
                              color: 'var(--text-secondary)',
                              fontSize: '11px',
                              fontWeight: 500,
                            }}
                          >
                            {role}
                          </span>
                        ))}
                      </div>
                    </div>
                    <div style={{ display: 'flex', gap: 'var(--spacing-sm)', alignItems: 'center' }}>
                      <div ref={openMenuId === user.id ? menuRef : null} style={{ position: 'relative' }}>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            setOpenMenuId(openMenuId === user.id ? null : user.id);
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
                        {openMenuId === user.id && (
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
                            minWidth: '140px',
                          }}>
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                router.push(`/domains/${domainId}/users/${user.id}`);
                              }}
                              style={{
                                width: '100%',
                                padding: 'var(--spacing-sm) var(--spacing-md)',
                                backgroundColor: 'transparent',
                                border: 'none',
                                color: 'var(--text-primary)',
                                fontSize: '13px',
                                textAlign: 'left',
                                cursor: 'pointer',
                              }}
                            >
                              View details
                            </button>
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                handleUserAction(user.id, user.is_frozen ? 'unfreeze' : 'freeze');
                              }}
                              style={{
                                width: '100%',
                                padding: 'var(--spacing-sm) var(--spacing-md)',
                                backgroundColor: 'transparent',
                                border: 'none',
                                color: 'var(--text-primary)',
                                fontSize: '13px',
                                textAlign: 'left',
                                cursor: 'pointer',
                              }}
                            >
                              {user.is_frozen ? 'Unfreeze' : 'Freeze'}
                            </button>
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                handleUserAction(user.id, user.is_whitelisted ? 'unwhitelist' : 'whitelist');
                              }}
                              style={{
                                width: '100%',
                                padding: 'var(--spacing-sm) var(--spacing-md)',
                                backgroundColor: 'transparent',
                                border: 'none',
                                color: 'var(--text-primary)',
                                fontSize: '13px',
                                textAlign: 'left',
                                cursor: 'pointer',
                              }}
                            >
                              {user.is_whitelisted ? 'Remove from whitelist' : 'Whitelist'}
                            </button>
                            <div style={{ borderTop: '1px solid var(--border-primary)', margin: '4px 0' }} />
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                setOpenMenuId(null);
                                setDeleteUserConfirmId(user.id);
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
                    </div>
                  </div>
                ))
              )}
            </>
          )}

      {/* API Keys Tab */}
      {activeTab === 'api-keys' && domain.status === 'verified' && (
        <>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 'var(--spacing-md)' }}>
            <div>
              <p className="text-muted" style={{ margin: 0, fontSize: '14px' }}>
                Use API keys to authenticate server-to-server requests from your backend.
              </p>
            </div>
            <button className="primary" onClick={() => setShowCreateKeyModal(true)}>
              Create API Key
            </button>
          </div>

          {loadingApiKeys ? (
            <div style={{ display: 'flex', justifyContent: 'center', padding: 'var(--spacing-xl)' }}>
              <span className="spinner" />
            </div>
          ) : apiKeys.length === 0 ? (
            <div className="card" style={{ textAlign: 'center' }}>
              <p className="text-muted">No API keys created yet.</p>
            </div>
          ) : (
            apiKeys.map((apiKey) => (
              <div
                key={apiKey.id}
                className="card"
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  marginBottom: 'var(--spacing-sm)',
                }}
              >
                <div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--spacing-sm)', marginBottom: 'var(--spacing-xs)' }}>
                    <span style={{ fontWeight: 600 }}>{apiKey.name}</span>
                    <code style={{
                      backgroundColor: 'var(--bg-tertiary)',
                      padding: '2px 6px',
                      borderRadius: '4px',
                      fontSize: '12px',
                      color: 'var(--text-muted)',
                    }}>
                      {apiKey.key_prefix}...
                    </code>
                  </div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--spacing-md)' }}>
                    {apiKey.created_at && (
                      <span className="text-muted" style={{ fontSize: '12px' }}>
                        Created {formatDate(apiKey.created_at)}
                      </span>
                    )}
                    {apiKey.last_used_at && (
                      <span className="text-muted" style={{ fontSize: '12px' }}>
                        Last used {formatDate(apiKey.last_used_at)}
                      </span>
                    )}
                  </div>
                </div>
                <HoldToConfirmButton
                  label="Revoke"
                  holdingLabel="Hold to revoke..."
                  onConfirm={() => handleRevokeApiKeyDirect(apiKey.id)}
                  variant="danger"
                  duration={3000}
                  style={{ fontSize: '13px', padding: '6px 12px' }}
                />
              </div>
            ))
          )}
        </>
      )}


      {/* Delete User Confirmation Modal */}
      <ConfirmModal
        isOpen={deleteUserConfirmId !== null}
        title="Delete User"
        message="Are you sure you want to delete this user? This cannot be undone."
        confirmLabel="Delete"
        cancelLabel="Cancel"
        variant="danger"
        onConfirm={() => deleteUserConfirmId && handleUserAction(deleteUserConfirmId, 'delete')}
        onCancel={() => setDeleteUserConfirmId(null)}
      />

      {/* Whitelist Enable Modal - Custom 3-button modal */}
      {showWhitelistModal && (
        <div
          style={{
            position: 'fixed',
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            backgroundColor: 'rgba(0, 0, 0, 0.6)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            zIndex: 1000,
          }}
          onClick={() => setShowWhitelistModal(false)}
        >
          <div
            style={{
              backgroundColor: 'var(--bg-secondary)',
              borderRadius: 'var(--radius-md)',
              border: '1px solid var(--border-primary)',
              boxShadow: 'var(--shadow-lg)',
              maxWidth: '450px',
              width: '90%',
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                padding: 'var(--spacing-md)',
                borderBottom: '1px solid var(--border-primary)',
              }}
            >
              <h3 style={{ margin: 0, fontSize: '16px', fontWeight: 600 }}>Enable Whitelist Mode</h3>
              <button
                onClick={() => setShowWhitelistModal(false)}
                style={{
                  background: 'none',
                  border: 'none',
                  cursor: 'pointer',
                  padding: '4px',
                  color: 'var(--text-secondary)',
                  fontSize: '18px',
                  lineHeight: 1,
                }}
              >
                &times;
              </button>
            </div>

            <div
              style={{
                padding: 'var(--spacing-lg) var(--spacing-md)',
                color: 'var(--text-secondary)',
                fontSize: '14px',
                lineHeight: 1.5,
              }}
            >
              When whitelist mode is enabled, only whitelisted users can sign in.
              <br /><br />
              Would you like to add all current users to the whitelist?
            </div>

            <div
              style={{
                display: 'flex',
                justifyContent: 'flex-end',
                gap: 'var(--spacing-sm)',
                padding: 'var(--spacing-md)',
                borderTop: '1px solid var(--border-primary)',
              }}
            >
              <button onClick={() => setShowWhitelistModal(false)}>
                Cancel
              </button>
              <button onClick={() => handleWhitelistConfirm(false)}>
                Enable Only
              </button>
              <button className="primary" onClick={() => handleWhitelistConfirm(true)}>
                Whitelist All
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Invite User Modal */}
      {showInviteModal && (
        <div
          style={{
            position: 'fixed',
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            backgroundColor: 'rgba(0, 0, 0, 0.6)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            zIndex: 1000,
          }}
          onClick={() => {
            setShowInviteModal(false);
            setInviteEmail('');
            setInvitePreWhitelist(false);
            setInviteError('');
          }}
        >
          <div
            style={{
              backgroundColor: 'var(--bg-secondary)',
              borderRadius: 'var(--radius-md)',
              border: '1px solid var(--border-primary)',
              boxShadow: 'var(--shadow-lg)',
              maxWidth: '450px',
              width: '90%',
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                padding: 'var(--spacing-md)',
                borderBottom: '1px solid var(--border-primary)',
              }}
            >
              <h3 style={{ margin: 0, fontSize: '16px', fontWeight: 600 }}>Invite User</h3>
              <button
                onClick={() => {
                  setShowInviteModal(false);
                  setInviteEmail('');
                  setInvitePreWhitelist(false);
                  setInviteError('');
                }}
                style={{
                  background: 'none',
                  border: 'none',
                  cursor: 'pointer',
                  padding: '4px',
                  color: 'var(--text-secondary)',
                  fontSize: '18px',
                  lineHeight: 1,
                }}
              >
                &times;
              </button>
            </div>

            <form onSubmit={handleInviteUser}>
              <div style={{ padding: 'var(--spacing-lg) var(--spacing-md)' }}>
                <div style={{ marginBottom: 'var(--spacing-md)' }}>
                  <label htmlFor="inviteEmail" style={{ fontSize: '13px', color: 'var(--text-muted)', marginBottom: 'var(--spacing-xs)', display: 'block' }}>
                    Email address
                  </label>
                  <input
                    id="inviteEmail"
                    type="email"
                    value={inviteEmail}
                    onChange={(e) => setInviteEmail(e.target.value)}
                    placeholder="user@example.com"
                    required
                    autoFocus
                  />
                </div>

                <label style={{ display: 'flex', alignItems: 'center', gap: 'var(--spacing-sm)', cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={invitePreWhitelist}
                    onChange={(e) => setInvitePreWhitelist(e.target.checked)}
                    style={{ width: 16, height: 16 }}
                  />
                  <span style={{ fontSize: '14px' }}>Pre-approve (add to whitelist)</span>
                </label>
                <p className="text-muted" style={{ fontSize: '12px', marginTop: 'var(--spacing-xs)', marginLeft: '24px' }}>
                  If whitelist mode is enabled, this user will be able to sign in immediately.
                </p>

                {inviteError && (
                  <div className="message error" style={{ marginTop: 'var(--spacing-md)' }}>
                    {inviteError}
                  </div>
                )}
              </div>

              <div
                style={{
                  display: 'flex',
                  justifyContent: 'flex-end',
                  gap: 'var(--spacing-sm)',
                  padding: 'var(--spacing-md)',
                  borderTop: '1px solid var(--border-primary)',
                }}
              >
                <button
                  type="button"
                  onClick={() => {
                    setShowInviteModal(false);
                    setInviteEmail('');
                    setInvitePreWhitelist(false);
                    setInviteError('');
                  }}
                >
                  Cancel
                </button>
                <button type="submit" className="primary" disabled={inviting || !inviteEmail.trim()}>
                  {inviting ? 'Sending...' : 'Send Invitation'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Create API Key Modal */}
      {showCreateKeyModal && !newlyCreatedKey && (
        <div
          style={{
            position: 'fixed',
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            backgroundColor: 'rgba(0, 0, 0, 0.6)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            zIndex: 1000,
          }}
          onClick={() => {
            setShowCreateKeyModal(false);
            setNewKeyName('');
          }}
        >
          <div
            style={{
              backgroundColor: 'var(--bg-secondary)',
              borderRadius: 'var(--radius-md)',
              border: '1px solid var(--border-primary)',
              boxShadow: 'var(--shadow-lg)',
              maxWidth: '450px',
              width: '90%',
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                padding: 'var(--spacing-md)',
                borderBottom: '1px solid var(--border-primary)',
              }}
            >
              <h3 style={{ margin: 0, fontSize: '16px', fontWeight: 600 }}>Create API Key</h3>
              <button
                onClick={() => {
                  setShowCreateKeyModal(false);
                  setNewKeyName('');
                }}
                style={{
                  background: 'none',
                  border: 'none',
                  cursor: 'pointer',
                  padding: '4px',
                  color: 'var(--text-secondary)',
                  fontSize: '18px',
                  lineHeight: 1,
                }}
              >
                &times;
              </button>
            </div>

            <form onSubmit={handleCreateApiKey}>
              <div style={{ padding: 'var(--spacing-lg) var(--spacing-md)' }}>
                <div>
                  <label htmlFor="keyName" style={{ fontSize: '13px', color: 'var(--text-muted)', marginBottom: 'var(--spacing-xs)', display: 'block' }}>
                    Key name (optional)
                  </label>
                  <input
                    id="keyName"
                    type="text"
                    value={newKeyName}
                    onChange={(e) => setNewKeyName(e.target.value)}
                    placeholder="e.g., Production API"
                    autoFocus
                  />
                  <p className="text-muted" style={{ fontSize: '12px', marginTop: 'var(--spacing-xs)' }}>
                    A name to help you identify this key.
                  </p>
                </div>
              </div>

              <div
                style={{
                  display: 'flex',
                  justifyContent: 'flex-end',
                  gap: 'var(--spacing-sm)',
                  padding: 'var(--spacing-md)',
                  borderTop: '1px solid var(--border-primary)',
                }}
              >
                <button
                  type="button"
                  onClick={() => {
                    setShowCreateKeyModal(false);
                    setNewKeyName('');
                  }}
                >
                  Cancel
                </button>
                <button type="submit" className="primary" disabled={creatingKey}>
                  {creatingKey ? 'Creating...' : 'Create Key'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Show Newly Created Key Modal */}
      {newlyCreatedKey && (
        <div
          style={{
            position: 'fixed',
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            backgroundColor: 'rgba(0, 0, 0, 0.6)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            zIndex: 1000,
          }}
        >
          <div
            style={{
              backgroundColor: 'var(--bg-secondary)',
              borderRadius: 'var(--radius-md)',
              border: '1px solid var(--border-primary)',
              boxShadow: 'var(--shadow-lg)',
              maxWidth: '550px',
              width: '90%',
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                padding: 'var(--spacing-md)',
                borderBottom: '1px solid var(--border-primary)',
              }}
            >
              <h3 style={{ margin: 0, fontSize: '16px', fontWeight: 600 }}>API Key Created</h3>
            </div>

            <div style={{ padding: 'var(--spacing-lg) var(--spacing-md)' }}>
              <div className="message warning" style={{ marginBottom: 'var(--spacing-md)' }}>
                Copy this key now. You won&apos;t be able to see it again!
              </div>

              <div
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 'var(--spacing-sm)',
                  backgroundColor: 'var(--bg-tertiary)',
                  padding: 'var(--spacing-sm) var(--spacing-md)',
                  borderRadius: 'var(--radius-sm)',
                  border: '1px solid var(--border-primary)',
                }}
              >
                <code style={{ flex: 1, fontSize: '13px', wordBreak: 'break-all' }}>
                  {newlyCreatedKey}
                </code>
                <button
                  onClick={() => copyToClipboard(newlyCreatedKey, 'newKey')}
                  style={{ padding: '6px 12px', fontSize: '12px', whiteSpace: 'nowrap' }}
                >
                  {copiedField === 'newKey' ? 'Copied!' : 'Copy'}
                </button>
              </div>
            </div>

            <div
              style={{
                display: 'flex',
                justifyContent: 'flex-end',
                padding: 'var(--spacing-md)',
                borderTop: '1px solid var(--border-primary)',
              }}
            >
              <button
                className="primary"
                onClick={() => {
                  setNewlyCreatedKey(null);
                  setShowCreateKeyModal(false);
                }}
              >
                Done
              </button>
            </div>
          </div>
        </div>
      )}

    </>
  );
}
