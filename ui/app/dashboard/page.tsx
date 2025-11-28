"use client";
import { useEffect, useState } from "react";

export default function Dashboard() {
  const [ok, setOk] = useState<boolean | null>(null);

  useEffect(() => {
    // In a real app, call a protected endpoint; here we just check cookies presence by calling a no-op
    (async () => {
      try {
        const res = await fetch("/api/auth/request", { method: "POST", body: JSON.stringify({ email: "noop@example.com" }), headers: {"Content-Type":"application/json"}});
        setOk(res.ok);
      } catch {
        setOk(false);
      }
    })();
  }, []);

  return (
    <main>
      <h1>Dashboard</h1>
      <p>Welcome! If you see this after following the magic link, cookies are set.</p>
      {ok === false && <p style={{color:'orange'}}>Note: demo check failed, but login may still be fine.</p>}
    </main>
  );
}
