'use client';

import { useState, useEffect, useCallback } from 'react';
import Link from 'next/link';
import { ArrowLeft, CreditCard, Check, ExternalLink } from 'lucide-react';
import { Card, Button, Badge, HoldButton } from '@/components/ui';
import { useToast } from '@/contexts/ToastContext';
import { useAppContext } from '../layout';
import { getRootDomain } from '@/lib/domain-utils';
import { formatPrice, formatInterval, getStatusLabel } from '@/types/billing';

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
  status: string;
  current_period_end: number | null;
  trial_end: number | null;
  cancel_at_period_end: boolean | null;
};

export default function BillingPage() {
  const { user, displayDomain, isIngress } = useAppContext();
  const { addToast } = useToast();

  const [subscription, setSubscription] = useState<UserSubscription | null>(null);
  const [plans, setPlans] = useState<SubscriptionPlan[]>([]);
  const [loading, setLoading] = useState(true);
  const [subscribing, setSubscribing] = useState<string | null>(null);
  const [canceling, setCanceling] = useState(false);

  const apiDomain = typeof window !== 'undefined' ? getRootDomain(window.location.hostname) : '';

  const fetchData = useCallback(async () => {
    if (!apiDomain) return;
    setLoading(true);
    try {
      const [subRes, plansRes] = await Promise.all([
        fetch(`/api/public/domain/${apiDomain}/billing/subscription`, { credentials: 'include' }),
        fetch(`/api/public/domain/${apiDomain}/billing/plans`, { credentials: 'include' }),
      ]);

      if (subRes.ok) {
        const subData = await subRes.json();
        setSubscription(subData);
      }
      if (plansRes.ok) {
        const plansData = await plansRes.json();
        setPlans(plansData.sort((a: SubscriptionPlan, b: SubscriptionPlan) => a.display_order - b.display_order));
      }
    } catch {
      addToast('Failed to load billing information', 'error');
    } finally {
      setLoading(false);
    }
  }, [apiDomain, addToast]);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  const handleSubscribe = async (planCode: string) => {
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

  const hasActiveSubscription = subscription && subscription.status !== 'none' && subscription.status !== 'canceled';

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

          <div className="flex items-center justify-between p-4 bg-zinc-800/50 rounded-lg">
            <div>
              <div className="flex items-center gap-2 mb-1">
                <span className="font-medium text-white">{subscription.plan_name}</span>
                <Badge variant={
                  subscription.status === 'active' ? 'success' :
                  subscription.status === 'trialing' ? 'info' :
                  subscription.status === 'past_due' ? 'warning' : 'default'
                }>
                  {getStatusLabel(subscription.status as any)}
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

      {/* Available Plans */}
      {plans.length > 0 && (
        <Card className="p-6">
          <h2 className="text-lg font-semibold text-white mb-4">
            {hasActiveSubscription ? 'Change Plan' : 'Choose a Plan'}
          </h2>

          <div className="grid gap-4 sm:grid-cols-2">
            {plans.map((plan) => {
              const isCurrentPlan = subscription?.plan_code === plan.code;
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
                    <Button
                      variant="primary"
                      className="w-full"
                      onClick={() => handleSubscribe(plan.code)}
                      disabled={subscribing === plan.code}
                    >
                      {subscribing === plan.code ? 'Processing...' :
                       hasActiveSubscription ? 'Switch to this plan' : 'Subscribe'}
                    </Button>
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
    </div>
  );
}
