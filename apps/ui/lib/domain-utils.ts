/**
 * Domain utilities for handling reauth.dev routing
 */

/** Main app domains that show the dashboard */
export const MAIN_DOMAINS = ['reauth.dev', 'www.reauth.dev'];

/** The auth ingress for the main app (where users log in to reauth.dev) */
export const AUTH_INGRESS = 'reauth.reauth.dev';

/** Full URLs for common redirects */
export const URLS = {
  authIngress: `https://${AUTH_INGRESS}/`,
  waitlist: `https://${AUTH_INGRESS}/waitlist`,
  profile: `https://${AUTH_INGRESS}/profile`,
} as const;

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
 * Special case: "reauth.dev" stays as "reauth.dev"
 */
export function getRootDomain(hostname: string): string {
  if (hostname.startsWith('reauth.') && hostname !== 'reauth.dev') {
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
