import type { ReauthSession, ReauthConfig, User } from './types';

/**
 * Create a reauth client for server-side session validation.
 * Use this in API routes to verify user authentication.
 *
 * @example
 * ```typescript
 * // Next.js API Route
 * import { createServerClient } from '@reauth/sdk/server';
 *
 * const reauth = createServerClient({ domain: 'yourdomain.com' });
 *
 * export async function GET(request: NextRequest) {
 *   const user = await reauth.getUser(request.headers.get('cookie') || '');
 *
 *   if (!user) {
 *     return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
 *   }
 *
 *   return NextResponse.json({ user });
 * }
 * ```
 */
export function createServerClient(config: ReauthConfig) {
  const { domain } = config;
  const baseUrl = `https://reauth.${domain}/api/public/domain/${domain}`;

  return {
    /**
     * Get the raw session data by forwarding cookies to reauth.dev.
     * @param cookies - The Cookie header from the incoming request
     */
    async getSession(cookies: string): Promise<ReauthSession> {
      const res = await fetch(`${baseUrl}/auth/session`, {
        headers: { Cookie: cookies },
      });
      return res.json();
    },

    /**
     * Get the authenticated user, or null if not authenticated.
     * Handles session validation and returns a simple User object.
     * @param cookies - The Cookie header from the incoming request
     */
    async getUser(cookies: string): Promise<User | null> {
      const session = await this.getSession(cookies);

      if (!session.valid || session.error_code || !session.end_user_id) {
        return null;
      }

      return {
        id: session.end_user_id,
        email: session.email!,
        roles: session.roles || [],
      };
    },
  };
}

export type ServerClient = ReturnType<typeof createServerClient>;
