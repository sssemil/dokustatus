import { createHmac, timingSafeEqual } from "node:crypto";

const SIGNATURE_HEADER = "reauth-webhook-signature";
const ID_HEADER = "reauth-webhook-id";
const TIMESTAMP_HEADER = "reauth-webhook-timestamp";
const DEFAULT_TOLERANCE_SECONDS = 300; // 5 minutes

export type WebhookEvent = {
  id: string;
  type: string;
  api_version: string;
  created_at: string;
  domain_id: string;
  data: Record<string, unknown>;
};

export type VerifyWebhookOptions = {
  payload: string;
  signature: string;
  secret: string;
  tolerance?: number;
};

export class WebhookVerificationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "WebhookVerificationError";
  }
}

function parseSignatureHeader(header: string): { timestamp: string; signatures: string[] } {
  const parts = header.split(",");
  let timestamp = "";
  const signatures: string[] = [];

  for (const part of parts) {
    const [key, value] = part.split("=", 2);
    if (key === "t") {
      timestamp = value;
    } else if (key === "v1") {
      signatures.push(value);
    }
  }

  return { timestamp, signatures };
}

function computeSignature(secret: string, timestamp: string, payload: string): string {
  const signedPayload = `${timestamp}.${payload}`;
  return createHmac("sha256", secret).update(signedPayload).digest("hex");
}

export function verifyWebhookSignature(options: VerifyWebhookOptions): WebhookEvent {
  const { payload, signature, secret, tolerance = DEFAULT_TOLERANCE_SECONDS } = options;

  if (!signature) {
    throw new WebhookVerificationError("Missing webhook signature header");
  }

  if (!secret) {
    throw new WebhookVerificationError("Missing webhook secret");
  }

  // Strip whsec_ prefix if present
  const rawSecret = secret.startsWith("whsec_") ? secret.slice(6) : secret;

  const { timestamp, signatures } = parseSignatureHeader(signature);

  if (!timestamp || signatures.length === 0) {
    throw new WebhookVerificationError(
      "Invalid signature header format: expected t=<timestamp>,v1=<signature>"
    );
  }

  // Validate timestamp
  const timestampSeconds = parseInt(timestamp, 10);
  if (isNaN(timestampSeconds)) {
    throw new WebhookVerificationError("Invalid timestamp in signature header");
  }

  const now = Math.floor(Date.now() / 1000);
  const age = now - timestampSeconds;

  if (age > tolerance) {
    throw new WebhookVerificationError(
      `Webhook timestamp too old: ${age}s exceeds tolerance of ${tolerance}s`
    );
  }

  if (age < -tolerance) {
    throw new WebhookVerificationError(
      `Webhook timestamp too far in the future: ${-age}s exceeds tolerance of ${tolerance}s`
    );
  }

  // Compute expected signature
  const expectedSignature = computeSignature(rawSecret, timestamp, payload);

  // Timing-safe comparison against each v1 signature
  const expectedBuffer = Buffer.from(expectedSignature, "hex");
  let verified = false;

  for (const sig of signatures) {
    const sigBuffer = Buffer.from(sig, "hex");
    if (sigBuffer.length === expectedBuffer.length && timingSafeEqual(sigBuffer, expectedBuffer)) {
      verified = true;
      break;
    }
  }

  if (!verified) {
    throw new WebhookVerificationError("Webhook signature verification failed");
  }

  // Parse and return the event
  try {
    const event = JSON.parse(payload) as WebhookEvent;
    return event;
  } catch {
    throw new WebhookVerificationError("Failed to parse webhook payload as JSON");
  }
}
