"use client";
import { useState } from "react";

export default function Page() {
  const [email, setEmail] = useState("");
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    try {
      const res = await fetch("/api/auth/request", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ email }),
      });
      if (!res.ok) throw new Error("Failed to send magic link");
      setSent(true);
    } catch (err: any) {
      setError(err.message || "Error");
    }
  };

  return (
    <main>
      <h1>Sign in</h1>
      {sent ? (
        <p>Check your email for the login link.</p>
      ) : (
        <form onSubmit={submit}>
          <input
            type="email"
            placeholder="you@example.com"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
            style={{ padding: 8, minWidth: 280 }}
          />
          <button type="submit" style={{ marginLeft: 8 }}>Send link</button>
        </form>
      )}
      {error && <p style={{ color: 'red' }}>{error}</p>}
    </main>
  );
}
