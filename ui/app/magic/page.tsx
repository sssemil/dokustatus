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

  useEffect(() => {
    const token = getTokenFromSearch();
    if (!token) {
      setStatus("Missing token");
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
        if (!res.ok) throw new Error("Invalid or expired token");
        setDone(true);
        window.location.href = "/dashboard";
      } catch (e: any) {
        setStatus(e.message || "Error");
      }
    })();
  }, []);

  return <p>{done ? "Logged in." : status}</p>;
}
