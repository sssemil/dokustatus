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

/** Authenticated user object */
export type User = {
  id: string;
  email: string;
  roles: string[];
};

/** Configuration for reauth client */
export type ReauthConfig = {
  /** Your verified domain (e.g., "yourdomain.com") */
  domain: string;
};
