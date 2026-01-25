"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { Suspense } from "react";
import { Clock, AlertTriangle, LogOut } from "lucide-react";
import { isMainApp as checkIsMainApp, getRootDomain } from "@/lib/domain-utils";
import { Card, Button } from "@/components/ui";

function WaitlistContent() {
  const router = useRouter();
  const [status, setStatus] = useState<"loading" | "waitlist" | "error">(
    "loading",
  );
  const [position, setPosition] = useState<number | null>(null);
  const [email, setEmail] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState("");

  useEffect(() => {
    const checkStatus = async () => {
      const hostname = window.location.hostname;
      const isMainApp = checkIsMainApp(hostname);
      const apiDomain = getRootDomain(hostname);

      try {
        const res = await fetch(
          `/api/public/domain/${apiDomain}/auth/session`,
          {
            credentials: "include",
          },
        );

        if (!res.ok) {
          router.push("/");
          return;
        }

        const data = await res.json();

        if (data.error) {
          setStatus("error");
          setErrorMessage(data.error);
          await fetch(`/api/public/domain/${apiDomain}/auth/logout`, {
            method: "POST",
            credentials: "include",
          });
          return;
        }

        if (!data.valid) {
          router.push("/");
          return;
        }

        if (data.waitlist_position) {
          setPosition(data.waitlist_position);
          setEmail(data.email || null);
          setStatus("waitlist");
        } else {
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
        }
      } catch {
        router.push("/");
      }
    };

    checkStatus();
  }, [router]);

  const handleLogout = async () => {
    const apiDomain = getRootDomain(window.location.hostname);
    await fetch(`/api/public/domain/${apiDomain}/auth/logout`, {
      method: "POST",
      credentials: "include",
    });
    window.location.href = "/";
  };

  if (status === "loading") {
    return (
      <main className="min-h-screen flex items-center justify-center p-4 bg-zinc-950">
        <Card className="w-full max-w-sm p-8 text-center">
          <div className="w-8 h-8 border-2 border-zinc-600 border-t-blue-500 rounded-full animate-spin mx-auto mb-4" />
          <h2 className="text-lg font-semibold text-white">
            Checking status...
          </h2>
        </Card>
      </main>
    );
  }

  if (status === "error") {
    return (
      <main className="min-h-screen flex items-center justify-center p-4 bg-zinc-950">
        <Card className="w-full max-w-md p-8 text-center">
          <div className="w-16 h-16 bg-red-500/10 rounded-full flex items-center justify-center mx-auto mb-6">
            <AlertTriangle size={32} className="text-red-400" />
          </div>
          <h2 className="text-xl font-bold text-white mb-2">
            Account Suspended
          </h2>
          <p className="text-sm text-zinc-400 mb-6">
            {errorMessage || "Your account has been suspended."}
          </p>
          <Button
            variant="primary"
            onClick={() => (window.location.href = "/")}
            className="w-full"
          >
            Go to login
          </Button>
        </Card>
      </main>
    );
  }

  return (
    <main className="min-h-screen flex items-center justify-center p-4 bg-zinc-950">
      <Card className="w-full max-w-md p-8 text-center">
        <div className="w-16 h-16 bg-blue-500/10 rounded-full flex items-center justify-center mx-auto mb-6">
          <Clock size={32} className="text-blue-400" />
        </div>

        <h1 className="text-xl font-bold text-white mb-2">
          You&apos;re on the waitlist!
        </h1>

        {email && <p className="text-sm text-zinc-400 mb-6">{email}</p>}

        {position && (
          <div className="bg-zinc-800/50 rounded-xl p-6 mb-6 border border-zinc-700">
            <div className="text-sm text-zinc-500 mb-1">Your position</div>
            <div className="text-5xl font-bold text-blue-400">#{position}</div>
          </div>
        )}

        <p className="text-sm text-zinc-400 mb-6">
          We&apos;ll notify you when your account is approved. Thank you for
          your patience!
        </p>

        <Button variant="ghost" onClick={handleLogout} className="w-full">
          <LogOut size={16} className="mr-2" />
          Sign out
        </Button>
      </Card>
    </main>
  );
}

export default function WaitlistPage() {
  return (
    <Suspense
      fallback={
        <main className="min-h-screen flex items-center justify-center p-4 bg-zinc-950">
          <Card className="w-full max-w-sm p-8 text-center">
            <div className="w-8 h-8 border-2 border-zinc-600 border-t-blue-500 rounded-full animate-spin mx-auto mb-4" />
            <h2 className="text-lg font-semibold text-white">Loading...</h2>
          </Card>
        </main>
      }
    >
      <WaitlistContent />
    </Suspense>
  );
}
