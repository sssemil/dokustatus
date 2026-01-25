"use client";

import { useEffect, useState } from "react";
import { useSearchParams } from "next/navigation";
import { Suspense } from "react";
import { getRootDomain } from "@/lib/domain-utils";
import { Button } from "@/components/ui";

type Status = "loading" | "success" | "waitlist" | "error";

function GoogleCompleteHandler() {
  const [status, setStatus] = useState<Status>("loading");
  const [errorMessage, setErrorMessage] = useState("");
  const [waitlistPosition, setWaitlistPosition] = useState<number | null>(null);
  const searchParams = useSearchParams();

  useEffect(() => {
    const handleComplete = async () => {
      const token = searchParams.get("token");

      if (!token) {
        setStatus("error");
        setErrorMessage("Missing completion token.");
        return;
      }

      const hostname = window.location.hostname;
      const apiDomain = getRootDomain(hostname);

      try {
        // Call the complete endpoint to set cookies on this domain
        const res = await fetch(
          `/api/public/domain/${apiDomain}/auth/google/complete`,
          {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ token }),
            credentials: "include",
          },
        );

        if (res.ok) {
          const data = await res.json();

          // Check if user is on waitlist
          if (data.waitlist_position != null) {
            setStatus("waitlist");
            setWaitlistPosition(data.waitlist_position);
            // Redirect to waitlist page after a short delay
            setTimeout(() => {
              window.location.href = `/waitlist?position=${data.waitlist_position}`;
            }, 2000);
          } else {
            setStatus("success");
            // Redirect to the configured redirect URL
            setTimeout(() => {
              if (data.redirect_url) {
                window.location.href = data.redirect_url;
              } else {
                // Fallback to domain root
                window.location.href = `https://${apiDomain}`;
              }
            }, 1000);
          }
        } else {
          const errData = await res.json().catch(() => ({}));
          setStatus("error");
          if (errData.message?.includes("expired")) {
            setErrorMessage(
              "The sign-in session has expired. Please try again.",
            );
          } else if (errData.message?.includes("mismatch")) {
            setErrorMessage(
              "Security error: domain mismatch. Please try again.",
            );
          } else {
            setErrorMessage(errData.message || "Failed to complete sign-in.");
          }
        }
      } catch {
        setStatus("error");
        setErrorMessage("Network error. Please check your connection.");
      }
    };

    handleComplete();
  }, [searchParams]);

  return (
    <main className="flex items-center justify-center min-h-screen">
      <div className="bg-zinc-900 rounded-lg p-8 border border-zinc-800 text-center max-w-[450px] w-full">
        {status === "loading" && (
          <>
            <div className="spinner mx-auto mb-6" />
            <h2 className="text-xl font-semibold text-white">
              Completing sign-in...
            </h2>
            <p className="text-zinc-400 mt-2">Please wait a moment.</p>
          </>
        )}

        {status === "success" && (
          <>
            <div className="mb-6 text-emerald-400">
              <svg
                width="48"
                height="48"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                className="mx-auto"
              >
                <path d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            </div>
            <h2 className="text-xl font-semibold text-white">
              You&apos;re in!
            </h2>
            <p className="text-zinc-400 mt-2">Redirecting...</p>
          </>
        )}

        {status === "waitlist" && (
          <>
            <div className="mb-6 text-blue-400">
              <svg
                width="48"
                height="48"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                className="mx-auto"
              >
                <path d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            </div>
            <h2 className="text-xl font-semibold text-white">
              You&apos;re on the waitlist!
            </h2>
            <p className="text-zinc-400 mt-2">
              {waitlistPosition != null
                ? `Your position: #${waitlistPosition}`
                : "Checking your position..."}
            </p>
            <p className="text-zinc-400 text-sm mt-2">
              Redirecting to waitlist page...
            </p>
          </>
        )}

        {status === "error" && (
          <>
            <div className="mb-6 text-red-400">
              <svg
                width="48"
                height="48"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                className="mx-auto"
              >
                <path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
            </div>
            <h2 className="text-xl font-semibold text-white">
              Something went wrong
            </h2>
            <p className="text-zinc-400 mt-2">{errorMessage}</p>
            <Button
              onClick={() => (window.location.href = "/")}
              variant="primary"
              className="mt-4"
            >
              Try again
            </Button>
          </>
        )}
      </div>
    </main>
  );
}

export default function GoogleCompletePage() {
  return (
    <Suspense
      fallback={
        <main className="flex items-center justify-center min-h-screen">
          <div className="bg-zinc-900 rounded-lg p-8 border border-zinc-800 text-center max-w-[400px] w-full">
            <div className="spinner mx-auto" />
            <h2 className="text-xl font-semibold text-white mt-6">
              Loading...
            </h2>
          </div>
        </main>
      }
    >
      <GoogleCompleteHandler />
    </Suspense>
  );
}
