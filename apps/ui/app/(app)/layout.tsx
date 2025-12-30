'use client';

import { useState, useEffect, createContext, useContext, ReactNode } from 'react';
import { useRouter, usePathname } from 'next/navigation';
import { isMainApp as checkIsMainApp, getRootDomain, URLS } from '@/lib/domain-utils';
import { Sidebar } from '@/components/layout';
import { ToastProvider } from '@/contexts/ToastContext';

type User = {
  email: string;
  id: string;
  roles: string[];
  googleLinked: boolean;
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
  const [loading, setLoading] = useState(true);
  const [isIngress, setIsIngress] = useState(false);
  const [displayDomain, setDisplayDomain] = useState('');
  const router = useRouter();
  const pathname = usePathname();

  useEffect(() => {
    const hostname = window.location.hostname;
    const isMainApp = checkIsMainApp(hostname);
    setIsIngress(!isMainApp && hostname !== 'localhost');
    setDisplayDomain(getRootDomain(hostname));
  }, []);

  const fetchUser = async () => {
    const hostname = window.location.hostname;
    const apiDomain = getRootDomain(hostname);
    try {
      let res = await fetch(`/api/public/domain/${apiDomain}/auth/session`, { credentials: 'include' });

      // If session check fails with 401, try to refresh the token
      if (res.status === 401) {
        const refreshRes = await fetch(`/api/public/domain/${apiDomain}/auth/refresh`, {
          method: 'POST',
          credentials: 'include',
        });
        if (refreshRes.ok) {
          // Retry session check after refresh
          res = await fetch(`/api/public/domain/${apiDomain}/auth/session`, { credentials: 'include' });
        }
      }

      if (res.status === 401) {
        router.push('/');
        return;
      }
      if (res.ok) {
        const data = await res.json();

        // Check for error (e.g., account suspended)
        if (data.error) {
          // Logout and redirect
          await fetch(`/api/public/domain/${apiDomain}/auth/logout`, { method: 'POST', credentials: 'include' });
          router.push('/');
          return;
        }

        if (!data.valid) {
          router.push('/');
          return;
        }

        // Check if user is on waitlist
        if (data.waitlist_position) {
          const hostname = window.location.hostname;
          if (checkIsMainApp(hostname)) {
            window.location.href = URLS.waitlist;
          } else {
            router.push('/waitlist');
          }
          return;
        }

        setUser({
          email: data.email || '',
          id: data.end_user_id || '',
          roles: data.roles || [],
          googleLinked: data.google_linked ?? false,
        });
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

  const handleLogout = async () => {
    const apiDomain = getRootDomain(window.location.hostname);
    await fetch(`/api/public/domain/${apiDomain}/auth/logout`, { method: 'POST', credentials: 'include' });
    // Full page reload to clear state
    window.location.href = '/';
  };

  const handleProfileClick = () => {
    window.location.href = URLS.profile;
  };

  if (loading || !user) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="w-6 h-6 border-2 border-zinc-600 border-t-white rounded-full animate-spin" />
      </div>
    );
  }

  // Simplified layout for ingress domains (custom domains)
  // Only /profile is allowed on ingress
  if (isIngress) {
    if (pathname !== '/profile') {
      window.location.href = '/profile';
      return (
        <div className="flex items-center justify-center min-h-screen">
          <div className="w-6 h-6 border-2 border-zinc-600 border-t-white rounded-full animate-spin" />
        </div>
      );
    }

    return (
      <AppContext.Provider value={{ user, refetchUser: fetchUser, isIngress, displayDomain }}>
        <ToastProvider>
          <main className="flex items-center justify-center p-8">
            <div className="max-w-md w-full">
              {children}
            </div>
          </main>
        </ToastProvider>
      </AppContext.Provider>
    );
  }

  // Full dashboard layout for reauth.dev
  return (
    <AppContext.Provider value={{ user, refetchUser: fetchUser, isIngress, displayDomain }}>
      <ToastProvider>
        <div className="flex h-screen overflow-hidden">
          <Sidebar
            email={user.email}
            onLogout={handleLogout}
            onProfileClick={handleProfileClick}
          />

          <div className="flex-1 overflow-auto">
            <main className="flex flex-col gap-6 w-full max-w-5xl mx-auto p-6 lg:p-8">
              {children}
            </main>
          </div>
        </div>
      </ToastProvider>
    </AppContext.Provider>
  );
}
