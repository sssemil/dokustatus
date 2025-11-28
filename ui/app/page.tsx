"use client";
import { useState, useEffect } from "react";

export default function Page() {
  const [email, setEmail] = useState("");
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [checkingAuth, setCheckingAuth] = useState(true);
  const [authenticated, setAuthenticated] = useState(false);
  const [userEmail, setUserEmail] = useState<string>("");

  useEffect(() => {
    // Extract email from cookie
    const emailCookie = document.cookie
      .split('; ')
      .find(row => row.startsWith('user_email='))
      ?.split('=')[1];
    
    if (emailCookie) {
      setUserEmail(decodeURIComponent(emailCookie));
    }

    // Check if user is already logged in
    (async () => {
      try {
        const res = await fetch("/api/auth/verify", { 
          method: "GET",
          credentials: "include"
        });
        if (res.ok) {
          setAuthenticated(true);
        }
      } catch {
        // Not logged in
      } finally {
        setCheckingAuth(false);
      }
    })();
  }, []);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setLoading(true);
    
    try {
      const res = await fetch("/api/auth/request", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ email }),
      });
      
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        throw new Error(data.message || "Failed to send magic link");
      }
      
      setSent(true);
    } catch (err: any) {
      setError(err.message || "An error occurred");
    } finally {
      setLoading(false);
    }
  };

  const handleLogout = async () => {
    try {
      await fetch("/api/auth/logout", {
        method: "POST",
        credentials: "include"
      });
    } catch {
      // Ignore errors
    } finally {
      window.location.reload();
    }
  };

  if (checkingAuth) {
    return (
      <main style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', minHeight: '100vh' }}>
        <div className="loading-text">
          <span className="spinner" />
          <span>Checking session...</span>
        </div>
      </main>
    );
  }

  // Dashboard view (authenticated)
  if (authenticated) {
    return (
      <div style={{ display: 'flex', flexDirection: 'column', minHeight: '100vh' }}>
        {/* Top Panel */}
        <header style={{
          backgroundColor: 'var(--bg-secondary)',
          borderBottom: '1px solid var(--border-primary)',
          padding: 'var(--spacing-md) var(--spacing-xl)',
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          position: 'sticky',
          top: 0,
          zIndex: 100
        }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--spacing-md)' }}>
            <h1 style={{ margin: 0, fontSize: '18px', borderBottom: 'none', padding: 0 }}>
              📊 dokustatus
            </h1>
            <span style={{ color: 'var(--text-muted)', fontSize: '14px' }}>|</span>
            <span style={{ color: 'var(--text-secondary)', fontSize: '14px' }}>
              {userEmail || 'Authenticated'}
            </span>
          </div>
          <button onClick={handleLogout} className="danger">
            Logout →
          </button>
        </header>

        {/* Main Content */}
        <main style={{ flex: 1, padding: 'var(--spacing-xl)' }}>
          <div style={{ maxWidth: '1200px', margin: '0 auto' }}>
            <div className="card">
              <h2>Welcome to dokustatus</h2>
              <p style={{ marginBottom: 0 }}>
                You are successfully authenticated. Your dashboard content will appear here.
              </p>
            </div>
          </div>
        </main>
      </div>
    );
  }

  // Login view (not authenticated)
  return (
    <main style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', minHeight: '100vh' }}>
      <div className="container" style={{ maxWidth: '480px' }}>
        <h1>🔐 Sign In</h1>
        
        {sent ? (
          <div className="message success">
            <strong>✓ Email sent!</strong>
            <p style={{ marginTop: '8px', marginBottom: 0 }}>
              Check your inbox for the magic link to complete sign-in.
            </p>
          </div>
        ) : (
          <>
            <p className="text-muted mb-lg">
              Enter your email address to receive a secure login link.
            </p>
            
            <form onSubmit={submit}>
              <div>
                <label htmlFor="email">Email Address</label>
                <input
                  id="email"
                  type="email"
                  placeholder="you@example.com"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  required
                  autoComplete="email"
                  disabled={loading}
                />
              </div>
              
              <button 
                type="submit" 
                className="primary" 
                disabled={loading}
                style={{ width: '100%' }}
              >
                {loading ? (
                  <span className="loading-text">
                    <span className="spinner" />
                    Sending...
                  </span>
                ) : (
                  '→ Send Magic Link'
                )}
              </button>
            </form>
          </>
        )}
        
        {error && (
          <div className="message error">
            <strong>✗ Error</strong>
            <p style={{ marginTop: '8px', marginBottom: 0 }}>{error}</p>
          </div>
        )}
      </div>
    </main>
  );
}
