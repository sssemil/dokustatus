'use client';

import { useEffect } from 'react';
import { useRouter } from 'next/navigation';

export default function LoginPage() {
  const router = useRouter();

  useEffect(() => {
    router.replace('/');
  }, [router]);

  return (
    <main className="flex items-center justify-center">
      <div className="card text-center" style={{ maxWidth: '400px', width: '100%' }}>
        <div className="spinner" style={{ margin: '0 auto' }} />
      </div>
    </main>
  );
}
