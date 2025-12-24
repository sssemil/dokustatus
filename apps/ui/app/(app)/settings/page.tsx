'use client';

import { useState } from 'react';

type Tab = 'usage' | 'billing';

export default function SettingsPage() {
  const [activeTab, setActiveTab] = useState<Tab>('usage');

  const tabs: { id: Tab; label: string }[] = [
    { id: 'usage', label: 'Usage' },
    { id: 'billing', label: 'Billing' },
  ];

  return (
    <>
      <h1>Settings</h1>

      {/* Tabs */}
      <div style={{
        display: 'flex',
        gap: 'var(--spacing-xs)',
        marginBottom: 'var(--spacing-lg)',
        borderBottom: '1px solid var(--border-primary)',
        paddingBottom: 'var(--spacing-sm)',
      }}>
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            style={{
              padding: 'var(--spacing-sm) var(--spacing-md)',
              backgroundColor: activeTab === tab.id ? 'var(--bg-tertiary)' : 'transparent',
              border: activeTab === tab.id ? '1px solid var(--border-primary)' : '1px solid transparent',
              borderBottom: 'none',
              borderRadius: 'var(--radius-sm) var(--radius-sm) 0 0',
              color: activeTab === tab.id ? 'var(--text-primary)' : 'var(--text-muted)',
              cursor: 'pointer',
              fontSize: '14px',
              fontWeight: activeTab === tab.id ? 600 : 400,
            }}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Tab content */}
      {activeTab === 'usage' && (
        <div className="card">
          <h2>Usage</h2>
          <p className="text-muted">
            Track your API usage, authentication requests, and email sends.
          </p>
          <div style={{
            padding: 'var(--spacing-xl)',
            backgroundColor: 'var(--bg-tertiary)',
            borderRadius: 'var(--radius-md)',
            textAlign: 'center',
            marginTop: 'var(--spacing-md)',
          }}>
            <p className="text-muted" style={{ margin: 0 }}>
              Usage metrics will appear here once you start using reauth.dev
            </p>
          </div>
        </div>
      )}

      {activeTab === 'billing' && (
        <div className="card">
          <h2>Billing</h2>
          <p className="text-muted">
            Manage your subscription, payment methods, and invoices.
          </p>
          <div style={{
            padding: 'var(--spacing-xl)',
            backgroundColor: 'var(--bg-tertiary)',
            borderRadius: 'var(--radius-md)',
            textAlign: 'center',
            marginTop: 'var(--spacing-md)',
          }}>
            <p className="text-muted" style={{ margin: 0 }}>
              Billing options will be available when reauth.dev launches
            </p>
          </div>
        </div>
      )}
    </>
  );
}
