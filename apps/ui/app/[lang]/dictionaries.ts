import 'server-only'

export type SupportedLang = 'en' | 'de'

export async function getDictionary(locale: SupportedLang) {
  const base =
    process.env.NEXT_PUBLIC_API_BASE ||
    process.env.API_BASE ||
    'http://localhost:3001';
  const url = new URL(`/api/dictionaries/${locale}`, base).toString();
  const res = await fetch(url, {
    cache: 'no-store',
  })
  if (!res.ok) {
    throw new Error(`Failed to load dictionary for ${locale}`)
  }
  return res.json()
}
