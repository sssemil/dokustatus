"use client";
import { useEffect, useState } from "react";

function getTokenFromSearch(): string | null {
  if (typeof window === "undefined") return null;
  const url = new URL(window.location.href);
  return url.searchParams.get("token");
}

export default function MagicPage() {
  const [status, setStatus] = useState("Processing magic link...");
  const [done, setDone] = useState(false);
  const [error, setError] = useState(false);

  useEffect(() => {
    const token = getTokenFromSearch();
    if (!token) {
      setStatus("Missing authentication token");
      setError(true);
      return;
    }
    
    (async () => {
      try {
        const res = await fetch("/api/auth/consume", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ token }),
          credentials: "include",
        });
        
        if (!res.ok) {
          throw new Error("Invalid or expired token");
        }
        
        setStatus("Authentication successful! Redirecting...");
        setDone(true);
        
        setTimeout(() => {
          window.location.href = "/";
        }, 500);
      } catch (e: any) {
        setStatus(e.message || "Authentication failed");
        setError(true);
      }
    })();
  }, []);

  return (
    <main>
      <div className="container" style={{ maxWidth: '480px', margin: '80px auto', textAlign: 'center' }}>
        <h1>🔑 Magic Link</h1>
        
        {done ? (
          <div className="message success">
            <div className="loading-text" style={{ justifyContent: 'center' }}>
              <span className="spinner" />
              <span>{status}</span>
            </div>
          </div>
        ) : error ? (
          <div className="message error">
            <strong>✗ Authentication Failed</strong>
            <p style={{ marginTop: '8px', marginBottom: 0 }}>{status}</p>
            <p style={{ marginTop: '16px', marginBottom: 0 }}>
              <a href="/">← Return to sign in</a>
            </p>
          </div>
        ) : (
          <div className="message info">
            <div className="loading-text" style={{ justifyContent: 'center' }}>
              <span className="spinner" />
              <span>{status}</span>
            </div>
          </div>
        )}
      </div>
    </main>
  );
}
