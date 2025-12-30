/**
 * Domain utilities for handling reauth routing
 * Configurable via NEXT_PUBLIC_MAIN_DOMAIN env var for local dev
 */

/** Get the main domain from env or default to reauth.dev */
const MAIN_DOMAIN = process.env.NEXT_PUBLIC_MAIN_DOMAIN || 'reauth.dev';

/** Main app domains that show the dashboard */
export const MAIN_DOMAINS = [MAIN_DOMAIN, `www.${MAIN_DOMAIN}`];

/** The auth ingress for the main app (where users log in) */
export const AUTH_INGRESS = `reauth.${MAIN_DOMAIN}`;

/** Full URLs for common redirects */
export const URLS = {
  authIngress: `https://${AUTH_INGRESS}/`,
  waitlist: `https://${AUTH_INGRESS}/waitlist`,
  profile: `https://${AUTH_INGRESS}/profile`,
} as const;

/** Google OAuth callback URL - must be configured in Google Cloud Console */
export const GOOGLE_OAUTH_REDIRECT_URI = `https://${AUTH_INGRESS}/callback/google`;

/**
 * Check if hostname is the main reauth.dev app
 */
export function isMainApp(hostname: string): boolean {
  return MAIN_DOMAINS.includes(hostname);
}

/**
 * Check if hostname is an auth ingress (reauth.* subdomain)
 */
export function isAuthIngress(hostname: string): boolean {
  return hostname.startsWith('reauth.') && !isMainApp(hostname);
}

/**
 * Extract the root domain from a reauth.* hostname
 * e.g., "reauth.example.com" -> "example.com"
 * Special case: main domain stays as-is
 */
export function getRootDomain(hostname: string): string {
  if (hostname.startsWith('reauth.') && !isMainApp(hostname)) {
    return hostname.slice('reauth.'.length);
  }
  return hostname;
}

/**
 * Get the API hostname for making requests
 * For main app, this is the auth ingress
 * For auth ingress domains, use the hostname as-is
 */
export function getApiHostname(hostname: string): string {
  if (isMainApp(hostname)) {
    return AUTH_INGRESS;
  }
  return hostname;
}

/**
 * Check if we're in a development environment
 */
export function isDevelopment(): boolean {
  return typeof window !== 'undefined' &&
    (window.location.hostname === 'localhost' ||
     window.location.hostname === '127.0.0.1');
}
