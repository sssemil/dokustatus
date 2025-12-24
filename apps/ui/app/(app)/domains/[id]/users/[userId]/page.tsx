'use client';

import { useState, useEffect, useCallback } from 'react';
import { useParams, useRouter } from 'next/navigation';
import ConfirmModal from '@/components/ConfirmModal';

type EndUser = {
  id: string;
  email: string;
  email_verified_at: string | null;
  last_login_at: string | null;
  is_frozen: boolean;
  is_whitelisted: boolean;
  created_at: string | null;
};

export default function UserDetailPage() {
  const params = useParams();
  const router = useRouter();
  const domainId = params.id as string;
  const userId = params.userId as string;

  const [user, setUser] = useState<EndUser | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [actionLoading, setActionLoading] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

  const fetchUser = useCallback(async () => {
    try {
      const res = await fetch(`/api/domains/${domainId}/end-users/${userId}`, { credentials: 'include' });
      if (res.ok) {
        const data = await res.json();
        setUser(data);
      } else {
        setError('User not found');
      }
    } catch {
      setError('Failed to load user');
    } finally {
      setLoading(false);
    }
  }, [domainId, userId]);

  useEffect(() => {
    fetchUser();
  }, [fetchUser]);

  const handleAction = async (action: 'freeze' | 'unfreeze' | 'whitelist' | 'unwhitelist' | 'delete') => {
    setActionLoading(true);
    if (action === 'delete') {
      setShowDeleteConfirm(false);
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
          </div>
        </div>
        <button className="danger" onClick={() => setShowDeleteConfirm(true)} disabled={actionLoading}>
          Delete
        </button>
      </div>

      {error && (
        <div className="message error" style={{ marginBottom: 'var(--spacing-md)' }}>
          {error}
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

      <ConfirmModal
        isOpen={showDeleteConfirm}
        title="Delete User"
        message="Are you sure you want to delete this user? This cannot be undone."
        confirmLabel="Delete"
        cancelLabel="Cancel"
        variant="danger"
        onConfirm={() => handleAction('delete')}
        onCancel={() => setShowDeleteConfirm(false)}
      />
    </>
  );
}
