/** Response from GET /api/public/domain/{domain}/auth/session */
export type ReauthSession = {
  valid: boolean;
  end_user_id: string | null;
  email: string | null;
  roles: string[] | null;
  waitlist_position: number | null;
  error: string | null;
  error_code: "ACCOUNT_SUSPENDED" | null;
  /** Subscription information (if billing is configured) */
  subscription?: {
    status: string;
    plan_code: string | null;
    plan_name: string | null;
    current_period_end: number | null;
    cancel_at_period_end: boolean | null;
    trial_ends_at: number | null;
  };
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

/** JWT claims for domain end-user tokens */
export type DomainEndUserClaims = {
  /** User ID (subject) */
  sub: string;
  /** Domain ID */
  domain_id: string;
  /** Root domain (e.g., "example.com") */
  domain: string;
  /** User roles */
  roles: string[];
  /** Subscription information */
  subscription: {
    status: string;
    plan_code: string | null;
    plan_name: string | null;
    current_period_end: number | null;
    cancel_at_period_end: boolean | null;
    trial_ends_at: number | null;
  };
  /** Token expiration (Unix timestamp) */
  exp: number;
  /** Token issued at (Unix timestamp) */
  iat: number;
};

/** Configuration for browser-side reauth client */
export type ReauthConfig = {
  /** Your verified domain (e.g., "yourdomain.com") */
  domain: string;
};

/** Configuration for server-side reauth client */
export type ReauthServerConfig = ReauthConfig & {
  /** API key for server-to-server authentication (required, e.g., "sk_live_...") */
  apiKey: string;
};

// ============================================================================
// Subscription Types
// ============================================================================

/** Subscription status values */
export type SubscriptionStatus =
  | "active"
  | "past_due"
  | "canceled"
  | "trialing"
  | "incomplete"
  | "incomplete_expired"
  | "unpaid"
  | "paused"
  | "none";

/** Subscription info included in JWT claims */
export type SubscriptionInfo = {
  /** Current subscription status */
  status: SubscriptionStatus;
  /** Machine-readable plan identifier (e.g., "pro") */
  planCode: string | null;
  /** Human-readable plan name (e.g., "Pro Plan") */
  planName: string | null;
  /** Unix timestamp when current period ends */
  currentPeriodEnd: number | null;
  /** Whether subscription will cancel at period end */
  cancelAtPeriodEnd: boolean | null;
  /** Unix timestamp when trial ends (if applicable) */
  trialEndsAt: number | null;
};

/** Authentication result from verifyToken/authenticate */
export type AuthResult = {
  valid: boolean;
  user: {
    id: string;
    roles: string[];
    subscription: SubscriptionInfo;
  } | null;
  claims: DomainEndUserClaims | null;
  error?: string;
};

/** Request-like object for extractToken/authenticate */
export type RequestLike = {
  headers?: {
    authorization?: string;
    cookie?: string;
  };
};

/** Response from GET /auth/token endpoint */
export type TokenResponse = {
  /** The JWT access token */
  accessToken: string;
  /** Token expiration time in seconds */
  expiresIn: number;
  /** Token type (always "Bearer") */
  tokenType: string;
};

/** Subscription plan available for purchase */
export type SubscriptionPlan = {
  id: string;
  code: string;
  name: string;
  description: string | null;
  priceCents: number;
  currency: string;
  interval: "monthly" | "yearly" | "custom";
  intervalCount: number;
  trialDays: number;
  features: string[];
  displayOrder: number;
};

/** User's current subscription details */
export type UserSubscription = {
  id: string | null;
  planCode: string | null;
  planName: string | null;
  status: SubscriptionStatus;
  currentPeriodEnd: number | null;
  trialEnd: number | null;
  cancelAtPeriodEnd: boolean | null;
};

/** Checkout session response */
export type CheckoutSession = {
  checkoutUrl: string;
};

/** Portal session response */
export type PortalSession = {
  portalUrl: string;
};
