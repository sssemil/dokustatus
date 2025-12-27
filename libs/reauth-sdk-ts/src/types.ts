/** Response from GET /api/public/domain/{domain}/auth/session */
export type ReauthSession = {
  valid: boolean;
  end_user_id: string | null;
  email: string | null;
  roles: string[] | null;
  waitlist_position: number | null;
  error: string | null;
  error_code: 'ACCOUNT_SUSPENDED' | null;
};

/** Authenticated user object (basic) */
export type User = {
  id: string;
  email: string;
  roles: string[];
};

/** Full user details (from Developer API) */
export type UserDetails = {
  id: string;
  email: string;
  roles: string[];
  emailVerifiedAt: string | null;
  lastLoginAt: string | null;
  isFrozen: boolean;
  isWhitelisted: boolean;
  createdAt: string | null;
};

/** Token verification result */
export type TokenVerification = {
  valid: boolean;
  user: UserDetails | null;
};

/** Configuration for browser-side reauth client */
export type ReauthConfig = {
  /** Your verified domain (e.g., "yourdomain.com") */
  domain: string;
};

/** Configuration for server-side reauth client with API key */
export type ReauthServerConfig = ReauthConfig & {
  /** API key for server-to-server authentication (e.g., "sk_live_...") */
  apiKey?: string;
};
