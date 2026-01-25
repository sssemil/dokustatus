"use client";

import { useState, useEffect } from "react";
import { useRouter } from "next/navigation";
import { Mail, Shield, AlertTriangle } from "lucide-react";
import {
  isMainApp as checkIsMainApp,
  getRootDomain,
  URLS,
} from "@/lib/domain-utils";
import { Card, Button, Input } from "@/components/ui";

interface AuthMethods {
  magic_link: boolean;
  google_oauth: boolean;
}

export default function Home() {
  const [email, setEmail] = useState("");
  const [status, setStatus] = useState<
    "checking" | "idle" | "loading" | "sent" | "error"
  >("checking");
  const [errorMessage, setErrorMessage] = useState("");
  const [displayDomain, setDisplayDomain] = useState("");
  const [authMethods, setAuthMethods] = useState<AuthMethods>({
    magic_link: true,
    google_oauth: false,
  });
  const [googleLoading, setGoogleLoading] = useState(false);
  const router = useRouter();

  useEffect(() => {
    const hostname = window.location.hostname;
    const isMainApp = checkIsMainApp(hostname);
    const apiDomain = getRootDomain(hostname);
    setDisplayDomain(apiDomain);

    const fetchAuthMethods = async () => {
      try {
        const res = await fetch(`/api/public/domain/${apiDomain}/config`);
        if (res.ok) {
          const config = await res.json();
          setAuthMethods({
            magic_link: config.auth_methods?.magic_link ?? true,
            google_oauth: config.auth_methods?.google_oauth ?? false,
          });
        }
      } catch {}
    };
    fetchAuthMethods();

    const checkAuth = async () => {
      try {
        const res = await fetch(
          `/api/public/domain/${apiDomain}/auth/session`,
          { credentials: "include" },
        );
        if (res.ok) {
          const data = await res.json();

          if (data.error) {
            await fetch(`/api/public/domain/${apiDomain}/auth/logout`, {
              method: "POST",
              credentials: "include",
            });
            if (isMainApp) {
              window.location.href = URLS.authIngress;
            } else {
              setStatus("idle");
            }
            return;
          }

          if (data.valid) {
            if (data.waitlist_position) {
              if (isMainApp) {
                window.location.href = URLS.waitlist;
              } else {
                router.push("/waitlist");
              }
              return;
            }

            if (isMainApp) {
              router.push("/dashboard");
            } else {
              try {
                const configRes = await fetch(
                  `/api/public/domain/${apiDomain}/config`,
                );
                if (configRes.ok) {
                  const config = await configRes.json();
                  if (config.redirect_url) {
                    window.location.href = config.redirect_url;
                    return;
                  }
                }
              } catch {}
              router.push("/profile");
            }
            return;
          }
        }
      } catch {}

      if (isMainApp) {
        window.location.href = URLS.authIngress;
      } else {
        setStatus("idle");
      }
    };
    checkAuth();
  }, [router]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setStatus("loading");
    setErrorMessage("");

    const apiDomain = getRootDomain(window.location.hostname);

    try {
      const res = await fetch(
        `/api/public/domain/${apiDomain}/auth/request-magic-link`,
        {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ email }),
          credentials: "include",
        },
      );

      if (res.ok) {
        setStatus("sent");
      } else if (res.status === 429) {
        setStatus("error");
        setErrorMessage(
          "Too many requests. Please wait a moment and try again.",
        );
      } else {
        setStatus("error");
        setErrorMessage("Something went wrong. Please try again.");
      }
    } catch {
      setStatus("error");
      setErrorMessage("Network error. Please check your connection.");
    }
  };

  const handleGoogleSignIn = async () => {
    setGoogleLoading(true);
    setErrorMessage("");

    const apiDomain = getRootDomain(window.location.hostname);

    try {
      const res = await fetch(
        `/api/public/domain/${apiDomain}/auth/google/start`,
        {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          credentials: "include",
        },
      );

      if (res.ok) {
        const data = await res.json();
        window.location.href = data.auth_url;
      } else {
        const errData = await res.json().catch(() => ({}));
        setStatus("error");
        setErrorMessage(errData.message || "Failed to start Google sign-in.");
        setGoogleLoading(false);
      }
    } catch {
      setStatus("error");
      setErrorMessage("Network error. Please check your connection.");
      setGoogleLoading(false);
    }
  };

  // Loading state
  if (status === "checking") {
    return (
      <main className="min-h-screen flex items-center justify-center p-4 bg-zinc-950">
        <Card className="w-full max-w-sm p-8 text-center">
          <div className="w-8 h-8 border-2 border-zinc-600 border-t-blue-500 rounded-full animate-spin mx-auto" />
        </Card>
      </main>
    );
  }

  // Email sent state
  if (status === "sent") {
    return (
      <main className="min-h-screen flex items-center justify-center p-4 bg-zinc-950">
        <Card className="w-full max-w-sm p-8 text-center">
          <div className="w-16 h-16 bg-blue-500/10 rounded-full flex items-center justify-center mx-auto mb-6">
            <Mail size={32} className="text-blue-400" />
          </div>
          <h1 className="text-xl font-bold text-white mb-2">
            Check your email
          </h1>
          <p className="text-sm text-zinc-400 mb-4">
            We sent a sign-in link to
          </p>
          <code className="block bg-zinc-800 px-3 py-2 rounded-lg border border-zinc-700 text-sm text-white mb-4">
            {email}
          </code>
          <p className="text-xs text-zinc-500 mb-6">
            Click the link in the email to sign in. The link expires in 15
            minutes.
          </p>
          <Button
            variant="ghost"
            onClick={() => setStatus("idle")}
            className="w-full"
          >
            Use a different email
          </Button>
        </Card>
      </main>
    );
  }

  const noAuthMethods = !authMethods.magic_link && !authMethods.google_oauth;

  return (
    <main className="min-h-screen flex items-center justify-center p-4 bg-zinc-950">
      <Card className="w-full max-w-sm p-8">
        {/* Logo and header */}
        <div className="text-center mb-8">
          <div className="w-12 h-12 bg-gradient-to-br from-blue-500 to-purple-600 rounded-xl flex items-center justify-center mx-auto mb-4">
            <Shield size={24} className="text-white" />
          </div>
          <h1 className="text-xl font-bold text-white">
            {displayDomain || "Sign In"}
          </h1>
          <p className="text-sm text-zinc-400 mt-1">Sign in to your account</p>
        </div>

        {/* No auth methods warning */}
        {noAuthMethods && (
          <div className="flex items-center gap-2 p-3 bg-red-500/10 border border-red-500/20 rounded-lg text-sm text-red-400 mb-6">
            <AlertTriangle size={16} />
            No login methods are configured for this domain.
          </div>
        )}

        {/* Error message */}
        {status === "error" && (
          <div className="flex items-center gap-2 p-3 bg-red-500/10 border border-red-500/20 rounded-lg text-sm text-red-400 mb-6">
            <AlertTriangle size={16} />
            {errorMessage}
          </div>
        )}

        {/* Google Sign In */}
        {authMethods.google_oauth && (
          <button
            type="button"
            onClick={handleGoogleSignIn}
            disabled={googleLoading}
            className={`
              w-full flex items-center justify-center gap-3 px-4 py-3
              bg-zinc-800 border border-zinc-700 rounded-lg
              text-white text-sm font-medium
              hover:bg-zinc-700 hover:border-zinc-600
              transition-all duration-200
              disabled:opacity-50 disabled:cursor-not-allowed
              ${authMethods.magic_link ? "mb-6" : ""}
            `}
          >
            {googleLoading ? (
              <div className="w-5 h-5 border-2 border-zinc-600 border-t-white rounded-full animate-spin" />
            ) : (
              <svg width="20" height="20" viewBox="0 0 24 24">
                <path
                  d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"
                  fill="#4285F4"
                />
                <path
                  d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
                  fill="#34A853"
                />
                <path
                  d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"
                  fill="#FBBC05"
                />
                <path
                  d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"
                  fill="#EA4335"
                />
              </svg>
            )}
            {googleLoading ? "Connecting..." : "Continue with Google"}
          </button>
        )}

        {/* Separator */}
        {authMethods.magic_link && authMethods.google_oauth && (
          <div className="flex items-center gap-4 mb-6">
            <div className="flex-1 h-px bg-zinc-800" />
            <span className="text-xs text-zinc-500">or</span>
            <div className="flex-1 h-px bg-zinc-800" />
          </div>
        )}

        {/* Magic Link Form */}
        {authMethods.magic_link && (
          <form onSubmit={handleSubmit} className="space-y-4">
            <Input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="you@example.com"
              required
              disabled={status === "loading"}
            />
            <Button
              type="submit"
              variant="primary"
              disabled={status === "loading" || !email}
              className="w-full"
            >
              {status === "loading" ? "Sending..." : "Send magic link"}
            </Button>
          </form>
        )}

        {authMethods.magic_link && (
          <p className="text-xs text-zinc-500 text-center mt-6">
            No password needed. We&apos;ll email you a secure sign-in link.
          </p>
        )}
      </Card>
    </main>
  );
}
