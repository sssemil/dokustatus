"use client";
import { useState, useEffect } from "react";

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

const STATUS_CALLOUTS: Record<string, Callout> = {
  IN_BEARBEITUNG: {
    tone: "warning",
    title: "Ihr Ausweisdokument ist noch in Arbeit",
    message:
      "Bearbeitungszeiten:<br>• Express-Reisepass: 3–6 Werktage<br>• Reisepass/Personalausweis: 4–5 Wochen<br>• eID-Karte: 5–6 Wochen",
    icon: "⚠️",
  },
  BEREIT_ZUR_ABHOLUNG: {
    tone: "success",
    title: "Ihr Ausweisdokument ist fertig",
    message: "",
    icon: "✅",
  },
  AUSGEHAENDIGT: {
    tone: "warning",
    title: "Ihr Ausweisdokument ist bereits ausgehändigt",
    message: "Das Dokument wurde schon ausgegeben.",
    icon: "⚠️",
  },
  IN_DIREKTVERSAND: {
    tone: "warning",
    title: "Ihr Ausweisdokument wurde versandt",
    message: "Versand durch die Bundesdruckerei GmbH, unterwegs.",
    icon: "⚠️",
  },
  DIREKTVERSAND_ZUGESTELLT: {
    tone: "success",
    title: "Ihr Ausweisdokument wurde zugestellt",
    message: "",
    icon: "✅",
  },
  DIREKTVERSAND_FEHLGESCHLAGEN: {
    tone: "error",
    title: "Ihr Ausweisdokument konnte Ihnen leider nicht zugestellt werden",
    message:
      "Wird 7 Tage bei der Deutschen Post bereitgehalten, danach Rücksendung an Bürgerbüro Ruppertstr. 19.",
    icon: "❌",
  },
  UNBEKANNT: {
    tone: "error",
    title: "Der Status Ihres Ausweisdokuments ist unbekannt",
    message: "Keine Statusinformationen verfügbar.",
    icon: "❌",
  },
  DOKUMENT_UNBEKANNT: {
    tone: "error",
    title: "Die eingegebene Ausweisnummer ist uns nicht bekannt",
    message: "Bitte Eingabe prüfen und erneut versuchen.",
    icon: "❌",
  },
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

const getCallout = (status?: string | null): Callout => {
  if (!status) {
    return {
      tone: "info",
      title: "Status wird geladen",
      message: "Keine Statusinformationen verfügbar.",
      icon: "ℹ️",
    };
  }
  return STATUS_CALLOUTS[status] || {
    tone: "info",
    title: `Status: ${status}`,
    message: "Noch keine Detailinformationen.",
    icon: "ℹ️",
  };
};

const CACHE_KEY = "cachedDocNumber";
const CACHE_MS = 15 * 60 * 1000;

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

export default function Page() {
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

  const fetchMetadata = async () => {
    try {
      const res = await fetch("/api/pass/documents/info", {
        method: "GET",
        credentials: "include",
      });
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
      const res = await fetch("/api/pass/documents/check", {
        method: "POST",
        credentials: "include",
      });
      if (!res.ok) {
        throw new Error("Failed to refresh status");
      }
      const data = await res.json();
      setTracks(data.items || []);
    } catch (err: any) {
      setTracksError(err.message || "Could not refresh status");
    } finally {
      setTracksLoading(false);
    }
  };

  const addDocument = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!docNumber.trim()) {
      setTracksError("Please enter a process number");
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
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        throw new Error(data.message || "Failed to save document");
      }
      setDocNumber("");
      setTracksSuccess("Tracking added and confirmed. We'll email you on status changes.");
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

  return (
    <div style={{ display: 'flex', flexDirection: 'column', minHeight: '100vh' }}>
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
        <h1 style={{ margin: 0, fontSize: '18px', borderBottom: 'none', padding: 0 }}>
          📊 dokustatus
        </h1>
        
        <div style={{ position: 'relative' }}>
          {authenticated ? (
            <>
              <button
                onClick={() => setDropdownOpen(!dropdownOpen)}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 'var(--spacing-sm)',
                  padding: 'var(--spacing-sm) var(--spacing-md)',
                  backgroundColor: dropdownOpen ? 'var(--bg-hover)' : 'var(--bg-tertiary)',
                  border: '1px solid var(--border-primary)',
                  borderRadius: 'var(--radius-sm)',
                  cursor: 'pointer',
                  color: 'var(--text-primary)'
                }}
              >
                <span style={{ 
                  width: '24px', 
                  height: '24px', 
                  borderRadius: '50%', 
                  backgroundColor: 'var(--accent-blue)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  fontSize: '12px',
                  fontWeight: 'bold',
                  color: '#000'
                }}>
                  {userEmail ? userEmail[0].toUpperCase() : 'U'}
                </span>
                <span style={{ fontSize: '14px' }}>
                  {userEmail || 'User'}
                </span>
                <span style={{ fontSize: '10px', color: 'var(--text-muted)' }}>
                  {dropdownOpen ? '▲' : '▼'}
                </span>
              </button>

              {dropdownOpen && (
                <>
                  <div
                    onClick={() => setDropdownOpen(false)}
                    style={{
                      position: 'fixed',
                      top: 0,
                      left: 0,
                      right: 0,
                      bottom: 0,
                      zIndex: 1000
                    }}
                  />
                  <div style={{
                    position: 'absolute',
                    top: 'calc(100% + 8px)',
                    right: 0,
                    backgroundColor: 'var(--bg-secondary)',
                    border: '1px solid var(--border-primary)',
                    borderRadius: 'var(--radius-md)',
                    minWidth: '200px',
                    boxShadow: 'var(--shadow-md)',
                    overflow: 'hidden',
                    zIndex: 1001
                  }}>
                    <div style={{
                      padding: 'var(--spacing-md)',
                      borderBottom: '1px solid var(--border-primary)',
                      backgroundColor: 'var(--bg-tertiary)'
                    }}>
                      <div style={{ fontSize: '12px', color: 'var(--text-muted)', marginBottom: '4px' }}>
                        Signed in as
                      </div>
                      <div style={{ fontSize: '14px', color: 'var(--text-primary)', fontWeight: 500 }}>
                        {userEmail || 'User'}
                      </div>
                    </div>
                    <button
                      onClick={() => {
                        setDropdownOpen(false);
                        handleLogout();
                      }}
                      style={{
                        width: '100%',
                        textAlign: 'left',
                        padding: 'var(--spacing-md)',
                        backgroundColor: 'transparent',
                        border: 'none',
                        color: 'var(--text-error)',
                        cursor: 'pointer',
                        fontSize: '14px',
                        display: 'flex',
                        alignItems: 'center',
                        gap: 'var(--spacing-sm)'
                      }}
                    >
                      <span>🚪</span>
                      <span>Logout</span>
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
                padding: '10px 14px',
                borderRadius: '8px',
                border: '1px solid var(--border-primary)',
                background: 'var(--bg-tertiary)',
                cursor: 'pointer',
                color: 'var(--text-primary)'
              }}
            >
              Sign in
            </button>
          )}
        </div>
      </header>

      <main style={{ flex: 1, padding: 'var(--spacing-xl)' }}>
        <div style={{ maxWidth: '1200px', margin: '0 auto' }}>
          <div className="card" style={{ marginBottom: 'var(--spacing-lg)' }}>
            <h2 style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '12px' }}>
              <span>🛂</span> Document status tracker
            </h2>
            <p style={{ marginTop: 0, color: 'var(--text-muted)' }}>
              Add your passport or ID process number and we will pull the latest status and email you when it changes.
            </p>

            <form
              onSubmit={addDocument}
              style={{
                display: 'grid',
                gap: '12px',
                gridTemplateColumns: 'minmax(0, 1fr) auto',
                alignItems: 'end',
                width: '100%'
              }}
            >
              <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                <label style={{ fontSize: '13px', color: 'var(--text-muted)' }}>Process number</label>
                <input
                  type="text"
                  value={docNumber}
                  onChange={(e) => setDocNumber(e.target.value)}
                  placeholder="e.g., CH1H123456"
                  style={{
                    width: '100%',
                    padding: '14px 12px',
                    borderRadius: '10px',
                    border: '1px solid var(--border-primary)',
                    background: 'var(--bg-secondary)',
                    color: 'var(--text-primary)',
                    fontWeight: 600,
                    letterSpacing: '0.4px'
                  }}
                />
              </div>
              <button
                type="submit"
                disabled={tracksLoading}
                aria-label="Add tracking number"
                style={{
                  width: '52px',
                  height: '52px',
                  borderRadius: '12px',
                  border: 'none',
                  background: 'linear-gradient(120deg, var(--accent-blue), #7dd3fc)',
                  color: '#000',
                  cursor: 'pointer',
                  fontWeight: 800,
                  fontSize: '20px',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center'
                }}
              >
                {tracksLoading ? '…' : '+'}
              </button>
            </form>

            {showLoginPrompt && !authenticated && (
              <div className="message info" style={{ marginTop: '12px' }}>
                Please sign in to add a tracking number. Your input is saved locally for 15 minutes.
              </div>
            )}


            {docTypes.length > 0 && (
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px', marginTop: '8px', fontSize: '12px', color: 'var(--text-muted)' }}>
                {docTypes.map((dt) => (
                  <div key={dt.code} style={{ padding: '6px 10px', borderRadius: '999px', border: '1px solid var(--border-primary)', background: 'var(--bg-secondary)' }}>
                    <strong style={{ marginRight: '6px', color: 'var(--text-primary)' }}>{dt.code}</strong>
                    <span>prefixes: {dt.prefixes.join(", ")}</span>
                  </div>
                ))}
              </div>
            )}

            {tracksError && (
              <div className="message error" style={{ marginTop: '12px' }}>
                {tracksError}
              </div>
            )}
            {tracksSuccess && (
              <div className="message success" style={{ marginTop: '12px' }}>
                {tracksSuccess}
              </div>
            )}

            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: '20px', marginBottom: '8px' }}>
              <h3 style={{ margin: 0 }}>Tracked documents</h3>
              <button
                onClick={fetchTracks}
                disabled={tracksLoading || !authenticated}
                style={{
                  padding: '8px 12px',
                  borderRadius: '8px',
                  border: '1px solid var(--border-primary)',
                  background: 'var(--bg-secondary)',
                  cursor: authenticated ? 'pointer' : 'not-allowed',
                  color: 'var(--text-primary)',
                  opacity: authenticated ? 1 : 0.6
                }}
              >
                {tracksLoading ? 'Refreshing…' : 'Refresh now'}
              </button>
            </div>

            {tracksLoading && tracks.length === 0 ? (
              <div className="loading-text" style={{ justifyContent: 'flex-start' }}>
                <span className="spinner" />
                <span>Checking status…</span>
              </div>
            ) : tracks.length === 0 ? (
              <div className="message info">
                No documents added yet. Add your first number above to start tracking.
              </div>
            ) : (
              <div style={{ display: 'grid', gap: '12px' }}>
                {tracks.map((track) => {
                  const callout = getCallout(track.status);
                  const tone = toneStyle(callout.tone);
                  return (
                    <div
                      key={track.id}
                      style={{
                        padding: '12px',
                        border: '1px solid var(--border-primary)',
                        borderRadius: '10px',
                        background: 'var(--bg-secondary)',
                        display: 'grid',
                        gridTemplateColumns: '1fr auto',
                        gap: '12px',
                      }}
                    >
                      <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
                        <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}>
                          <div>
                            <div style={{ fontSize: '12px', color: 'var(--text-muted)' }}>Number</div>
                            <div style={{ fontWeight: 700, letterSpacing: '0.4px' }}>{track.number}</div>
                          </div>
                          <div>
                            <div style={{ fontSize: '12px', color: 'var(--text-muted)' }}>Type</div>
                            <div>{track.typ || 'Automatisch erkannt'}</div>
                          </div>
                          {track.checkedAt && (
                            <div style={{ fontSize: '12px', color: 'var(--text-muted)' }}>
                              Checked at {new Date(track.checkedAt).toLocaleString()}
                            </div>
                          )}
                        </div>
                        <div
                          style={{
                            padding: '10px',
                            borderRadius: '8px',
                            background: tone.bg,
                            border: tone.border,
                            color: tone.color,
                            display: 'grid',
                            gridTemplateColumns: 'auto 1fr',
                            gap: '10px',
                            alignItems: 'center',
                          }}
                        >
                          <div style={{ fontSize: '20px' }}>{callout.icon}</div>
                          <div>
                            <div style={{ fontWeight: 700, marginBottom: '4px' }}>{callout.title}</div>
                            {callout.message && (
                              <div
                                style={{ fontSize: '13px' }}
                                dangerouslySetInnerHTML={{ __html: callout.message }}
                              />
                            )}
                            {track.pickup && (
                              <div style={{ marginTop: '6px', fontSize: '13px' }}>
                                Abholort: {track.pickup}
                              </div>
                            )}
                          </div>
                        </div>
                      </div>
                      <div style={{ textAlign: 'right', display: 'flex', flexDirection: 'column', gap: '6px', justifyContent: 'space-between' }}>
                        {track.changed && (
                          <div style={{ color: 'var(--accent-blue)', fontWeight: 600 }}>
                            Updated
                          </div>
                        )}
                        <button
                          onClick={() => deleteDocument(track.id)}
                          disabled={tracksLoading || !authenticated}
                          style={{
                            border: 'none',
                            background: 'transparent',
                            color: 'var(--text-error)',
                            cursor: authenticated ? 'pointer' : 'not-allowed',
                            alignSelf: 'flex-end',
                            opacity: authenticated ? 1 : 0.6
                          }}
                        >
                          Delete
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
            />
          )}
        </div>
      </main>
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
}: {
  open: boolean;
  onClose: () => void;
  sent: boolean;
  error: string | null;
  loading: boolean;
  email: string;
  setEmail: (v: string) => void;
  submit: (e: React.FormEvent) => Promise<void>;
}) {
  if (!open) return null;
  return (
    <div style={{
      position: 'fixed',
      inset: 0,
      background: 'rgba(0,0,0,0.35)',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      zIndex: 2000
    }}>
      <div style={{
        background: 'var(--bg-primary)',
        padding: '24px',
        borderRadius: '12px',
        boxShadow: 'var(--shadow-md)',
        width: 'min(420px, 90vw)',
        border: '1px solid var(--border-primary)',
      }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
          <h3 style={{ margin: 0 }}>🔐 Sign In</h3>
          <button
            onClick={onClose}
            style={{ border: 'none', background: 'transparent', cursor: 'pointer', fontSize: '18px', color: 'var(--text-muted)' }}
            aria-label="Close"
          >
            ×
          </button>
        </div>

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
            
            <form onSubmit={submit} style={{ display: 'grid', gap: '12px' }}>
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
          <div className="message error" style={{ marginTop: '12px' }}>
            <strong>✗ Error</strong>
            <p style={{ marginTop: '8px', marginBottom: 0 }}>{error}</p>
          </div>
        )}
      </div>
    </div>
  );
}
