"use client";
import { useState, useEffect } from "react";
import { useRouter } from "next/navigation";

type Track = {
  id: string;
  number: string;
  typ?: string | null;
  status?: string | null;
  pickup?: string | null;
  checkedAt?: string | null;
  changed?: boolean;
};

type DocTypeInfo = {
  code: string;
  prefixes: string[];
};

type Callout = {
  tone: "success" | "warning" | "error" | "info";
  title: string;
  message?: string;
  icon: string;
};

type Lang = "en" | "de";

type Dictionary = {
  trackerTitle: string;
  trackerSubtitle: string;
  processLabel: string;
  placeholder: string;
  loginPrompt: string;
  trackedDocuments: string;
  refreshNow: string;
  refreshing: string;
  noDocs: string;
  signIn: string;
  signInTitle: string;
  signInSubtitle: string;
  emailSentTitle: string;
  emailSentDesc: string;
  emailLabel: string;
  emailPlaceholder: string;
  sendMagic: string;
  sending: string;
  errorTitle: string;
  checkingStatus: string;
  statusLoading: string;
  cachedNotice: string;
  checkingSession: string;
  termsLinkText: string;
  footerNote: string;
  privacyLinkText: string;
  imprintLinkText: string;
  disclaimerTitle: string;
  disclaimerBody: string;
  disclaimerLinkNote: string;
  rateLimitTitle: string;
  rateLimitBody: string;
  trackRateLimitBody: string;
  logout: string;
  delete: string;
  updated: string;
  number: string;
  type: string;
  autoDetected: string;
  checkedAt: string;
  pickupLocation: string;
  trackingAdded: string;
  enterProcessNumber: string;
  prefixes: string;
  statusCallouts: {
    IN_BEARBEITUNG?: { title: string; message?: string };
    BEREIT_ZUR_ABHOLUNG?: { title: string; message?: string };
    AUSGEHAENDIGT?: { title: string; message?: string };
    IN_DIREKTVERSAND?: { title: string; message?: string };
    DIREKTVERSAND_ZUGESTELLT?: { title: string; message?: string };
    DIREKTVERSAND_FEHLGESCHLAGEN?: { title: string; message?: string };
    UNBEKANNT?: { title: string; message?: string };
    DOKUMENT_UNBEKANNT?: { title: string; message?: string };
    default: { title: string; message?: string };
  };
};

const STATUS_CALLOUT_METADATA: Record<
  string,
  {
    tone: Callout["tone"];
    icon: string;
  }
> = {
  IN_BEARBEITUNG: { tone: "warning", icon: "⚠️" },
  BEREIT_ZUR_ABHOLUNG: { tone: "success", icon: "✅" },
  AUSGEHAENDIGT: { tone: "warning", icon: "⚠️" },
  IN_DIREKTVERSAND: { tone: "warning", icon: "⚠️" },
  DIREKTVERSAND_ZUGESTELLT: { tone: "success", icon: "✅" },
  DIREKTVERSAND_FEHLGESCHLAGEN: { tone: "error", icon: "❌" },
  UNBEKANNT: { tone: "error", icon: "❌" },
  DOKUMENT_UNBEKANNT: { tone: "error", icon: "❌" },
};

const toneStyle = (tone: Callout["tone"]) => {
  switch (tone) {
    case "success":
      return { bg: "rgba(16,185,129,0.12)", border: "1px solid rgba(16,185,129,0.35)", color: "#0f5132" };
    case "warning":
      return { bg: "rgba(234,179,8,0.14)", border: "1px solid rgba(234,179,8,0.35)", color: "#7c5e00" };
    case "error":
      return { bg: "rgba(248,113,113,0.14)", border: "1px solid rgba(248,113,113,0.35)", color: "#7f1d1d" };
    default:
      return { bg: "rgba(59,130,246,0.12)", border: "1px solid rgba(59,130,246,0.35)", color: "#1d4ed8" };
  }
};

const getCallout = (status: string | null | undefined, dict: Dictionary): Callout => {
  if (!status) {
    return {
      tone: "info",
      title: dict.statusLoading,
      message: dict.statusCallouts.UNBEKANNT?.message || "",
      icon: "ℹ️",
    };
  }

  const metadata = STATUS_CALLOUT_METADATA[status];
  const statusDef = dict.statusCallouts[status as keyof typeof dict.statusCallouts];

  if (metadata && statusDef && typeof statusDef === "object") {
    return {
      tone: metadata.tone,
      icon: metadata.icon,
      title: statusDef.title,
      message: statusDef.message,
    };
  }

  // Default fallback
  const defaultTitle = dict.statusCallouts.default.title.replace("{status}", status);
  return {
    tone: "info",
    title: defaultTitle,
    message: dict.statusCallouts.default.message,
    icon: "ℹ️",
  };
};

const CACHE_KEY = "cachedDocNumber";
const CACHE_MS = 15 * 60 * 1000;

const formatRelativeTime = (lang: Lang, date: Date): string => {
  const diffMs = date.getTime() - Date.now();
  const diffSec = Math.round(diffMs / 1000);
  const absSec = Math.abs(diffSec);

  const rtf = new Intl.RelativeTimeFormat(lang, { numeric: "auto" });
  if (absSec < 60) {
    return rtf.format(Math.round(diffSec), "second");
  }
  const diffMin = Math.round(diffSec / 60);
  if (Math.abs(diffMin) < 60) {
    return rtf.format(diffMin, "minute");
  }
  const diffHour = Math.round(diffMin / 60);
  if (Math.abs(diffHour) < 24) {
    return rtf.format(diffHour, "hour");
  }
  const diffDay = Math.round(diffHour / 24);
  return rtf.format(diffDay, "day");
};

const normalizeTrack = (track: any): Track => ({
  ...track,
  status: track.status ?? track.lastStatus ?? null,
  pickup: track.pickup ?? track.lastPickup ?? null,
  checkedAt: track.checkedAt ?? track.lastCheckedAt ?? null,
});

const parseUtcTimestamp = (value: string): Date | null => {
  if (!value) return null;
  // If the timestamp has no timezone, treat it as UTC by appending Z.
  const normalized = /[zZ]|[+-]\d{2}:?\d{2}$/.test(value) ? value : `${value}Z`;
  const d = new Date(normalized);
  return Number.isNaN(d.getTime()) ? null : d;
};

const loadCachedNumber = (): string | null => {
  try {
    const raw = localStorage.getItem(CACHE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    if (Date.now() - (parsed.ts || 0) > CACHE_MS) {
      localStorage.removeItem(CACHE_KEY);
      return null;
    }
    return parsed.value || null;
  } catch {
    return null;
  }
};

const saveCachedNumber = (value: string) => {
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify({ value, ts: Date.now() }));
  } catch {
    // ignore
  }
};

const clearCachedNumber = () => {
  try {
    localStorage.removeItem(CACHE_KEY);
  } catch {
    // ignore
  }
};

export default function PageClient({
  lang,
  dict,
}: {
  lang: Lang;
  dict: Dictionary;
}) {
  const router = useRouter();
  const [email, setEmail] = useState("");
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [checkingAuth, setCheckingAuth] = useState(true);
  const [authenticated, setAuthenticated] = useState(false);
  const [userEmail, setUserEmail] = useState<string>("");
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [docNumber, setDocNumber] = useState("");
  const [tracks, setTracks] = useState<Track[]>([]);
  const [tracksLoading, setTracksLoading] = useState(false);
  const [tracksError, setTracksError] = useState<string | null>(null);
  const [tracksSuccess, setTracksSuccess] = useState<string | null>(null);
  const [docTypes, setDocTypes] = useState<DocTypeInfo[]>([]);
  const [showLoginPrompt, setShowLoginPrompt] = useState(false);
  const [showLoginModal, setShowLoginModal] = useState(false);
  const [rateLimited, setRateLimited] = useState(false);

  const flagRateLimit = (res: Response, context?: string) => {
    if (res.status === 429) {
      setRateLimited(true);
      return true;
    }
    return false;
  };

  const fetchMetadata = async () => {
    try {
      const res = await fetch("/api/pass/documents/info", {
        method: "GET",
        credentials: "include",
      });
      if (flagRateLimit(res, "GET /api/pass/documents/info")) return;
      if (!res.ok) return;
      const data = await res.json();
      setDocTypes(data.docTypes || []);
    } catch {
      // ignore metadata errors
    }
  };

  useEffect(() => {
    const cached = loadCachedNumber();
    if (cached) setDocNumber(cached);

    // Extract email from cookie
    const emailCookie = document.cookie
      .split("; ")
      .find((row) => row.startsWith("user_email="))
      ?.split("=")[1];

    if (emailCookie) {
      setUserEmail(decodeURIComponent(emailCookie));
    }

    // Check if user is already logged in
    (async () => {
      try {
        const res = await fetch("/api/auth/verify", {
          method: "GET",
          credentials: "include",
        });
        if (flagRateLimit(res, "GET /api/auth/verify")) {
          return;
        }
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
        credentials: "include",
      });

      if (flagRateLimit(res, "POST /api/auth/request")) {
        return;
      }
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

  const fetchTracks = async () => {
    setTracksLoading(true);
    setTracksError(null);
    setTracksSuccess(null);
    try {
      const res = await fetch("/api/pass/documents", {
        method: "GET",
        credentials: "include",
      });
      if (flagRateLimit(res, "GET /api/pass/documents")) {
        return;
      }
      if (!res.ok) {
        throw new Error("Failed to load tracked documents");
      }
      const data = await res.json();
      const items = Array.isArray(data.items) ? data.items : [];
      setTracks(items.map(normalizeTrack));
    } catch (err: any) {
      setTracksError(err.message || "Could not load tracked documents");
    } finally {
      setTracksLoading(false);
    }
  };

  const addDocument = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!docNumber.trim()) {
      setTracksError(dict.enterProcessNumber);
      return;
    }
    const trimmed = docNumber.trim();
    if (!authenticated) {
      saveCachedNumber(trimmed);
      setShowLoginPrompt(true);
      setShowLoginModal(true);
      return;
    }
    clearCachedNumber();
    setTracksError(null);
    setTracksLoading(true);
    try {
      const res = await fetch("/api/pass/documents", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({
          number: trimmed,
        }),
      });
      if (flagRateLimit(res, "POST /api/pass/documents")) {
        return;
      }
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        throw new Error(data.message || "Failed to save document");
      }
      setDocNumber("");
      setTracksSuccess(dict.trackingAdded);
      clearCachedNumber();
      await fetchTracks();
    } catch (err: any) {
      setTracksError(err.message || "Could not save document");
      setTracksLoading(false);
    }
  };

  const deleteDocument = async (id: string) => {
    setTracksError(null);
    setTracksSuccess(null);
    setTracksLoading(true);
    try {
      const res = await fetch(`/api/pass/documents/${id}`, {
        method: "DELETE",
        credentials: "include",
      });
      if (flagRateLimit(res, "DELETE /api/pass/documents/{id}")) {
        return;
      }
      if (!res.ok) {
        throw new Error("Failed to delete");
      }
      await fetchTracks();
    } catch (err: any) {
      setTracksError(err.message || "Could not delete document");
      setTracksLoading(false);
    }
  };

  useEffect(() => {
    if (authenticated) {
      fetchTracks();
      fetchMetadata();
    }
  }, [authenticated]);

  const handleLogout = async () => {
    try {
      await fetch("/api/auth/logout", {
        method: "POST",
        credentials: "include",
      });
    } catch {
      // Ignore errors
    } finally {
      window.location.reload();
    }
  };

  const switchLanguage = () => {
    const newLang = lang === "de" ? "en" : "de";
    router.push(`/${newLang}`);
  };

  if (checkingAuth) {
    return (
      <main
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          minHeight: "100vh",
        }}
      >
        <div className="loading-text">
          <span className="spinner" />
          <span>{dict.checkingSession}</span>
        </div>
      </main>
    );
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", minHeight: "100vh" }}>
      <header
        style={{
          backgroundColor: "var(--bg-secondary)",
          borderBottom: "1px solid var(--border-primary)",
          padding: "var(--spacing-md) var(--spacing-xl)",
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          position: "sticky",
          top: 0,
          zIndex: 100,
        }}
      >
        <h1 style={{ margin: 0, fontSize: "18px", borderBottom: "none", padding: 0 }}>
          📊 dokustatus
        </h1>
        <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
          <button
            onClick={switchLanguage}
            style={{
              padding: "8px 10px",
              borderRadius: "8px",
              border: "1px solid var(--border-primary)",
              background: "var(--bg-secondary)",
              cursor: "pointer",
              color: "var(--text-primary)",
            }}
          >
            {lang === "de" ? "EN" : "DE"}
          </button>
          <div style={{ position: "relative" }}>
            {authenticated ? (
              <>
                <button
                  onClick={() => setDropdownOpen(!dropdownOpen)}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: "var(--spacing-sm)",
                    padding: "var(--spacing-sm) var(--spacing-md)",
                    backgroundColor: dropdownOpen ? "var(--bg-hover)" : "var(--bg-tertiary)",
                    border: "1px solid var(--border-primary)",
                    borderRadius: "var(--radius-sm)",
                    cursor: "pointer",
                    color: "var(--text-primary)",
                  }}
                >
                  <span
                    style={{
                      width: "24px",
                      height: "24px",
                      borderRadius: "50%",
                      backgroundColor: "var(--accent-blue)",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      fontSize: "12px",
                      fontWeight: "bold",
                      color: "#000",
                    }}
                  >
                    {userEmail ? userEmail[0].toUpperCase() : "U"}
                  </span>
                  <span style={{ fontSize: "14px" }}>{userEmail || "User"}</span>
                  <span style={{ fontSize: "10px", color: "var(--text-muted)" }}>
                    {dropdownOpen ? "▲" : "▼"}
                  </span>
                </button>

                {dropdownOpen && (
                  <>
                    <div
                      onClick={() => setDropdownOpen(false)}
                      style={{
                        position: "fixed",
                        top: 0,
                        left: 0,
                        right: 0,
                        bottom: 0,
                        zIndex: 1000,
                      }}
                    />
                    <div
                      style={{
                        position: "absolute",
                        top: "calc(100% + 8px)",
                        right: 0,
                        backgroundColor: "var(--bg-secondary)",
                        border: "1px solid var(--border-primary)",
                        borderRadius: "var(--radius-md)",
                        minWidth: "200px",
                        boxShadow: "var(--shadow-md)",
                        overflow: "hidden",
                        zIndex: 1001,
                      }}
                    >
                      <button
                        onClick={() => {
                          setDropdownOpen(false);
                          handleLogout();
                        }}
                        style={{
                          width: "100%",
                          textAlign: "left",
                          padding: "var(--spacing-md)",
                          backgroundColor: "transparent",
                          border: "none",
                          color: "var(--text-error)",
                          cursor: "pointer",
                          fontSize: "14px",
                          display: "flex",
                          alignItems: "center",
                          gap: "var(--spacing-sm)",
                        }}
                      >
                        <span>🚪</span>
                        <span>{dict.logout}</span>
                      </button>
                    </div>
                  </>
                )}
              </>
            ) : (
              <button
                onClick={() => {
                  setShowLoginPrompt(true);
                  setShowLoginModal(true);
                }}
                style={{
                  padding: "10px 14px",
                  borderRadius: "8px",
                  border: "1px solid var(--border-primary)",
                  background: "var(--bg-tertiary)",
                  cursor: "pointer",
                  color: "var(--text-primary)",
                }}
              >
                {dict.signIn}
              </button>
            )}
          </div>
        </div>
      </header>

      <main
        style={{
          flex: 1,
          padding: "var(--spacing-xl)",
          minHeight: 0,
          display: "flex",
          flexDirection: "column",
        }}
      >
        <div style={{ maxWidth: "1200px", margin: "0 auto" }}>
          <div
            className="card"
            style={{
              marginBottom: "var(--spacing-lg)",
              borderLeft: "3px solid var(--accent-orange)",
              background: "var(--bg-secondary)",
            }}
          >
            <h3 style={{ marginTop: 0, marginBottom: "8px" }}>{dict.disclaimerTitle}</h3>
            <p style={{ margin: 0, color: "var(--text-muted)" }}>{dict.disclaimerBody}</p>
            <p style={{ marginTop: "8px", marginBottom: 0, color: "var(--text-muted)" }}>
              <a
                href="https://www.muenchen.de/pass"
                target="_blank"
                rel="noreferrer"
                style={{ color: "var(--text-link)" }}
              >
                https://www.muenchen.de/pass
              </a>{" "}
              {dict.disclaimerLinkNote}
            </p>
          </div>

          <div className="card" style={{ marginBottom: "var(--spacing-lg)" }}>
            <h2
              style={{
                display: "flex",
                alignItems: "center",
                gap: "8px",
                marginBottom: "12px",
              }}
            >
              <span>🛂</span> {dict.trackerTitle}
            </h2>
            <p style={{ marginTop: 0, color: "var(--text-muted)" }}>
              {dict.trackerSubtitle}
            </p>

            <form
              onSubmit={addDocument}
              style={{
                display: "grid",
                gap: "12px",
                gridTemplateColumns: "minmax(0, 1fr) auto",
                alignItems: "end",
                width: "100%",
              }}
            >
              <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
                <label style={{ fontSize: "13px", color: "var(--text-muted)" }}>
                  {dict.processLabel}
                </label>
                <input
                  type="text"
                  value={docNumber}
                  onChange={(e) => setDocNumber(e.target.value)}
                  placeholder={dict.placeholder}
                  style={{
                    width: "100%",
                    padding: "14px 12px",
                    borderRadius: "10px",
                    border: "1px solid var(--border-primary)",
                    background: "var(--bg-secondary)",
                    color: "var(--text-primary)",
                    fontWeight: 600,
                    letterSpacing: "0.4px",
                  }}
                />
              </div>
              <button
                type="submit"
                disabled={tracksLoading}
                aria-label="Add tracking number"
                style={{
                  width: "52px",
                  height: "52px",
                  borderRadius: "12px",
                  border: "none",
                  background: "linear-gradient(120deg, var(--accent-blue), #7dd3fc)",
                  color: "#000",
                  cursor: "pointer",
                  fontWeight: 800,
                  fontSize: "20px",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                }}
              >
                {tracksLoading ? "…" : "+"}
              </button>
            </form>

            {showLoginPrompt && !authenticated && (
              <div className="message info" style={{ marginTop: "12px" }}>
                {dict.loginPrompt}
              </div>
            )}

            {docTypes.length > 0 && (
              <div
                style={{
                  display: "flex",
                  flexWrap: "wrap",
                  gap: "8px",
                  marginTop: "8px",
                  fontSize: "12px",
                  color: "var(--text-muted)",
                }}
              >
                {docTypes.map((dt) => (
                  <div
                    key={dt.code}
                    style={{
                      padding: "6px 10px",
                      borderRadius: "999px",
                      border: "1px solid var(--border-primary)",
                      background: "var(--bg-secondary)",
                    }}
                  >
                    <strong style={{ marginRight: "6px", color: "var(--text-primary)" }}>
                      {dt.code}
                    </strong>
                    <span>
                      {dict.prefixes}: {dt.prefixes.join(", ")}
                    </span>
                  </div>
                ))}
              </div>
            )}

            {tracksError && (
              <div className="message error" style={{ marginTop: "12px" }}>
                {tracksError}
              </div>
            )}
            {tracksSuccess && (
              <div className="message success" style={{ marginTop: "12px" }}>
                {tracksSuccess}
              </div>
            )}

            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                marginTop: "20px",
                marginBottom: "8px",
              }}
            >
              <h3 style={{ margin: 0 }}>{dict.trackedDocuments}</h3>
            </div>

            {tracksLoading && tracks.length === 0 ? (
              <div className="loading-text" style={{ justifyContent: "flex-start" }}>
                <span className="spinner" />
                <span>{dict.checkingStatus}</span>
              </div>
            ) : tracks.length === 0 ? (
              <div className="message info">{dict.noDocs}</div>
            ) : (
              <div style={{ display: "grid", gap: "12px" }}>
                {tracks.map((track) => {
                  const callout = getCallout(track.status, dict);
                  const tone = toneStyle(callout.tone);
                  const checkedDate = track.checkedAt ? parseUtcTimestamp(track.checkedAt) : null;
                  const relativeChecked = checkedDate
                    ? formatRelativeTime(lang, checkedDate)
                    : null;
                  return (
                    <div
                      key={track.id}
                      style={{
                        padding: "12px",
                        border: "1px solid var(--border-primary)",
                        borderRadius: "10px",
                        background: "var(--bg-secondary)",
                        display: "grid",
                        gridTemplateColumns: "1fr auto",
                        gap: "12px",
                      }}
                    >
                      <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
                        <div
                          style={{ display: "flex", gap: "12px", alignItems: "center" }}
                        >
                          <div>
                            <div style={{ fontSize: "12px", color: "var(--text-muted)" }}>
                              {dict.number}
                            </div>
                            <div style={{ fontWeight: 700, letterSpacing: "0.4px" }}>
                              {track.number}
                            </div>
                          </div>
                          <div>
                            <div style={{ fontSize: "12px", color: "var(--text-muted)" }}>
                              {dict.type}
                            </div>
                            <div>{track.typ || dict.autoDetected}</div>
                          </div>
                          {checkedDate && relativeChecked && (
                            <div style={{ fontSize: "12px", color: "var(--text-muted)" }}>
                              {dict.checkedAt}:{" "}
                              <span title={checkedDate.toLocaleString()}>
                                {relativeChecked}
                              </span>
                            </div>
                          )}
                        </div>
                        <div
                          style={{
                            padding: "10px",
                            borderRadius: "8px",
                            background: tone.bg,
                            border: tone.border,
                            color: tone.color,
                            display: "grid",
                            gridTemplateColumns: "auto 1fr",
                            gap: "10px",
                            alignItems: "center",
                          }}
                        >
                          <div style={{ fontSize: "20px" }}>{callout.icon}</div>
                          <div>
                            <div style={{ fontWeight: 700, marginBottom: "4px" }}>
                              {callout.title}
                            </div>
                            {callout.message && (
                              <div
                                style={{ fontSize: "13px" }}
                                dangerouslySetInnerHTML={{ __html: callout.message }}
                              />
                            )}
                            {track.pickup && (
                              <div style={{ marginTop: "6px", fontSize: "13px" }}>
                                {dict.pickupLocation}: {track.pickup}
                              </div>
                            )}
                          </div>
                        </div>
                      </div>
                      <div
                        style={{
                          textAlign: "right",
                          display: "flex",
                          flexDirection: "column",
                          gap: "6px",
                          justifyContent: "space-between",
                        }}
                      >
                        {track.changed && (
                          <div style={{ color: "var(--accent-blue)", fontWeight: 600 }}>
                            {dict.updated}
                          </div>
                        )}
                        <button
                          onClick={() => deleteDocument(track.id)}
                          disabled={tracksLoading || !authenticated}
                          style={{
                            border: "none",
                            background: "transparent",
                            color: "var(--text-error)",
                            cursor: authenticated ? "pointer" : "not-allowed",
                            alignSelf: "flex-end",
                            opacity: authenticated ? 1 : 0.6,
                          }}
                        >
                          {dict.delete}
                        </button>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>

          {!authenticated && (
            <LoginModal
              open={showLoginModal}
              onClose={() => setShowLoginModal(false)}
              sent={sent}
              error={error}
              loading={loading}
              email={email}
              setEmail={setEmail}
              submit={submit}
              dict={dict}
            />
          )}
        </div>
      </main>

      <footer
        style={{
          borderTop: "1px solid var(--border-primary)",
          background: "var(--bg-secondary)",
          padding: "var(--spacing-md) var(--spacing-xl)",
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          color: "var(--text-muted)",
          fontSize: "12px",
          marginTop: "auto",
        }}
      >
        <span>{dict.footerNote}</span>
        <div style={{ display: "flex", gap: "12px", alignItems: "center" }}>
          <a
            href={`/${lang}/terms`}
            style={{
              color: "var(--text-link)",
              textDecoration: "none",
              fontSize: "13px",
            }}
          >
            {dict.termsLinkText}
          </a>
          <a
            href={`/${lang}/privacy`}
            style={{
              color: "var(--text-link)",
              textDecoration: "none",
              fontSize: "13px",
            }}
          >
            {dict.privacyLinkText}
          </a>
          <a
            href={`/${lang}/impressum`}
            style={{
              color: "var(--text-link)",
              textDecoration: "none",
              fontSize: "13px",
            }}
          >
            {dict.imprintLinkText}
          </a>
        </div>
      </footer>

      {rateLimited && (
        <RateLimitModal
          dict={dict}
          onClose={() => setRateLimited(false)}
        />
      )}
    </div>
  );
}

function LoginModal({
  open,
  onClose,
  sent,
  error,
  loading,
  email,
  setEmail,
  submit,
  dict,
}: {
  open: boolean;
  onClose: () => void;
  sent: boolean;
  error: string | null;
  loading: boolean;
  email: string;
  setEmail: (v: string) => void;
  submit: (e: React.FormEvent) => Promise<void>;
  dict: Dictionary;
}) {
  if (!open) return null;
  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.35)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 2000,
      }}
    >
      <div
        style={{
          background: "var(--bg-primary)",
          padding: "24px",
          borderRadius: "12px",
          boxShadow: "var(--shadow-md)",
          width: "min(420px, 90vw)",
          border: "1px solid var(--border-primary)",
        }}
      >
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            marginBottom: "12px",
          }}
        >
          <h3 style={{ margin: 0 }}>🔐 {dict.signInTitle}</h3>
          <button
            onClick={onClose}
            style={{
              border: "none",
              background: "transparent",
              cursor: "pointer",
              fontSize: "18px",
              color: "var(--text-muted)",
            }}
            aria-label="Close"
          >
            ×
          </button>
        </div>

        {sent ? (
          <div className="message success">
            <strong>{dict.emailSentTitle}</strong>
            <p style={{ marginTop: "8px", marginBottom: 0 }}>{dict.emailSentDesc}</p>
          </div>
        ) : (
          <>
            <p className="text-muted mb-lg">{dict.signInSubtitle}</p>

            <form onSubmit={submit} style={{ display: "grid", gap: "12px" }}>
              <div>
                <label htmlFor="email">{dict.emailLabel}</label>
                <input
                  id="email"
                  type="email"
                  placeholder={dict.emailPlaceholder}
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
                style={{ width: "100%" }}
              >
                {loading ? (
                  <span className="loading-text">
                    <span className="spinner" />
                    {dict.sending}
                  </span>
                ) : (
                  dict.sendMagic
                )}
              </button>
            </form>
          </>
        )}

        {error && (
          <div className="message error" style={{ marginTop: "12px" }}>
            <strong>{dict.errorTitle}</strong>
            <p style={{ marginTop: "8px", marginBottom: 0 }}>{error}</p>
          </div>
        )}
      </div>
    </div>
  );
}

function RateLimitModal({ dict, onClose }: { dict: Dictionary; onClose: () => void }) {
  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.5)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 3000,
      }}
    >
      <div
        style={{
          background: "var(--bg-primary)",
          padding: "24px",
          borderRadius: "12px",
          boxShadow: "var(--shadow-md)",
          width: "min(440px, 92vw)",
          border: "1px solid var(--border-primary)",
        }}
      >
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            marginBottom: "12px",
          }}
        >
          <h3 style={{ margin: 0 }}>⏳ {dict.rateLimitTitle}</h3>
          <button
            onClick={onClose}
            style={{
              border: "none",
              background: "transparent",
              cursor: "pointer",
              fontSize: "18px",
              color: "var(--text-muted)",
            }}
            aria-label="Close"
          >
            ×
          </button>
        </div>
        <p className="text-muted" style={{ marginTop: 0 }}>
          {dict.rateLimitBody}
        </p>
        <button
          onClick={onClose}
          className="primary"
          style={{ width: "100%", marginTop: "12px" }}
        >
          OK
        </button>
      </div>
    </div>
  );
}
