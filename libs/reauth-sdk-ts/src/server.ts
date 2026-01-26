import { hkdf } from "crypto";
import * as jose from "jose";
import type {
  ReauthServerConfig,
  UserDetails,
  DomainEndUserClaims,
  SubscriptionInfo,
  AuthResult,
  RequestLike,
} from "./types";

/**
 * Derives a JWT signing secret from an API key using HKDF-SHA256.
 * This must match the Rust backend implementation exactly.
 *
 * IMPORTANT: Returns hex-encoded string (64 chars), NOT raw bytes.
 * The Rust API uses the ASCII bytes of the hex string as the JWT secret,
 * so we must do the same for compatibility.
 *
 * @param apiKey - The raw API key (e.g., "sk_live_...")
 * @param domainId - UUID of the domain (used as salt for domain isolation)
 * @returns Promise<string> - 64-char hex string (to be used as ASCII bytes for JWT signing)
 */
async function deriveJwtSecret(
  apiKey: string,
  domainId: string
): Promise<string> {
  // Convert domain_id UUID to 16 bytes (remove hyphens and decode as hex)
  const salt = Buffer.from(domainId.replace(/-/g, ""), "hex");
  const info = Buffer.from("reauth-jwt-v1");

  return new Promise((resolve, reject) => {
    hkdf("sha256", apiKey, salt, info, 32, (err: Error | null, derivedKey: ArrayBuffer) => {
      if (err) reject(err);
      else resolve(Buffer.from(derivedKey).toString("hex"));
    });
  });
}

/**
 * Transform snake_case subscription from JWT to camelCase SubscriptionInfo
 */
function transformSubscription(sub: DomainEndUserClaims["subscription"]): SubscriptionInfo {
  return {
    status: sub.status as SubscriptionInfo["status"],
    planCode: sub.plan_code,
    planName: sub.plan_name,
    currentPeriodEnd: sub.current_period_end,
    cancelAtPeriodEnd: sub.cancel_at_period_end,
    trialEndsAt: sub.trial_ends_at,
  };
}

/**
 * Parse cookies from a cookie header string.
 * Handles URL encoding and multiple cookies properly.
 */
function parseCookies(cookieHeader: string): Record<string, string> {
  const cookies: Record<string, string> = {};
  for (const cookie of cookieHeader.split(";")) {
    const [key, ...valueParts] = cookie.trim().split("=");
    if (key) {
      try {
        cookies[key] = decodeURIComponent(valueParts.join("="));
      } catch {
        cookies[key] = valueParts.join("=");
      }
    }
  }
  return cookies;
}

/**
 * Create a reauth client for server-side authentication.
 * Uses local JWT verification for fast, reliable auth checks.
 *
 * @example
 * ```typescript
 * import { createServerClient } from '@reauth/sdk/server';
 *
 * const reauth = createServerClient({
 *   domain: 'yourdomain.com',
 *   apiKey: 'sk_live_...',
 * });
 *
 * // In your API route handler
 * export async function GET(request: Request) {
 *   const result = await reauth.authenticate({
 *     headers: {
 *       authorization: request.headers.get('authorization') ?? undefined,
 *       cookie: request.headers.get('cookie') ?? undefined,
 *     },
 *   });
 *
 *   if (!result.valid || !result.user) {
 *     return Response.json({ error: result.error || 'Unauthorized' }, { status: 401 });
 *   }
 *
 *   // Access user info from JWT claims (no network call needed!)
 *   console.log('User ID:', result.user.id);
 *   console.log('Roles:', result.user.roles);
 *   console.log('Subscription:', result.user.subscription);
 *
 *   return Response.json({ user: result.user });
 * }
 * ```
 */
export function createServerClient(config: ReauthServerConfig) {
  const { domain, apiKey } = config;

  if (!apiKey) {
    throw new Error(
      "apiKey is required for createServerClient. Get one from the Reauth dashboard."
    );
  }

  const developerBaseUrl = `https://reauth.${domain}/api/developer/${domain}`;

  return {
    /**
     * Verify a JWT token locally using HKDF-derived secret.
     * No network call required - fast and reliable.
     *
     * The domain_id is extracted from the token claims automatically.
     *
     * @param token - The JWT token to verify
     * @returns AuthResult with user info from claims
     *
     * @example
     * ```typescript
     * const result = await reauth.verifyToken(token);
     * if (result.valid && result.user) {
     *   console.log('User ID:', result.user.id);
     *   console.log('Roles:', result.user.roles);
     *   console.log('Subscription:', result.user.subscription);
     * }
     * ```
     */
    async verifyToken(token: string): Promise<AuthResult> {
      try {
        // 1. Decode token to get domain_id (no verification yet)
        const unverified = jose.decodeJwt(token);
        const domainId = unverified.domain_id as string | undefined;
        const unverifiedDomain = unverified.domain as string | undefined;

        if (!domainId) {
          return { valid: false, user: null, claims: null, error: "Missing domain_id in token" };
        }

        // SECURITY: Validate domain BEFORE secret derivation to prevent
        // attacker from forcing arbitrary HKDF derivations (timing side-channel)
        if (unverifiedDomain !== domain) {
          return { valid: false, user: null, claims: null, error: "Domain mismatch" };
        }

        // 2. Derive secret and verify signature
        // Note: secret is a hex string (64 chars). We use its ASCII bytes as the JWT secret
        // to match the Rust API which does: EncodingKey::from_secret(hex_string.as_bytes())
        const secret = await deriveJwtSecret(apiKey, domainId);
        const { payload } = await jose.jwtVerify(
          token,
          new TextEncoder().encode(secret),
          {
            algorithms: ["HS256"],
            clockTolerance: 60, // 60 seconds clock skew tolerance
          }
        );

        const claims = payload as unknown as DomainEndUserClaims;

        // 3. Double-check domain after verification (defense in depth)
        if (claims.domain !== domain) {
          return { valid: false, user: null, claims: null, error: "Domain mismatch" };
        }

        return {
          valid: true,
          user: {
            id: claims.sub,
            roles: claims.roles,
            subscription: transformSubscription(claims.subscription),
          },
          claims,
        };
      } catch (err) {
        const error = err instanceof Error ? err.message : "Unknown error";
        return { valid: false, user: null, claims: null, error };
      }
    },

    /**
     * Extract a token from a request object.
     * Tries Authorization: Bearer header first, then falls back to cookies.
     *
     * @param request - Object with headers (authorization and/or cookie)
     * @returns The token string or null if not found
     *
     * @example
     * ```typescript
     * const token = reauth.extractToken({
     *   headers: {
     *     authorization: req.headers.authorization,
     *     cookie: req.headers.cookie,
     *   },
     * });
     * ```
     */
    extractToken(request: RequestLike): string | null {
      // 1. Try Authorization: Bearer header (preferred)
      const authHeader = request.headers?.authorization;
      if (authHeader?.startsWith("Bearer ")) {
        return authHeader.slice(7);
      }

      // 2. Try cookie (fallback for same-origin requests)
      const cookieHeader = request.headers?.cookie;
      if (cookieHeader) {
        const cookies = parseCookies(cookieHeader);
        if (cookies["end_user_access_token"]) {
          return cookies["end_user_access_token"];
        }
      }

      return null;
    },

    /**
     * Authenticate a request by extracting and verifying the token.
     * This is a convenience method combining extractToken and verifyToken.
     *
     * @param request - Object with headers (authorization and/or cookie)
     * @returns AuthResult with user info from claims
     *
     * @example
     * ```typescript
     * // Express/Node.js
     * async function authMiddleware(req, res, next) {
     *   const result = await reauth.authenticate({
     *     headers: {
     *       authorization: req.headers.authorization,
     *       cookie: req.headers.cookie,
     *     },
     *   });
     *
     *   if (!result.valid || !result.user) {
     *     res.status(401).json({ error: result.error || 'Unauthorized' });
     *     return;
     *   }
     *
     *   req.user = result.user;
     *   next();
     * }
     *
     * // Next.js App Router
     * export async function GET(request: NextRequest) {
     *   const result = await reauth.authenticate({
     *     headers: {
     *       authorization: request.headers.get('authorization') ?? undefined,
     *       cookie: request.headers.get('cookie') ?? undefined,
     *     },
     *   });
     *
     *   if (!result.valid) {
     *     return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
     *   }
     *
     *   return NextResponse.json({ user: result.user });
     * }
     * ```
     */
    async authenticate(request: RequestLike): Promise<AuthResult> {
      const token = this.extractToken(request);
      if (!token) {
        return { valid: false, user: null, claims: null, error: "No token provided" };
      }
      return this.verifyToken(token);
    },

    /**
     * Get user details by ID from the backend.
     * Use this when you need full user info like email, frozen status, etc.
     * that isn't available in the JWT claims.
     *
     * @param userId - The user ID to fetch
     * @returns UserDetails or null if not found
     *
     * @example
     * ```typescript
     * // After verifying token, fetch full user details if needed
     * const result = await reauth.authenticate(request);
     * if (result.valid && result.user) {
     *   // If you need email or other details not in JWT
     *   const details = await reauth.getUserById(result.user.id);
     *   if (details) {
     *     console.log('Email:', details.email);
     *     console.log('Frozen:', details.isFrozen);
     *   }
     * }
     * ```
     */
    async getUserById(userId: string): Promise<UserDetails | null> {
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
