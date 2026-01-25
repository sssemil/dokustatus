"use client";

import { useState, useEffect } from "react";
import Link from "next/link";
import {
  AlertTriangle,
  Globe,
  Users,
  ChevronRight,
  Plus,
  Activity,
} from "lucide-react";
import { Card, Button, Skeleton, ProgressBar } from "@/components/ui";

type UsageStats = {
  domains_count: number;
  total_users: number;
  domains_limit?: number;
  users_limit?: number;
};

type Domain = {
  id: string;
  domain: string;
  status: string;
  has_auth_methods: boolean;
  user_count?: number;
};

// Plan limits (placeholder - could be from API)
const planLimits = {
  authentications: { used: 247, limit: 1000 },
};

export default function DashboardPage() {
  const [stats, setStats] = useState<UsageStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [domainsNeedingAttention, setDomainsNeedingAttention] = useState<
    Domain[]
  >([]);
  const [verifiedDomains, setVerifiedDomains] = useState<Domain[]>([]);

  useEffect(() => {
    const fetchData = async () => {
      try {
        // Fetch stats
        const statsRes = await fetch("/api/domains/stats", {
          credentials: "include",
        });
        if (statsRes.ok) {
          const data = await statsRes.json();
          setStats(data);
        }

        // Fetch domains
        const domainsRes = await fetch("/api/domains", {
          credentials: "include",
        });
        if (domainsRes.ok) {
          const domains: Domain[] = await domainsRes.json();
          // Domains needing attention: unverified OR verified but no auth methods
          const needAttention = domains.filter(
            (d) =>
              d.status !== "verified" ||
              (d.status === "verified" && !d.has_auth_methods),
          );
          const verified = domains.filter((d) => d.status === "verified");
          setDomainsNeedingAttention(needAttention);
          setVerifiedDomains(verified);
        }
      } catch {
        // Ignore
      } finally {
        setLoading(false);
      }
    };
    fetchData();
  }, []);

  // Default limits for free tier
  const domainsLimit = stats?.domains_limit ?? 5;
  const usersLimit = stats?.users_limit ?? 50;

  return (
    <div className="space-y-6">
      {/* Page header */}
      <div className="flex items-center justify-between">
        <h1 className="text-xl sm:text-2xl font-bold text-white">Dashboard</h1>
        <Link href="/domains">
          <Button variant="primary">
            <Plus size={16} />
            <span className="hidden sm:inline ml-1">Add Domain</span>
          </Button>
        </Link>
      </div>

      {/* Domains Needing Attention */}
      {domainsNeedingAttention.length > 0 && (
        <Card className="p-4 sm:p-5 border-amber-600/30 bg-amber-900/10">
          <div className="flex items-start gap-3">
            <AlertTriangle
              className="text-amber-400 mt-0.5 flex-shrink-0"
              size={20}
            />
            <div className="flex-1 min-w-0">
              <h3 className="font-semibold text-amber-400 mb-2">
                Domains need attention
              </h3>
              <div className="space-y-2">
                {domainsNeedingAttention.map((d) => (
                  <Link
                    key={d.id}
                    href={`/domains/${d.id}`}
                    className="flex items-center justify-between w-full p-2 bg-amber-900/20 rounded-lg border border-amber-600/40 hover:bg-amber-900/30 transition-colors text-left"
                  >
                    <span className="text-sm truncate text-zinc-200">
                      {d.domain}
                    </span>
                    <span className="text-xs text-amber-400 flex-shrink-0 ml-2">
                      {d.status !== "verified"
                        ? "DNS not verified"
                        : "No auth methods"}
                    </span>
                  </Link>
                ))}
              </div>
            </div>
          </div>
        </Card>
      )}

      {/* Stats Grid */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
        {/* Authentications */}
        <Card className="p-4 sm:p-5">
          <div className="flex items-center justify-between mb-3">
            <span className="text-sm text-zinc-400">Authentications</span>
            <Activity size={18} className="text-zinc-600" />
          </div>
          {loading ? (
            <Skeleton className="h-8 w-16 mb-2" />
          ) : (
            <div className="text-2xl sm:text-3xl font-bold mb-2">
              {planLimits.authentications.used}
            </div>
          )}
          <ProgressBar
            value={planLimits.authentications.used}
            max={planLimits.authentications.limit}
            className="mb-2"
          />
          <div className="text-xs text-zinc-500">
            {planLimits.authentications.limit - planLimits.authentications.used}{" "}
            remaining this month
          </div>
        </Card>

        {/* Domains */}
        <Card className="p-4 sm:p-5">
          <div className="flex items-center justify-between mb-3">
            <span className="text-sm text-zinc-400">Domains</span>
            <Globe size={18} className="text-zinc-600" />
          </div>
          {loading ? (
            <Skeleton className="h-8 w-16 mb-2" />
          ) : (
            <div className="text-2xl sm:text-3xl font-bold mb-2">
              {stats?.domains_count ?? 0}
            </div>
          )}
          <ProgressBar
            value={stats?.domains_count ?? 0}
            max={domainsLimit}
            className="mb-2"
          />
          <div className="text-xs text-zinc-500">
            {domainsLimit - (stats?.domains_count ?? 0)} remaining
          </div>
        </Card>

        {/* Total Users */}
        <Card className="p-4 sm:p-5">
          <div className="flex items-center justify-between mb-3">
            <span className="text-sm text-zinc-400">Total Users</span>
            <Users size={18} className="text-zinc-600" />
          </div>
          {loading ? (
            <Skeleton className="h-8 w-16 mb-2" />
          ) : (
            <div className="text-2xl sm:text-3xl font-bold mb-2">
              {stats?.total_users ?? 0}
            </div>
          )}
          <ProgressBar
            value={stats?.total_users ?? 0}
            max={usersLimit}
            className="mb-2"
          />
          <div className="text-xs text-zinc-500">
            {usersLimit - (stats?.total_users ?? 0)} remaining
          </div>
        </Card>

        {/* Quick Access Domains */}
        <Card className="p-4 sm:p-5">
          <div className="flex items-center justify-between mb-3">
            <span className="text-sm text-zinc-400">Quick Access</span>
            <Globe size={18} className="text-zinc-600" />
          </div>
          {loading ? (
            <div className="space-y-2">
              <Skeleton className="h-10 w-full" />
              <Skeleton className="h-10 w-full" />
            </div>
          ) : verifiedDomains.length === 0 ? (
            <p className="text-sm text-zinc-500">No verified domains yet</p>
          ) : (
            <div className="space-y-2">
              {verifiedDomains.slice(0, 2).map((domain) => (
                <Link
                  key={domain.id}
                  href={`/domains/${domain.id}`}
                  className="flex items-center justify-between w-full p-2 bg-zinc-900 rounded-lg border border-zinc-800 hover:bg-zinc-800 transition-colors text-left"
                >
                  <div className="flex items-center gap-2 min-w-0">
                    <div className="min-w-0">
                      <div className="text-sm font-medium truncate">
                        {domain.domain}
                      </div>
                    </div>
                  </div>
                  <ChevronRight
                    size={14}
                    className="text-zinc-500 flex-shrink-0"
                  />
                </Link>
              ))}
            </div>
          )}
        </Card>
      </div>
    </div>
  );
}
