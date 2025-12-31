-- ============================================================================
-- Billing Payments - Payment History Storage
-- ============================================================================
-- Stores invoice/payment data synced from Stripe webhooks.
-- Enables payment history views for both end-users and domain owners.

-- Payment status enum
CREATE TYPE payment_status AS ENUM (
    'pending',         -- Invoice created, payment not yet attempted
    'paid',            -- Successfully paid
    'failed',          -- Payment attempt failed
    'refunded',        -- Fully refunded
    'partial_refund',  -- Partially refunded
    'uncollectible',   -- Marked as uncollectible
    'void'             -- Invoice voided
);

-- Billing payments table
CREATE TABLE billing_payments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    stripe_mode stripe_mode NOT NULL,
    end_user_id UUID NOT NULL REFERENCES domain_end_users(id) ON DELETE CASCADE,
    subscription_id UUID REFERENCES user_subscriptions(id) ON DELETE SET NULL,

    -- Stripe identifiers
    stripe_invoice_id TEXT NOT NULL,
    stripe_payment_intent_id TEXT,
    stripe_customer_id TEXT NOT NULL,

    -- Payment details
    amount_cents INTEGER NOT NULL,
    amount_paid_cents INTEGER NOT NULL DEFAULT 0,
    amount_refunded_cents INTEGER NOT NULL DEFAULT 0,
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    status payment_status NOT NULL DEFAULT 'pending',

    -- Plan info (denormalized for historical record - preserves data even if plan is renamed/deleted)
    plan_id UUID REFERENCES subscription_plans(id) ON DELETE SET NULL,
    plan_code VARCHAR(50),
    plan_name VARCHAR(100),

    -- Invoice details from Stripe
    hosted_invoice_url TEXT,
    invoice_pdf_url TEXT,
    invoice_number TEXT,
    billing_reason TEXT,  -- subscription_create, subscription_cycle, subscription_update, etc.

    -- Failure reason (for failed payments)
    failure_message TEXT,

    -- Timestamps
    invoice_created_at TIMESTAMP,
    payment_date TIMESTAMP,
    refunded_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,

    -- Unique constraint scoped to domain and mode - ensures tenant isolation
    -- while preventing duplicates from webhook retries within the same domain/mode
    UNIQUE(domain_id, stripe_mode, stripe_invoice_id)
);

-- Indexes for efficient queries
CREATE INDEX idx_billing_payments_domain_mode ON billing_payments(domain_id, stripe_mode);
CREATE INDEX idx_billing_payments_user ON billing_payments(end_user_id);
CREATE INDEX idx_billing_payments_subscription ON billing_payments(subscription_id) WHERE subscription_id IS NOT NULL;
CREATE INDEX idx_billing_payments_status ON billing_payments(status);
CREATE INDEX idx_billing_payments_date ON billing_payments(payment_date DESC NULLS LAST);
CREATE INDEX idx_billing_payments_created ON billing_payments(created_at DESC);
CREATE INDEX idx_billing_payments_stripe_customer ON billing_payments(stripe_customer_id);

-- Composite index for common dashboard query (domain + mode + filters)
CREATE INDEX idx_billing_payments_domain_mode_date ON billing_payments(domain_id, stripe_mode, payment_date DESC NULLS LAST);

-- Updated at trigger
CREATE OR REPLACE FUNCTION set_billing_payments_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_billing_payments_set_updated_at
BEFORE UPDATE ON billing_payments
FOR EACH ROW
EXECUTE FUNCTION set_billing_payments_updated_at();
