'use client';

import { useState, useEffect, useCallback } from 'react';
import { useParams, useRouter } from 'next/navigation';
import HoldToConfirmButton from '@/components/HoldToConfirmButton';

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

type Role = {
  id: string;
  name: string;
  user_count: number;
};

export default function UserDetailPage() {
  const params = useParams();
  const router = useRouter();
  const domainId = params.id as string;
  const userId = params.userId as string;

  const [user, setUser] = useState<EndUser | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');
  const [actionLoading, setActionLoading] = useState(false);

  // Roles state
  const [availableRoles, setAvailableRoles] = useState<Role[]>([]);
  const [selectedRoles, setSelectedRoles] = useState<string[]>([]);
  const [savingRoles, setSavingRoles] = useState(false);

  const fetchUser = useCallback(async () => {
    try {
      const res = await fetch(`/api/domains/${domainId}/end-users/${userId}`, { credentials: 'include' });
      if (res.ok) {
        const data = await res.json();
        setUser(data);
        setSelectedRoles(data.roles || []);
      } else {
        setError('User not found');
      }
    } catch {
      setError('Failed to load user');
    } finally {
      setLoading(false);
    }
  }, [domainId, userId]);

  const fetchRoles = useCallback(async () => {
    try {
      const res = await fetch(`/api/domains/${domainId}/roles`, { credentials: 'include' });
      if (res.ok) {
        const data = await res.json();
        setAvailableRoles(data);
      }
    } catch {
      // Ignore
    }
  }, [domainId]);

  useEffect(() => {
    fetchUser();
    fetchRoles();
  }, [fetchUser, fetchRoles]);

  const handleAction = async (action: 'freeze' | 'unfreeze' | 'whitelist' | 'unwhitelist' | 'delete') => {
    setActionLoading(true);
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
        if (action === 'delete') {
          router.push(`/domains/${domainId}`);
        } else {
          fetchUser();
        }
      } else {
        setError(`Failed to ${action} user`);
      }
    } catch {
      setError('Network error. Please try again.');
    } finally {
      setActionLoading(false);
    }
  };

  const handleRoleToggle = (roleName: string) => {
    setSelectedRoles((prev) =>
      prev.includes(roleName)
        ? prev.filter((r) => r !== roleName)
        : [...prev, roleName]
    );
  };

  const handleSaveRoles = async () => {
    setSavingRoles(true);
    setError('');
    setSuccess('');

    try {
      const res = await fetch(`/api/domains/${domainId}/end-users/${userId}/roles`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ roles: selectedRoles }),
        credentials: 'include',
      });

      if (res.ok) {
        setSuccess('Roles updated successfully');
        fetchUser();
      } else {
        const errData = await res.json().catch(() => ({}));
        setError(errData.message || 'Failed to update roles');
      }
    } catch {
      setError('Network error. Please try again.');
    } finally {
      setSavingRoles(false);
    }
  };

  const rolesChanged = user ? JSON.stringify([...selectedRoles].sort()) !== JSON.stringify([...(user.roles || [])].sort()) : false;

  const formatDate = (dateString: string | null) => {
    if (!dateString) return 'Never';
    const date = new Date(dateString);
    return date.toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  if (loading) {
    return (
      <div style={{ display: 'flex', justifyContent: 'center', padding: 'var(--spacing-xl)' }}>
        <span className="spinner" />
      </div>
    );
  }

  if (!user) {
    return (
      <div className="card">
        <p className="text-muted">{error || 'User not found'}</p>
        <button onClick={() => router.push(`/domains/${domainId}`)}>Back to domain</button>
      </div>
    );
  }

  return (
    <>
      {/* Header */}
      <button
        onClick={() => router.push(`/domains/${domainId}`)}
        style={{
          background: 'none',
          border: 'none',
          color: 'var(--text-muted)',
          cursor: 'pointer',
          padding: 0,
          marginBottom: 'var(--spacing-md)',
        }}
      >
        &larr; Back to domain
      </button>

      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: 'var(--spacing-lg)' }}>
        <div>
          <h1 style={{ marginBottom: 'var(--spacing-xs)' }}>{user.email}</h1>
          <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--spacing-sm)' }}>
            {user.is_frozen && (
              <span style={{
                padding: '4px 8px',
                borderRadius: 'var(--radius-sm)',
                backgroundColor: 'var(--accent-red)',
                color: '#fff',
                fontSize: '12px',
                fontWeight: 500,
              }}>
                Frozen
              </span>
            )}
            {user.is_whitelisted && (
              <span style={{
                padding: '4px 8px',
                borderRadius: 'var(--radius-sm)',
                backgroundColor: 'var(--accent-green)',
                color: '#000',
                fontSize: '12px',
                fontWeight: 500,
              }}>
                Whitelisted
              </span>
            )}
            {user.email_verified_at && (
              <span style={{
                padding: '4px 8px',
                borderRadius: 'var(--radius-sm)',
                backgroundColor: 'var(--accent-blue)',
                color: '#000',
                fontSize: '12px',
                fontWeight: 500,
              }}>
                Verified
              </span>
            )}
            {user.roles && user.roles.map((role) => (
              <span
                key={role}
                style={{
                  padding: '4px 8px',
                  borderRadius: 'var(--radius-sm)',
                  backgroundColor: 'var(--bg-tertiary)',
                  color: 'var(--text-secondary)',
                  fontSize: '12px',
                  fontWeight: 500,
                }}
              >
                {role}
              </span>
            ))}
          </div>
        </div>
        <HoldToConfirmButton
          label="Delete"
          holdingLabel="Hold to delete..."
          onConfirm={() => handleAction('delete')}
          variant="danger"
          disabled={actionLoading}
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

      {/* User Details */}
      <div className="card" style={{ marginBottom: 'var(--spacing-lg)' }}>
        <h2 style={{ marginBottom: 'var(--spacing-md)' }}>User Details</h2>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-md)' }}>
          <div>
            <span className="text-muted" style={{ fontSize: '12px', display: 'block', marginBottom: '4px' }}>Email</span>
            <span>{user.email}</span>
          </div>
          <div>
            <span className="text-muted" style={{ fontSize: '12px', display: 'block', marginBottom: '4px' }}>Last Login</span>
            <span>{formatDate(user.last_login_at)}</span>
          </div>
          <div>
            <span className="text-muted" style={{ fontSize: '12px', display: 'block', marginBottom: '4px' }}>Email Verified</span>
            <span>{formatDate(user.email_verified_at)}</span>
          </div>
          <div>
            <span className="text-muted" style={{ fontSize: '12px', display: 'block', marginBottom: '4px' }}>Created</span>
            <span>{formatDate(user.created_at)}</span>
          </div>
        </div>
      </div>

      {/* Roles */}
      <div className="card" style={{ marginBottom: 'var(--spacing-lg)' }}>
        <h2 style={{ marginBottom: 'var(--spacing-md)' }}>Roles</h2>
        {availableRoles.length === 0 ? (
          <p className="text-muted" style={{ margin: 0 }}>
            No roles available. Create roles in the Roles tab of the domain page.
          </p>
        ) : (
          <>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 'var(--spacing-sm)', marginBottom: 'var(--spacing-md)' }}>
              {availableRoles.map((role) => {
                const isSelected = selectedRoles.includes(role.name);
                return (
                  <button
                    key={role.id}
                    onClick={() => handleRoleToggle(role.name)}
                    style={{
                      padding: '6px 12px',
                      borderRadius: 'var(--radius-sm)',
                      border: isSelected ? '2px solid var(--accent-blue)' : '2px solid var(--border-primary)',
                      backgroundColor: isSelected ? 'var(--accent-blue)' : 'var(--bg-tertiary)',
                      color: isSelected ? '#000' : 'var(--text-primary)',
                      cursor: 'pointer',
                      fontSize: '13px',
                      fontWeight: 500,
                      transition: 'all 0.15s ease',
                    }}
                  >
                    {role.name}
                  </button>
                );
              })}
            </div>
            <button
              className="primary"
              onClick={handleSaveRoles}
              disabled={!rolesChanged || savingRoles}
              style={{ opacity: rolesChanged ? 1 : 0.5 }}
            >
              {savingRoles ? 'Saving...' : 'Save Roles'}
            </button>
          </>
        )}
      </div>

      {/* Actions */}
      <div className="card">
        <h2 style={{ marginBottom: 'var(--spacing-md)' }}>Actions</h2>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--spacing-md)' }}>
          {/* Freeze/Unfreeze */}
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <div>
              <div style={{ fontWeight: 600, marginBottom: '4px' }}>
                {user.is_frozen ? 'Unfreeze Account' : 'Freeze Account'}
              </div>
              <p className="text-muted" style={{ margin: 0, fontSize: '13px' }}>
                {user.is_frozen
                  ? 'Allow this user to sign in again.'
                  : 'Prevent this user from signing in.'}
              </p>
            </div>
            <button
              onClick={() => handleAction(user.is_frozen ? 'unfreeze' : 'freeze')}
              disabled={actionLoading}
            >
              {user.is_frozen ? 'Unfreeze' : 'Freeze'}
            </button>
          </div>

          <div style={{ borderTop: '1px solid var(--border-primary)' }} />

          {/* Whitelist/Unwhitelist */}
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <div>
              <div style={{ fontWeight: 600, marginBottom: '4px' }}>
                {user.is_whitelisted ? 'Remove from Whitelist' : 'Add to Whitelist'}
              </div>
              <p className="text-muted" style={{ margin: 0, fontSize: '13px' }}>
                {user.is_whitelisted
                  ? 'Remove this user from the whitelist.'
                  : 'Add this user to the whitelist. When whitelist mode is enabled, only whitelisted users can sign in.'}
              </p>
            </div>
            <button
              onClick={() => handleAction(user.is_whitelisted ? 'unwhitelist' : 'whitelist')}
              disabled={actionLoading}
            >
              {user.is_whitelisted ? 'Remove' : 'Whitelist'}
            </button>
          </div>
        </div>
      </div>
    </>
  );
}
