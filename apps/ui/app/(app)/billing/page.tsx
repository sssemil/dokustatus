'use client';

import { useState, useEffect, useCallback } from 'react';
import Link from 'next/link';
import { ArrowLeft, CreditCard, Check, ExternalLink, Receipt, FileText, ChevronLeft, ChevronRight, TestTube } from 'lucide-react';
import { Card, Button, Badge, HoldButton, Table } from '@/components/ui';
import { PlanChangeModal, DummyCheckoutModal } from '@/components/billing';
import { useToast } from '@/contexts/ToastContext';
import { useAppContext } from '../layout';
import { getRootDomain } from '@/lib/domain-utils';
import {
  formatPrice,
  formatInterval,
  getStatusLabel,
  BillingPayment,
  PaginatedPayments,
  getPaymentStatusLabel,
  getPaymentStatusBadgeColor,
  formatPaymentDate,
  EnabledPaymentProvider,
  SubscriptionStatus,
  PaymentProvider,
  getProviderLabel,
  getProviderBadgeColor,
  formatProviderConfig,
} from '@/types/billing';

type SubscriptionPlan = {
  id: string;
  code: string;
  name: string;
  description: string | null;
  price_cents: number;
  currency: string;
  interval: string;
  interval_count: number;
  trial_days: number;
  features: string[];
  display_order: number;
};

type UserSubscription = {
  id: string | null;
  plan_code: string | null;
  plan_name: string | null;
  status: SubscriptionStatus;
  current_period_end: number | null;
  trial_end: number | null;
  cancel_at_period_end: boolean | null;
};

export default function BillingPage() {
  const { user, displayDomain, isIngress } = useAppContext();
  const { addToast } = useToast();

  const [subscription, setSubscription] = useState<UserSubscription | null>(null);
  const [plans, setPlans] = useState<SubscriptionPlan[]>([]);
  const [payments, setPayments] = useState<BillingPayment[]>([]);
  const [paymentsPagination, setPaymentsPagination] = useState({ page: 1, total: 0, total_pages: 0 });
  const [loading, setLoading] = useState(true);
  const [loadingPayments, setLoadingPayments] = useState(false);
  const [subscribing, setSubscribing] = useState<string | null>(null);
  const [canceling, setCanceling] = useState(false);

  // Plan change modal state
  const [planChangeModalOpen, setPlanChangeModalOpen] = useState(false);
  const [selectedPlanForChange, setSelectedPlanForChange] = useState<SubscriptionPlan | null>(null);
  const [stripePublishableKey, setStripePublishableKey] = useState<string | null>(null);

  // Enabled providers and dummy checkout modal
  const [enabledProviders, setEnabledProviders] = useState<EnabledPaymentProvider[]>([]);
  const [dummyCheckoutModalOpen, setDummyCheckoutModalOpen] = useState(false);
  const [selectedPlanForDummy, setSelectedPlanForDummy] = useState<SubscriptionPlan | null>(null);

  const apiDomain = typeof window !== 'undefined' ? getRootDomain(window.location.hostname) : '';

  const fetchPayments = useCallback(async (page: number = 1) => {
    if (!apiDomain) return;
    setLoadingPayments(true);
    try {
      const res = await fetch(
        `/api/public/domain/${apiDomain}/billing/payments?page=${page}&per_page=5`,
        { credentials: 'include' }
      );
      if (res.ok) {
        const data: PaginatedPayments = await res.json();
        setPayments(data.payments);
        setPaymentsPagination({ page: data.page, total: data.total, total_pages: data.total_pages });
      }
    } catch {
      // Silent fail for payments
    } finally {
      setLoadingPayments(false);
    }
  }, [apiDomain]);

  const fetchData = useCallback(async () => {
    if (!apiDomain) return;
    setLoading(true);
    try {
      const [subRes, plansRes, providersRes] = await Promise.all([
        fetch(`/api/public/domain/${apiDomain}/billing/subscription`, { credentials: 'include' }),
        fetch(`/api/public/domain/${apiDomain}/billing/plans`, { credentials: 'include' }),
        fetch(`/api/public/domain/${apiDomain}/billing/providers`, { credentials: 'include' }),
      ]);

      if (subRes.ok) {
        const subData = await subRes.json();
        setSubscription(subData);
      }
      if (plansRes.ok) {
        const plansData = await plansRes.json();
        setPlans(plansData.sort((a: SubscriptionPlan, b: SubscriptionPlan) => a.display_order - b.display_order));
      }
      if (providersRes.ok) {
        const providersData = await providersRes.json();
        setEnabledProviders(providersData.filter((p: EnabledPaymentProvider) => p.is_active));
      }

      // Fetch payments
      await fetchPayments(1);
    } catch {
      addToast('Failed to load billing information', 'error');
    } finally {
      setLoading(false);
    }
  }, [apiDomain, addToast, fetchPayments]);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  const hasActiveSubscription = subscription && subscription.status !== 'none' && subscription.status !== 'canceled';

  // Get current plan for the modal
  const currentPlan = hasActiveSubscription && subscription?.plan_code
    ? plans.find((p) => p.code === subscription.plan_code)
    : null;

  // Provider helpers
  const hasStripeProvider = enabledProviders.some(p => p.provider === 'stripe');
  const hasDummyProvider = enabledProviders.some(p => p.provider === 'dummy');
  const hasMultipleProviders = enabledProviders.length > 1;

  const handleDummyCheckout = (planCode: string) => {
    const plan = plans.find((p) => p.code === planCode);
    if (plan) {
      setSelectedPlanForDummy(plan);
      setDummyCheckoutModalOpen(true);
    }
  };

  const handleDummyCheckoutSuccess = () => {
    addToast('Test subscription created successfully!', 'success');
    fetchData();
  };

  const handleSubscribe = async (planCode: string) => {
    // If user has an active subscription, open the plan change modal instead
    if (hasActiveSubscription) {
      const plan = plans.find((p) => p.code === planCode);
      if (plan) {
        setSelectedPlanForChange(plan);
        setPlanChangeModalOpen(true);
      }
      return;
    }

    // Otherwise, proceed with checkout for new subscription
    setSubscribing(planCode);
    try {
      const currentUrl = window.location.href;
      const res = await fetch(`/api/public/domain/${apiDomain}/billing/checkout`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          plan_code: planCode,
          success_url: `${currentUrl}?success=true`,
          cancel_url: currentUrl,
        }),
        credentials: 'include',
      });

      if (res.ok) {
        const data = await res.json();
        window.location.href = data.checkout_url;
      } else {
        const err = await res.json().catch(() => ({}));
        addToast(err.message || 'Failed to start checkout', 'error');
      }
    } catch {
      addToast('Network error', 'error');
    } finally {
      setSubscribing(null);
    }
  };

  const handlePlanChangeSuccess = () => {
    addToast('Plan changed successfully!', 'success');
    fetchData();
  };

  const handleManageSubscription = async () => {
    try {
      const res = await fetch(`/api/public/domain/${apiDomain}/billing/portal`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ return_url: window.location.href }),
        credentials: 'include',
      });

      if (res.ok) {
        const data = await res.json();
        window.location.href = data.portal_url;
      } else {
        addToast('Failed to open billing portal', 'error');
      }
    } catch {
      addToast('Network error', 'error');
    }
  };

  const handleCancelSubscription = async () => {
    setCanceling(true);
    try {
      const res = await fetch(`/api/public/domain/${apiDomain}/billing/cancel`, {
        method: 'POST',
        credentials: 'include',
      });

      if (res.ok) {
        addToast('Subscription will cancel at period end', 'success');
        fetchData();
      } else {
        addToast('Failed to cancel subscription', 'error');
      }
    } catch {
      addToast('Network error', 'error');
    } finally {
      setCanceling(false);
    }
  };

  const formatPeriodEnd = (timestamp: number | null) => {
    if (!timestamp) return null;
    return new Date(timestamp * 1000).toLocaleDateString('en-US', {
      month: 'long',
      day: 'numeric',
      year: 'numeric',
    });
  };

  if (loading) {
    return (
      <div className="flex justify-center py-20">
        <div className="w-6 h-6 border-2 border-zinc-600 border-t-blue-500 rounded-full animate-spin" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <Link href="/profile" className="inline-flex items-center gap-1 text-sm text-zinc-400 hover:text-white transition-colors mb-2">
            <ArrowLeft size={16} />
            Back to profile
          </Link>
          <h1 className="text-2xl font-bold text-white">Billing</h1>
          <p className="text-sm text-zinc-400 mt-1">{displayDomain}</p>
        </div>
      </div>

      {/* Current Subscription */}
      {hasActiveSubscription && subscription && (
        <Card className="p-6">
          <div className="flex items-center gap-2 mb-4">
            <CreditCard size={20} className="text-purple-400" />
            <h2 className="text-lg font-semibold text-white">Your Subscription</h2>
          </div>

          <div className="flex items-center justify-between p-4 bg-zinc-800/50 rounded-lg border border-zinc-700">
            <div>
              <div className="flex items-center gap-2 mb-1">
                <span className="font-medium text-white">{subscription.plan_name}</span>
                <Badge variant={
                  subscription.status === 'active' ? 'success' :
                  subscription.status === 'trialing' ? 'info' :
                  subscription.status === 'past_due' ? 'warning' : 'default'
                }>
                  {getStatusLabel(subscription.status)}
                </Badge>
              </div>
              <div className="text-sm text-zinc-400">
                {subscription.current_period_end && (
                  <span>
                    {subscription.cancel_at_period_end ? 'Ends' : 'Renews'}{' '}
                    {formatPeriodEnd(subscription.current_period_end)}
                  </span>
                )}
                {subscription.trial_end && subscription.status === 'trialing' && (
                  <span className="ml-2 text-blue-400">Trial ends {formatPeriodEnd(subscription.trial_end)}</span>
                )}
              </div>
            </div>
            <div className="flex items-center gap-2">
              <Button variant="ghost" onClick={handleManageSubscription}>
                <ExternalLink size={14} className="mr-1" />
                Manage
              </Button>
              {!subscription.cancel_at_period_end && (
                <HoldButton onComplete={handleCancelSubscription} variant="danger" duration={2000} disabled={canceling}>
                  {canceling ? 'Canceling...' : 'Cancel'}
                </HoldButton>
              )}
            </div>
          </div>

          {subscription.cancel_at_period_end && (
            <div className="mt-4 p-3 bg-yellow-500/10 border border-yellow-500/20 rounded-lg">
              <p className="text-sm text-yellow-200">
                Your subscription will be canceled at the end of your current billing period.
                You will retain access until {formatPeriodEnd(subscription.current_period_end)}.
              </p>
            </div>
          )}
        </Card>
      )}

      {/* Payment History */}
      {payments.length > 0 && (
        <Card className="p-6">
          <div className="flex items-center gap-2 mb-4">
            <Receipt size={20} className="text-purple-400" />
            <h2 className="text-lg font-semibold text-white">Payment History</h2>
          </div>

          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-zinc-700">
                  <th className="text-left py-3 px-2 text-zinc-400 font-medium">Date</th>
                  <th className="text-left py-3 px-2 text-zinc-400 font-medium">Plan</th>
                  <th className="text-left py-3 px-2 text-zinc-400 font-medium">Provider</th>
                  <th className="text-left py-3 px-2 text-zinc-400 font-medium">Amount</th>
                  <th className="text-left py-3 px-2 text-zinc-400 font-medium">Status</th>
                  <th className="text-right py-3 px-2 text-zinc-400 font-medium">Invoice</th>
                </tr>
              </thead>
              <tbody>
                {payments.map((payment) => (
                  <tr key={payment.id} className="border-b border-zinc-800 last:border-0">
                    <td className="py-3 px-2 text-white">
                      {formatPaymentDate(payment.payment_date || payment.created_at)}
                    </td>
                    <td className="py-3 px-2 text-zinc-300">
                      {payment.plan_name || '-'}
                    </td>
                    <td className="py-3 px-2">
                      {payment.payment_provider ? (
                        <Badge variant={
                          payment.payment_provider === 'stripe' ? 'default' :
                          payment.payment_provider === 'dummy' ? 'warning' : 'info'
                        }>
                          {payment.payment_provider === 'dummy' && <TestTube size={12} className="mr-1" />}
                          {formatProviderConfig(payment.payment_provider, payment.payment_mode || 'test')}
                        </Badge>
                      ) : (
                        <span className="text-zinc-500">-</span>
                      )}
                    </td>
                    <td className="py-3 px-2 text-white">
                      {formatPrice(payment.amount_cents, payment.currency)}
                      {payment.amount_refunded_cents > 0 && (
                        <span className="text-xs text-blue-400 ml-1">
                          (-{formatPrice(payment.amount_refunded_cents, payment.currency)})
                        </span>
                      )}
                    </td>
                    <td className="py-3 px-2">
                      <Badge variant={
                        getPaymentStatusBadgeColor(payment.status) === 'green' ? 'success' :
                        getPaymentStatusBadgeColor(payment.status) === 'red' ? 'error' :
                        getPaymentStatusBadgeColor(payment.status) === 'yellow' ? 'warning' :
                        getPaymentStatusBadgeColor(payment.status) === 'blue' ? 'info' : 'default'
                      }>
                        {getPaymentStatusLabel(payment.status)}
                      </Badge>
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
                Page {paymentsPagination.page} of {paymentsPagination.total_pages}
              </span>
              <div className="flex gap-2">
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={paymentsPagination.page <= 1 || loadingPayments}
                  onClick={() => fetchPayments(paymentsPagination.page - 1)}
                >
                  <ChevronLeft size={14} />
                  Previous
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={paymentsPagination.page >= paymentsPagination.total_pages || loadingPayments}
                  onClick={() => fetchPayments(paymentsPagination.page + 1)}
                >
                  Next
                  <ChevronRight size={14} />
                </Button>
              </div>
            </div>
          )}
        </Card>
      )}

      {/* Available Plans */}
      {plans.length > 0 && (
        <Card className="p-6">
          <h2 className="text-lg font-semibold text-white mb-4">
            {hasActiveSubscription ? 'Change Plan' : 'Choose a Plan'}
          </h2>

          <div className="grid gap-4 sm:grid-cols-2">
            {plans.map((plan) => {
              // Only show as "current" if subscription is active (not canceled/none)
              const isCurrentPlan = hasActiveSubscription && subscription?.plan_code === plan.code;
              return (
                <div
                  key={plan.id}
                  className={`
                    p-5 rounded-lg border-2 transition-all
                    ${isCurrentPlan
                      ? 'border-blue-500 bg-blue-500/10'
                      : 'border-zinc-700 hover:border-zinc-600 bg-zinc-800/50'
                    }
                  `}
                >
                  <div className="flex items-start justify-between mb-3">
                    <div>
                      <h3 className="font-semibold text-white">{plan.name}</h3>
                      {plan.description && (
                        <p className="text-sm text-zinc-400 mt-1">{plan.description}</p>
                      )}
                    </div>
                    {isCurrentPlan && (
                      <Badge variant="info">Current</Badge>
                    )}
                  </div>

                  <div className="mb-4">
                    <span className="text-2xl font-bold text-white">{formatPrice(plan.price_cents)}</span>
                    <span className="text-zinc-400 ml-1">{formatInterval(plan.interval, plan.interval_count)}</span>
                    {plan.trial_days > 0 && (
                      <div className="text-sm text-blue-400 mt-1">{plan.trial_days} day free trial</div>
                    )}
                  </div>

                  {plan.features.length > 0 && (
                    <ul className="space-y-2 mb-4">
                      {plan.features.map((feature, i) => (
                        <li key={i} className="flex items-center gap-2 text-sm text-zinc-300">
                          <Check size={14} className="text-green-400 flex-shrink-0" />
                          {feature}
                        </li>
                      ))}
                    </ul>
                  )}

                  {!isCurrentPlan && (
                    <div className="space-y-2">
                      {/* Show Stripe button if enabled */}
                      {hasStripeProvider && (
                        <Button
                          variant="primary"
                          className="w-full"
                          onClick={() => handleSubscribe(plan.code)}
                          disabled={subscribing === plan.code}
                        >
                          {subscribing === plan.code ? 'Processing...' :
                           hasActiveSubscription && currentPlan
                             ? (plan.price_cents > currentPlan.price_cents ? 'Upgrade' : 'Downgrade')
                             : hasMultipleProviders ? 'Pay with Stripe' : 'Subscribe'}
                        </Button>
                      )}
                      {/* Show dummy button if enabled */}
                      {hasDummyProvider && (
                        <Button
                          variant={hasStripeProvider ? 'ghost' : 'primary'}
                          className="w-full"
                          onClick={() => handleDummyCheckout(plan.code)}
                        >
                          <TestTube size={14} className="mr-1" />
                          {hasMultipleProviders ? 'Test Payment' : 'Subscribe (Test)'}
                        </Button>
                      )}
                      {/* Fallback if no providers configured */}
                      {!hasStripeProvider && !hasDummyProvider && (
                        <Button
                          variant="primary"
                          className="w-full"
                          onClick={() => handleSubscribe(plan.code)}
                          disabled={subscribing === plan.code}
                        >
                          {subscribing === plan.code ? 'Processing...' : 'Subscribe'}
                        </Button>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </Card>
      )}

      {/* No Plans Available */}
      {plans.length === 0 && !hasActiveSubscription && (
        <Card className="p-8 text-center">
          <CreditCard size={48} className="mx-auto text-zinc-600 mb-4" />
          <h3 className="font-semibold text-white mb-2">No plans available</h3>
          <p className="text-sm text-zinc-400">
            Subscription plans have not been configured for this domain yet.
          </p>
        </Card>
      )}

      {/* Plan Change Modal */}
      {currentPlan && selectedPlanForChange && (
        <PlanChangeModal
          open={planChangeModalOpen}
          onClose={() => {
            setPlanChangeModalOpen(false);
            setSelectedPlanForChange(null);
          }}
          currentPlan={currentPlan}
          newPlan={selectedPlanForChange}
          apiDomain={apiDomain}
          stripePublishableKey={stripePublishableKey}
          onSuccess={handlePlanChangeSuccess}
        />
      )}

      {/* Dummy Checkout Modal */}
      {selectedPlanForDummy && (
        <DummyCheckoutModal
          open={dummyCheckoutModalOpen}
          onClose={() => {
            setDummyCheckoutModalOpen(false);
            setSelectedPlanForDummy(null);
          }}
          plan={selectedPlanForDummy}
          apiDomain={apiDomain}
          onSuccess={handleDummyCheckoutSuccess}
        />
      )}
    </div>
  );
}
