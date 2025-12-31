// Billing Types for Reauth

export type StripeMode = 'test' | 'live';

export interface ModeConfigStatus {
  publishable_key_last4: string;
  is_connected: boolean;
}

export interface StripeConfigStatus {
  active_mode: StripeMode;
  test: ModeConfigStatus | null;
  live: ModeConfigStatus | null;
}

// Legacy interface for backwards compatibility
export interface StripeConfig {
  publishable_key: string | null;
  has_secret_key: boolean;
  is_connected: boolean;
  // NOTE: No using_fallback field - each domain must configure their own Stripe account.
}

export interface SubscriptionPlan {
  id: string;
  stripe_mode: StripeMode;
  code: string;
  name: string;
  description: string | null;
  price_cents: number;
  currency: string;
  interval: 'monthly' | 'yearly' | 'custom' | string;
  interval_count: number;
  trial_days: number;
  features: string[];
  is_public: boolean;
  display_order: number;
  stripe_product_id: string | null;
  stripe_price_id: string | null;
  is_archived: boolean;
  created_at: string | null;
}

export interface UserSubscription {
  id: string;
  user_id: string;
  user_email: string;
  plan_id: string;
  plan_name: string;
  plan_code: string;
  status: SubscriptionStatus;
  current_period_start: string | null;
  current_period_end: string | null;
  trial_start: string | null;
  trial_end: string | null;
  cancel_at_period_end: boolean;
  manually_granted: boolean;
  created_at: string | null;
}

export type SubscriptionStatus =
  | 'active'
  | 'past_due'
  | 'canceled'
  | 'trialing'
  | 'incomplete'
  | 'incomplete_expired'
  | 'unpaid'
  | 'paused'
  | 'none';

export interface BillingAnalytics {
  mrr_cents: number;
  active_subscribers: number;
  trialing_subscribers: number;
  past_due_subscribers: number;
  plan_distribution: PlanDistribution[];
}

export interface PlanDistribution {
  plan_id: string;
  plan_name: string;
  subscriber_count: number;
  revenue_cents: number;
}

export interface CreatePlanInput {
  code: string;
  name: string;
  description?: string;
  price_cents: number;
  currency: string;
  interval: string;
  interval_count: number;
  trial_days: number;
  features: string[];
  is_public: boolean;
}

export interface UpdatePlanInput {
  name?: string;
  description?: string;
  price_cents?: number;
  interval?: string;
  interval_count?: number;
  trial_days?: number;
  features?: string[];
  is_public?: boolean;
}

export interface UpdateStripeConfigInput {
  mode: StripeMode;
  secret_key: string;
  publishable_key: string;
  webhook_secret: string;
}

export interface DeleteStripeConfigInput {
  mode: StripeMode;
}

export interface SetBillingModeInput {
  mode: StripeMode;
}

// Helper functions
export function formatPrice(cents: number, currency: string = 'USD'): string {
  const dollars = cents / 100;
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency,
  }).format(dollars);
}

export function formatInterval(interval: string, count: number): string {
  if (count === 1) {
    return interval === 'monthly' ? 'per month' : interval === 'yearly' ? 'per year' : `per ${interval}`;
  }
  return `every ${count} ${interval === 'monthly' ? 'months' : interval === 'yearly' ? 'years' : interval}`;
}

export function getStatusBadgeColor(status: SubscriptionStatus): string {
  switch (status) {
    case 'active':
      return 'green';
    case 'trialing':
      return 'blue';
    case 'past_due':
      return 'yellow';
    case 'canceled':
    case 'incomplete_expired':
    case 'unpaid':
      return 'red';
    case 'incomplete':
    case 'paused':
      return 'gray';
    default:
      return 'gray';
  }
}

export function getStatusLabel(status: SubscriptionStatus): string {
  switch (status) {
    case 'active':
      return 'Active';
    case 'trialing':
      return 'Trial';
    case 'past_due':
      return 'Past Due';
    case 'canceled':
      return 'Canceled';
    case 'incomplete':
      return 'Incomplete';
    case 'incomplete_expired':
      return 'Expired';
    case 'unpaid':
      return 'Unpaid';
    case 'paused':
      return 'Paused';
    case 'none':
      return 'No Subscription';
    default:
      return status;
  }
}

export function getModeLabel(mode: StripeMode): string {
  return mode === 'test' ? 'Test Mode' : 'Live Mode';
}

export function getModeBadgeColor(mode: StripeMode): 'yellow' | 'green' {
  return mode === 'test' ? 'yellow' : 'green';
}

// ============================================================================
// Payment History Types
// ============================================================================

export type PaymentStatus =
  | 'pending'
  | 'paid'
  | 'failed'
  | 'refunded'
  | 'partial_refund'
  | 'uncollectible'
  | 'void';

export interface BillingPayment {
  id: string;
  user_id?: string;
  user_email?: string;
  amount_cents: number;
  amount_paid_cents: number;
  amount_refunded_cents: number;
  currency: string;
  status: PaymentStatus;
  plan_code: string | null;
  plan_name: string | null;
  invoice_url: string | null;
  invoice_pdf: string | null;
  invoice_number: string | null;
  billing_reason?: string | null;
  failure_message?: string | null;
  payment_date: number | null; // Unix timestamp
  created_at: number | null; // Unix timestamp
}

export interface PaginatedPayments {
  payments: BillingPayment[];
  total: number;
  page: number;
  per_page: number;
  total_pages: number;
}

export interface PaymentSummary {
  total_revenue_cents: number;
  total_refunded_cents: number;
  payment_count: number;
  successful_payments: number;
  failed_payments: number;
}

export interface DashboardPaymentListResponse extends PaginatedPayments {
  summary: PaymentSummary;
}

export interface PaymentListFilters {
  status?: PaymentStatus;
  date_from?: number; // Unix timestamp
  date_to?: number; // Unix timestamp
  plan_code?: string;
  user_email?: string;
}

// Payment helper functions
export function getPaymentStatusLabel(status: PaymentStatus): string {
  switch (status) {
    case 'pending':
      return 'Pending';
    case 'paid':
      return 'Paid';
    case 'failed':
      return 'Failed';
    case 'refunded':
      return 'Refunded';
    case 'partial_refund':
      return 'Partially Refunded';
    case 'uncollectible':
      return 'Uncollectible';
    case 'void':
      return 'Voided';
    default:
      return status;
  }
}

export function getPaymentStatusBadgeColor(status: PaymentStatus): string {
  switch (status) {
    case 'paid':
      return 'green';
    case 'pending':
      return 'yellow';
    case 'failed':
    case 'uncollectible':
    case 'void':
      return 'red';
    case 'refunded':
    case 'partial_refund':
      return 'blue';
    default:
      return 'gray';
  }
}

export function formatPaymentDate(timestamp: number | null): string {
  if (!timestamp) return '-';
  return new Date(timestamp * 1000).toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

export function formatPaymentDateTime(timestamp: number | null): string {
  if (!timestamp) return '-';
  return new Date(timestamp * 1000).toLocaleString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}
