'use client';

import { useState, useEffect } from 'react';
import { BarChart3, CreditCard, Sparkles } from 'lucide-react';
import { Card, Button, Tabs, ProgressBar } from '@/components/ui';

type Tab = 'usage' | 'billing';

type UsageStats = {
  domains_count: number;
  total_users: number;
  domains_limit?: number;
  users_limit?: number;
};

export default function SettingsPage() {
  const [activeTab, setActiveTab] = useState<Tab>('usage');
  const [stats, setStats] = useState<UsageStats | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const fetchStats = async () => {
      try {
        const res = await fetch('/api/domains/stats', { credentials: 'include' });
        if (res.ok) {
          setStats(await res.json());
        }
      } catch {
        // Ignore
      } finally {
        setLoading(false);
      }
    };
    fetchStats();
  }, []);

  const tabs = [
    { id: 'usage' as Tab, label: 'Usage' },
    { id: 'billing' as Tab, label: 'Billing' },
  ];

  // Default limits for free tier
  const domainsLimit = stats?.domains_limit ?? 5;
  const usersLimit = stats?.users_limit ?? 100;
  const domainsPercent = stats ? Math.round((stats.domains_count / domainsLimit) * 100) : 0;
  const usersPercent = stats ? Math.round((stats.total_users / usersLimit) * 100) : 0;

  return (
    <div className="space-y-6">
      {/* Page header */}
      <div>
        <h1 className="text-2xl font-bold text-white">Settings</h1>
        <p className="text-sm text-zinc-400 mt-1">
          Manage your account and subscription
        </p>
      </div>

      {/* Tabs */}
      <Tabs
        tabs={tabs}
        activeTab={activeTab}
        onChange={(id) => setActiveTab(id as Tab)}
      />

      {/* Usage tab */}
      {activeTab === 'usage' && (
        <div className="space-y-4">
          <Card className="p-6">
            <div className="flex items-center gap-2 mb-4">
              <BarChart3 size={20} className="text-blue-400" />
              <h2 className="text-lg font-semibold text-white">Usage Overview</h2>
            </div>

            {loading ? (
              <div className="flex justify-center py-8">
                <div className="w-6 h-6 border-2 border-zinc-600 border-t-blue-500 rounded-full animate-spin" />
              </div>
            ) : stats ? (
              <div className="space-y-6">
                {/* Domains usage */}
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-sm text-zinc-400">Domains</span>
                    <span className="text-sm text-zinc-300">
                      {stats.domains_count} / {domainsLimit}
                    </span>
                  </div>
                  <ProgressBar value={stats.domains_count} max={domainsLimit} />
                </div>

                {/* Users usage */}
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-sm text-zinc-400">Total Users</span>
                    <span className="text-sm text-zinc-300">
                      {stats.total_users} / {usersLimit}
                    </span>
                  </div>
                  <ProgressBar value={stats.total_users} max={usersLimit} />
                </div>
              </div>
            ) : (
              <div className="bg-zinc-800/50 rounded-lg p-8 text-center border border-zinc-700">
                <p className="text-zinc-400">
                  Usage metrics will appear here once you start using reauth.dev
                </p>
              </div>
            )}
          </Card>
        </div>
      )}

      {/* Billing tab */}
      {activeTab === 'billing' && (
        <div className="space-y-4">
          <Card className="p-6">
            <div className="flex items-center gap-2 mb-4">
              <CreditCard size={20} className="text-blue-400" />
              <h2 className="text-lg font-semibold text-white">Current Plan</h2>
            </div>

            <div className="flex items-center justify-between p-4 bg-zinc-800/50 rounded-lg mb-6 border border-zinc-700">
              <div>
                <p className="font-medium text-white">Free Tier</p>
                <p className="text-sm text-zinc-400">Up to 5 domains, 100 users</p>
              </div>
              <span className="px-3 py-1 bg-blue-500/10 text-blue-400 text-sm rounded-full border border-blue-500/30">
                Active
              </span>
            </div>

            <div className="bg-gradient-to-r from-blue-500/10 to-purple-500/10 border border-blue-500/20 rounded-lg p-6">
              <div className="flex items-center gap-2 mb-2">
                <Sparkles size={18} className="text-blue-400" />
                <h3 className="font-medium text-white">Need more?</h3>
              </div>
              <p className="text-sm text-zinc-400 mb-4">
                Upgrade to Pro for unlimited domains, users, and premium features.
              </p>
              <Button variant="primary" disabled>
                Coming Soon
              </Button>
            </div>
          </Card>
        </div>
      )}
    </div>
  );
}
