'use client';

import { useState, useEffect, useRef, createContext, useContext, ReactNode } from 'react';
import { useRouter, usePathname } from 'next/navigation';
import Link from 'next/link';
import { useTheme } from '../components/ThemeContext';

type User = {
  email: string;
  id: string;
  roles: string[];
};

type AppContextType = {
  user: User | null;
  refetchUser: () => Promise<void>;
  isIngress: boolean;
  displayDomain: string;
};

const AppContext = createContext<AppContextType | null>(null);

export function useAppContext() {
  const context = useContext(AppContext);
  if (!context) {
    throw new Error('useAppContext must be used within AppLayout');
  }
  return context;
}

export default function AppLayout({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const [loading, setLoading] = useState(true);
  const [isIngress, setIsIngress] = useState(false);
  const [displayDomain, setDisplayDomain] = useState('');
  const [apiDomain, setApiDomain] = useState('');
  const menuRef = useRef<HTMLDivElement>(null);
  const router = useRouter();
  const pathname = usePathname();
  const { theme, cycleTheme } = useTheme();

  useEffect(() => {
    const hostname = window.location.hostname;
    const isMainApp = hostname === 'reauth.dev' || hostname === 'www.reauth.dev';
    setIsIngress(!isMainApp && hostname !== 'localhost');
    setApiDomain(hostname);

    // Display domain: strip "reauth." prefix for ingress subdomains
    const rootDomain = hostname.startsWith('reauth.') && hostname !== 'reauth.dev'
      ? hostname.slice(7)
      : hostname;
    setDisplayDomain(rootDomain);
  }, []);

  const fetchUser = async () => {
    const hostname = window.location.hostname;
    try {
      let res = await fetch(`/api/public/domain/${hostname}/auth/session`, { credentials: 'include' });

      // If session check fails with 401, try to refresh the token
      if (res.status === 401) {
        const refreshRes = await fetch(`/api/public/domain/${hostname}/auth/refresh`, {
          method: 'POST',
          credentials: 'include',
        });
        if (refreshRes.ok) {
          // Retry session check after refresh
          res = await fetch(`/api/public/domain/${hostname}/auth/session`, { credentials: 'include' });
        }
      }

      if (res.status === 401) {
        router.push('/');
        return;
      }
      if (res.ok) {
        const data = await res.json();
        if (!data.valid) {
          router.push('/');
          return;
        }
        setUser({ email: data.email || '', id: data.end_user_id || '', roles: data.roles || [] });
      }
    } catch {
      router.push('/');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchUser();
  }, []);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const handleLogout = async () => {
    const hostname = window.location.hostname;
    await fetch(`/api/public/domain/${hostname}/auth/logout`, { method: 'POST', credentials: 'include' });
    // Full page reload to clear state
    window.location.href = '/';
  };

  const themeLabel = theme === 'dark' ? 'Dark' : theme === 'light' ? 'Light' : 'System';

  if (loading || !user) {
    return (
      <div className="flex items-center justify-center" style={{ minHeight: '100vh' }}>
        <div className="spinner" />
      </div>
    );
  }

  // Simplified layout for ingress domains (custom domains)
  // Only /profile is allowed on ingress
  if (isIngress) {
    if (pathname !== '/profile') {
      window.location.href = '/profile';
      return (
        <div className="flex items-center justify-center" style={{ minHeight: '100vh' }}>
          <div className="spinner" />
        </div>
      );
    }

    return (
      <AppContext.Provider value={{ user, refetchUser: fetchUser, isIngress, displayDomain }}>
        <main className="flex items-center justify-center" style={{ padding: 'var(--spacing-xl)' }}>
          <div style={{ maxWidth: '400px', width: '100%' }}>
            {children}
          </div>
        </main>
      </AppContext.Provider>
    );
  }

  // Full dashboard layout for reauth.dev
  const navItems = [
    { href: '/dashboard', label: 'Dashboard', icon: 'M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6' },
    { href: '/domains', label: 'Domains', icon: 'M21 12a9 9 0 01-9 9m9-9a9 9 0 00-9-9m9 9H3m9 9a9 9 0 01-9-9m9 9c1.657 0 3-4.03 3-9s-1.343-9-3-9m0 18c-1.657 0-3-4.03-3-9s1.343-9 3-9m-9 9a9 9 0 019-9' },
    { href: '/settings', label: 'Settings', icon: 'M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z M15 12a3 3 0 11-6 0 3 3 0 016 0z' },
  ];

  const emailInitial = user.email ? user.email.charAt(0).toUpperCase() : '?';

  return (
    <AppContext.Provider value={{ user, refetchUser: fetchUser, isIngress, displayDomain }}>
      <div style={{ display: 'flex', minHeight: '100vh' }}>
        {/* Sidebar */}
        <aside style={{
          width: '240px',
          backgroundColor: 'var(--bg-secondary)',
          borderRight: '1px solid var(--border-primary)',
          display: 'flex',
          flexDirection: 'column',
          padding: 'var(--spacing-md)',
          flexShrink: 0,
        }}>
          {/* Logo */}
          <div style={{ padding: 'var(--spacing-sm) var(--spacing-md)', marginBottom: 'var(--spacing-lg)' }}>
            <span style={{ fontSize: '1.25rem', fontWeight: 700, color: 'var(--text-primary)' }}>reauth.dev</span>
          </div>

          {/* Nav items */}
          <nav style={{ flex: 1 }}>
            {navItems.map((item) => {
              const isActive = pathname === item.href;
              return (
                <Link
                  key={item.href}
                  href={item.href}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 'var(--spacing-sm)',
                    padding: 'var(--spacing-sm) var(--spacing-md)',
                    marginBottom: 'var(--spacing-xs)',
                    borderRadius: 'var(--radius-sm)',
                    color: isActive ? 'var(--text-primary)' : 'var(--text-secondary)',
                    backgroundColor: isActive ? 'var(--bg-tertiary)' : 'transparent',
                    textDecoration: 'none',
                    fontSize: '14px',
                    transition: 'all 0.15s',
                  }}
                >
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <path d={item.icon} />
                  </svg>
                  {item.label}
                </Link>
              );
            })}
          </nav>

          {/* User menu */}
          <div ref={menuRef} style={{ position: 'relative' }}>
            <button
              onClick={() => setMenuOpen(!menuOpen)}
              style={{
                width: '100%',
                display: 'flex',
                alignItems: 'center',
                gap: 'var(--spacing-sm)',
                padding: 'var(--spacing-sm)',
                backgroundColor: 'transparent',
                border: '1px solid var(--border-primary)',
                borderRadius: 'var(--radius-sm)',
                cursor: 'pointer',
                textAlign: 'left',
              }}
            >
              <div style={{
                width: '32px',
                height: '32px',
                borderRadius: '50%',
                backgroundColor: 'var(--accent-blue)',
                color: '#000',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                fontWeight: 600,
                fontSize: '14px',
                flexShrink: 0,
              }}>
                {emailInitial}
              </div>
              <span style={{
                flex: 1,
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
                fontSize: '13px',
                color: 'var(--text-secondary)',
              }}>
                {user.email}
              </span>
            </button>

            {menuOpen && (
              <div style={{
                position: 'absolute',
                bottom: '100%',
                left: 0,
                right: 0,
                marginBottom: 'var(--spacing-xs)',
                backgroundColor: 'var(--bg-secondary)',
                border: '1px solid var(--border-primary)',
                borderRadius: 'var(--radius-sm)',
                overflow: 'hidden',
                boxShadow: 'var(--shadow-md)',
                zIndex: 100,
              }}>
                <button
                  onClick={() => { setMenuOpen(false); window.location.href = 'https://reauth.reauth.dev/profile'; }}
                  style={{
                    width: '100%',
                    padding: 'var(--spacing-sm) var(--spacing-md)',
                    backgroundColor: 'transparent',
                    border: 'none',
                    color: 'var(--text-secondary)',
                    fontSize: '13px',
                    textAlign: 'left',
                    cursor: 'pointer',
                  }}
                >
                  My profile
                </button>
                <button
                  onClick={() => { cycleTheme(); }}
                  style={{
                    width: '100%',
                    padding: 'var(--spacing-sm) var(--spacing-md)',
                    backgroundColor: 'transparent',
                    border: 'none',
                    color: 'var(--text-secondary)',
                    fontSize: '13px',
                    textAlign: 'left',
                    cursor: 'pointer',
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                  }}
                >
                  <span>Theme</span>
                  <span style={{ color: 'var(--text-muted)' }}>{themeLabel}</span>
                </button>
                <div style={{ height: '1px', backgroundColor: 'var(--border-primary)' }} />
                <button
                  onClick={handleLogout}
                  style={{
                    width: '100%',
                    padding: 'var(--spacing-sm) var(--spacing-md)',
                    backgroundColor: 'transparent',
                    border: 'none',
                    color: 'var(--accent-red)',
                    fontSize: '13px',
                    textAlign: 'left',
                    cursor: 'pointer',
                  }}
                >
                  Log out
                </button>
              </div>
            )}
          </div>
        </aside>

        {/* Main content */}
        <main style={{
          flex: 1,
          padding: 'var(--spacing-xl)',
          overflow: 'auto',
        }}>
          <div style={{
            display: 'flex',
            flexDirection: 'column',
            gap: 'var(--spacing-lg)',
            marginLeft: 'auto',
            marginRight: 'auto',
            width: '100%',
            maxWidth: '72rem',
            paddingLeft: 'var(--spacing-lg)',
            paddingRight: 'var(--spacing-lg)',
          }}>
            {children}
          </div>
        </main>
      </div>
    </AppContext.Provider>
  );
}
