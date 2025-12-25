'use client';

import { createContext, useContext, ReactNode } from 'react';
import { useAuth } from './useAuth';
import type { User, ReauthConfig } from '../types';

type AuthContextType = {
  user: User | null;
  loading: boolean;
  error: string | null;
  isOnWaitlist: boolean;
  waitlistPosition: number | null;
  login: () => void;
  logout: () => Promise<void>;
  refetch: () => Promise<void>;
};

const AuthContext = createContext<AuthContextType | null>(null);

type AuthProviderProps = {
  config: ReauthConfig;
  children: ReactNode;
};

/**
 * Provider component that wraps your app and provides auth context.
 *
 * @example
 * ```typescript
 * // app/layout.tsx
 * import { AuthProvider } from '@reauth/sdk/react';
 *
 * export default function RootLayout({ children }) {
 *   return (
 *     <AuthProvider config={{ domain: 'yourdomain.com' }}>
 *       {children}
 *     </AuthProvider>
 *   );
 * }
 * ```
 */
export function AuthProvider({ config, children }: AuthProviderProps) {
  const auth = useAuth(config);
  return <AuthContext.Provider value={auth}>{children}</AuthContext.Provider>;
}

/**
 * Hook to access auth context from within AuthProvider.
 *
 * @example
 * ```typescript
 * function UserMenu() {
 *   const { user, logout } = useAuthContext();
 *
 *   return (
 *     <div>
 *       {user?.email}
 *       <button onClick={logout}>Sign out</button>
 *     </div>
 *   );
 * }
 * ```
 */
export function useAuthContext() {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error('useAuthContext must be used within AuthProvider');
  }
  return context;
}
