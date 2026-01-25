import type {
  ReauthSession,
  ReauthServerConfig,
  User,
  UserDetails,
  TokenVerification,
} from "./types";

/**
 * Create a reauth client for server-side session validation.
 * Use this in API routes to verify user authentication.
 *
 * @example
 * ```typescript
 * // Next.js API Route - Cookie-based authentication
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
 *
 * @example
 * ```typescript
 * // API Key-based authentication (for token verification)
 * import { createServerClient } from '@reauth/sdk/server';
 *
 * const reauth = createServerClient({
 *   domain: 'yourdomain.com',
 *   apiKey: 'sk_live_...',
 * });
 *
 * export async function GET(request: NextRequest) {
 *   const token = request.headers.get('authorization')?.replace('Bearer ', '');
 *   if (!token) {
 *     return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
 *   }
 *
 *   const result = await reauth.verifyToken(token);
 *   if (!result.valid) {
 *     return NextResponse.json({ error: 'Invalid token' }, { status: 401 });
 *   }
 *
 *   return NextResponse.json({ user: result.user });
 * }
 * ```
 */
export function createServerClient(config: ReauthServerConfig) {
  const { domain, apiKey } = config;
  const publicBaseUrl = `https://reauth.${domain}/api/public/domain/${domain}`;
  const developerBaseUrl = `https://reauth.${domain}/api/developer/${domain}`;

  return {
    /**
     * Get the raw session data by forwarding cookies to reauth.
     * @param cookies - The Cookie header from the incoming request
     */
    async getSession(cookies: string): Promise<ReauthSession> {
      const res = await fetch(`${publicBaseUrl}/auth/session`, {
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

    /**
     * Verify a user's JWT token and get user details.
     * Requires an API key to be configured.
     * @param token - The JWT token to verify
     * @throws Error if API key is not configured
     */
    async verifyToken(token: string): Promise<TokenVerification> {
      if (!apiKey) {
        throw new Error(
          "API key is required for verifyToken. Configure it with createServerClient({ domain, apiKey }).",
        );
      }

      const res = await fetch(`${developerBaseUrl}/auth/verify-token`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${apiKey}`,
        },
        body: JSON.stringify({ token }),
      });

      if (!res.ok) {
        if (res.status === 401) {
          throw new Error("Invalid API key");
        }
        throw new Error(`Failed to verify token: ${res.status}`);
      }

      const data = await res.json();

      // Transform snake_case response to camelCase
      return {
        valid: data.valid,
        user: data.user
          ? {
              id: data.user.id,
              email: data.user.email,
              roles: data.user.roles,
              emailVerifiedAt: data.user.email_verified_at,
              lastLoginAt: data.user.last_login_at,
              isFrozen: data.user.is_frozen,
              isWhitelisted: data.user.is_whitelisted,
              createdAt: data.user.created_at,
            }
          : null,
      };
    },

    /**
     * Get user details by ID.
     * Requires an API key to be configured.
     * @param userId - The user ID to fetch
     * @throws Error if API key is not configured
     */
    async getUserById(userId: string): Promise<UserDetails | null> {
      if (!apiKey) {
        throw new Error(
          "API key is required for getUserById. Configure it with createServerClient({ domain, apiKey }).",
        );
      }

      const res = await fetch(`${developerBaseUrl}/users/${userId}`, {
        headers: {
          Authorization: `Bearer ${apiKey}`,
        },
      });

      if (!res.ok) {
        if (res.status === 401) {
          throw new Error("Invalid API key");
        }
        if (res.status === 404) {
          return null;
        }
        throw new Error(`Failed to get user: ${res.status}`);
      }

      const data = await res.json();

      // Transform snake_case response to camelCase
      return {
        id: data.id,
        email: data.email,
        roles: data.roles,
        emailVerifiedAt: data.email_verified_at,
        lastLoginAt: data.last_login_at,
        isFrozen: data.is_frozen,
        isWhitelisted: data.is_whitelisted,
        createdAt: data.created_at,
      };
    },
  };
}

export type ServerClient = ReturnType<typeof createServerClient>;
