/**
 * Server-side API fetch utility that forwards client IP headers
 * for proper rate limiting
 */
import { headers } from 'next/headers';

export async function serverApiFetch(
  path: string,
  options?: RequestInit
): Promise<Response> {
  const base =
    process.env.NEXT_PUBLIC_API_BASE ||
    process.env.API_BASE ||
    'http://localhost:3001';
  
  const url = new URL(path, base).toString();
  
  // Forward client IP headers from nginx to the API
  const headersList = await headers();
  const xForwardedFor = headersList.get('x-forwarded-for');
  const xRealIp = headersList.get('x-real-ip');
  
  // Build headers object with proper typing
  const fetchHeaders: Record<string, string> = {};
  
  // Copy existing headers if provided
  if (options?.headers) {
    const existingHeaders = new Headers(options.headers);
    existingHeaders.forEach((value, key) => {
      fetchHeaders[key] = value;
    });
  }
  
  // Add IP forwarding headers if present
  if (xForwardedFor) {
    fetchHeaders['x-forwarded-for'] = xForwardedFor;
  }
  if (xRealIp) {
    fetchHeaders['x-real-ip'] = xRealIp;
  }
  
  return fetch(url, {
    ...options,
    headers: fetchHeaders,
  });
}
