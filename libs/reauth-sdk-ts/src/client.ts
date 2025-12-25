import type { ReauthSession, ReauthConfig } from './types';

/**
 * Create a reauth client for browser-side authentication.
 *
 * @example
 * ```typescript
 * const reauth = createReauthClient({ domain: 'yourdomain.com' });
 *
 * // Check if user is authenticated
 * const session = await reauth.getSession();
 * if (!session.valid) {
 *   reauth.login(); // Redirect to login page
 * }
 *
 * // Log out
 * await reauth.logout();
 * ```
 */
export function createReauthClient(config: ReauthConfig) {
  const { domain } = config;
  const baseUrl = `https://reauth.${domain}/api/public/domain/${domain}`;

  return {
    /**
     * Redirect the user to the reauth.dev login page.
     * After successful login, they'll be redirected back to your configured redirect URL.
     */
    login(): void {
      if (typeof window === 'undefined') {
        throw new Error('login() can only be called in browser');
      }
      window.location.href = `https://reauth.${domain}/`;
    },

    /**
     * Check if the user is authenticated.
     * Returns session info including user ID, email, and roles.
     */
    async getSession(): Promise<ReauthSession> {
      const res = await fetch(`${baseUrl}/auth/session`, {
        credentials: 'include',
      });
      return res.json();
    },

    /**
     * Refresh the access token using the refresh token.
     * Call this when getSession() returns valid: false but no error_code.
     * @returns true if refresh succeeded, false otherwise
     */
    async refresh(): Promise<boolean> {
      const res = await fetch(`${baseUrl}/auth/refresh`, {
        method: 'POST',
        credentials: 'include',
      });
      return res.ok;
    },

    /**
     * Log out the user by clearing all session cookies.
     */
    async logout(): Promise<void> {
      await fetch(`${baseUrl}/auth/logout`, {
        method: 'POST',
        credentials: 'include',
      });
    },

    /**
     * Delete the user's own account (self-service).
     * @returns true if deletion succeeded, false otherwise
     */
    async deleteAccount(): Promise<boolean> {
      const res = await fetch(`${baseUrl}/auth/account`, {
        method: 'DELETE',
        credentials: 'include',
      });
      return res.ok;
    },
  };
}

export type ReauthClient = ReturnType<typeof createReauthClient>;
