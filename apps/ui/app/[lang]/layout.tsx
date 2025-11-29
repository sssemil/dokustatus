import '../globals.css'
import type { Metadata } from 'next'

export const metadata: Metadata = {
  title: 'dokustatus',
  description: 'Track document status updates',
}

export async function generateStaticParams() {
  return [{ lang: 'en' }, { lang: 'de' }]
}

export default async function LangLayout({
  children,
  params,
}: {
  children: React.ReactNode
  params: Promise<{ lang: 'en' | 'de' }>
}) {
  const { lang } = await params
  
  return (
    <html lang={lang}>
      <body>{children}</body>
    </html>
  )
}
