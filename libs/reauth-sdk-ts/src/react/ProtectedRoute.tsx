'use client';

import { useEffect, ReactNode } from 'react';
import { useAuthContext } from './AuthProvider';

type ProtectedRouteProps = {
  children: ReactNode;
  /** Content to show while loading */
  fallback?: ReactNode;
  /** Custom handler when user is not authenticated (default: redirect to login) */
  onUnauthenticated?: () => void;
  /** Custom handler when user is on waitlist */
  onWaitlist?: () => void;
};

/**
 * Component that protects its children from unauthenticated access.
 * Automatically redirects to login if user is not authenticated.
 *
 * @example
 * ```typescript
 * // Basic usage
 * <ProtectedRoute>
 *   <Dashboard />
 * </ProtectedRoute>
 *
 * // With loading fallback
 * <ProtectedRoute fallback={<LoadingSpinner />}>
 *   <Dashboard />
 * </ProtectedRoute>
 *
 * // With custom handlers
 * <ProtectedRoute
 *   onUnauthenticated={() => router.push('/login')}
 *   onWaitlist={() => router.push('/waitlist')}
 * >
 *   <Dashboard />
 * </ProtectedRoute>
 * ```
 */
export function ProtectedRoute({
  children,
  fallback = null,
  onUnauthenticated,
  onWaitlist,
}: ProtectedRouteProps) {
  const { user, loading, isOnWaitlist, login } = useAuthContext();

  useEffect(() => {
    if (!loading && !user) {
      if (onUnauthenticated) {
        onUnauthenticated();
      } else {
        login();
      }
    }
  }, [loading, user, login, onUnauthenticated]);

  useEffect(() => {
    if (!loading && isOnWaitlist && onWaitlist) {
      onWaitlist();
    }
  }, [loading, isOnWaitlist, onWaitlist]);

  if (loading) {
    return <>{fallback}</>;
  }

  if (!user || isOnWaitlist) {
    return null;
  }

  return <>{children}</>;
}
