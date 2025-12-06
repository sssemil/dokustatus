import { getDictionary } from "./dictionaries";
import PageClient from "./page-client";

type SupportedLang = 'en' | 'de'

function isSupportedLang(lang: string): lang is SupportedLang {
  return lang === 'en' || lang === 'de'
}

export default async function Page(props: PageProps<'/[lang]'>) {
  const { params } = props
  const { lang } = await params

  const safeLang: SupportedLang = isSupportedLang(lang) ? lang : 'de'
  const dict = await getDictionary(safeLang)
  
  return <PageClient lang={safeLang} dict={dict} />
}
