'use client';

import { useState, useEffect } from 'react';
import { LogOut, Trash2, AlertTriangle, Link2, Unlink } from 'lucide-react';
import { useAppContext } from '../layout';
import { getRootDomain } from '@/lib/domain-utils';
import { Card, Button, Input, ConfirmModal, Badge } from '@/components/ui';
import { useToast } from '@/contexts/ToastContext';

export default function ProfilePage() {
  const { user, displayDomain, refetchUser } = useAppContext();
  const { addToast } = useToast();
  const [email, setEmail] = useState('');
  const [originalEmail, setOriginalEmail] = useState('');
  const [updating, setUpdating] = useState(false);
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const [showUnlinkModal, setShowUnlinkModal] = useState(false);
  const [unlinking, setUnlinking] = useState(false);

  useEffect(() => {
    if (user) {
      setEmail(user.email);
      setOriginalEmail(user.email);
    }
  }, [user]);

  const handleUpdateEmail = async (e: React.FormEvent) => {
    e.preventDefault();
    if (email === originalEmail) return;

    setUpdating(true);
    // For now, just show a message that email update requires verification
    setTimeout(() => {
      setUpdating(false);
      addToast('Email updates are not yet available. Coming soon!', 'info');
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
    const apiDomain = getRootDomain(window.location.hostname);
    try {
      const res = await fetch(`/api/public/domain/${apiDomain}/auth/account`, {
        method: 'DELETE',
        credentials: 'include',
      });

      if (res.ok) {
        window.location.href = '/';
      } else {
        addToast('Failed to delete account', 'error');
        setShowDeleteModal(false);
      }
    } catch {
      addToast('Network error', 'error');
      setShowDeleteModal(false);
    }
  };

  const handleUnlinkGoogle = async () => {
    const apiDomain = getRootDomain(window.location.hostname);
    setUnlinking(true);
    try {
      const res = await fetch(`/api/public/domain/${apiDomain}/auth/google/unlink`, {
        method: 'POST',
        credentials: 'include',
      });

      if (res.ok) {
        addToast('Google account unlinked', 'success');
        setShowUnlinkModal(false);
        refetchUser();
      } else {
        addToast('Failed to unlink Google account', 'error');
      }
    } catch {
      addToast('Network error', 'error');
    } finally {
      setUnlinking(false);
    }
  };

  return (
    <div className="space-y-6 max-w-md mx-auto">
      {/* Page header */}
      <div className="text-center">
        <h1 className="text-2xl font-bold text-white">My Profile</h1>
        <p className="text-sm text-zinc-400 mt-1">{displayDomain}</p>
      </div>

      {/* Profile card */}
      <Card className="p-6">
        <div className="flex items-center gap-4 mb-6">
          <div className="w-16 h-16 bg-gradient-to-br from-blue-500 to-purple-600 rounded-full flex items-center justify-center text-2xl font-bold text-white">
            {user?.email?.charAt(0).toUpperCase() || 'U'}
          </div>
          <div>
            <p className="font-medium text-white">{user?.email}</p>
            <p className="text-sm text-zinc-500">Account</p>
          </div>
        </div>

        <form onSubmit={handleUpdateEmail} className="space-y-4">
          <div className="space-y-2">
            <label htmlFor="email" className="text-sm text-zinc-400">Email address</label>
            <Input
              id="email"
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
            />
          </div>

          <Button
            type="submit"
            variant="primary"
            disabled={!hasEmailChanged || updating}
            className="w-full"
          >
            {updating ? 'Updating...' : 'Update email'}
          </Button>
        </form>

        <div className="border-t border-zinc-800 mt-6 pt-6">
          <Button variant="ghost" onClick={handleLogout} className="w-full">
            <LogOut size={16} className="mr-2" />
            Sign out
          </Button>
        </div>
      </Card>

      {/* Linked Accounts */}
      <Card className="p-6">
        <div className="flex items-center gap-2 mb-4">
          <Link2 size={18} className="text-blue-400" />
          <h2 className="font-semibold text-white">Linked Accounts</h2>
        </div>

        <div className="space-y-3">
          {/* Google Account */}
          <div className="flex items-center justify-between p-3 bg-zinc-800/50 rounded-lg">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 bg-zinc-700 rounded-lg flex items-center justify-center">
                <svg className="w-5 h-5" viewBox="0 0 24 24">
                  <path fill="#4285F4" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"/>
                  <path fill="#34A853" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"/>
                  <path fill="#FBBC05" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"/>
                  <path fill="#EA4335" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"/>
                </svg>
              </div>
              <div>
                <p className="font-medium text-zinc-300">Google</p>
                <p className="text-xs text-zinc-500">
                  {user?.googleLinked ? 'Connected' : 'Not connected'}
                </p>
              </div>
            </div>
            {user?.googleLinked ? (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setShowUnlinkModal(true)}
                className="text-zinc-400 hover:text-red-400"
              >
                <Unlink size={14} className="mr-1" />
                Unlink
              </Button>
            ) : (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => window.location.href = '/auth/google'}
                className="text-blue-400 hover:text-blue-300"
              >
                <Link2 size={14} className="mr-1" />
                Link
              </Button>
            )}
          </div>
        </div>

        <p className="text-xs text-zinc-500 mt-4">
          {user?.googleLinked
            ? 'Your Google account is linked for faster sign-in.'
            : 'Link your Google account for faster sign-in next time.'}
        </p>
      </Card>

      {/* Danger zone */}
      <Card className="p-6 border-red-500/30">
        <div className="flex items-center gap-2 mb-4">
          <AlertTriangle size={18} className="text-red-400" />
          <h2 className="font-semibold text-red-400">Danger Zone</h2>
        </div>
        <p className="text-sm text-zinc-400 mb-4">
          Permanently delete your account and all associated data.
        </p>
        <Button variant="danger" onClick={() => setShowDeleteModal(true)} className="w-full">
          <Trash2 size={16} className="mr-2" />
          Delete Account
        </Button>
      </Card>

      {/* Unlink Google confirmation modal */}
      <ConfirmModal
        isOpen={showUnlinkModal}
        title="Unlink Google Account"
        message="You will no longer be able to sign in with Google. You can re-link anytime by signing in with Google."
        variant="default"
        confirmLabel={unlinking ? 'Unlinking...' : 'Unlink'}
        onConfirm={handleUnlinkGoogle}
        onCancel={() => setShowUnlinkModal(false)}
      />

      {/* Delete confirmation modal */}
      <ConfirmModal
        isOpen={showDeleteModal}
        title="Delete Account"
        message="This will permanently delete your account and all associated data. This action cannot be undone."
        variant="danger"
        confirmLabel="Delete"
        confirmText="DELETE"
        confirmPlaceholder="Type DELETE to confirm"
        useHoldToConfirm
        onConfirm={handleDeleteAccount}
        onCancel={() => setShowDeleteModal(false)}
      />
    </div>
  );
}
