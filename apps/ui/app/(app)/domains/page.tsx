'use client';

import { useState, useEffect, useCallback, useRef } from 'react';
import { useRouter } from 'next/navigation';
import { Globe, Plus, MoreVertical, RefreshCw, AlertTriangle } from 'lucide-react';
import { Card, Button, Badge, EmptyState, ConfirmModal, SearchInput, Modal, Input } from '@/components/ui';
import { zIndex } from '@/lib/design-tokens';

type Domain = {
  id: string;
  domain: string;
  status: 'pending_dns' | 'verifying' | 'verified' | 'failed';
  verified_at: string | null;
  created_at: string | null;
  has_auth_methods: boolean;
};

export default function DomainsPage() {
  const router = useRouter();
  const [domains, setDomains] = useState<Domain[]>([]);
  const [loading, setLoading] = useState(true);
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [showAddModal, setShowAddModal] = useState(false);
  const [newDomainName, setNewDomainName] = useState('');
  const [addingDomain, setAddingDomain] = useState(false);
  const [addError, setAddError] = useState('');
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

  const validateDomain = (domain: string): string | null => {
    const trimmed = domain.trim().toLowerCase();
    if (!trimmed) return 'Please enter a domain name';
    if (trimmed.startsWith('http://') || trimmed.startsWith('https://')) {
      return 'Enter just the domain name without http:// or https://';
    }
    if (trimmed.includes('/')) return 'Enter just the domain name without any path';
    const parts = trimmed.split('.');
    if (parts.length < 2) return 'Please enter a valid domain (e.g., example.com)';
    if (parts.some(p => p.length === 0)) return 'Invalid domain format';
    const multiPartTlds = ['co.uk', 'com.au', 'co.nz', 'com.br', 'co.jp', 'org.uk', 'net.au'];
    for (const tld of multiPartTlds) {
      if (trimmed.endsWith(tld)) {
        if (parts.length > 3) return `Please enter your root domain (e.g., example.${tld}), not a subdomain`;
        if (parts.length < 3) return 'Please enter a valid domain';
        return null;
      }
    }
    if (parts.length > 2) return `Please enter your root domain (e.g., ${parts.slice(-2).join('.')}), not a subdomain`;
    return null;
  };

  const handleAddDomain = async (e: React.FormEvent) => {
    e.preventDefault();
    setAddError('');
    const validationError = validateDomain(newDomainName);
    if (validationError) {
      setAddError(validationError);
      return;
    }
    setAddingDomain(true);
    try {
      const res = await fetch('/api/domains', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ domain: newDomainName.trim().toLowerCase() }),
        credentials: 'include',
      });
      if (res.ok) {
        const data = await res.json();
        const domainName = newDomainName.trim().toLowerCase();
        // Set default redirect URL to the domain itself
        await fetch(`/api/domains/${data.id}/auth-config`, {
          method: 'PATCH',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ redirect_url: `https://${domainName}` }),
          credentials: 'include',
        }).catch(() => {}); // Ignore errors
        setShowAddModal(false);
        setNewDomainName('');
        router.push(`/domains/${data.id}`);
      } else {
        const errData = await res.json().catch(() => ({}));
        setAddError(errData.message || 'Failed to add domain');
      }
    } catch {
      setAddError('Network error. Please try again.');
    } finally {
      setAddingDomain(false);
    }
  };

  // Filter domains by search
  const filteredDomains = domains.filter(d =>
    d.domain.toLowerCase().includes(search.toLowerCase())
  );

  return (
    <div className="space-y-6">
      {/* Page header */}
      <div className="flex items-center justify-between">
        <h1 className="text-xl sm:text-2xl font-bold text-white">Domains</h1>
        <Button variant="primary" onClick={() => setShowAddModal(true)}>
          <Plus size={16} />
          <span className="hidden sm:inline ml-1">Add domain</span>
        </Button>
      </div>

      {/* Search */}
      {domains.length > 0 && (
        <SearchInput
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search domains..."
        />
      )}

      {/* Domains List */}
      {loading ? (
        <div className="flex justify-center py-12">
          <div className="w-6 h-6 border-2 border-zinc-600 border-t-blue-500 rounded-full animate-spin" />
        </div>
      ) : domains.length === 0 ? (
        <EmptyState
          icon={Globe}
          title="No domains found"
          description="Get started by adding your first domain"
          action={
            <Button variant="primary" onClick={() => setShowAddModal(true)}>
              <Plus size={16} className="mr-1" />
              Add your first domain
            </Button>
          }
        />
      ) : filteredDomains.length === 0 && search ? (
        <EmptyState
          icon={Globe}
          title="No domains found"
          description="Try adjusting your search"
        />
      ) : (
        <div className="space-y-3">
          {filteredDomains.map((domain) => (
            <Card
              key={domain.id}
              className="p-3 sm:p-4"
              hover
              onClick={() => router.push(`/domains/${domain.id}`)}
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3 min-w-0">
                  <div className={`w-9 h-9 sm:w-10 sm:h-10 rounded-lg flex items-center justify-center flex-shrink-0 ${
                    domain.status === 'verified' ? 'bg-emerald-900/30' :
                    domain.status === 'failed' ? 'bg-red-900/30' : 'bg-zinc-700'
                  }`}>
                    <Globe size={18} className={
                      domain.status === 'verified' ? 'text-emerald-400' :
                      domain.status === 'failed' ? 'text-red-400' : 'text-zinc-400'
                    } />
                  </div>
                  <div className="min-w-0">
                    <div className="flex items-center gap-2 flex-wrap">
                      <span className="font-medium truncate">{domain.domain}</span>
                      {domain.status === 'verified' ? (
                        <Badge variant="success">verified</Badge>
                      ) : domain.status === 'failed' ? (
                        <Badge variant="error">failed</Badge>
                      ) : (
                        <Badge variant="warning">pending</Badge>
                      )}
                    </div>
                    {domain.status !== 'verified' && (
                      <div className="flex items-center gap-1 text-sm text-amber-400 mt-0.5">
                        <AlertTriangle size={12} />
                        <span className="truncate">DNS verification pending</span>
                      </div>
                    )}
                    {domain.status === 'verified' && !domain.has_auth_methods && (
                      <div className="flex items-center gap-1 text-sm text-amber-400 mt-0.5">
                        <AlertTriangle size={12} />
                        <span className="truncate">No auth methods configured</span>
                      </div>
                    )}
                  </div>
                </div>

                <div className="flex items-center gap-2 flex-shrink-0 ml-2">
                  {domain.status === 'failed' && (
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleRetryVerification(domain);
                      }}
                    >
                      <RefreshCw size={14} className="mr-1" />
                      Retry
                    </Button>
                  )}

                  <div
                    ref={openMenuId === domain.id ? menuRef : null}
                    className="relative"
                  >
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        setOpenMenuId(openMenuId === domain.id ? null : domain.id);
                      }}
                      className="p-2 text-zinc-400 hover:text-white hover:bg-zinc-700 rounded transition-colors"
                    >
                      <MoreVertical size={16} />
                    </button>

                    {openMenuId === domain.id && (
                      <div
                        className="absolute top-full right-0 mt-1 bg-zinc-800 border border-zinc-700 rounded-lg shadow-xl min-w-[120px] overflow-hidden animate-scale-in"
                        style={{ zIndex: zIndex.dropdown }}
                      >
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            setOpenMenuId(null);
                            setDeleteConfirmId(domain.id);
                          }}
                          className="w-full px-4 py-2 text-sm text-red-400 hover:bg-zinc-700 text-left transition-colors"
                        >
                          Delete
                        </button>
                      </div>
                    )}
                  </div>
                </div>
              </div>
            </Card>
          ))}
        </div>
      )}

      {/* Delete Confirmation Modal */}
      <ConfirmModal
        isOpen={deleteConfirmId !== null}
        title="Delete Domain"
        message="This action cannot be undone. All users, API keys, and authentication settings for this domain will be permanently deleted."
        confirmLabel="Delete"
        cancelLabel="Cancel"
        variant="danger"
        confirmText={domains.find(d => d.id === deleteConfirmId)?.domain}
        useHoldToConfirm
        onConfirm={() => deleteConfirmId && handleDeleteDomain(deleteConfirmId)}
        onCancel={() => setDeleteConfirmId(null)}
      />

      {/* Add Domain Modal */}
      <Modal
        open={showAddModal}
        onClose={() => { setShowAddModal(false); setNewDomainName(''); setAddError(''); }}
        title="Add Domain"
      >
        <form onSubmit={handleAddDomain} className="space-y-4">
          <div className="space-y-2">
            <label className="text-sm text-zinc-400">Domain name</label>
            <Input
              value={newDomainName}
              onChange={(e) => setNewDomainName(e.target.value)}
              placeholder="example.com"
              autoFocus
            />
            <p className="text-xs text-zinc-500">
              Your login page will be at{' '}
              <code className="text-blue-400">https://reauth.{newDomainName || 'example.com'}</code>
            </p>
          </div>
          {addError && (
            <div className="flex items-center gap-2 p-3 bg-red-500/10 border border-red-500/20 rounded-lg text-sm text-red-400">
              <AlertTriangle size={16} />
              {addError}
            </div>
          )}
          <div className="flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={() => { setShowAddModal(false); setNewDomainName(''); setAddError(''); }}>
              Cancel
            </Button>
            <Button type="submit" variant="primary" disabled={addingDomain || !newDomainName.trim()}>
              {addingDomain ? 'Adding...' : 'Add Domain'}
            </Button>
          </div>
        </form>
      </Modal>
    </div>
  );
}
