"use client";

import { useState, useEffect, useCallback, useRef } from "react";
import { useParams, useRouter, useSearchParams } from "next/navigation";
import Link from "next/link";
import {
  ExternalLink,
  Globe,
  Trash2,
  RefreshCw,
  AlertTriangle,
  Mail,
  Key,
  Users,
  Shield,
  Settings,
  MoreVertical,
  Plus,
  Check,
  ChevronRight,
  CreditCard,
  DollarSign,
  TrendingUp,
  Receipt,
  FileText,
  Download,
  ChevronLeft,
  Search,
  X,
  TestTube,
} from "lucide-react";
import {
  StripeConfigStatus,
  SubscriptionPlan,
  UserSubscription,
  BillingAnalytics,
  formatPrice,
  formatInterval,
  getStatusBadgeColor,
  getStatusLabel,
  getModeLabel,
  getModeBadgeColor,
  BillingPayment,
  PaymentSummary,
  DashboardPaymentListResponse,
  PaymentListFilters,
  getPaymentStatusLabel,
  getPaymentStatusBadgeColor,
  formatPaymentDate,
  formatPaymentDateTime,
  EnabledPaymentProvider,
  PaymentProvider,
  PaymentMode,
  getProviderLabel,
} from "@/types/billing";
import {
  Card,
  Button,
  Badge,
  Input,
  Toggle,
  Tabs,
  Modal,
  ConfirmModal,
  CopyButton,
  CodeBlock,
  EmptyState,
  HoldButton,
  SearchInput,
} from "@/components/ui";
import { useToast } from "@/contexts/ToastContext";
import { zIndex } from "@/lib/design-tokens";

// Types
type Domain = {
  id: string;
  domain: string;
  status: "pending_dns" | "verifying" | "verified" | "failed";
  dns_records?: {
    cname_name: string;
    cname_value: string;
    txt_name: string;
    txt_value: string;
  };
  verified_at: string | null;
  created_at: string | null;
};

type AuthConfig = {
  magic_link_enabled: boolean;
  google_oauth_enabled: boolean;
  redirect_url: string | null;
  whitelist_enabled: boolean;
  magic_link_config: { from_email: string; has_api_key: boolean } | null;
  using_fallback: boolean;
  fallback_from_email: string | null;
  google_oauth_config: {
    client_id_prefix: string;
    has_client_secret: boolean;
  } | null;
  using_google_fallback: boolean;
};

type EndUser = {
  id: string;
  email: string;
  roles: string[];
  email_verified_at: string | null;
  last_login_at: string | null;
  is_frozen: boolean;
  is_whitelisted: boolean;
  created_at: string | null;
};

type Role = {
  id: string;
  name: string;
  user_count: number;
  created_at: string | null;
};
type ApiKey = {
  id: string;
  key_prefix: string;
  name: string;
  last_used_at: string | null;
  created_at: string | null;
};

type Tab = "overview" | "auth" | "users" | "api" | "billing" | "settings";
const VALID_TABS: Tab[] = [
  "overview",
  "auth",
  "users",
  "api",
  "billing",
  "settings",
];

export default function DomainDetailPage() {
  const params = useParams();
  const router = useRouter();
  const searchParams = useSearchParams();
  const domainId = params.id as string;
  const { addToast } = useToast();

  const tabFromUrl = searchParams.get("tab") as Tab | null;
  const initialTab =
    tabFromUrl && VALID_TABS.includes(tabFromUrl) ? tabFromUrl : "overview";

  // Core state
  const [domain, setDomain] = useState<Domain | null>(null);
  const [authConfig, setAuthConfig] = useState<AuthConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [activeTab, setActiveTab] = useState<Tab>(initialTab);

  // DNS verification state
  const [cnameVerified, setCnameVerified] = useState(false);
  const [txtVerified, setTxtVerified] = useState(false);

  // Auth config form state
  const [magicLinkEnabled, setMagicLinkEnabled] = useState(false);
  const [resendApiKey, setResendApiKey] = useState("");
  const [fromEmail, setFromEmail] = useState("");
  const [redirectUrl, setRedirectUrl] = useState("");
  const [whitelistEnabled, setWhitelistEnabled] = useState(false);
  const [googleOAuthEnabled, setGoogleOAuthEnabled] = useState(false);
  const [googleClientId, setGoogleClientId] = useState("");
  const [googleClientSecret, setGoogleClientSecret] = useState("");

  // Users state
  const [endUsers, setEndUsers] = useState<EndUser[]>([]);
  const [loadingUsers, setLoadingUsers] = useState(false);
  const [userSearch, setUserSearch] = useState("");
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [deleteUserConfirmId, setDeleteUserConfirmId] = useState<string | null>(
    null,
  );
  const menuRef = useRef<HTMLDivElement>(null);

  // Invite modal state
  const [showInviteModal, setShowInviteModal] = useState(false);
  const [inviteEmail, setInviteEmail] = useState("");
  const [invitePreWhitelist, setInvitePreWhitelist] = useState(false);
  const [inviting, setInviting] = useState(false);

  // Roles state
  const [roles, setRoles] = useState<Role[]>([]);
  const [loadingRoles, setLoadingRoles] = useState(false);
  const [showRoleForm, setShowRoleForm] = useState(false);
  const [newRoleName, setNewRoleName] = useState("");
  const [creatingRole, setCreatingRole] = useState(false);
  const [deleteRoleConfirm, setDeleteRoleConfirm] = useState<Role | null>(null);

  // API Keys state
  const [apiKeys, setApiKeys] = useState<ApiKey[]>([]);
  const [loadingApiKeys, setLoadingApiKeys] = useState(false);
  const [showCreateKeyModal, setShowCreateKeyModal] = useState(false);
  const [newKeyName, setNewKeyName] = useState("");
  const [creatingKey, setCreatingKey] = useState(false);
  const [newlyCreatedKey, setNewlyCreatedKey] = useState<string | null>(null);

  // Delete domain modal
  const [showDeleteDomainModal, setShowDeleteDomainModal] = useState(false);

  // Whitelist modal
  const [showWhitelistModal, setShowWhitelistModal] = useState(false);

  // Billing state
  const [billingConfig, setBillingConfig] = useState<StripeConfigStatus | null>(
    null,
  );
  const [billingPlans, setBillingPlans] = useState<SubscriptionPlan[]>([]);
  const [billingAnalytics, setBillingAnalytics] =
    useState<BillingAnalytics | null>(null);
  const [loadingBilling, setLoadingBilling] = useState(false);
  const [editingMode, setEditingMode] = useState<PaymentMode>("test");
  const [stripeSecretKey, setStripeSecretKey] = useState("");
  const [stripePublishableKey, setStripePublishableKey] = useState("");
  const [stripeWebhookSecret, setStripeWebhookSecret] = useState("");
  const [savingBillingConfig, setSavingBillingConfig] = useState(false);
  const [switchingMode, setSwitchingMode] = useState(false);

  // Plan modal state
  const [showPlanModal, setShowPlanModal] = useState(false);
  const [editingPlan, setEditingPlan] = useState<SubscriptionPlan | null>(null);
  const [planCode, setPlanCode] = useState("");
  const [planName, setPlanName] = useState("");
  const [planDescription, setPlanDescription] = useState("");
  const [planPriceCents, setPlanPriceCents] = useState("");
  const [planInterval, setPlanInterval] = useState("monthly");
  const [planIntervalCount, setPlanIntervalCount] = useState("1");
  const [planTrialDays, setPlanTrialDays] = useState("0");
  const [planFeatures, setPlanFeatures] = useState<string[]>([]);
  const [planIsPublic, setPlanIsPublic] = useState(true);
  const [savingPlan, setSavingPlan] = useState(false);
  const [newFeature, setNewFeature] = useState("");

  // Payment history state
  const [payments, setPayments] = useState<BillingPayment[]>([]);
  const [paymentsSummary, setPaymentsSummary] = useState<PaymentSummary | null>(
    null,
  );
  const [paymentsPagination, setPaymentsPagination] = useState({
    page: 1,
    total: 0,
    total_pages: 0,
    per_page: 10,
  });
  const [loadingPayments, setLoadingPayments] = useState(false);
  const [paymentFilters, setPaymentFilters] = useState<PaymentListFilters>({});
  const [paymentEmailSearch, setPaymentEmailSearch] = useState("");
  const [exportingPayments, setExportingPayments] = useState(false);

  // Payment providers state
  const [enabledProviders, setEnabledProviders] = useState<
    EnabledPaymentProvider[]
  >([]);
  const [enablingProvider, setEnablingProvider] = useState<string | null>(null);

  // Config modal state
  const [authConfigModal, setAuthConfigModal] = useState<
    "magic_link" | "google_oauth" | null
  >(null);
  const [stripeConfigModal, setStripeConfigModal] = useState<
    "test" | "live" | null
  >(null);

  // Tab change handler
  const handleTabChange = useCallback(
    (tab: Tab) => {
      setActiveTab(tab);
      const newParams = new URLSearchParams(searchParams.toString());
      newParams.set("tab", tab);
      router.replace(`/domains/${domainId}?${newParams.toString()}`, {
        scroll: false,
      });
    },
    [domainId, router, searchParams],
  );

  // Fetch functions
  const fetchData = useCallback(async () => {
    try {
      const domainRes = await fetch(`/api/domains/${domainId}`, {
        credentials: "include",
      });
      if (domainRes.ok) {
        const domainData = await domainRes.json();
        setDomain(domainData);
        // Fetch auth config for all domains
        const configRes = await fetch(`/api/domains/${domainId}/auth-config`, {
          credentials: "include",
        });
        if (configRes.ok) {
          const configData = await configRes.json();
          setAuthConfig(configData);
          setMagicLinkEnabled(configData.magic_link_enabled);
          setGoogleOAuthEnabled(configData.google_oauth_enabled);
          setRedirectUrl(configData.redirect_url || "");
          setWhitelistEnabled(configData.whitelist_enabled);
          if (configData.magic_link_config) {
            setFromEmail(configData.magic_link_config.from_email);
          }
        }
      }
    } catch {
      addToast("Failed to load domain", "error");
    } finally {
      setLoading(false);
    }
  }, [domainId, addToast]);

  const fetchEndUsers = useCallback(async () => {
    if (!domain) return;
    setLoadingUsers(true);
    try {
      const res = await fetch(`/api/domains/${domainId}/end-users`, {
        credentials: "include",
      });
      if (res.ok) setEndUsers(await res.json());
    } catch {
      /* ignore */
    } finally {
      setLoadingUsers(false);
    }
  }, [domainId, domain]);

  const fetchRoles = useCallback(async () => {
    if (!domain) return;
    setLoadingRoles(true);
    try {
      const res = await fetch(`/api/domains/${domainId}/roles`, {
        credentials: "include",
      });
      if (res.ok) setRoles(await res.json());
    } catch {
      /* ignore */
    } finally {
      setLoadingRoles(false);
    }
  }, [domainId, domain]);

  const fetchApiKeys = useCallback(async () => {
    if (!domain) return;
    setLoadingApiKeys(true);
    try {
      const res = await fetch(`/api/domains/${domainId}/api-keys`, {
        credentials: "include",
      });
      if (res.ok) setApiKeys(await res.json());
    } catch {
      /* ignore */
    } finally {
      setLoadingApiKeys(false);
    }
  }, [domainId, domain]);

  const fetchBillingData = useCallback(async () => {
    if (!domain) return;
    setLoadingBilling(true);
    try {
      const [configRes, plansRes, analyticsRes] = await Promise.all([
        fetch(`/api/domains/${domainId}/billing/config`, {
          credentials: "include",
        }),
        fetch(`/api/domains/${domainId}/billing/plans`, {
          credentials: "include",
        }),
        fetch(`/api/domains/${domainId}/billing/analytics`, {
          credentials: "include",
        }),
      ]);
      if (configRes.ok) setBillingConfig(await configRes.json());
      if (plansRes.ok) setBillingPlans(await plansRes.json());
      if (analyticsRes.ok) setBillingAnalytics(await analyticsRes.json());
    } catch {
      /* ignore */
    } finally {
      setLoadingBilling(false);
    }
  }, [domainId, domain]);

  const fetchPayments = useCallback(
    async (page: number = 1, filters: PaymentListFilters = {}) => {
      if (!domain) return;
      setLoadingPayments(true);
      try {
        const params = new URLSearchParams({
          page: page.toString(),
          per_page: "10",
        });
        if (filters.status) params.set("status", filters.status);
        if (filters.date_from)
          params.set("date_from", filters.date_from.toString());
        if (filters.date_to) params.set("date_to", filters.date_to.toString());
        if (filters.plan_code) params.set("plan_code", filters.plan_code);
        if (filters.user_email) params.set("user_email", filters.user_email);

        const res = await fetch(
          `/api/domains/${domainId}/billing/payments?${params}`,
          { credentials: "include" },
        );
        if (res.ok) {
          const data: DashboardPaymentListResponse = await res.json();
          setPayments(data.payments);
          setPaymentsSummary(data.summary);
          setPaymentsPagination({
            page: data.page,
            total: data.total,
            total_pages: data.total_pages,
            per_page: data.per_page,
          });
        }
      } catch {
        /* ignore */
      } finally {
        setLoadingPayments(false);
      }
    },
    [domainId, domain],
  );

  const handleExportPayments = async () => {
    if (!domain) return;
    setExportingPayments(true);
    try {
      const params = new URLSearchParams();
      if (paymentFilters.status) params.set("status", paymentFilters.status);
      if (paymentFilters.date_from)
        params.set("date_from", paymentFilters.date_from.toString());
      if (paymentFilters.date_to)
        params.set("date_to", paymentFilters.date_to.toString());
      if (paymentFilters.plan_code)
        params.set("plan_code", paymentFilters.plan_code);
      if (paymentFilters.user_email)
        params.set("user_email", paymentFilters.user_email);

      const res = await fetch(
        `/api/domains/${domainId}/billing/payments/export?${params}`,
        { credentials: "include" },
      );
      if (res.ok) {
        const blob = await res.blob();
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = `payments-${domain.domain}-${new Date().toISOString().split("T")[0]}.csv`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
      } else {
        addToast("Failed to export payments", "error");
      }
    } catch {
      addToast("Failed to export payments", "error");
    } finally {
      setExportingPayments(false);
    }
  };

  const handlePaymentSearch = () => {
    const newFilters = {
      ...paymentFilters,
      user_email: paymentEmailSearch || undefined,
    };
    setPaymentFilters(newFilters);
    fetchPayments(1, newFilters);
  };

  const handlePaymentStatusFilter = (status: string | undefined) => {
    const newFilters = {
      ...paymentFilters,
      status: status as PaymentListFilters["status"],
    };
    setPaymentFilters(newFilters);
    fetchPayments(1, newFilters);
  };

  const clearPaymentFilters = () => {
    setPaymentFilters({});
    setPaymentEmailSearch("");
    fetchPayments(1, {});
  };

  const fetchEnabledProviders = useCallback(async () => {
    if (!domain) return;
    try {
      const res = await fetch(`/api/domains/${domainId}/billing/providers`, {
        credentials: "include",
      });
      if (res.ok) {
        const data = await res.json();
        setEnabledProviders(data);
      }
    } catch {
      /* ignore */
    }
  }, [domainId, domain]);

  const handleEnableProvider = async (
    provider: PaymentProvider,
    mode: PaymentMode,
  ) => {
    setEnablingProvider(`${provider}_${mode}`);
    try {
      const res = await fetch(`/api/domains/${domainId}/billing/providers`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ provider, mode }),
        credentials: "include",
      });
      if (res.ok) {
        addToast(`${getProviderLabel(provider)} enabled`, "success");
        fetchEnabledProviders();
      } else {
        const err = await res.json().catch(() => ({}));
        addToast(err.message || "Failed to enable provider", "error");
      }
    } catch {
      addToast("Network error", "error");
    } finally {
      setEnablingProvider(null);
    }
  };

  const handleDisableProvider = async (
    provider: PaymentProvider,
    mode: PaymentMode,
  ) => {
    try {
      const res = await fetch(
        `/api/domains/${domainId}/billing/providers/${provider}/${mode}`,
        {
          method: "DELETE",
          credentials: "include",
        },
      );
      if (res.ok) {
        addToast("Provider disabled", "success");
        fetchEnabledProviders();
      } else {
        addToast("Failed to disable provider", "error");
      }
    } catch {
      addToast("Network error", "error");
    }
  };

  useEffect(() => {
    fetchData();
  }, [fetchData]);
  useEffect(() => {
    if ((activeTab === "users" || activeTab === "overview") && domain) {
      fetchEndUsers();
      fetchRoles();
    }
  }, [activeTab, domain, fetchEndUsers, fetchRoles]);
  useEffect(() => {
    if ((activeTab === "api" || activeTab === "overview") && domain)
      fetchApiKeys();
  }, [activeTab, domain, fetchApiKeys]);
  useEffect(() => {
    if (activeTab === "billing" && domain) {
      fetchBillingData();
      fetchPayments(1, paymentFilters);
      fetchEnabledProviders();
    }
  }, [
    activeTab,
    domain,
    fetchBillingData,
    fetchPayments,
    paymentFilters,
    fetchEnabledProviders,
  ]);

  // Poll for verification status
  useEffect(() => {
    if (
      !domain ||
      (domain.status !== "verifying" && domain.status !== "pending_dns")
    )
      return;
    const checkStatus = async () => {
      try {
        const res = await fetch(`/api/domains/${domainId}/status`, {
          credentials: "include",
        });
        if (res.ok) {
          const data = await res.json();
          setCnameVerified(data.cname_verified);
          setTxtVerified(data.txt_verified);
          if (data.status !== domain.status) fetchData();
        }
      } catch {
        /* continue */
      }
    };
    checkStatus();
    const interval = setInterval(checkStatus, 5000);
    return () => clearInterval(interval);
  }, [domain, domainId, fetchData]);

  // Close menu on click outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node))
        setOpenMenuId(null);
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  // Handlers
  const handleStartVerification = async () => {
    try {
      const res = await fetch(`/api/domains/${domainId}/verify`, {
        method: "POST",
        credentials: "include",
      });
      if (res.ok) fetchData();
      else addToast("Failed to start verification", "error");
    } catch {
      addToast("Network error", "error");
    }
  };

  const handleSaveConfig = async (
    e: React.FormEvent,
    whitelistAllExisting = false,
  ) => {
    e.preventDefault();
    setSaving(true);
    try {
      const payload: Record<string, unknown> = {
        magic_link_enabled: magicLinkEnabled,
        google_oauth_enabled: googleOAuthEnabled,
        redirect_url: redirectUrl || null,
        whitelist_enabled: whitelistEnabled,
        whitelist_all_existing: whitelistAllExisting,
      };
      if (magicLinkEnabled) {
        if (resendApiKey) payload.resend_api_key = resendApiKey;
        if (fromEmail) payload.from_email = fromEmail;
      }
      if (googleOAuthEnabled && googleClientId && googleClientSecret) {
        payload.google_client_id = googleClientId;
        payload.google_client_secret = googleClientSecret;
      }
      const res = await fetch(`/api/domains/${domainId}/auth-config`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
        credentials: "include",
      });
      if (res.ok) {
        addToast("Configuration saved successfully", "success");
        setResendApiKey("");
        setGoogleClientId("");
        setGoogleClientSecret("");
        fetchData();
      } else {
        const err = await res.json().catch(() => ({}));
        addToast(err.message || "Failed to save configuration", "error");
      }
    } catch {
      addToast("Network error", "error");
    } finally {
      setSaving(false);
    }
  };

  const handleRemoveCustomConfig = async () => {
    try {
      const res = await fetch(
        `/api/domains/${domainId}/auth-config/magic-link`,
        { method: "DELETE", credentials: "include" },
      );
      if (res.ok) {
        addToast("Custom email configuration removed", "success");
        fetchData();
      } else addToast("Failed to remove configuration", "error");
    } catch {
      addToast("Network error", "error");
    }
  };

  const handleRemoveGoogleOAuthConfig = async () => {
    try {
      const res = await fetch(
        `/api/domains/${domainId}/auth-config/google-oauth`,
        { method: "DELETE", credentials: "include" },
      );
      if (res.ok) {
        addToast("Google OAuth configuration removed", "success");
        fetchData();
      } else addToast("Failed to remove configuration", "error");
    } catch {
      addToast("Network error", "error");
    }
  };

  const handleWhitelistToggle = (enabled: boolean) => {
    if (enabled && !authConfig?.whitelist_enabled) setShowWhitelistModal(true);
    else setWhitelistEnabled(enabled);
  };

  const handleWhitelistConfirm = async (whitelistAllExisting: boolean) => {
    setShowWhitelistModal(false);
    setWhitelistEnabled(true);
    if (whitelistAllExisting) {
      setSaving(true);
      try {
        const res = await fetch(`/api/domains/${domainId}/auth-config`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            magic_link_enabled: magicLinkEnabled,
            google_oauth_enabled: googleOAuthEnabled,
            redirect_url: redirectUrl || null,
            whitelist_enabled: true,
            whitelist_all_existing: true,
          }),
          credentials: "include",
        });
        if (res.ok) {
          addToast(
            "Whitelist enabled and all existing users whitelisted",
            "success",
          );
          fetchData();
        } else addToast("Failed to enable whitelist", "error");
      } catch {
        addToast("Network error", "error");
      } finally {
        setSaving(false);
      }
    }
  };

  const handleDeleteDomain = async () => {
    try {
      const res = await fetch(`/api/domains/${domainId}`, {
        method: "DELETE",
        credentials: "include",
      });
      if (res.ok) router.push("/domains");
      else addToast("Failed to delete domain", "error");
    } catch {
      addToast("Network error", "error");
    }
  };

  const handleUserAction = async (
    userId: string,
    action: "freeze" | "unfreeze" | "whitelist" | "unwhitelist" | "delete",
  ) => {
    if (action === "delete") setDeleteUserConfirmId(null);
    const methods = {
      freeze: "POST",
      unfreeze: "DELETE",
      whitelist: "POST",
      unwhitelist: "DELETE",
      delete: "DELETE",
    };
    const urls = {
      freeze: `/api/domains/${domainId}/end-users/${userId}/freeze`,
      unfreeze: `/api/domains/${domainId}/end-users/${userId}/freeze`,
      whitelist: `/api/domains/${domainId}/end-users/${userId}/whitelist`,
      unwhitelist: `/api/domains/${domainId}/end-users/${userId}/whitelist`,
      delete: `/api/domains/${domainId}/end-users/${userId}`,
    };
    try {
      const res = await fetch(urls[action], {
        method: methods[action],
        credentials: "include",
      });
      if (res.ok) {
        setOpenMenuId(null);
        fetchEndUsers();
      } else addToast(`Failed to ${action} user`, "error");
    } catch {
      addToast("Network error", "error");
    }
  };

  const handleInviteUser = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!inviteEmail.trim()) return;
    setInviting(true);
    try {
      const res = await fetch(`/api/domains/${domainId}/end-users/invite`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          email: inviteEmail.trim(),
          pre_whitelist: invitePreWhitelist,
        }),
        credentials: "include",
      });
      if (res.ok) {
        setShowInviteModal(false);
        setInviteEmail("");
        setInvitePreWhitelist(false);
        addToast("Invitation sent", "success");
        fetchEndUsers();
      } else {
        const err = await res.json().catch(() => ({}));
        addToast(err.message || "Failed to invite user", "error");
      }
    } catch {
      addToast("Network error", "error");
    } finally {
      setInviting(false);
    }
  };

  const handleCreateRole = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newRoleName.trim()) return;
    setCreatingRole(true);
    try {
      const res = await fetch(`/api/domains/${domainId}/roles`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name: newRoleName.trim().toLowerCase() }),
        credentials: "include",
      });
      if (res.ok) {
        setNewRoleName("");
        setShowRoleForm(false);
        fetchRoles();
        addToast("Role created", "success");
      } else {
        const err = await res.json().catch(() => ({}));
        addToast(err.message || "Failed to create role", "error");
      }
    } catch {
      addToast("Network error", "error");
    } finally {
      setCreatingRole(false);
    }
  };

  const handleDeleteRole = async () => {
    if (!deleteRoleConfirm) return;
    const roleName = deleteRoleConfirm.name;
    try {
      const res = await fetch(
        `/api/domains/${domainId}/roles/${encodeURIComponent(roleName)}`,
        { method: "DELETE", credentials: "include" },
      );
      if (res.ok) {
        addToast(`Role "${roleName}" deleted`, "success");
        fetchRoles();
        fetchEndUsers();
      } else addToast("Failed to delete role", "error");
    } catch {
      addToast("Network error", "error");
    } finally {
      setDeleteRoleConfirm(null);
    }
  };

  const handleCreateApiKey = async (e: React.FormEvent) => {
    e.preventDefault();
    setCreatingKey(true);
    try {
      const res = await fetch(`/api/domains/${domainId}/api-keys`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name: newKeyName.trim() || "Default" }),
        credentials: "include",
      });
      if (res.ok) {
        const data = await res.json();
        setNewlyCreatedKey(data.key);
        setNewKeyName("");
        fetchApiKeys();
      } else {
        const err = await res.json().catch(() => ({}));
        addToast(err.message || "Failed to create API key", "error");
        setShowCreateKeyModal(false);
      }
    } catch {
      addToast("Network error", "error");
    } finally {
      setCreatingKey(false);
    }
  };

  const handleRevokeApiKey = async (keyId: string) => {
    try {
      const res = await fetch(`/api/domains/${domainId}/api-keys/${keyId}`, {
        method: "DELETE",
        credentials: "include",
      });
      if (res.ok) {
        addToast("API key revoked", "success");
        fetchApiKeys();
      } else addToast("Failed to revoke API key", "error");
    } catch {
      addToast("Network error", "error");
    }
  };

  // Billing handlers
  const handleSaveBillingConfig = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!stripeSecretKey || !stripePublishableKey || !stripeWebhookSecret) {
      addToast("All fields are required", "error");
      return;
    }
    setSavingBillingConfig(true);
    try {
      const res = await fetch(`/api/domains/${domainId}/billing/config`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          mode: editingMode,
          secret_key: stripeSecretKey,
          publishable_key: stripePublishableKey,
          webhook_secret: stripeWebhookSecret,
        }),
        credentials: "include",
      });
      if (res.ok) {
        addToast(`Stripe ${editingMode} mode configuration saved`, "success");
        setStripeSecretKey("");
        setStripePublishableKey("");
        setStripeWebhookSecret("");
        fetchBillingData();
      } else {
        const err = await res.json().catch(() => ({}));
        addToast(err.message || "Failed to save configuration", "error");
      }
    } catch {
      addToast("Network error", "error");
    } finally {
      setSavingBillingConfig(false);
    }
  };

  const handleRemoveBillingConfig = async (mode: PaymentMode) => {
    try {
      const res = await fetch(`/api/domains/${domainId}/billing/config`, {
        method: "DELETE",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ mode }),
        credentials: "include",
      });
      if (res.ok) {
        addToast(`Stripe ${mode} mode configuration removed`, "success");
        fetchBillingData();
      } else {
        const err = await res.json().catch(() => ({}));
        addToast(err.message || "Failed to remove configuration", "error");
      }
    } catch {
      addToast("Network error", "error");
    }
  };

  const handleSwitchBillingMode = async (mode: PaymentMode) => {
    setSwitchingMode(true);
    try {
      const res = await fetch(`/api/domains/${domainId}/billing/mode`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ mode }),
        credentials: "include",
      });
      if (res.ok) {
        addToast(`Switched to ${mode} mode`, "success");
        fetchBillingData();
      } else {
        const err = await res.json().catch(() => ({}));
        addToast(err.message || "Failed to switch mode", "error");
      }
    } catch {
      addToast("Network error", "error");
    } finally {
      setSwitchingMode(false);
    }
  };

  const openPlanModal = (plan?: SubscriptionPlan) => {
    if (plan) {
      setEditingPlan(plan);
      setPlanCode(plan.code);
      setPlanName(plan.name);
      setPlanDescription(plan.description || "");
      setPlanPriceCents(String(plan.price_cents));
      setPlanInterval(plan.interval);
      setPlanIntervalCount(String(plan.interval_count));
      setPlanTrialDays(String(plan.trial_days));
      setPlanFeatures([...plan.features]);
      setPlanIsPublic(plan.is_public);
    } else {
      setEditingPlan(null);
      setPlanCode("");
      setPlanName("");
      setPlanDescription("");
      setPlanPriceCents("");
      setPlanInterval("monthly");
      setPlanIntervalCount("1");
      setPlanTrialDays("0");
      setPlanFeatures([]);
      setPlanIsPublic(true);
    }
    setShowPlanModal(true);
  };

  const handleSavePlan = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!planCode || !planName || !planPriceCents) {
      addToast("Code, name, and price are required", "error");
      return;
    }
    setSavingPlan(true);
    try {
      const payload = {
        code: planCode,
        name: planName,
        description: planDescription || null,
        price_cents: parseInt(planPriceCents, 10),
        currency: "USD",
        interval: planInterval,
        interval_count: parseInt(planIntervalCount, 10),
        trial_days: parseInt(planTrialDays, 10),
        features: planFeatures,
        is_public: planIsPublic,
      };
      const url = editingPlan
        ? `/api/domains/${domainId}/billing/plans/${editingPlan.id}`
        : `/api/domains/${domainId}/billing/plans`;
      const method = editingPlan ? "PATCH" : "POST";
      const res = await fetch(url, {
        method,
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
        credentials: "include",
      });
      if (res.ok) {
        addToast(editingPlan ? "Plan updated" : "Plan created", "success");
        setShowPlanModal(false);
        fetchBillingData();
      } else {
        const err = await res.json().catch(() => ({}));
        addToast(err.message || "Failed to save plan", "error");
      }
    } catch {
      addToast("Network error", "error");
    } finally {
      setSavingPlan(false);
    }
  };

  const handleArchivePlan = async (planId: string) => {
    try {
      const res = await fetch(
        `/api/domains/${domainId}/billing/plans/${planId}`,
        {
          method: "DELETE",
          credentials: "include",
        },
      );
      if (res.ok) {
        addToast("Plan archived", "success");
        fetchBillingData();
      } else {
        addToast("Failed to archive plan", "error");
      }
    } catch {
      addToast("Network error", "error");
    }
  };

  const addFeature = () => {
    if (newFeature.trim()) {
      setPlanFeatures([...planFeatures, newFeature.trim()]);
      setNewFeature("");
    }
  };

  const removeFeature = (index: number) => {
    setPlanFeatures(planFeatures.filter((_, i) => i !== index));
  };

  // Helpers
  const formatDate = (dateString: string | null) => {
    if (!dateString) return "";
    return new Date(dateString).toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
      year: "numeric",
    });
  };

  const getStatusBadge = (status: Domain["status"]) => {
    const variants: Record<
      Domain["status"],
      "default" | "success" | "error" | "warning" | "info"
    > = {
      pending_dns: "warning",
      verifying: "info",
      verified: "success",
      failed: "error",
    };
    const labels: Record<Domain["status"], string> = {
      pending_dns: "Pending DNS",
      verifying: "Verifying...",
      verified: "Verified",
      failed: "Failed",
    };
    return <Badge variant={variants[status]}>{labels[status]}</Badge>;
  };

  // Loading state
  if (loading) {
    return (
      <div className="flex justify-center py-20">
        <div className="w-6 h-6 border-2 border-zinc-600 border-t-blue-500 rounded-full animate-spin" />
      </div>
    );
  }

  if (!domain) {
    return (
      <Card className="p-8 text-center">
        <p className="text-zinc-400 mb-4">Domain not found</p>
        <Button onClick={() => router.push("/domains")}>Back to domains</Button>
      </Card>
    );
  }

  // Provider toggle row component
  // Unified provider row component for auth and payment providers
  const ProviderRow = ({
    icon: Icon,
    label,
    description,
    enabled,
    configured,
    onToggle,
    onSettings,
    loading,
    showSettings = true,
  }: {
    icon: React.ElementType;
    label: string;
    description: string;
    enabled: boolean;
    configured?: boolean;
    onToggle: () => void;
    onSettings?: () => void;
    loading?: boolean;
    showSettings?: boolean;
  }) => (
    <div className="flex items-center justify-between p-4 bg-zinc-800/50 rounded-lg border border-zinc-700">
      <div className="flex items-center gap-3">
        <Icon
          size={20}
          className={enabled ? "text-green-400" : "text-zinc-500"}
        />
        <div>
          <div className="flex items-center gap-2">
            <span className="font-medium text-white">{label}</span>
            {enabled && <Badge variant="success">Enabled</Badge>}
            {enabled && configured === false && (
              <Badge variant="warning">Not configured</Badge>
            )}
          </div>
          <p className="text-sm text-zinc-400">{description}</p>
        </div>
      </div>
      <div className="flex items-center gap-2">
        {showSettings && onSettings && (
          <Button variant="ghost" size="sm" onClick={onSettings}>
            <Settings size={16} />
          </Button>
        )}
        {enabled ? (
          <HoldButton onComplete={onToggle} variant="danger" duration={2000}>
            Disable
          </HoldButton>
        ) : (
          <Button variant="primary" onClick={onToggle} disabled={loading}>
            {loading ? "Enabling..." : "Enable"}
          </Button>
        )}
      </div>
    </div>
  );

  const tabs = [
    { id: "overview" as Tab, label: "Overview" },
    { id: "auth" as Tab, label: "Auth" },
    { id: "users" as Tab, label: "Users" },
    { id: "api" as Tab, label: "API" },
    { id: "billing" as Tab, label: "Billing" },
    { id: "settings" as Tab, label: "Settings" },
  ];

  return (
    <div className="flex flex-col h-full">
      {/* Sticky Header */}
      <div className="sticky top-0 z-10 bg-zinc-950 border-b border-zinc-800 px-4 sm:px-6 py-3 sm:py-4">
        <div>
          <div className="flex items-center gap-2 text-sm text-zinc-400 mb-2 sm:mb-3">
            <Link
              href="/domains"
              className="hover:text-white transition-colors"
            >
              Domains
            </Link>
            <ChevronRight size={14} />
            <span className="text-white truncate">{domain.domain}</span>
          </div>

          <div className="flex items-center justify-between gap-2">
            <div className="flex items-center gap-2 sm:gap-3 min-w-0">
              <h1 className="text-lg sm:text-2xl font-bold truncate">
                {domain.domain}
              </h1>
              {domain.status === "verified" ? (
                <Badge variant="success">verified</Badge>
              ) : (
                <Badge variant="error">pending</Badge>
              )}
            </div>
            {domain.status === "verified" && (
              <a
                href={`https://reauth.${domain.domain}`}
                target="_blank"
                rel="noopener noreferrer"
                className="hidden sm:flex items-center gap-1 text-sm text-blue-400 hover:text-blue-300 transition-colors flex-shrink-0"
              >
                <ExternalLink size={14} /> Open login page
              </a>
            )}
          </div>

          <div className="mt-3 sm:mt-4 -mx-4 sm:mx-0 px-4 sm:px-0 overflow-x-auto">
            <Tabs
              tabs={tabs.map((t) => ({ id: t.id, label: t.label }))}
              activeTab={activeTab}
              onChange={(id) => handleTabChange(id as Tab)}
            />
          </div>
        </div>
      </div>

      {/* Tab Content */}
      <div className="flex-1 overflow-auto p-4 sm:p-6">
        <div className="space-y-6">
          {/* Overview Tab */}
          {activeTab === "overview" && (
            <div className="space-y-6">
              {/* Unverified Warning */}
              {domain.status !== "verified" && (
                <Card className="p-4 border-amber-600/30 bg-amber-900/10">
                  <div className="flex items-start gap-3">
                    <AlertTriangle
                      className="text-amber-400 mt-0.5"
                      size={20}
                    />
                    <div>
                      <h3 className="font-semibold text-amber-400">
                        DNS verification required
                      </h3>
                      <p className="text-sm text-zinc-400 mt-1">
                        Your domain is not yet verified. Please add the DNS
                        records below to complete setup.
                      </p>
                    </div>
                  </div>
                </Card>
              )}

              {/* Verifying banner */}
              {domain.status === "verifying" && (
                <Card className="p-4">
                  <div className="flex items-center gap-4">
                    <div className="w-6 h-6 border-2 border-zinc-600 border-t-blue-500 rounded-full animate-spin" />
                    <div>
                      <p className="font-medium text-white">
                        Looking for DNS records...
                      </p>
                      <p className="text-sm text-zinc-400">
                        May take a few minutes depending on your DNS provider.
                      </p>
                    </div>
                  </div>
                </Card>
              )}

              {/* Quick Stats - only show when verified */}
              {domain.status === "verified" && (
                <div className="grid grid-cols-2 gap-4">
                  <Card className="p-4 text-center">
                    <div className="text-2xl font-bold">{roles.length}</div>
                    <div className="text-sm text-zinc-400">Roles</div>
                  </Card>
                  <Card className="p-4 text-center">
                    <div className="text-2xl font-bold">{apiKeys.length}</div>
                    <div className="text-sm text-zinc-400">API Keys</div>
                  </Card>
                </div>
              )}

              {/* Login URL */}
              {domain.status === "verified" && (
                <Card className="p-5">
                  <h3 className="font-semibold mb-3">Login URL</h3>
                  <CodeBlock value={`https://reauth.${domain.domain}`} />
                </Card>
              )}

              {/* DNS Records */}
              {domain.dns_records && (
                <Card className="p-5">
                  <div className="flex items-start justify-between mb-4">
                    <h3 className="font-semibold">DNS Records</h3>
                    <a
                      href="https://resend.com/docs/knowledge-base/godaddy"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="flex items-center gap-1 text-sm text-blue-400 hover:text-blue-300"
                    >
                      How to add records
                      <ExternalLink size={14} />
                    </a>
                  </div>

                  <div className="space-y-4">
                    {/* CNAME Record */}
                    <div className="bg-zinc-800/50 rounded-lg p-4 border border-zinc-700">
                      <div className="flex items-center justify-between mb-3">
                        <h4 className="font-medium text-white text-sm">
                          CNAME Record
                        </h4>
                        {domain.status === "verified" || cnameVerified ? (
                          <Badge variant="success">Verified</Badge>
                        ) : domain.status === "verifying" ? (
                          <Badge variant="warning">Verifying</Badge>
                        ) : null}
                      </div>
                      <div className="grid grid-cols-[80px_1fr_auto] gap-3 items-center text-sm">
                        <span className="text-zinc-500">Name</span>
                        <code className="bg-zinc-900 px-3 py-1.5 rounded border border-zinc-800 text-zinc-300 font-mono text-xs">
                          {domain.dns_records.cname_name}
                        </code>
                        <CopyButton text={domain.dns_records.cname_name} />
                        <span className="text-zinc-500">Value</span>
                        <code className="bg-zinc-900 px-3 py-1.5 rounded border border-zinc-800 text-zinc-300 font-mono text-xs">
                          {domain.dns_records.cname_value}
                        </code>
                        <CopyButton text={domain.dns_records.cname_value} />
                      </div>
                    </div>

                    {/* TXT Record */}
                    <div className="bg-zinc-800/50 rounded-lg p-4 border border-zinc-700">
                      <div className="flex items-center justify-between mb-3">
                        <h4 className="font-medium text-white text-sm">
                          TXT Record
                        </h4>
                        {domain.status === "verified" || txtVerified ? (
                          <Badge variant="success">Verified</Badge>
                        ) : domain.status === "verifying" ? (
                          <Badge variant="warning">Verifying</Badge>
                        ) : null}
                      </div>
                      <div className="grid grid-cols-[80px_1fr_auto] gap-3 items-center text-sm">
                        <span className="text-zinc-500">Name</span>
                        <code className="bg-zinc-900 px-3 py-1.5 rounded border border-zinc-800 text-zinc-300 font-mono text-xs">
                          {domain.dns_records.txt_name}
                        </code>
                        <CopyButton text={domain.dns_records.txt_name} />
                        <span className="text-zinc-500">Value</span>
                        <code className="bg-zinc-900 px-3 py-1.5 rounded border border-zinc-800 text-zinc-300 font-mono text-xs truncate">
                          {domain.dns_records.txt_value}
                        </code>
                        <CopyButton text={domain.dns_records.txt_value} />
                      </div>
                    </div>
                  </div>

                  {domain.status === "pending_dns" && (
                    <Button
                      variant="primary"
                      className="mt-6"
                      onClick={handleStartVerification}
                    >
                      I&apos;ve added the records
                    </Button>
                  )}

                  {domain.status === "failed" && (
                    <div className="mt-6">
                      <div className="flex items-center gap-2 p-3 bg-red-500/10 border border-red-500/20 rounded-lg text-sm text-red-400 mb-4">
                        <AlertTriangle size={16} />
                        DNS verification failed. Please check your DNS records
                        and try again.
                      </div>
                      <Button
                        variant="primary"
                        onClick={handleStartVerification}
                      >
                        <RefreshCw size={16} className="mr-1" />
                        Retry verification
                      </Button>
                    </div>
                  )}
                </Card>
              )}
            </div>
          )}

          {/* Auth Tab */}
          {activeTab === "auth" && (
            <div className="space-y-4">
              {/* Login URL */}
              <Card className="p-6">
                <h2 className="text-lg font-semibold text-white mb-2">
                  Login URL
                </h2>
                <p className="text-sm text-zinc-400">
                  Your users can sign in at{" "}
                  <a
                    href={`https://reauth.${domain.domain}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-blue-400 hover:underline"
                  >
                    https://reauth.{domain.domain}
                  </a>
                </p>
              </Card>

              {/* Authentication Methods */}
              <Card className="p-6">
                <div className="flex items-center justify-between mb-4">
                  <div>
                    <h2 className="text-lg font-semibold text-white flex items-center gap-2">
                      <Shield size={20} className="text-blue-400" />
                      Authentication Methods
                    </h2>
                    <p className="text-sm text-zinc-400 mt-1">
                      Configure how users sign in.
                    </p>
                  </div>
                </div>

                <div className="space-y-3">
                  <ProviderRow
                    icon={Mail}
                    label="Magic Link"
                    description="Sign in via email link"
                    enabled={magicLinkEnabled}
                    configured={
                      authConfig?.magic_link_config?.has_api_key ||
                      authConfig?.using_fallback
                    }
                    onToggle={() => {
                      const newValue = !magicLinkEnabled;
                      setMagicLinkEnabled(newValue);
                      // Auto-save toggle
                      handleSaveConfig({
                        preventDefault: () => {},
                      } as React.FormEvent);
                    }}
                    onSettings={() => setAuthConfigModal("magic_link")}
                  />
                  <ProviderRow
                    icon={Globe}
                    label="Google OAuth"
                    description="Sign in with Google account"
                    enabled={googleOAuthEnabled}
                    configured={
                      authConfig?.google_oauth_config?.has_client_secret ||
                      authConfig?.using_google_fallback
                    }
                    onToggle={() => {
                      const newValue = !googleOAuthEnabled;
                      setGoogleOAuthEnabled(newValue);
                      // Auto-save toggle
                      handleSaveConfig({
                        preventDefault: () => {},
                      } as React.FormEvent);
                    }}
                    onSettings={() => setAuthConfigModal("google_oauth")}
                  />
                </div>
              </Card>

              {/* Redirect URL */}
              <Card className="p-6">
                <div className="space-y-2">
                  <label className="text-sm font-medium text-zinc-300">
                    Redirect URL
                  </label>
                  <div className="flex gap-2">
                    <Input
                      value={redirectUrl}
                      onChange={(e) => setRedirectUrl(e.target.value)}
                      placeholder="https://yourapp.com/callback"
                      className="flex-1"
                    />
                    <Button
                      variant="primary"
                      onClick={(e) =>
                        handleSaveConfig(e as unknown as React.FormEvent)
                      }
                      disabled={saving}
                    >
                      {saving ? "Saving..." : "Save"}
                    </Button>
                  </div>
                  <p className="text-xs text-zinc-500">
                    Where to redirect users after successful authentication.
                  </p>
                </div>
              </Card>
            </div>
          )}

          {/* Users Tab */}
          {activeTab === "users" && (
            <div className="space-y-6">
              {/* Roles Section */}
              <Card className="p-5">
                <div className="flex items-center justify-between mb-4">
                  <h3 className="font-semibold">Roles</h3>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setShowRoleForm(true)}
                  >
                    <Plus size={14} /> Add role
                  </Button>
                </div>

                {showRoleForm && (
                  <form onSubmit={handleCreateRole} className="flex gap-2 mb-4">
                    <Input
                      value={newRoleName}
                      onChange={(e) =>
                        setNewRoleName(
                          e.target.value
                            .toLowerCase()
                            .replace(/[^a-z0-9-]/g, ""),
                        )
                      }
                      placeholder="Role name"
                      autoFocus
                    />
                    <Button
                      type="submit"
                      variant="primary"
                      disabled={creatingRole || !newRoleName.trim()}
                    >
                      Add
                    </Button>
                    <Button
                      type="button"
                      variant="ghost"
                      onClick={() => {
                        setShowRoleForm(false);
                        setNewRoleName("");
                      }}
                    >
                      Cancel
                    </Button>
                  </form>
                )}

                {loadingRoles ? (
                  <div className="flex justify-center py-4">
                    <div className="w-5 h-5 border-2 border-zinc-600 border-t-blue-500 rounded-full animate-spin" />
                  </div>
                ) : roles.length === 0 ? (
                  <p className="text-sm text-zinc-500">No roles configured</p>
                ) : (
                  <div className="flex flex-wrap gap-2">
                    {roles.map((role) => (
                      <div
                        key={role.id}
                        className="inline-flex items-center gap-1.5 bg-blue-900/50 text-blue-400 border border-blue-700 px-2 py-0.5 rounded text-xs font-medium group"
                      >
                        <span>{role.name}</span>
                        <span className="text-blue-400/50">
                          ({role.user_count})
                        </span>
                        <button
                          onClick={() => setDeleteRoleConfirm(role)}
                          className="text-blue-400/50 hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity"
                        >
                          <Trash2 size={10} />
                        </button>
                      </div>
                    ))}
                  </div>
                )}
              </Card>

              {/* Users Section */}
              <Card className="p-5">
                <div className="flex items-center justify-between mb-4">
                  <h3 className="font-semibold">Users ({endUsers.length})</h3>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setShowInviteModal(true)}
                  >
                    <Plus size={14} /> Invite user
                  </Button>
                </div>

                {endUsers.length > 0 && (
                  <div className="mb-4">
                    <SearchInput
                      value={userSearch}
                      onChange={(e) => setUserSearch(e.target.value)}
                      placeholder="Search users..."
                    />
                  </div>
                )}

                {loadingUsers ? (
                  <div className="flex justify-center py-8">
                    <div className="w-5 h-5 border-2 border-zinc-600 border-t-blue-500 rounded-full animate-spin" />
                  </div>
                ) : endUsers.length === 0 ? (
                  <EmptyState
                    icon={Users}
                    title="No users yet"
                    description="Users will appear here once they sign in, or you can invite them"
                    action={
                      <Button
                        variant="primary"
                        onClick={() => setShowInviteModal(true)}
                      >
                        <Plus size={14} /> Invite first user
                      </Button>
                    }
                  />
                ) : (
                  <div className="space-y-2">
                    {endUsers
                      .filter((u) =>
                        u.email
                          .toLowerCase()
                          .includes(userSearch.toLowerCase()),
                      )
                      .map((user) => (
                        <div
                          key={user.id}
                          className="flex items-center justify-between p-3 bg-zinc-900 rounded-lg border border-zinc-800 group hover:bg-zinc-800/80 transition-colors cursor-pointer"
                          onClick={() =>
                            router.push(`/domains/${domainId}/users/${user.id}`)
                          }
                        >
                          <div>
                            <div className="font-medium">{user.email}</div>
                            <div className="flex items-center gap-2 mt-1">
                              {user.is_whitelisted && (
                                <Badge variant="info">Whitelisted</Badge>
                              )}
                              {user.is_frozen && (
                                <Badge variant="error">Frozen</Badge>
                              )}
                              {user.roles?.map((role) => (
                                <Badge key={role} variant="default">
                                  {role}
                                </Badge>
                              ))}
                            </div>
                          </div>
                          <div className="flex items-center gap-3">
                            <div className="text-sm text-zinc-500">
                              {user.last_login_at
                                ? formatDate(user.last_login_at)
                                : "Never"}
                            </div>
                            <div
                              ref={openMenuId === user.id ? menuRef : null}
                              className="relative"
                            >
                              <button
                                onClick={(e) => {
                                  e.stopPropagation();
                                  setOpenMenuId(
                                    openMenuId === user.id ? null : user.id,
                                  );
                                }}
                                className="opacity-0 group-hover:opacity-100 p-1 text-zinc-500 hover:text-white transition-all"
                              >
                                <MoreVertical size={16} />
                              </button>
                              {openMenuId === user.id && (
                                <div
                                  className="absolute top-full right-0 mt-1 bg-zinc-800 border border-zinc-700 rounded-lg shadow-xl min-w-[140px] overflow-hidden animate-scale-in"
                                  style={{ zIndex: zIndex.dropdown }}
                                >
                                  <button
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      handleUserAction(
                                        user.id,
                                        user.is_frozen ? "unfreeze" : "freeze",
                                      );
                                    }}
                                    className="w-full px-4 py-2 text-sm text-left hover:bg-zinc-700"
                                  >
                                    {user.is_frozen ? "Unfreeze" : "Freeze"}
                                  </button>
                                  <button
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      handleUserAction(
                                        user.id,
                                        user.is_whitelisted
                                          ? "unwhitelist"
                                          : "whitelist",
                                      );
                                    }}
                                    className="w-full px-4 py-2 text-sm text-left hover:bg-zinc-700"
                                  >
                                    {user.is_whitelisted
                                      ? "Remove whitelist"
                                      : "Whitelist"}
                                  </button>
                                  <div className="border-t border-zinc-700" />
                                  <button
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      setOpenMenuId(null);
                                      setDeleteUserConfirmId(user.id);
                                    }}
                                    className="w-full px-4 py-2 text-sm text-left text-red-400 hover:bg-zinc-700"
                                  >
                                    Delete
                                  </button>
                                </div>
                              )}
                            </div>
                          </div>
                        </div>
                      ))}
                  </div>
                )}
              </Card>
            </div>
          )}

          {/* API Tab */}
          {activeTab === "api" && (
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <p className="text-sm text-zinc-400">
                  Use API keys to authenticate server-to-server requests.
                </p>
                <Button
                  variant="primary"
                  onClick={() => setShowCreateKeyModal(true)}
                >
                  <Plus size={16} className="mr-1" />
                  Create API Key
                </Button>
              </div>

              {loadingApiKeys ? (
                <div className="flex justify-center py-8">
                  <div className="w-6 h-6 border-2 border-zinc-600 border-t-blue-500 rounded-full animate-spin" />
                </div>
              ) : apiKeys.length === 0 ? (
                <EmptyState
                  icon={Key}
                  title="No API keys"
                  description="Create an API key to get started."
                />
              ) : (
                <div className="space-y-2">
                  {apiKeys.map((key) => (
                    <Card
                      key={key.id}
                      className="p-4 flex items-center justify-between"
                    >
                      <div>
                        <div className="flex items-center gap-2">
                          <span className="font-medium text-white">
                            {key.name}
                          </span>
                          <code className="bg-zinc-800 px-2 py-0.5 rounded border border-zinc-700 text-xs text-zinc-400">
                            {key.key_prefix}...
                          </code>
                        </div>
                        <div className="flex items-center gap-3 mt-1 text-xs text-zinc-500">
                          {key.created_at && (
                            <span>Created {formatDate(key.created_at)}</span>
                          )}
                          {key.last_used_at && (
                            <span>
                              Last used {formatDate(key.last_used_at)}
                            </span>
                          )}
                        </div>
                      </div>
                      <HoldButton
                        onComplete={() => handleRevokeApiKey(key.id)}
                        variant="danger"
                        duration={3000}
                      >
                        Revoke
                      </HoldButton>
                    </Card>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Billing Tab */}
          {activeTab === "billing" && (
            <div className="space-y-6">
              {/* Analytics Cards */}
              {billingAnalytics && (
                <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
                  <Card className="p-4 text-center">
                    <div className="text-sm text-zinc-400 mb-1">MRR</div>
                    <div className="text-2xl font-bold text-green-400">
                      {formatPrice(billingAnalytics.mrr_cents)}
                    </div>
                  </Card>
                  <Card className="p-4 text-center">
                    <div className="text-sm text-zinc-400 mb-1">Active</div>
                    <div className="text-2xl font-bold">
                      {billingAnalytics.active_subscribers}
                    </div>
                  </Card>
                  <Card className="p-4 text-center">
                    <div className="text-sm text-zinc-400 mb-1">Trialing</div>
                    <div className="text-2xl font-bold text-blue-400">
                      {billingAnalytics.trialing_subscribers}
                    </div>
                  </Card>
                  <Card className="p-4 text-center">
                    <div className="text-sm text-zinc-400 mb-1">Past Due</div>
                    <div className="text-2xl font-bold text-yellow-400">
                      {billingAnalytics.past_due_subscribers}
                    </div>
                  </Card>
                </div>
              )}

              {/* Payment Providers */}
              <Card className="p-6">
                <div className="flex items-center justify-between mb-4">
                  <div>
                    <h2 className="text-lg font-semibold text-white flex items-center gap-2">
                      <CreditCard size={20} className="text-purple-400" />
                      Payment Providers
                    </h2>
                    <p className="text-sm text-zinc-400 mt-1">
                      Enable payment providers for end-users.
                    </p>
                  </div>
                </div>

                <div className="space-y-3">
                  {/* Stripe Test */}
                  <ProviderRow
                    icon={CreditCard}
                    label="Stripe (Test)"
                    description="Accept test payments via Stripe"
                    enabled={enabledProviders.some(
                      (p) => p.provider === "stripe" && p.mode === "test",
                    )}
                    configured={billingConfig?.test != null}
                    onToggle={() =>
                      enabledProviders.some(
                        (p) => p.provider === "stripe" && p.mode === "test",
                      )
                        ? handleDisableProvider("stripe", "test")
                        : handleEnableProvider("stripe", "test")
                    }
                    onSettings={() => setStripeConfigModal("test")}
                    loading={enablingProvider === "stripe_test"}
                  />

                  {/* Stripe Live */}
                  <ProviderRow
                    icon={CreditCard}
                    label="Stripe (Live)"
                    description="Accept real payments via Stripe"
                    enabled={enabledProviders.some(
                      (p) => p.provider === "stripe" && p.mode === "live",
                    )}
                    configured={billingConfig?.live != null}
                    onToggle={() =>
                      enabledProviders.some(
                        (p) => p.provider === "stripe" && p.mode === "live",
                      )
                        ? handleDisableProvider("stripe", "live")
                        : handleEnableProvider("stripe", "live")
                    }
                    onSettings={() => setStripeConfigModal("live")}
                    loading={enablingProvider === "stripe_live"}
                  />

                  {/* Dummy Test Provider */}
                  <ProviderRow
                    icon={TestTube}
                    label="Test Provider"
                    description="Simulated payments for testing (no real charges)"
                    enabled={enabledProviders.some(
                      (p) => p.provider === "dummy" && p.mode === "test",
                    )}
                    onToggle={() =>
                      enabledProviders.some(
                        (p) => p.provider === "dummy" && p.mode === "test",
                      )
                        ? handleDisableProvider("dummy", "test")
                        : handleEnableProvider("dummy", "test")
                    }
                    showSettings={false}
                    loading={enablingProvider === "dummy_test"}
                  />
                </div>
              </Card>

              {/* Active Mode Switcher */}
              {billingConfig && (billingConfig.test || billingConfig.live) && (
                <div className="flex items-center gap-4 p-4 bg-zinc-800/50 rounded-lg border border-zinc-700">
                  <span className="text-sm text-zinc-300">
                    Active Payment Mode:
                  </span>
                  <div className="flex gap-2">
                    <button
                      type="button"
                      className={`px-3 py-1.5 rounded text-sm font-medium transition-colors ${
                        billingConfig.active_mode === "test"
                          ? "bg-yellow-500/20 text-yellow-300 border border-yellow-500/30"
                          : "bg-zinc-700 text-zinc-400 hover:bg-zinc-600"
                      }`}
                      onClick={() =>
                        billingConfig.test && handleSwitchBillingMode("test")
                      }
                      disabled={
                        !billingConfig.test ||
                        switchingMode ||
                        billingConfig.active_mode === "test"
                      }
                    >
                      Test {billingConfig.test && "✓"}
                    </button>
                    <button
                      type="button"
                      className={`px-3 py-1.5 rounded text-sm font-medium transition-colors ${
                        billingConfig.active_mode === "live"
                          ? "bg-green-500/20 text-green-300 border border-green-500/30"
                          : "bg-zinc-700 text-zinc-400 hover:bg-zinc-600"
                      }`}
                      onClick={() =>
                        billingConfig.live && handleSwitchBillingMode("live")
                      }
                      disabled={
                        !billingConfig.live ||
                        switchingMode ||
                        billingConfig.active_mode === "live"
                      }
                    >
                      Live {billingConfig.live && "✓"}
                    </button>
                  </div>
                  {switchingMode && (
                    <span className="text-xs text-zinc-500">Switching...</span>
                  )}
                </div>
              )}

              {/* Subscription Plans */}
              <Card className="p-6">
                <div className="flex items-center justify-between mb-4">
                  <div>
                    <h2 className="text-lg font-semibold text-white flex items-center gap-2">
                      <DollarSign size={20} className="text-green-400" />
                      Subscription Plans
                    </h2>
                    <p className="text-sm text-zinc-400 mt-1">
                      Create and manage subscription plans for your users.
                    </p>
                  </div>
                  <Button variant="primary" onClick={() => openPlanModal()}>
                    <Plus size={16} className="mr-1" /> Create Plan
                  </Button>
                </div>

                {loadingBilling ? (
                  <div className="flex justify-center py-8">
                    <div className="w-6 h-6 border-2 border-zinc-600 border-t-blue-500 rounded-full animate-spin" />
                  </div>
                ) : billingPlans.length === 0 ? (
                  <EmptyState
                    icon={DollarSign}
                    title="No plans yet"
                    description="Create your first subscription plan to start accepting payments."
                    action={
                      <Button variant="primary" onClick={() => openPlanModal()}>
                        <Plus size={14} /> Create Plan
                      </Button>
                    }
                  />
                ) : (
                  <div className="space-y-3">
                    {billingPlans
                      .filter((p) => !p.is_archived)
                      .map((plan) => (
                        <div
                          key={plan.id}
                          className="flex items-center justify-between p-4 bg-zinc-900 rounded-lg border border-zinc-800"
                        >
                          <div className="flex-1">
                            <div className="flex items-center gap-2">
                              <span className="font-medium text-white">
                                {plan.name}
                              </span>
                              <code className="text-xs bg-zinc-800 px-2 py-0.5 rounded border border-zinc-700 text-zinc-400">
                                {plan.code}
                              </code>
                              {!plan.is_public && (
                                <Badge variant="warning">Private</Badge>
                              )}
                              {plan.trial_days > 0 && (
                                <Badge variant="info">
                                  {plan.trial_days}d trial
                                </Badge>
                              )}
                            </div>
                            <div className="text-sm text-zinc-400 mt-1">
                              {formatPrice(plan.price_cents)}{" "}
                              {formatInterval(
                                plan.interval,
                                plan.interval_count,
                              )}
                            </div>
                            {plan.features.length > 0 && (
                              <div className="flex flex-wrap gap-1 mt-2">
                                {plan.features.slice(0, 3).map((feature, i) => (
                                  <span
                                    key={i}
                                    className="text-xs bg-zinc-800 px-2 py-0.5 rounded border border-zinc-700 text-zinc-400"
                                  >
                                    {feature}
                                  </span>
                                ))}
                                {plan.features.length > 3 && (
                                  <span className="text-xs text-zinc-500">
                                    +{plan.features.length - 3} more
                                  </span>
                                )}
                              </div>
                            )}
                          </div>
                          <div className="flex items-center gap-2">
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => openPlanModal(plan)}
                            >
                              Edit
                            </Button>
                            <HoldButton
                              onComplete={() => handleArchivePlan(plan.id)}
                              variant="danger"
                              duration={2000}
                            >
                              Archive
                            </HoldButton>
                          </div>
                        </div>
                      ))}
                  </div>
                )}
              </Card>

              {/* Payment History */}
              <Card className="p-6">
                <div className="flex items-center justify-between mb-4">
                  <div>
                    <h2 className="text-lg font-semibold text-white flex items-center gap-2">
                      <Receipt size={20} className="text-purple-400" />
                      Payment History
                    </h2>
                    <p className="text-sm text-zinc-400 mt-1">
                      View and export payment records.
                    </p>
                  </div>
                  <Button
                    variant="ghost"
                    onClick={handleExportPayments}
                    disabled={exportingPayments || payments.length === 0}
                  >
                    <Download size={14} className="mr-1" />
                    {exportingPayments ? "Exporting..." : "Export CSV"}
                  </Button>
                </div>

                {/* Summary Cards */}
                {paymentsSummary && (
                  <div className="grid grid-cols-2 sm:grid-cols-5 gap-3 mb-4">
                    <div className="bg-zinc-800/50 rounded-lg p-3 text-center border border-zinc-700">
                      <div className="text-xs text-zinc-400">Revenue</div>
                      <div className="text-lg font-semibold text-green-400">
                        {formatPrice(paymentsSummary.total_revenue_cents)}
                      </div>
                    </div>
                    <div className="bg-zinc-800/50 rounded-lg p-3 text-center border border-zinc-700">
                      <div className="text-xs text-zinc-400">Refunded</div>
                      <div className="text-lg font-semibold text-blue-400">
                        {formatPrice(paymentsSummary.total_refunded_cents)}
                      </div>
                    </div>
                    <div className="bg-zinc-800/50 rounded-lg p-3 text-center border border-zinc-700">
                      <div className="text-xs text-zinc-400">Payments</div>
                      <div className="text-lg font-semibold">
                        {paymentsSummary.payment_count}
                      </div>
                    </div>
                    <div className="bg-zinc-800/50 rounded-lg p-3 text-center border border-zinc-700">
                      <div className="text-xs text-zinc-400">Successful</div>
                      <div className="text-lg font-semibold text-green-400">
                        {paymentsSummary.successful_payments}
                      </div>
                    </div>
                    <div className="bg-zinc-800/50 rounded-lg p-3 text-center border border-zinc-700">
                      <div className="text-xs text-zinc-400">Failed</div>
                      <div className="text-lg font-semibold text-red-400">
                        {paymentsSummary.failed_payments}
                      </div>
                    </div>
                  </div>
                )}

                {/* Filters */}
                <div className="flex flex-wrap items-center gap-2 mb-4">
                  <div className="flex-1 min-w-[200px]">
                    <div className="relative">
                      <input
                        type="text"
                        placeholder="Search by email..."
                        value={paymentEmailSearch}
                        onChange={(e) => setPaymentEmailSearch(e.target.value)}
                        onKeyDown={(e) =>
                          e.key === "Enter" && handlePaymentSearch()
                        }
                        className="w-full bg-zinc-800 border border-zinc-700 rounded-lg px-3 py-2 pr-8 text-sm text-white placeholder:text-zinc-500 focus:outline-none focus:border-zinc-600"
                      />
                      <button
                        type="button"
                        onClick={handlePaymentSearch}
                        className="absolute right-2 top-1/2 -translate-y-1/2 text-zinc-400 hover:text-white"
                      >
                        <Search size={16} />
                      </button>
                    </div>
                  </div>
                  <select
                    value={paymentFilters.status || ""}
                    onChange={(e) =>
                      handlePaymentStatusFilter(e.target.value || undefined)
                    }
                    className="bg-zinc-800 border border-zinc-700 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-zinc-600"
                  >
                    <option value="">All statuses</option>
                    <option value="paid">Paid</option>
                    <option value="pending">Pending</option>
                    <option value="failed">Failed</option>
                    <option value="refunded">Refunded</option>
                    <option value="partial_refund">Partial Refund</option>
                    <option value="void">Voided</option>
                    <option value="uncollectible">Uncollectible</option>
                  </select>
                  {(paymentFilters.status || paymentFilters.user_email) && (
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={clearPaymentFilters}
                    >
                      <X size={14} className="mr-1" />
                      Clear
                    </Button>
                  )}
                </div>

                {/* Payments Table */}
                {loadingPayments ? (
                  <div className="flex justify-center py-8">
                    <div className="w-6 h-6 border-2 border-zinc-600 border-t-blue-500 rounded-full animate-spin" />
                  </div>
                ) : payments.length === 0 ? (
                  <EmptyState
                    icon={Receipt}
                    title="No payments yet"
                    description="Payment records will appear here as users subscribe."
                  />
                ) : (
                  <>
                    <div className="overflow-x-auto">
                      <table className="w-full text-sm">
                        <thead>
                          <tr className="border-b border-zinc-700">
                            <th className="text-left py-3 px-2 text-zinc-400 font-medium">
                              Date
                            </th>
                            <th className="text-left py-3 px-2 text-zinc-400 font-medium">
                              User
                            </th>
                            <th className="text-left py-3 px-2 text-zinc-400 font-medium">
                              Plan
                            </th>
                            <th className="text-left py-3 px-2 text-zinc-400 font-medium">
                              Amount
                            </th>
                            <th className="text-left py-3 px-2 text-zinc-400 font-medium">
                              Status
                            </th>
                            <th className="text-right py-3 px-2 text-zinc-400 font-medium">
                              Invoice
                            </th>
                          </tr>
                        </thead>
                        <tbody>
                          {payments.map((payment) => (
                            <tr
                              key={payment.id}
                              className="border-b border-zinc-800 last:border-0 hover:bg-zinc-800/30"
                            >
                              <td className="py-3 px-2 text-white whitespace-nowrap">
                                {formatPaymentDate(
                                  payment.payment_date || payment.created_at,
                                )}
                              </td>
                              <td className="py-3 px-2 text-zinc-300">
                                <div
                                  className="max-w-[180px] truncate"
                                  title={payment.user_email}
                                >
                                  {payment.user_email || "-"}
                                </div>
                              </td>
                              <td className="py-3 px-2 text-zinc-300">
                                {payment.plan_name || payment.plan_code || "-"}
                              </td>
                              <td className="py-3 px-2 text-white whitespace-nowrap">
                                {formatPrice(
                                  payment.amount_cents,
                                  payment.currency,
                                )}
                                {payment.amount_refunded_cents > 0 && (
                                  <span className="text-xs text-blue-400 ml-1">
                                    (-
                                    {formatPrice(
                                      payment.amount_refunded_cents,
                                      payment.currency,
                                    )}
                                    )
                                  </span>
                                )}
                              </td>
                              <td className="py-3 px-2">
                                <Badge
                                  variant={
                                    getPaymentStatusBadgeColor(
                                      payment.status,
                                    ) === "green"
                                      ? "success"
                                      : getPaymentStatusBadgeColor(
                                            payment.status,
                                          ) === "red"
                                        ? "error"
                                        : getPaymentStatusBadgeColor(
                                              payment.status,
                                            ) === "yellow"
                                          ? "warning"
                                          : getPaymentStatusBadgeColor(
                                                payment.status,
                                              ) === "blue"
                                            ? "info"
                                            : "default"
                                  }
                                >
                                  {getPaymentStatusLabel(payment.status)}
                                </Badge>
                                {payment.failure_message && (
                                  <span
                                    className="block text-xs text-red-400 mt-1"
                                    title={payment.failure_message}
                                  >
                                    {payment.failure_message.slice(0, 30)}...
                                  </span>
                                )}
                              </td>
                              <td className="py-3 px-2 text-right">
                                {payment.invoice_pdf ? (
                                  <a
                                    href={payment.invoice_pdf}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="inline-flex items-center gap-1 text-blue-400 hover:text-blue-300"
                                  >
                                    <FileText size={14} />
                                    PDF
                                  </a>
                                ) : payment.invoice_url ? (
                                  <a
                                    href={payment.invoice_url}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="inline-flex items-center gap-1 text-blue-400 hover:text-blue-300"
                                  >
                                    <ExternalLink size={14} />
                                    View
                                  </a>
                                ) : payment.invoice_number ? (
                                  <span className="text-zinc-500">
                                    #{payment.invoice_number}
                                  </span>
                                ) : (
                                  <span className="text-zinc-500">-</span>
                                )}
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>

                    {/* Pagination */}
                    {paymentsPagination.total_pages > 1 && (
                      <div className="flex items-center justify-between mt-4 pt-4 border-t border-zinc-800">
                        <span className="text-sm text-zinc-400">
                          Showing{" "}
                          {(paymentsPagination.page - 1) *
                            paymentsPagination.per_page +
                            1}{" "}
                          to{" "}
                          {Math.min(
                            paymentsPagination.page *
                              paymentsPagination.per_page,
                            paymentsPagination.total,
                          )}{" "}
                          of {paymentsPagination.total} payments
                        </span>
                        <div className="flex gap-2">
                          <Button
                            variant="ghost"
                            size="sm"
                            disabled={
                              paymentsPagination.page <= 1 || loadingPayments
                            }
                            onClick={() =>
                              fetchPayments(
                                paymentsPagination.page - 1,
                                paymentFilters,
                              )
                            }
                          >
                            <ChevronLeft size={14} />
                            Previous
                          </Button>
                          <Button
                            variant="ghost"
                            size="sm"
                            disabled={
                              paymentsPagination.page >=
                                paymentsPagination.total_pages ||
                              loadingPayments
                            }
                            onClick={() =>
                              fetchPayments(
                                paymentsPagination.page + 1,
                                paymentFilters,
                              )
                            }
                          >
                            Next
                            <ChevronRight size={14} />
                          </Button>
                        </div>
                      </div>
                    )}
                  </>
                )}
              </Card>
            </div>
          )}

          {/* Settings Tab */}
          {activeTab === "settings" && (
            <div className="space-y-6">
              {/* Redirect URL */}
              <Card className="p-5">
                <div className="flex items-center justify-between mb-3">
                  <div>
                    <h3 className="font-semibold">Redirect URL</h3>
                    <p className="text-sm text-zinc-400">
                      Where users go after successful authentication
                    </p>
                  </div>
                </div>
                <Input
                  value={redirectUrl}
                  onChange={(e) => setRedirectUrl(e.target.value)}
                  placeholder="https://yourapp.com/dashboard"
                />
                <p className="text-xs text-zinc-500 mt-2">
                  Must be on {domain.domain} or a subdomain.
                </p>
                <Button
                  variant="primary"
                  className="mt-4"
                  disabled={saving}
                  onClick={(e) =>
                    handleSaveConfig(e as unknown as React.FormEvent)
                  }
                >
                  {saving ? "Saving..." : "Save"}
                </Button>
              </Card>

              {/* Whitelist Mode */}
              <Card className="p-5">
                <div className="flex items-center justify-between">
                  <div>
                    <h3 className="font-semibold">Whitelist Mode</h3>
                    <p className="text-sm text-zinc-400">
                      Only allow pre-approved email addresses to sign in
                    </p>
                  </div>
                  <Toggle
                    enabled={whitelistEnabled}
                    onChange={handleWhitelistToggle}
                  />
                </div>
              </Card>

              {/* Danger Zone */}
              <Card className="p-5 border-red-900/50">
                <h3 className="font-semibold text-red-400 mb-4">Danger Zone</h3>
                <div className="flex items-center justify-between p-4 bg-red-900/10 border border-red-900/30 rounded-lg">
                  <div>
                    <div className="font-medium">Delete this domain</div>
                    <div className="text-sm text-zinc-400">
                      This action cannot be undone. All users and settings will
                      be permanently deleted.
                    </div>
                  </div>
                  <Button
                    variant="danger"
                    onClick={() => setShowDeleteDomainModal(true)}
                  >
                    <Trash2 size={14} className="mr-1" /> Delete domain
                  </Button>
                </div>
              </Card>
            </div>
          )}
        </div>
      </div>

      {/* Modals */}
      <ConfirmModal
        isOpen={deleteUserConfirmId !== null}
        title="Delete User"
        message="This action cannot be undone."
        variant="danger"
        confirmLabel="Delete"
        onConfirm={() =>
          deleteUserConfirmId && handleUserAction(deleteUserConfirmId, "delete")
        }
        onCancel={() => setDeleteUserConfirmId(null)}
      />

      <ConfirmModal
        isOpen={deleteRoleConfirm !== null}
        title="Delete Role"
        message={
          deleteRoleConfirm
            ? `This will remove the "${deleteRoleConfirm.name}" role from ${deleteRoleConfirm.user_count} user${deleteRoleConfirm.user_count === 1 ? "" : "s"}.`
            : ""
        }
        variant="danger"
        confirmLabel="Delete"
        useHoldToConfirm
        onConfirm={handleDeleteRole}
        onCancel={() => setDeleteRoleConfirm(null)}
      />

      <ConfirmModal
        isOpen={showDeleteDomainModal}
        title="Delete Domain"
        message="This will permanently delete this domain and all associated data."
        variant="danger"
        confirmLabel="Delete"
        confirmText={domain.domain}
        useHoldToConfirm
        onConfirm={handleDeleteDomain}
        onCancel={() => setShowDeleteDomainModal(false)}
      />

      {/* Whitelist Modal */}
      <Modal
        open={showWhitelistModal}
        onClose={() => setShowWhitelistModal(false)}
        title="Enable Whitelist Mode"
      >
        <div className="space-y-4">
          <p className="text-sm text-zinc-400">
            When whitelist mode is enabled, only whitelisted users can sign in.
            Would you like to add all current users to the whitelist?
          </p>
          <div className="flex justify-end gap-2">
            <Button
              variant="ghost"
              onClick={() => setShowWhitelistModal(false)}
            >
              Cancel
            </Button>
            <Button onClick={() => handleWhitelistConfirm(false)}>
              Enable Only
            </Button>
            <Button
              variant="primary"
              onClick={() => handleWhitelistConfirm(true)}
            >
              Whitelist All
            </Button>
          </div>
        </div>
      </Modal>

      {/* Invite Modal */}
      <Modal
        open={showInviteModal}
        onClose={() => {
          setShowInviteModal(false);
          setInviteEmail("");
          setInvitePreWhitelist(false);
        }}
        title="Invite User"
      >
        <form onSubmit={handleInviteUser} className="space-y-4">
          <div className="space-y-2">
            <label className="text-sm text-zinc-400">Email address</label>
            <Input
              type="email"
              value={inviteEmail}
              onChange={(e) => setInviteEmail(e.target.value)}
              placeholder="user@example.com"
              autoFocus
              required
            />
          </div>
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={invitePreWhitelist}
              onChange={(e) => setInvitePreWhitelist(e.target.checked)}
              className="w-4 h-4"
            />
            <span className="text-sm">Pre-approve (add to whitelist)</span>
          </label>
          <div className="flex justify-end gap-2">
            <Button
              type="button"
              variant="ghost"
              onClick={() => {
                setShowInviteModal(false);
                setInviteEmail("");
              }}
            >
              Cancel
            </Button>
            <Button
              type="submit"
              variant="primary"
              disabled={inviting || !inviteEmail.trim()}
            >
              {inviting ? "Sending..." : "Send Invitation"}
            </Button>
          </div>
        </form>
      </Modal>

      {/* Create API Key Modal */}
      <Modal
        open={showCreateKeyModal && !newlyCreatedKey}
        onClose={() => {
          setShowCreateKeyModal(false);
          setNewKeyName("");
        }}
        title="Create API Key"
      >
        <form onSubmit={handleCreateApiKey} className="space-y-4">
          <div className="space-y-2">
            <label className="text-sm text-zinc-400">Key name (optional)</label>
            <Input
              value={newKeyName}
              onChange={(e) => setNewKeyName(e.target.value)}
              placeholder="e.g., Production API"
              autoFocus
            />
          </div>
          <div className="flex justify-end gap-2">
            <Button
              type="button"
              variant="ghost"
              onClick={() => {
                setShowCreateKeyModal(false);
                setNewKeyName("");
              }}
            >
              Cancel
            </Button>
            <Button type="submit" variant="primary" disabled={creatingKey}>
              {creatingKey ? "Creating..." : "Create Key"}
            </Button>
          </div>
        </form>
      </Modal>

      {/* New Key Display Modal */}
      <Modal
        open={!!newlyCreatedKey}
        onClose={() => {
          setNewlyCreatedKey(null);
          setShowCreateKeyModal(false);
        }}
        title="API Key Created"
      >
        <div className="space-y-4">
          <div className="flex items-center gap-2 p-3 bg-amber-500/10 border border-amber-500/20 rounded-lg">
            <AlertTriangle size={16} className="text-amber-500" />
            <span className="text-sm text-amber-200">
              Copy this key now. You won&apos;t see it again!
            </span>
          </div>
          <div className="flex items-center gap-2 bg-zinc-800 p-3 rounded-lg">
            <code className="flex-1 text-sm break-all">{newlyCreatedKey}</code>
            <CopyButton text={newlyCreatedKey || ""} />
          </div>
          <div className="flex justify-end">
            <Button
              variant="primary"
              onClick={() => {
                setNewlyCreatedKey(null);
                setShowCreateKeyModal(false);
              }}
            >
              Done
            </Button>
          </div>
        </div>
      </Modal>

      {/* Plan Modal */}
      <Modal
        open={showPlanModal}
        onClose={() => setShowPlanModal(false)}
        title={editingPlan ? "Edit Plan" : "Create Plan"}
      >
        <form onSubmit={handleSavePlan} className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <label className="text-sm font-medium text-zinc-300">
                Plan Code
              </label>
              <Input
                value={planCode}
                onChange={(e) =>
                  setPlanCode(
                    e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, ""),
                  )
                }
                placeholder="e.g., pro"
                disabled={!!editingPlan}
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium text-zinc-300">
                Plan Name
              </label>
              <Input
                value={planName}
                onChange={(e) => setPlanName(e.target.value)}
                placeholder="e.g., Pro Plan"
              />
            </div>
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium text-zinc-300">
              Description (optional)
            </label>
            <Input
              value={planDescription}
              onChange={(e) => setPlanDescription(e.target.value)}
              placeholder="A brief description of this plan"
            />
          </div>

          <div className="grid grid-cols-3 gap-4">
            <div className="space-y-2">
              <label className="text-sm font-medium text-zinc-300">
                Price (cents)
              </label>
              <Input
                type="number"
                value={planPriceCents}
                onChange={(e) => setPlanPriceCents(e.target.value)}
                placeholder="999"
              />
              <p className="text-xs text-zinc-500">e.g., 999 = $9.99</p>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium text-zinc-300">
                Interval
              </label>
              <select
                value={planInterval}
                onChange={(e) => setPlanInterval(e.target.value)}
                className="w-full bg-zinc-800 border border-zinc-700 rounded-lg px-3 py-2 text-white"
              >
                <option value="monthly">Monthly</option>
                <option value="yearly">Yearly</option>
              </select>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium text-zinc-300">
                Trial (days)
              </label>
              <Input
                type="number"
                value={planTrialDays}
                onChange={(e) => setPlanTrialDays(e.target.value)}
                placeholder="0"
              />
            </div>
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium text-zinc-300">
              Features
            </label>
            <div className="flex gap-2">
              <Input
                value={newFeature}
                onChange={(e) => setNewFeature(e.target.value)}
                placeholder="Add a feature..."
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    addFeature();
                  }
                }}
              />
              <Button type="button" variant="ghost" onClick={addFeature}>
                Add
              </Button>
            </div>
            {planFeatures.length > 0 && (
              <div className="flex flex-wrap gap-2 mt-2">
                {planFeatures.map((feature, i) => (
                  <span
                    key={i}
                    className="inline-flex items-center gap-1 bg-zinc-800 px-2 py-1 rounded text-sm text-zinc-300"
                  >
                    {feature}
                    <button
                      type="button"
                      onClick={() => removeFeature(i)}
                      className="text-zinc-500 hover:text-red-400"
                    >
                      <Trash2 size={12} />
                    </button>
                  </span>
                ))}
              </div>
            )}
          </div>

          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={planIsPublic}
              onChange={(e) => setPlanIsPublic(e.target.checked)}
              className="w-4 h-4"
            />
            <span className="text-sm">Show on public pricing page</span>
          </label>

          <div className="flex justify-end gap-2 pt-2">
            <Button
              type="button"
              variant="ghost"
              onClick={() => setShowPlanModal(false)}
            >
              Cancel
            </Button>
            <Button type="submit" variant="primary" disabled={savingPlan}>
              {savingPlan
                ? "Saving..."
                : editingPlan
                  ? "Update Plan"
                  : "Create Plan"}
            </Button>
          </div>
        </form>
      </Modal>

      {/* Magic Link Config Modal */}
      <Modal
        open={authConfigModal === "magic_link"}
        onClose={() => setAuthConfigModal(null)}
        title="Magic Link Configuration"
      >
        <div className="space-y-4">
          {/* Current status */}
          {authConfig?.magic_link_config?.has_api_key ? (
            <div className="flex items-center justify-between p-3 bg-emerald-500/10 border border-emerald-500/20 rounded-lg">
              <div className="flex items-center gap-2">
                <Check size={16} className="text-emerald-400" />
                <span className="text-sm text-emerald-200">
                  Using custom email: {authConfig.magic_link_config.from_email}
                </span>
              </div>
              <HoldButton
                onComplete={() => {
                  handleRemoveCustomConfig();
                  setAuthConfigModal(null);
                }}
                variant="danger"
                duration={2000}
              >
                Remove
              </HoldButton>
            </div>
          ) : (
            authConfig?.using_fallback && (
              <div className="p-3 bg-blue-500/10 border border-blue-500/20 rounded-lg">
                <p className="text-sm text-blue-200">
                  Using reauth&apos;s shared email service (
                  {authConfig.fallback_from_email})
                </p>
                <p className="text-xs text-blue-300/70 mt-1">
                  Add your own Resend API key for custom branding.
                </p>
              </div>
            )
          )}

          <div className="space-y-2">
            <label className="text-sm font-medium text-zinc-300">
              Resend API Key
            </label>
            <Input
              type="password"
              value={resendApiKey}
              onChange={(e) => setResendApiKey(e.target.value)}
              placeholder={
                authConfig?.magic_link_config?.has_api_key
                  ? "••••••••"
                  : "Enter API key"
              }
            />
            <p className="text-xs text-zinc-500">
              Get your key from{" "}
              <a
                href="https://resend.com/api-keys"
                target="_blank"
                rel="noopener noreferrer"
                className="text-blue-400"
              >
                resend.com
              </a>
            </p>
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium text-zinc-300">
              From Email
            </label>
            <Input
              type="email"
              value={fromEmail}
              onChange={(e) => setFromEmail(e.target.value)}
              placeholder={
                authConfig?.fallback_from_email || "noreply@yourdomain.com"
              }
            />
          </div>

          <div className="flex justify-end gap-2 pt-2">
            <Button variant="ghost" onClick={() => setAuthConfigModal(null)}>
              Cancel
            </Button>
            <Button
              variant="primary"
              onClick={(e) => {
                handleSaveConfig(e as unknown as React.FormEvent);
                setAuthConfigModal(null);
              }}
              disabled={saving}
            >
              {saving ? "Saving..." : "Save Configuration"}
            </Button>
          </div>
        </div>
      </Modal>

      {/* Google OAuth Config Modal */}
      <Modal
        open={authConfigModal === "google_oauth"}
        onClose={() => setAuthConfigModal(null)}
        title="Google OAuth Configuration"
      >
        <div className="space-y-4">
          {/* Current status */}
          {authConfig?.google_oauth_config?.has_client_secret ? (
            <div className="flex items-center justify-between p-3 bg-emerald-500/10 border border-emerald-500/20 rounded-lg">
              <div className="flex items-center gap-2">
                <Check size={16} className="text-emerald-400" />
                <span className="text-sm text-emerald-200">
                  Using custom OAuth (Client:{" "}
                  {authConfig.google_oauth_config.client_id_prefix}...)
                </span>
              </div>
              <HoldButton
                onComplete={() => {
                  handleRemoveGoogleOAuthConfig();
                  setAuthConfigModal(null);
                }}
                variant="danger"
                duration={2000}
              >
                Remove
              </HoldButton>
            </div>
          ) : (
            authConfig?.using_google_fallback && (
              <div className="p-3 bg-blue-500/10 border border-blue-500/20 rounded-lg">
                <p className="text-sm text-blue-200">
                  Using reauth&apos;s shared Google OAuth
                </p>
              </div>
            )
          )}

          {/* Redirect URI info */}
          <div className="bg-zinc-800/50 rounded-lg p-4 space-y-3 border border-zinc-700">
            <div className="flex items-center justify-between">
              <h3 className="font-medium text-white text-sm">Redirect URI</h3>
              <a
                href="https://console.cloud.google.com/apis/credentials"
                target="_blank"
                rel="noopener noreferrer"
                className="text-xs text-blue-400"
              >
                Open Console
              </a>
            </div>
            <div className="flex items-center gap-2">
              <code className="flex-1 bg-zinc-900 px-3 py-2 rounded border border-zinc-800 text-xs text-zinc-300 font-mono break-all">
                https://reauth.{domain.domain}/callback/google
              </code>
              <CopyButton
                text={`https://reauth.${domain.domain}/callback/google`}
              />
            </div>
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium text-zinc-300">
              Google Client ID
            </label>
            <Input
              value={googleClientId}
              onChange={(e) => setGoogleClientId(e.target.value)}
              placeholder={
                authConfig?.google_oauth_config?.has_client_secret
                  ? `${authConfig.google_oauth_config.client_id_prefix}...`
                  : "Enter Client ID"
              }
            />
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium text-zinc-300">
              Google Client Secret
            </label>
            <Input
              type="password"
              value={googleClientSecret}
              onChange={(e) => setGoogleClientSecret(e.target.value)}
              placeholder={
                authConfig?.google_oauth_config?.has_client_secret
                  ? "••••••••"
                  : "Enter Client Secret"
              }
            />
          </div>

          <div className="flex justify-end gap-2 pt-2">
            <Button variant="ghost" onClick={() => setAuthConfigModal(null)}>
              Cancel
            </Button>
            <Button
              variant="primary"
              onClick={(e) => {
                handleSaveConfig(e as unknown as React.FormEvent);
                setAuthConfigModal(null);
              }}
              disabled={saving}
            >
              {saving ? "Saving..." : "Save Configuration"}
            </Button>
          </div>
        </div>
      </Modal>

      {/* Stripe Config Modal */}
      <Modal
        open={stripeConfigModal !== null}
        onClose={() => setStripeConfigModal(null)}
        title={`Stripe ${stripeConfigModal === "test" ? "Test" : "Live"} Configuration`}
        size="lg"
      >
        <div className="space-y-4">
          {/* Mode indicator */}
          <div
            className={`p-3 rounded-lg border ${
              stripeConfigModal === "test"
                ? "bg-yellow-500/10 border-yellow-500/30"
                : "bg-green-500/10 border-green-500/30"
            }`}
          >
            <div className="flex items-center gap-2">
              <Badge
                variant={stripeConfigModal === "test" ? "warning" : "success"}
              >
                {stripeConfigModal === "test" ? "🧪 Test Mode" : "🔴 Live Mode"}
              </Badge>
              <span className="text-sm text-zinc-300">
                {stripeConfigModal === "test"
                  ? "Use test API keys from Stripe dashboard"
                  : "Use live API keys - real transactions will be processed"}
              </span>
            </div>
          </div>

          {/* Current status */}
          {stripeConfigModal === "test" && billingConfig?.test && (
            <div className="flex items-center justify-between p-3 bg-emerald-500/10 border border-emerald-500/20 rounded-lg">
              <div className="flex items-center gap-2">
                <Check size={16} className="text-emerald-400" />
                <span className="text-sm text-emerald-200">
                  Connected - Key: {billingConfig.test.publishable_key_last4}
                </span>
              </div>
              <HoldButton
                onComplete={() => {
                  handleRemoveBillingConfig("test");
                  setStripeConfigModal(null);
                }}
                variant="danger"
                duration={2000}
              >
                Disconnect
              </HoldButton>
            </div>
          )}
          {stripeConfigModal === "live" && billingConfig?.live && (
            <div className="flex items-center justify-between p-3 bg-emerald-500/10 border border-emerald-500/20 rounded-lg">
              <div className="flex items-center gap-2">
                <Check size={16} className="text-emerald-400" />
                <span className="text-sm text-emerald-200">
                  Connected - Key: {billingConfig.live.publishable_key_last4}
                </span>
              </div>
              <HoldButton
                onComplete={() => {
                  handleRemoveBillingConfig("live");
                  setStripeConfigModal(null);
                }}
                variant="danger"
                duration={2000}
              >
                Disconnect
              </HoldButton>
            </div>
          )}

          <div className="space-y-2">
            <label className="text-sm font-medium text-zinc-300">
              Secret Key
            </label>
            <Input
              type="password"
              value={stripeSecretKey}
              onChange={(e) => setStripeSecretKey(e.target.value)}
              placeholder={
                stripeConfigModal === "test" ? "sk_test_..." : "sk_live_..."
              }
            />
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium text-zinc-300">
              Publishable Key
            </label>
            <Input
              value={stripePublishableKey}
              onChange={(e) => setStripePublishableKey(e.target.value)}
              placeholder={
                stripeConfigModal === "test" ? "pk_test_..." : "pk_live_..."
              }
            />
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium text-zinc-300">
              Webhook Secret
            </label>
            <Input
              type="password"
              value={stripeWebhookSecret}
              onChange={(e) => setStripeWebhookSecret(e.target.value)}
              placeholder="whsec_..."
            />
          </div>

          {/* Webhook URL info */}
          <div className="bg-zinc-800/50 rounded-lg p-4 space-y-2 border border-zinc-700">
            <label className="text-sm font-medium text-zinc-300">
              Webhook URL
            </label>
            <div className="flex items-center gap-2">
              <code className="flex-1 bg-zinc-900 px-3 py-2 rounded border border-zinc-800 text-xs text-zinc-300 font-mono break-all">
                https://reauth.{domain.domain}/webhook/stripe
              </code>
              <CopyButton
                text={`https://reauth.${domain.domain}/webhook/stripe`}
              />
            </div>
            <p className="text-xs text-zinc-500">
              Add this URL in your Stripe webhook settings
            </p>
          </div>

          <div className="flex justify-end gap-2 pt-2">
            <Button variant="ghost" onClick={() => setStripeConfigModal(null)}>
              Cancel
            </Button>
            <Button
              variant="primary"
              onClick={() => {
                // Temporarily set editingMode to the modal mode for save handler
                const previousMode = editingMode;
                setEditingMode(stripeConfigModal!);
                handleSaveBillingConfig({
                  preventDefault: () => {},
                } as React.FormEvent);
                setEditingMode(previousMode);
                setStripeConfigModal(null);
              }}
              disabled={
                savingBillingConfig || !stripeSecretKey || !stripePublishableKey
              }
            >
              {savingBillingConfig ? "Saving..." : "Save Configuration"}
            </Button>
          </div>
        </div>
      </Modal>
    </div>
  );
}
