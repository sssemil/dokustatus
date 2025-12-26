'use client';

import { useState, useEffect } from 'react';
import { useAppContext } from '../layout';
import { getRootDomain } from '@/lib/domain-utils';

export default function ProfilePage() {
  const { user, displayDomain } = useAppContext();
  const [email, setEmail] = useState('');
  const [originalEmail, setOriginalEmail] = useState('');
  const [updateStatus, setUpdateStatus] = useState<'idle' | 'loading' | 'success' | 'error'>('idle');
  const [updateMessage, setUpdateMessage] = useState('');
  const [deleteStatus, setDeleteStatus] = useState<'idle' | 'confirming' | 'loading'>('idle');
  const [errorMessage, setErrorMessage] = useState('');

  useEffect(() => {
    if (user) {
      setEmail(user.email);
      setOriginalEmail(user.email);
    }
  }, [user]);

  const handleUpdateEmail = async (e: React.FormEvent) => {
    e.preventDefault();
    if (email === originalEmail) return;

    setUpdateStatus('loading');
    setUpdateMessage('');

    // For now, just show a message that email update requires verification
    setTimeout(() => {
      setUpdateStatus('idle');
      setUpdateMessage('Email updates are not yet available. Coming soon!');
    }, 500);
  };

  const hasEmailChanged = email !== originalEmail;

  const handleLogout = async () => {
    const apiDomain = getRootDomain(window.location.hostname);
    await fetch(`/api/public/domain/${apiDomain}/auth/logout`, {
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
    const apiDomain = getRootDomain(window.location.hostname);

    try {
      const res = await fetch(`/api/public/domain/${apiDomain}/auth/account`, {
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

  return (
    <>
      <div className="card">
        <div className="text-center" style={{ marginBottom: 'var(--spacing-lg)' }}>
          <h2 style={{ marginBottom: 'var(--spacing-xs)', borderBottom: 'none', paddingBottom: 0 }}>
            {displayDomain}
          </h2>
          <p className="text-muted" style={{ fontSize: '13px', marginBottom: 0 }}>Your account</p>
        </div>

        <form onSubmit={handleUpdateEmail}>
          <label htmlFor="email" style={{ fontSize: '13px', color: 'var(--text-muted)', marginBottom: 'var(--spacing-xs)', display: 'block' }}>
            Email address
          </label>
          <input
            id="email"
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            style={{ marginBottom: 'var(--spacing-sm)' }}
          />

          {updateMessage && (
            <div className={`message ${updateStatus === 'success' ? 'success' : 'error'}`} style={{ marginBottom: 'var(--spacing-sm)' }}>
              {updateMessage}
            </div>
          )}

          <div style={{ display: 'flex', gap: 'var(--spacing-sm)', marginBottom: 'var(--spacing-lg)' }}>
            <button
              type="submit"
              className="primary"
              disabled={!hasEmailChanged || updateStatus === 'loading'}
              style={{ flex: 1 }}
            >
              {updateStatus === 'loading' ? 'Updating...' : 'Update email'}
            </button>
          </div>
        </form>

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
