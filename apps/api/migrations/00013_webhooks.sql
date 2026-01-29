-- Webhooks: outbound HTTP callbacks for domain owners on user lifecycle and billing events

-- ============================================================================
-- Webhook Endpoints (per-domain webhook URL configurations)
-- ============================================================================

CREATE TABLE webhook_endpoints (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    description VARCHAR(200),
    secret_encrypted TEXT NOT NULL,
    event_types JSONB NOT NULL DEFAULT '["*"]'::jsonb,
    is_active BOOLEAN NOT NULL DEFAULT true,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    last_success_at TIMESTAMP,
    last_failure_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_webhook_endpoints_domain_id ON webhook_endpoints(domain_id);
CREATE INDEX idx_webhook_endpoints_active ON webhook_endpoints(domain_id, is_active) WHERE is_active = true;

CREATE OR REPLACE FUNCTION set_webhook_endpoints_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_webhook_endpoints_set_updated_at
BEFORE UPDATE ON webhook_endpoints
FOR EACH ROW
EXECUTE FUNCTION set_webhook_endpoints_updated_at();

-- ============================================================================
-- Webhook Events (immutable outbox of emitted events)
-- ============================================================================

CREATE TABLE webhook_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    event_type VARCHAR(60) NOT NULL,
    payload JSONB NOT NULL,
    payload_raw TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_webhook_events_domain_created ON webhook_events(domain_id, created_at DESC);
CREATE INDEX idx_webhook_events_event_type ON webhook_events(event_type);

-- ============================================================================
-- Webhook Deliveries (per-endpoint delivery tracking with retry state)
-- ============================================================================

CREATE TYPE webhook_delivery_status AS ENUM ('pending', 'in_progress', 'succeeded', 'failed', 'abandoned');

CREATE TABLE webhook_deliveries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    webhook_event_id UUID NOT NULL REFERENCES webhook_events(id) ON DELETE CASCADE,
    webhook_endpoint_id UUID NOT NULL REFERENCES webhook_endpoints(id) ON DELETE CASCADE,
    status webhook_delivery_status NOT NULL DEFAULT 'pending',
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    locked_at TIMESTAMP,
    last_response_status INTEGER,
    last_response_body TEXT,
    last_error TEXT,
    completed_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_webhook_deliveries_pending ON webhook_deliveries(next_attempt_at)
    WHERE status IN ('pending', 'in_progress');
CREATE INDEX idx_webhook_deliveries_event ON webhook_deliveries(webhook_event_id);
CREATE INDEX idx_webhook_deliveries_endpoint ON webhook_deliveries(webhook_endpoint_id);
