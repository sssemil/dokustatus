'use client';

import { useState, useEffect } from 'react';
import { useRouter } from 'next/navigation';

export default function ProfilePage() {
  const [email, setEmail] = useState('');
  const [originalEmail, setOriginalEmail] = useState('');
  const [updateStatus, setUpdateStatus] = useState<'idle' | 'loading' | 'success' | 'error'>('idle');
  const [deleteStatus, setDeleteStatus] = useState<'idle' | 'confirming' | 'loading'>('idle');
  const [errorMessage, setErrorMessage] = useState('');
  const router = useRouter();

  useEffect(() => {
    const fetchUser = async () => {
      try {
        const res = await fetch('/api/user/me', { credentials: 'include' });
        if (res.ok) {
          const data = await res.json();
          setEmail(data.email);
          setOriginalEmail(data.email);
        }
      } catch {
        // Error handled by layout
      }
    };
    fetchUser();
  }, []);

  const handleUpdateEmail = async (e: React.FormEvent) => {
    e.preventDefault();
    if (email === originalEmail) return;

    setUpdateStatus('loading');
    setErrorMessage('');

    // For now, just show a message that email update requires verification
    setTimeout(() => {
      setUpdateStatus('idle');
      setErrorMessage('Email updates are not yet available. Coming soon!');
    }, 500);
  };

  const handleDeleteAccount = async () => {
    if (deleteStatus === 'idle') {
      setDeleteStatus('confirming');
      return;
    }

    setDeleteStatus('loading');

    try {
      const res = await fetch('/api/user/delete', {
        method: 'DELETE',
        credentials: 'include',
      });

      if (res.ok) {
        router.push('/');
      } else {
        setDeleteStatus('idle');
        setErrorMessage('Failed to delete account. Please try again.');
      }
    } catch {
      setDeleteStatus('idle');
      setErrorMessage('Network error. Please try again.');
    }
  };

  const hasEmailChanged = email !== originalEmail;

  return (
    <>
      <h1>Profile</h1>

      <div className="card">
        <h2>Your email</h2>
        <p className="text-muted" style={{ marginBottom: 'var(--spacing-md)' }}>
          This is the email address associated with your account.
        </p>

        <form onSubmit={handleUpdateEmail}>
          <label htmlFor="email">Email address</label>
          <input
            id="email"
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            style={{ marginBottom: 'var(--spacing-md)' }}
          />

          {errorMessage && (
            <div className="message error" style={{ marginBottom: 'var(--spacing-md)' }}>
              {errorMessage}
            </div>
          )}

          {updateStatus === 'success' && (
            <div className="message success" style={{ marginBottom: 'var(--spacing-md)' }}>
              Email updated successfully!
            </div>
          )}

          <button
            type="submit"
            className="primary"
            disabled={!hasEmailChanged || updateStatus === 'loading'}
          >
            {updateStatus === 'loading' ? 'Updating...' : 'Update email'}
          </button>
        </form>
      </div>

      <div className="card" style={{ borderColor: 'var(--accent-red)' }}>
        <h2 style={{ color: 'var(--accent-red)' }}>Danger Zone</h2>
        <p className="text-muted" style={{ marginBottom: 'var(--spacing-md)' }}>
          Permanently delete your account and all associated data. This action cannot be undone.
        </p>

        {deleteStatus === 'confirming' && (
          <div className="message warning" style={{ marginBottom: 'var(--spacing-md)' }}>
            Are you sure? This will permanently delete your account and all data.
          </div>
        )}

        <div style={{ display: 'flex', gap: 'var(--spacing-sm)' }}>
          <button
            className="danger"
            onClick={handleDeleteAccount}
            disabled={deleteStatus === 'loading'}
          >
            {deleteStatus === 'loading'
              ? 'Deleting...'
              : deleteStatus === 'confirming'
              ? 'Yes, delete my account'
              : 'Delete Account'}
          </button>

          {deleteStatus === 'confirming' && (
            <button onClick={() => setDeleteStatus('idle')}>
              Cancel
            </button>
          )}
        </div>
      </div>
    </>
  );
}
