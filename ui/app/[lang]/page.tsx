import { getDictionary } from "./dictionaries";
import PageClient from "./page-client";

export default async function Page({
  params,
}: {
  params: Promise<{ lang: 'en' | 'de' }>
}) {
  const { lang } = await params
  const dict = await getDictionary(lang)
  
  return <PageClient lang={lang} dict={dict} />
}
