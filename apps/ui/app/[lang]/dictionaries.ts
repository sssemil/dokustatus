import 'server-only'

const dictionaries = {
  en: () => import('./dictionaries/en.json').then((m) => m.default),
  de: () => import('./dictionaries/de.json').then((m) => m.default),
}

export type SupportedLang = keyof typeof dictionaries

export async function getDictionary(locale: SupportedLang) {
  return dictionaries[locale]()
}
