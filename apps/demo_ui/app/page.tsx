'use client';

import { useAuth } from '@reauth/sdk/react';
import { useRouter } from 'next/navigation';
import { useEffect } from 'react';

const DOMAIN = process.env.NEXT_PUBLIC_DOMAIN || 'demo.test';

export default function Home() {
  const { user, loading, login } = useAuth({ domain: DOMAIN });
  const router = useRouter();

  useEffect(() => {
    if (!loading && user) {
      router.push('/todos');
    }
  }, [user, loading, router]);

  if (loading) {
    return (
      <div style={styles.container}>
        <p>Loading...</p>
      </div>
    );
  }

  return (
    <div style={styles.container}>
      <h1 style={styles.title}>Todo Demo</h1>
      <p style={styles.subtitle}>A simple todo app powered by Reauth SDK</p>
      <button onClick={login} style={styles.button}>
        Sign in to get started
      </button>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    maxWidth: '400px',
    margin: '100px auto',
    textAlign: 'center',
    padding: '40px',
    backgroundColor: 'white',
    borderRadius: '8px',
    boxShadow: '0 2px 4px rgba(0,0,0,0.1)',
  },
  title: {
    margin: '0 0 10px 0',
    fontSize: '32px',
    fontWeight: 'bold',
  },
  subtitle: {
    margin: '0 0 30px 0',
    color: '#666',
  },
  button: {
    backgroundColor: '#0070f3',
    color: 'white',
    border: 'none',
    padding: '12px 24px',
    fontSize: '16px',
    borderRadius: '6px',
    cursor: 'pointer',
  },
};
