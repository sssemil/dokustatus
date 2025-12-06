import '../globals.css'
import type { Metadata } from 'next'

export const metadata: Metadata = {
  title: 'dokustatus',
  description: 'Track document status updates',
}

export async function generateStaticParams() {
  return [{ lang: 'en' }, { lang: 'de' }]
}

type SupportedLang = 'en' | 'de'

function isSupportedLang(lang: string): lang is SupportedLang {
  return lang === 'en' || lang === 'de'
}

export default async function LangLayout(props: LayoutProps<'/[lang]'>) {
  const { children, params } = props
  const { lang } = await params

  const safeLang: SupportedLang = isSupportedLang(lang) ? lang : 'de'

  return (
    <html lang={safeLang}>
      <body>{children}</body>
    </html>
  )
}
