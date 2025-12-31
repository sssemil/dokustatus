import type {
  ReauthSession, ReauthConfig, SubscriptionPlan, UserSubscription,
  CheckoutSession, PortalSession
} from './types';

/**
 * Create a reauth client for browser-side authentication.
 *
 * @example
 * ```typescript
 * const reauth = createReauthClient({ domain: 'yourdomain.com' });
 *
 * // Check if user is authenticated
 * const session = await reauth.getSession();
 * if (!session.valid) {
 *   reauth.login(); // Redirect to login page
 * }
 *
 * // Log out
 * await reauth.logout();
 * ```
 */
export function createReauthClient(config: ReauthConfig) {
  const { domain } = config;
  const baseUrl = `https://reauth.${domain}/api/public/domain/${domain}`;

  return {
    /**
     * Redirect the user to the reauth.dev login page.
     * After successful login, they'll be redirected back to your configured redirect URL.
     */
    login(): void {
      if (typeof window === 'undefined') {
        throw new Error('login() can only be called in browser');
      }
      window.location.href = `https://reauth.${domain}/`;
    },

    /**
     * Check if the user is authenticated.
     * Returns session info including user ID, email, and roles.
     */
    async getSession(): Promise<ReauthSession> {
      const res = await fetch(`${baseUrl}/auth/session`, {
        credentials: 'include',
      });
      return res.json();
    },

    /**
     * Refresh the access token using the refresh token.
     * Call this when getSession() returns valid: false but no error_code.
     * @returns true if refresh succeeded, false otherwise
     */
    async refresh(): Promise<boolean> {
      const res = await fetch(`${baseUrl}/auth/refresh`, {
        method: 'POST',
        credentials: 'include',
      });
      return res.ok;
    },

    /**
     * Log out the user by clearing all session cookies.
     */
    async logout(): Promise<void> {
      await fetch(`${baseUrl}/auth/logout`, {
        method: 'POST',
        credentials: 'include',
      });
    },

    /**
     * Delete the user's own account (self-service).
     * @returns true if deletion succeeded, false otherwise
     */
    async deleteAccount(): Promise<boolean> {
      const res = await fetch(`${baseUrl}/auth/account`, {
        method: 'DELETE',
        credentials: 'include',
      });
      return res.ok;
    },

    // ========================================================================
    // Billing Methods
    // ========================================================================

    /**
     * Get available subscription plans for the domain.
     * Only returns public plans sorted by display order.
     */
    async getPlans(): Promise<SubscriptionPlan[]> {
      const res = await fetch(`${baseUrl}/billing/plans`, {
        credentials: 'include',
      });
      if (!res.ok) return [];
      const data = await res.json();
      return data.map((p: any) => ({
        id: p.id,
        code: p.code,
        name: p.name,
        description: p.description,
        priceCents: p.price_cents,
        currency: p.currency,
        interval: p.interval,
        intervalCount: p.interval_count,
        trialDays: p.trial_days,
        features: p.features,
        displayOrder: p.display_order,
      }));
    },

    /**
     * Get the current user's subscription status.
     */
    async getSubscription(): Promise<UserSubscription> {
      const res = await fetch(`${baseUrl}/billing/subscription`, {
        credentials: 'include',
      });
      const data = await res.json();
      return {
        id: data.id,
        planCode: data.plan_code,
        planName: data.plan_name,
        status: data.status,
        currentPeriodEnd: data.current_period_end,
        trialEnd: data.trial_end,
        cancelAtPeriodEnd: data.cancel_at_period_end,
      };
    },

    /**
     * Create a Stripe checkout session to subscribe to a plan.
     * @param planCode The plan code to subscribe to
     * @param successUrl URL to redirect to after successful payment
     * @param cancelUrl URL to redirect to if checkout is canceled
     * @returns Checkout session with URL to redirect the user to
     */
    async createCheckout(
      planCode: string,
      successUrl: string,
      cancelUrl: string
    ): Promise<CheckoutSession> {
      const res = await fetch(`${baseUrl}/billing/checkout`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
        body: JSON.stringify({
          plan_code: planCode,
          success_url: successUrl,
          cancel_url: cancelUrl,
        }),
      });
      if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        throw new Error(err.message || 'Failed to create checkout session');
      }
      const data = await res.json();
      return { checkoutUrl: data.checkout_url };
    },

    /**
     * Redirect user to subscribe to a plan.
     * Creates a checkout session and redirects to Stripe.
     * @param planCode The plan code to subscribe to
     */
    async subscribe(planCode: string): Promise<void> {
      if (typeof window === 'undefined') {
        throw new Error('subscribe() can only be called in browser');
      }
      const currentUrl = window.location.href;
      const { checkoutUrl } = await this.createCheckout(
        planCode,
        currentUrl,
        currentUrl
      );
      window.location.href = checkoutUrl;
    },

    /**
     * Open the Stripe customer portal for managing subscription.
     * @param returnUrl URL to return to after leaving the portal
     */
    async openBillingPortal(returnUrl?: string): Promise<void> {
      if (typeof window === 'undefined') {
        throw new Error('openBillingPortal() can only be called in browser');
      }
      const res = await fetch(`${baseUrl}/billing/portal`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
        body: JSON.stringify({
          return_url: returnUrl || window.location.href,
        }),
      });
      if (!res.ok) {
        throw new Error('Failed to open billing portal');
      }
      const data = await res.json();
      window.location.href = data.portal_url;
    },

    /**
     * Cancel the user's subscription at period end.
     * @returns true if cancellation succeeded
     */
    async cancelSubscription(): Promise<boolean> {
      const res = await fetch(`${baseUrl}/billing/cancel`, {
        method: 'POST',
        credentials: 'include',
      });
      return res.ok;
    },
  };
}

export type ReauthClient = ReturnType<typeof createReauthClient>;
