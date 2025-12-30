'use client';

import { useState, useEffect, useCallback } from 'react';
import { useParams, useRouter } from 'next/navigation';
import Link from 'next/link';
import { ArrowLeft, Mail, Calendar, Clock, Shield, Snowflake, CheckCircle } from 'lucide-react';
import { Card, Button, Badge, HoldButton } from '@/components/ui';
import { useToast } from '@/contexts/ToastContext';

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
  const { addToast } = useToast();

  const [user, setUser] = useState<EndUser | null>(null);
  const [loading, setLoading] = useState(true);
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
        addToast('User not found', 'error');
      }
    } catch {
      addToast('Failed to load user', 'error');
    } finally {
      setLoading(false);
    }
  }, [domainId, userId, addToast]);

  const fetchRoles = useCallback(async () => {
    try {
      const res = await fetch(`/api/domains/${domainId}/roles`, { credentials: 'include' });
      if (res.ok) {
        const data = await res.json();
        setAvailableRoles(data);
      }
    } catch {}
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
          addToast('User deleted', 'success');
          router.push(`/domains/${domainId}?tab=users`);
        } else {
          addToast(`User ${action}ed successfully`, 'success');
          fetchUser();
        }
      } else {
        addToast(`Failed to ${action} user`, 'error');
      }
    } catch {
      addToast('Network error', 'error');
    } finally {
      setActionLoading(false);
    }
  };

  const handleRoleToggle = (roleName: string) => {
    setSelectedRoles((prev) =>
      prev.includes(roleName) ? prev.filter((r) => r !== roleName) : [...prev, roleName]
    );
  };

  const handleSaveRoles = async () => {
    setSavingRoles(true);

    try {
      const res = await fetch(`/api/domains/${domainId}/end-users/${userId}/roles`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ roles: selectedRoles }),
        credentials: 'include',
      });

      if (res.ok) {
        addToast('Roles updated', 'success');
        fetchUser();
      } else {
        const errData = await res.json().catch(() => ({}));
        addToast(errData.message || 'Failed to update roles', 'error');
      }
    } catch {
      addToast('Network error', 'error');
    } finally {
      setSavingRoles(false);
    }
  };

  const rolesChanged = user ? JSON.stringify([...selectedRoles].sort()) !== JSON.stringify([...(user.roles || [])].sort()) : false;

  const formatDate = (dateString: string | null) => {
    if (!dateString) return 'Never';
    return new Date(dateString).toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  if (loading) {
    return (
      <div className="flex justify-center py-20">
        <div className="w-6 h-6 border-2 border-zinc-600 border-t-blue-500 rounded-full animate-spin" />
      </div>
    );
  }

  if (!user) {
    return (
      <Card className="p-8 text-center">
        <p className="text-zinc-400 mb-4">User not found</p>
        <Button onClick={() => router.push(`/domains/${domainId}`)}>Back to domain</Button>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      {/* Back link */}
      <Link href={`/domains/${domainId}?tab=users`} className="inline-flex items-center gap-1 text-sm text-zinc-400 hover:text-white transition-colors">
        <ArrowLeft size={16} />
        Back to users
      </Link>

      {/* Header */}
      <div className="flex items-start justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">{user.email}</h1>
          <div className="flex items-center gap-2 mt-2">
            {user.is_frozen && <Badge variant="error">Frozen</Badge>}
            {user.is_whitelisted && <Badge variant="success">Whitelisted</Badge>}
            {user.email_verified_at && <Badge variant="info">Verified</Badge>}
            {user.roles?.map((role) => <Badge key={role} variant="default">{role}</Badge>)}
          </div>
        </div>
        <HoldButton
          onComplete={() => handleAction('delete')}
          variant="danger"
          duration={3000}
          disabled={actionLoading}
        >
          Delete
        </HoldButton>
      </div>

      {/* User Details */}
      <Card className="p-6">
        <h2 className="text-lg font-semibold text-white mb-4">User Details</h2>
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-6">
          <div className="flex items-start gap-3">
            <div className="w-8 h-8 bg-zinc-800 rounded-lg flex items-center justify-center flex-shrink-0">
              <Mail size={16} className="text-zinc-400" />
            </div>
            <div>
              <p className="text-xs text-zinc-500">Email</p>
              <p className="text-sm text-white">{user.email}</p>
            </div>
          </div>
          <div className="flex items-start gap-3">
            <div className="w-8 h-8 bg-zinc-800 rounded-lg flex items-center justify-center flex-shrink-0">
              <Clock size={16} className="text-zinc-400" />
            </div>
            <div>
              <p className="text-xs text-zinc-500">Last Login</p>
              <p className="text-sm text-white">{formatDate(user.last_login_at)}</p>
            </div>
          </div>
          <div className="flex items-start gap-3">
            <div className="w-8 h-8 bg-zinc-800 rounded-lg flex items-center justify-center flex-shrink-0">
              <CheckCircle size={16} className="text-zinc-400" />
            </div>
            <div>
              <p className="text-xs text-zinc-500">Email Verified</p>
              <p className="text-sm text-white">{formatDate(user.email_verified_at)}</p>
            </div>
          </div>
          <div className="flex items-start gap-3">
            <div className="w-8 h-8 bg-zinc-800 rounded-lg flex items-center justify-center flex-shrink-0">
              <Calendar size={16} className="text-zinc-400" />
            </div>
            <div>
              <p className="text-xs text-zinc-500">Created</p>
              <p className="text-sm text-white">{formatDate(user.created_at)}</p>
            </div>
          </div>
        </div>
      </Card>

      {/* Roles */}
      <Card className="p-6">
        <h2 className="text-lg font-semibold text-white mb-4">Roles</h2>
        {availableRoles.length === 0 ? (
          <p className="text-sm text-zinc-400">
            No roles available. Create roles in the Roles tab of the domain page.
          </p>
        ) : (
          <div className="space-y-4">
            <div className="flex flex-wrap gap-2">
              {availableRoles.map((role) => {
                const isSelected = selectedRoles.includes(role.name);
                return (
                  <button
                    key={role.id}
                    onClick={() => handleRoleToggle(role.name)}
                    className={`
                      px-4 py-2 rounded-lg text-sm font-medium transition-all duration-200
                      ${isSelected
                        ? 'bg-blue-600 text-white border-2 border-blue-500'
                        : 'bg-zinc-800 text-zinc-300 border-2 border-zinc-700 hover:border-zinc-600'
                      }
                    `}
                  >
                    {role.name}
                  </button>
                );
              })}
            </div>
            <Button
              variant="primary"
              onClick={handleSaveRoles}
              disabled={!rolesChanged || savingRoles}
            >
              {savingRoles ? 'Saving...' : 'Save Roles'}
            </Button>
          </div>
        )}
      </Card>

      {/* Actions */}
      <Card className="p-6">
        <h2 className="text-lg font-semibold text-white mb-4">Actions</h2>
        <div className="space-y-4">
          {/* Freeze/Unfreeze */}
          <div className="flex items-center justify-between p-4 bg-zinc-800/50 rounded-lg">
            <div className="flex items-start gap-3">
              <div className="w-8 h-8 bg-zinc-800 rounded-lg flex items-center justify-center flex-shrink-0">
                <Snowflake size={16} className="text-zinc-400" />
              </div>
              <div>
                <p className="font-medium text-white">
                  {user.is_frozen ? 'Unfreeze Account' : 'Freeze Account'}
                </p>
                <p className="text-xs text-zinc-500 mt-0.5">
                  {user.is_frozen
                    ? 'Allow this user to sign in again.'
                    : 'Prevent this user from signing in.'}
                </p>
              </div>
            </div>
            <Button onClick={() => handleAction(user.is_frozen ? 'unfreeze' : 'freeze')} disabled={actionLoading}>
              {user.is_frozen ? 'Unfreeze' : 'Freeze'}
            </Button>
          </div>

          {/* Whitelist/Unwhitelist */}
          <div className="flex items-center justify-between p-4 bg-zinc-800/50 rounded-lg">
            <div className="flex items-start gap-3">
              <div className="w-8 h-8 bg-zinc-800 rounded-lg flex items-center justify-center flex-shrink-0">
                <Shield size={16} className="text-zinc-400" />
              </div>
              <div>
                <p className="font-medium text-white">
                  {user.is_whitelisted ? 'Remove from Whitelist' : 'Add to Whitelist'}
                </p>
                <p className="text-xs text-zinc-500 mt-0.5">
                  {user.is_whitelisted
                    ? 'Remove this user from the whitelist.'
                    : 'Add this user to the whitelist.'}
                </p>
              </div>
            </div>
            <Button onClick={() => handleAction(user.is_whitelisted ? 'unwhitelist' : 'whitelist')} disabled={actionLoading}>
              {user.is_whitelisted ? 'Remove' : 'Whitelist'}
            </Button>
          </div>
        </div>
      </Card>
    </div>
  );
}
