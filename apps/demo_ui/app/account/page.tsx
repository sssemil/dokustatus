"use client";

import { useAuth } from "@reauth/sdk/react";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

const DOMAIN = process.env.NEXT_PUBLIC_DOMAIN || "demo.test";

interface UserDetails {
  id: string;
  email: string;
  roles: string[];
  emailVerifiedAt: string | null;
  lastLoginAt: string | null;
  isFrozen: boolean;
  isWhitelisted: boolean;
  createdAt: string | null;
}

export default function AccountPage() {
  const { user, loading, logout } = useAuth({ domain: DOMAIN });
  const router = useRouter();
  const [userDetails, setUserDetails] = useState<UserDetails | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Redirect if not authenticated
  useEffect(() => {
    if (!loading && !user) {
      router.push("/");
    }
  }, [user, loading, router]);

  // Fetch user details
  useEffect(() => {
    if (!user) return;

    const fetchUserDetails = async () => {
      try {
        const res = await fetch("/api/me", { credentials: "include" });
        if (res.ok) {
          const data = await res.json();
          setUserDetails(data);
        } else {
          setError("Failed to load account details");
        }
      } catch (err) {
        console.error("Failed to fetch user details:", err);
        setError("Failed to load account details");
      } finally {
        setIsLoading(false);
      }
    };

    fetchUserDetails();
  }, [user]);

  const handleLogout = async () => {
    await logout();
    router.push("/");
  };

  const formatDate = (dateString: string | null) => {
    if (!dateString) return "Never";
    return new Date(dateString).toLocaleString();
  };

  if (loading || !user) {
    return (
      <div style={styles.container}>
        <p>Loading...</p>
      </div>
    );
  }

  return (
    <div style={styles.container}>
      <header style={styles.header}>
        <div>
          <h1 style={styles.title}>Account Settings</h1>
          <p style={styles.email}>{user.email}</p>
        </div>
        <div style={styles.headerButtons}>
          <button
            onClick={() => router.push("/todos")}
            style={styles.backButton}
          >
            Back to Todos
          </button>
          <a
            href={`https://reauth.${DOMAIN}/profile`}
            style={styles.profileLink}
          >
            Reauth Profile
          </a>
          <button onClick={handleLogout} style={styles.logoutButton}>
            Sign out
          </button>
        </div>
      </header>

      {error && <p style={styles.error}>{error}</p>}

      <div style={styles.comparison}>
        {/* Without API Key - from cookie/session */}
        <div style={styles.column}>
          <h2 style={styles.columnTitle}>Without Developer API Key</h2>
          <p style={styles.columnSubtitle}>From session cookie via getUser()</p>
          <div style={styles.detailsGrid}>
            <div style={styles.detailRow}>
              <span style={styles.label}>User ID</span>
              <span style={styles.value}>{user.id}</span>
            </div>
            <div style={styles.detailRow}>
              <span style={styles.label}>Email</span>
              <span style={styles.value}>{user.email}</span>
            </div>
            <div style={styles.detailRow}>
              <span style={styles.label}>Roles</span>
              <span style={styles.value}>
                {user.roles.length > 0 ? user.roles.join(", ") : "None"}
              </span>
            </div>
          </div>
        </div>

        {/* With API Key - full profile */}
        <div style={styles.column}>
          <h2 style={styles.columnTitle}>With Developer API Key</h2>
          <p style={styles.columnSubtitle}>From API via getUserById()</p>
          {isLoading ? (
            <p style={styles.loading}>Loading...</p>
          ) : userDetails ? (
            <div style={styles.detailsGrid}>
              <div style={styles.detailRow}>
                <span style={styles.label}>User ID</span>
                <span style={styles.value}>{userDetails.id}</span>
              </div>
              <div style={styles.detailRow}>
                <span style={styles.label}>Email</span>
                <span style={styles.value}>{userDetails.email}</span>
              </div>
              <div style={styles.detailRow}>
                <span style={styles.label}>Roles</span>
                <span style={styles.value}>
                  {userDetails.roles.length > 0
                    ? userDetails.roles.join(", ")
                    : "None"}
                </span>
              </div>
              <div style={styles.detailRow}>
                <span style={styles.label}>Email Verified</span>
                <span style={styles.value}>
                  {userDetails.emailVerifiedAt ? (
                    <span style={styles.verified}>
                      {formatDate(userDetails.emailVerifiedAt)}
                    </span>
                  ) : (
                    <span style={styles.notVerified}>Not verified</span>
                  )}
                </span>
              </div>
              <div style={styles.detailRow}>
                <span style={styles.label}>Account Created</span>
                <span style={styles.value}>
                  {formatDate(userDetails.createdAt)}
                </span>
              </div>
              <div style={styles.detailRow}>
                <span style={styles.label}>Last Login</span>
                <span style={styles.value}>
                  {formatDate(userDetails.lastLoginAt)}
                </span>
              </div>
              <div style={styles.detailRow}>
                <span style={styles.label}>Whitelisted</span>
                <span style={styles.value}>
                  {userDetails.isWhitelisted ? (
                    <span style={styles.verified}>Yes</span>
                  ) : (
                    <span style={styles.notVerified}>No</span>
                  )}
                </span>
              </div>
              {userDetails.isFrozen && (
                <div style={styles.detailRow}>
                  <span style={styles.label}>Status</span>
                  <span style={{ ...styles.value, ...styles.frozen }}>
                    Account Frozen
                  </span>
                </div>
              )}
            </div>
          ) : (
            <p style={styles.error}>Failed to load</p>
          )}
        </div>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    maxWidth: "900px",
    margin: "40px auto",
    padding: "30px",
    backgroundColor: "white",
    borderRadius: "8px",
    boxShadow: "0 2px 4px rgba(0,0,0,0.1)",
  },
  header: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    marginBottom: "30px",
    paddingBottom: "20px",
    borderBottom: "1px solid #eee",
  },
  title: {
    margin: "0 0 5px 0",
    fontSize: "24px",
  },
  email: {
    margin: 0,
    color: "#666",
    fontSize: "14px",
  },
  headerButtons: {
    display: "flex",
    gap: "10px",
    alignItems: "center",
  },
  backButton: {
    backgroundColor: "#0070f3",
    color: "white",
    border: "none",
    padding: "8px 16px",
    borderRadius: "4px",
    cursor: "pointer",
    fontSize: "14px",
    textDecoration: "none",
  },
  profileLink: {
    backgroundColor: "transparent",
    border: "1px solid #0070f3",
    color: "#0070f3",
    padding: "8px 16px",
    borderRadius: "4px",
    fontSize: "14px",
    textDecoration: "none",
  },
  logoutButton: {
    backgroundColor: "transparent",
    border: "1px solid #ddd",
    padding: "8px 16px",
    borderRadius: "4px",
    cursor: "pointer",
    color: "#666",
  },
  comparison: {
    display: "grid",
    gridTemplateColumns: "1fr 1fr",
    gap: "24px",
  },
  column: {
    padding: "20px",
    backgroundColor: "#fafafa",
    borderRadius: "8px",
    border: "1px solid #eee",
  },
  columnTitle: {
    fontSize: "16px",
    fontWeight: 600,
    margin: "0 0 4px 0",
    color: "#333",
  },
  columnSubtitle: {
    fontSize: "12px",
    color: "#888",
    margin: "0 0 16px 0",
  },
  detailsGrid: {
    display: "flex",
    flexDirection: "column",
    gap: "8px",
  },
  detailRow: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "10px 12px",
    backgroundColor: "white",
    borderRadius: "6px",
    border: "1px solid #eee",
  },
  label: {
    color: "#666",
    fontSize: "13px",
  },
  value: {
    fontWeight: 500,
    fontSize: "13px",
    textAlign: "right",
    maxWidth: "60%",
    wordBreak: "break-all",
  },
  verified: {
    color: "#22c55e",
  },
  notVerified: {
    color: "#888",
  },
  frozen: {
    color: "#ef4444",
    fontWeight: 600,
  },
  loading: {
    textAlign: "center",
    color: "#666",
    padding: "20px",
  },
  error: {
    color: "#ef4444",
    textAlign: "center",
    padding: "12px",
    backgroundColor: "#fef2f2",
    borderRadius: "6px",
    marginBottom: "20px",
  },
};
