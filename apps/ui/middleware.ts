import { NextResponse } from 'next/server'
import type { NextRequest } from 'next/server'
import { match } from '@formatjs/intl-localematcher'
import Negotiator from 'negotiator'

const locales = ['en', 'de'] as const
const defaultLocale = 'de' as const

function getLocale(request: NextRequest): string {
  // Get language from headers
  const acceptLanguage = request.headers.get('accept-language')
  if (!acceptLanguage) return defaultLocale

  // Parse accept-language header
  const headers = { 'accept-language': acceptLanguage }
  const languages = new Negotiator({ headers }).languages()

  // Match against supported locales
  try {
    return match(languages, locales as unknown as string[], defaultLocale)
  } catch {
    return defaultLocale
  }
}

export function middleware(request: NextRequest) {
  const { pathname } = request.nextUrl

  // Check if there is any supported locale in the pathname
  const pathnameHasLocale = locales.some(
    (locale) => pathname.startsWith(`/${locale}/`) || pathname === `/${locale}`
  )

  if (pathnameHasLocale) return NextResponse.next()

  // Redirect if there is no locale
  const locale = getLocale(request)
  request.nextUrl.pathname = `/${locale}${pathname}`
  return NextResponse.redirect(request.nextUrl)
}

export const config = {
  matcher: [
    // Skip all internal paths (_next, api, magic)
    '/((?!_next|api|magic).*)',
    // Optional: only run on root (/) URL
    // '/'
  ],
}
