import 'server-only'
import { serverApiFetch } from '../lib/api-fetch'

export type SupportedLang = 'en' | 'de'

export async function getDictionary(locale: SupportedLang) {
  const res = await serverApiFetch(`/api/dictionaries/${locale}`, {
    cache: 'no-store',
  })
  
  if (!res.ok) {
    throw new Error(`Failed to load dictionary for ${locale}`)
  }
  return res.json()
}
