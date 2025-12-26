import type { Metadata } from 'next';

export const metadata: Metadata = {
  title: 'Todo Demo - Reauth SDK',
  description: 'Demo todo app showcasing Reauth SDK integration',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body style={{ fontFamily: 'system-ui, sans-serif', margin: 0, padding: '20px', backgroundColor: '#f5f5f5' }}>
        {children}
      </body>
    </html>
  );
}
