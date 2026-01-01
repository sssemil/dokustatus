// Billing Types for Reauth

// ============================================================================
// Payment Provider Types
// ============================================================================

export type PaymentProvider = 'stripe' | 'dummy' | 'coinbase';
export type PaymentMode = 'test' | 'live';
export type BillingState = 'active' | 'pending_switch' | 'switch_failed';

export type PaymentScenario =
  | 'success'
  | 'decline'
  | 'insufficient_funds'
  | 'three_d_secure'
  | 'expired_card'
  | 'processing_error';

export interface EnabledPaymentProvider {
  id: string;
  domain_id: string;
  provider: PaymentProvider;
  mode: PaymentMode;
  is_active: boolean;
  display_order: number;
  created_at: string | null;
}

export interface ProviderConfig {
  provider: PaymentProvider;
  mode: PaymentMode;
}

/**
 * @deprecated Use PaymentMode instead. StripeMode is a legacy alias and will be removed in task 0015.
 */
export type StripeMode = 'test' | 'live';

export interface ModeConfigStatus {
  publishable_key_last4: string;
  is_connected: boolean;
}

export interface StripeConfigStatus {
  active_mode: PaymentMode;
  test: ModeConfigStatus | null;
  live: ModeConfigStatus | null;
}

export interface SubscriptionPlan {
  id: string;
  stripe_mode: StripeMode;
  payment_provider: PaymentProvider | null;
  payment_mode: PaymentMode | null;
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
  payment_provider: PaymentProvider | null;
  payment_mode: PaymentMode | null;
  billing_state: BillingState | null;
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
  mode: PaymentMode;
  secret_key: string;
  publishable_key: string;
  webhook_secret: string;
}

export interface DeleteStripeConfigInput {
  mode: PaymentMode;
}

export interface SetBillingModeInput {
  mode: PaymentMode;
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

/**
 * @deprecated Use getPaymentModeLabel instead.
 */
export function getModeLabel(mode: PaymentMode): string {
  return mode === 'test' ? 'Test Mode' : 'Live Mode';
}

/**
 * @deprecated Use getPaymentModeBadgeColor instead.
 */
export function getModeBadgeColor(mode: PaymentMode): 'yellow' | 'green' {
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
  payment_provider: PaymentProvider | null;
  payment_mode: PaymentMode | null;
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

// ============================================================================
// Plan Change Types (Upgrade/Downgrade)
// ============================================================================

export type PlanChangeType = 'upgrade' | 'downgrade';

export interface PlanChangePreview {
  prorated_amount_cents: number;
  currency: string;
  period_end: number; // Unix timestamp
  new_plan_name: string;
  new_plan_price_cents: number;
  change_type: PlanChangeType;
  effective_at: number; // Unix timestamp
}

export interface PlanChangeNewPlan {
  code: string;
  name: string;
  price_cents: number;
  currency: string;
  interval: string;
  interval_count: number;
  features: string[];
}

export interface PlanChangeResult {
  success: boolean;
  change_type: PlanChangeType;
  invoice_id: string | null;
  amount_charged_cents: number | null;
  currency: string | null;
  client_secret: string | null;  // For Stripe.js confirmCardPayment()
  hosted_invoice_url: string | null;  // Fallback for redirect flow
  payment_intent_status: 'succeeded' | 'requires_action' | 'requires_payment_method' | null;
  new_plan: PlanChangeNewPlan;
  effective_at: number; // Unix timestamp
  schedule_id: string | null; // For downgrades - can be canceled later
}

// Plan change helper functions
export function getPlanChangeTypeLabel(changeType: PlanChangeType): string {
  return changeType === 'upgrade' ? 'Upgrade' : 'Downgrade';
}

export function formatEffectiveDate(timestamp: number): string {
  return new Date(timestamp * 1000).toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
  });
}

// ============================================================================
// Payment Provider Helper Functions
// ============================================================================

export function getProviderLabel(provider: PaymentProvider): string {
  switch (provider) {
    case 'stripe':
      return 'Stripe';
    case 'dummy':
      return 'Test Provider';
    case 'coinbase':
      return 'Coinbase Commerce';
    default:
      return provider;
  }
}

export function getProviderBadgeColor(provider: PaymentProvider): string {
  switch (provider) {
    case 'stripe':
      return 'purple';
    case 'dummy':
      return 'gray';
    case 'coinbase':
      return 'blue';
    default:
      return 'gray';
  }
}

export function getPaymentModeLabel(mode: PaymentMode): string {
  return mode === 'test' ? 'Test' : 'Live';
}

export function getPaymentModeBadgeColor(mode: PaymentMode): 'yellow' | 'green' {
  return mode === 'test' ? 'yellow' : 'green';
}

export function formatProviderConfig(provider: PaymentProvider, mode: PaymentMode): string {
  const providerLabel = getProviderLabel(provider);
  return mode === 'live' ? providerLabel : `${providerLabel} (Test)`;
}

export function getScenarioLabel(scenario: PaymentScenario): string {
  switch (scenario) {
    case 'success':
      return 'Success';
    case 'decline':
      return 'Card Declined';
    case 'insufficient_funds':
      return 'Insufficient Funds';
    case 'three_d_secure':
      return '3D Secure Required';
    case 'expired_card':
      return 'Expired Card';
    case 'processing_error':
      return 'Processing Error';
    default:
      return scenario;
  }
}

export function getScenarioDescription(scenario: PaymentScenario): string {
  switch (scenario) {
    case 'success':
      return 'Payment will succeed immediately';
    case 'decline':
      return 'Card will be declined by the issuer';
    case 'insufficient_funds':
      return 'Card will fail due to insufficient funds';
    case 'three_d_secure':
      return 'Payment requires 3D Secure authentication';
    case 'expired_card':
      return 'Card is expired and will be rejected';
    case 'processing_error':
      return 'A processing error will occur';
    default:
      return '';
  }
}

export function getBillingStateLabel(state: BillingState): string {
  switch (state) {
    case 'active':
      return 'Active';
    case 'pending_switch':
      return 'Switching Provider';
    case 'switch_failed':
      return 'Switch Failed';
    default:
      return state;
  }
}

export function getBillingStateBadgeColor(state: BillingState): string {
  switch (state) {
    case 'active':
      return 'green';
    case 'pending_switch':
      return 'yellow';
    case 'switch_failed':
      return 'red';
    default:
      return 'gray';
  }
}

// ============================================================================
// Dummy Checkout Types
// ============================================================================

export interface DummyCheckoutPayload {
  plan_code: string;
  scenario: PaymentScenario;
}

export interface DummyCheckoutResponse {
  success: boolean;
  requires_confirmation: boolean;
  confirmation_token: string | null;
  error_message: string | null;
  subscription_id: string | null;
}

export interface DummyConfirmPayload {
  confirmation_token: string;
}
