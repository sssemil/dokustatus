'use client';

import { useState, useEffect, useCallback, useMemo } from 'react';
import { createReauthClient } from '../client';
import type { User, ReauthConfig } from '../types';

type AuthState = {
  user: User | null;
  loading: boolean;
  error: string | null;
  isOnWaitlist: boolean;
  waitlistPosition: number | null;
};

/**
 * React hook for authentication state management.
 *
 * @example
 * ```typescript
 * function MyComponent() {
 *   const { user, loading, login, logout } = useAuth({ domain: 'yourdomain.com' });
 *
 *   if (loading) return <div>Loading...</div>;
 *   if (!user) return <button onClick={login}>Sign in</button>;
 *
 *   return (
 *     <div>
 *       Welcome {user.email}
 *       <button onClick={logout}>Sign out</button>
 *     </div>
 *   );
 * }
 * ```
 */
export function useAuth(config: ReauthConfig) {
  const client = useMemo(() => createReauthClient(config), [config.domain]);

  const [state, setState] = useState<AuthState>({
    user: null,
    loading: true,
    error: null,
    isOnWaitlist: false,
    waitlistPosition: null,
  });

  const checkSession = useCallback(async () => {
    try {
      let session = await client.getSession();

      // Try refresh if access token expired
      if (!session.valid && !session.error_code && !session.end_user_id) {
        const refreshed = await client.refresh();
        if (refreshed) {
          session = await client.getSession();
        }
      }

      // Account suspended
      if (session.error_code === 'ACCOUNT_SUSPENDED') {
        setState({
          user: null,
          loading: false,
          error: 'Account suspended',
          isOnWaitlist: false,
          waitlistPosition: null,
        });
        return;
      }

      // On waitlist
      if (session.valid && session.waitlist_position) {
        setState({
          user: {
            id: session.end_user_id!,
            email: session.email!,
            roles: session.roles || [],
          },
          loading: false,
          error: null,
          isOnWaitlist: true,
          waitlistPosition: session.waitlist_position,
        });
        return;
      }

      // Authenticated
      if (session.valid && session.end_user_id) {
        setState({
          user: {
            id: session.end_user_id,
            email: session.email!,
            roles: session.roles || [],
          },
          loading: false,
          error: null,
          isOnWaitlist: false,
          waitlistPosition: null,
        });
        return;
      }

      // Not authenticated
      setState({
        user: null,
        loading: false,
        error: null,
        isOnWaitlist: false,
        waitlistPosition: null,
      });
    } catch {
      setState({
        user: null,
        loading: false,
        error: 'Auth check failed',
        isOnWaitlist: false,
        waitlistPosition: null,
      });
    }
  }, [client]);

  useEffect(() => {
    checkSession();
  }, [checkSession]);

  const logout = useCallback(async () => {
    await client.logout();
    setState({
      user: null,
      loading: false,
      error: null,
      isOnWaitlist: false,
      waitlistPosition: null,
    });
  }, [client]);

  return {
    ...state,
    login: client.login,
    logout,
    refetch: checkSession,
  };
}
