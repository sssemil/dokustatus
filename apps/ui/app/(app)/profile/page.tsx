'use client';

import { useState, useEffect } from 'react';
import { useAppContext } from '../layout';

export default function ProfilePage() {
  const { user, isIngress, displayDomain } = useAppContext();
  const [deleteStatus, setDeleteStatus] = useState<'idle' | 'confirming' | 'loading'>('idle');
  const [errorMessage, setErrorMessage] = useState('');

  // Redirect to ingress if accessed from main app
  useEffect(() => {
    if (!isIngress) {
      window.location.href = 'https://reauth.reauth.dev/profile';
    }
  }, [isIngress]);

  const handleLogout = async () => {
    const hostname = window.location.hostname;
    await fetch(`/api/public/domain/${hostname}/auth/logout`, {
      method: 'POST',
      credentials: 'include',
    });
    window.location.href = '/';
  };

  const handleDeleteAccount = async () => {
    if (deleteStatus === 'idle') {
      setDeleteStatus('confirming');
      return;
    }

    setDeleteStatus('loading');
    const hostname = window.location.hostname;

    try {
      const res = await fetch(`/api/public/domain/${hostname}/auth/account`, {
        method: 'DELETE',
        credentials: 'include',
      });

      if (res.ok) {
        window.location.href = '/';
      } else {
        setDeleteStatus('idle');
        setErrorMessage('Failed to delete account. Please try again.');
      }
    } catch {
      setDeleteStatus('idle');
      setErrorMessage('Network error. Please try again.');
    }
  };

  // Show loading while redirecting from main app
  if (!isIngress) {
    return (
      <div className="flex items-center justify-center" style={{ minHeight: '200px' }}>
        <div className="spinner" />
      </div>
    );
  }

  return (
    <>
      <div className="card">
        <div className="text-center" style={{ marginBottom: 'var(--spacing-lg)' }}>
          <h2 style={{ marginBottom: 'var(--spacing-xs)', borderBottom: 'none', paddingBottom: 0 }}>
            {displayDomain}
          </h2>
          <p className="text-muted" style={{ fontSize: '13px', marginBottom: 0 }}>Your account</p>
        </div>

        <div style={{ marginBottom: 'var(--spacing-lg)' }}>
          <label style={{ fontSize: '13px', color: 'var(--text-muted)', marginBottom: 'var(--spacing-xs)', display: 'block' }}>
            Email address
          </label>
          <p style={{ margin: 0, wordBreak: 'break-all' }}>{user?.email}</p>
        </div>

        <button onClick={handleLogout} style={{ width: '100%' }}>
          Sign out
        </button>
      </div>

      <div className="card" style={{ borderColor: 'var(--accent-red)', marginTop: 'var(--spacing-lg)' }}>
        <h3 style={{ color: 'var(--accent-red)', fontSize: '14px', marginBottom: 'var(--spacing-sm)' }}>Danger Zone</h3>
        <p className="text-muted" style={{ fontSize: '13px', marginBottom: 'var(--spacing-md)' }}>
          Permanently delete your account and all associated data.
        </p>

        {errorMessage && (
          <div className="message error" style={{ marginBottom: 'var(--spacing-md)' }}>
            {errorMessage}
          </div>
        )}

        {deleteStatus === 'confirming' && (
          <div className="message warning" style={{ marginBottom: 'var(--spacing-md)' }}>
            Are you sure? This action cannot be undone.
          </div>
        )}

        <div style={{ display: 'flex', gap: 'var(--spacing-sm)' }}>
          <button
            className="danger"
            onClick={handleDeleteAccount}
            disabled={deleteStatus === 'loading'}
            style={{ flex: 1 }}
          >
            {deleteStatus === 'loading'
              ? 'Deleting...'
              : deleteStatus === 'confirming'
              ? 'Yes, delete'
              : 'Delete Account'}
          </button>

          {deleteStatus === 'confirming' && (
            <button onClick={() => setDeleteStatus('idle')} style={{ flex: 1 }}>
              Cancel
            </button>
          )}
        </div>
      </div>
    </>
  );
}
