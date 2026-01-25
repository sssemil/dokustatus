"use client";

import { useState, useEffect } from "react";
import { loadStripe } from "@stripe/stripe-js";
import { Modal, Button, Badge } from "@/components/ui";
import {
  PlanChangePreview,
  PlanChangeResult,
  formatPrice,
  formatEffectiveDate,
} from "@/types/billing";
import {
  AlertTriangle,
  ArrowUp,
  ArrowDown,
  Loader2,
  CreditCard,
  ExternalLink,
} from "lucide-react";

interface Plan {
  code: string;
  name: string;
  price_cents: number;
  currency: string;
  interval: string;
  interval_count: number;
  features: string[];
}

interface PlanChangeModalProps {
  open: boolean;
  onClose: () => void;
  currentPlan: Plan;
  newPlan: Plan;
  apiDomain: string;
  stripePublishableKey: string | null;
  onSuccess: () => void;
}

type ModalState =
  | "loading"
  | "preview"
  | "processing"
  | "requires_action"
  | "success"
  | "error";

export function PlanChangeModal({
  open,
  onClose,
  currentPlan,
  newPlan,
  apiDomain,
  stripePublishableKey,
  onSuccess,
}: PlanChangeModalProps) {
  const [state, setState] = useState<ModalState>("loading");
  const [preview, setPreview] = useState<PlanChangePreview | null>(null);
  const [result, setResult] = useState<PlanChangeResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const isUpgrade = newPlan.price_cents > currentPlan.price_cents;
  const changeType = isUpgrade ? "upgrade" : "downgrade";

  // Fetch preview when modal opens
  useEffect(() => {
    if (!open) {
      // Reset state when modal closes
      setState("loading");
      setPreview(null);
      setResult(null);
      setError(null);
      return;
    }

    const fetchPreview = async () => {
      setState("loading");
      setError(null);
      try {
        const res = await fetch(
          `/api/public/domain/${apiDomain}/billing/plan-change/preview?plan_code=${newPlan.code}`,
          { credentials: "include" },
        );

        if (res.ok) {
          const data: PlanChangePreview = await res.json();
          setPreview(data);
          setState("preview");
        } else {
          const err = await res.json().catch(() => ({}));
          setError(err.message || "Failed to load preview");
          setState("error");
        }
      } catch {
        setError("Network error");
        setState("error");
      }
    };

    fetchPreview();
  }, [open, apiDomain, newPlan.code]);

  const handleConfirm = async () => {
    setState("processing");
    setError(null);

    try {
      const idempotencyKey = crypto.randomUUID();
      const res = await fetch(
        `/api/public/domain/${apiDomain}/billing/plan-change`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "Idempotency-Key": idempotencyKey,
          },
          body: JSON.stringify({ plan_code: newPlan.code }),
          credentials: "include",
        },
      );

      const data: PlanChangeResult = await res.json();

      if (!res.ok) {
        setError(
          (data as { message?: string }).message || "Failed to change plan",
        );
        setState("error");
        return;
      }

      setResult(data);

      // Handle different outcomes
      if (data.change_type === "downgrade") {
        // Downgrade scheduled successfully
        setState("success");
      } else if (data.payment_intent_status === "succeeded") {
        // Upgrade completed immediately
        setState("success");
      } else if (
        data.payment_intent_status === "requires_action" &&
        data.client_secret
      ) {
        // SCA/3DS required - use Stripe.js
        if (!stripePublishableKey) {
          // Fallback to hosted invoice URL
          if (data.hosted_invoice_url) {
            window.location.href = data.hosted_invoice_url;
          } else {
            setError("Payment requires authentication. Please try again.");
            setState("error");
          }
          return;
        }

        setState("requires_action");

        // Load Stripe and confirm payment
        const stripe = await loadStripe(stripePublishableKey);
        if (!stripe) {
          setError("Failed to load payment processor");
          setState("error");
          return;
        }

        const { error: stripeError } = await stripe.confirmCardPayment(
          data.client_secret,
        );

        if (stripeError) {
          setError(stripeError.message || "Payment failed");
          setState("error");
        } else {
          setState("success");
        }
      } else if (data.payment_intent_status === "requires_payment_method") {
        // No valid payment method
        setError(
          "Your payment method was declined. Please update your payment method and try again.",
        );
        setState("error");
      } else {
        // Unknown state - treat as success since request succeeded
        setState("success");
      }
    } catch {
      setError("Network error");
      setState("error");
    }
  };

  const handleClose = () => {
    if (state === "success") {
      onSuccess();
    }
    onClose();
  };

  const renderContent = () => {
    switch (state) {
      case "loading":
        return (
          <div className="flex flex-col items-center py-8">
            <Loader2 className="w-8 h-8 text-blue-400 animate-spin" />
            <p className="mt-4 text-zinc-400">Loading preview...</p>
          </div>
        );

      case "preview":
        return (
          <div className="space-y-4">
            {/* Change summary */}
            <div className="p-4 bg-zinc-800/50 rounded-lg border border-zinc-700">
              <div className="flex items-center gap-2 mb-3">
                {isUpgrade ? (
                  <ArrowUp className="w-5 h-5 text-green-400" />
                ) : (
                  <ArrowDown className="w-5 h-5 text-yellow-400" />
                )}
                <span className="font-medium text-white">
                  {isUpgrade ? "Upgrade" : "Downgrade"} to {newPlan.name}
                </span>
              </div>

              <div className="space-y-2 text-sm">
                <div className="flex justify-between text-zinc-400">
                  <span>From</span>
                  <span>
                    {currentPlan.name} (
                    {formatPrice(currentPlan.price_cents, currentPlan.currency)}
                    )
                  </span>
                </div>
                <div className="flex justify-between text-zinc-400">
                  <span>To</span>
                  <span>
                    {newPlan.name} (
                    {formatPrice(newPlan.price_cents, newPlan.currency)})
                  </span>
                </div>
              </div>
            </div>

            {/* Upgrade specifics */}
            {isUpgrade && preview && (
              <div className="p-4 bg-blue-500/10 border border-blue-500/30 rounded-lg">
                <div className="flex items-center gap-2 mb-2">
                  <CreditCard className="w-4 h-4 text-blue-400" />
                  <span className="font-medium text-blue-300">
                    Amount due today
                  </span>
                </div>
                <p className="text-2xl font-bold text-white">
                  {formatPrice(preview.prorated_amount_cents, preview.currency)}
                </p>
                <p className="text-sm text-zinc-400 mt-1">
                  Prorated for remaining time in your billing period
                </p>
                {preview.period_end > 0 && (
                  <p className="text-sm text-zinc-400 mt-2">
                    Next full payment on{" "}
                    {formatEffectiveDate(preview.period_end)}
                  </p>
                )}
              </div>
            )}

            {/* Downgrade specifics */}
            {!isUpgrade && preview && (
              <div className="p-4 bg-yellow-500/10 border border-yellow-500/30 rounded-lg space-y-2">
                <div className="flex items-center gap-2">
                  <AlertTriangle className="w-4 h-4 text-yellow-400" />
                  <span className="font-medium text-yellow-300">
                    Scheduled change
                  </span>
                </div>
                <ul className="text-sm text-zinc-300 space-y-1 ml-6 list-disc">
                  <li>
                    Your {currentPlan.name} access continues until{" "}
                    {formatEffectiveDate(preview.effective_at)}
                  </li>
                  <li>
                    On {formatEffectiveDate(preview.effective_at)}, you'll be
                    charged {formatPrice(newPlan.price_cents, newPlan.currency)}{" "}
                    for {newPlan.name}
                  </li>
                  <li>No refund for the current billing period</li>
                </ul>
              </div>
            )}

            {/* Actions */}
            <div className="flex gap-3 pt-2">
              <Button variant="ghost" onClick={onClose} className="flex-1">
                Cancel
              </Button>
              <Button
                variant="primary"
                onClick={handleConfirm}
                className="flex-1"
              >
                {isUpgrade ? "Upgrade Now" : "Confirm Downgrade"}
              </Button>
            </div>
          </div>
        );

      case "processing":
      case "requires_action":
        return (
          <div className="flex flex-col items-center py-8">
            <Loader2 className="w-8 h-8 text-blue-400 animate-spin" />
            <p className="mt-4 text-zinc-400">
              {state === "requires_action"
                ? "Completing payment..."
                : "Processing..."}
            </p>
          </div>
        );

      case "success":
        return (
          <div className="space-y-4">
            <div className="p-4 bg-green-500/10 border border-green-500/30 rounded-lg text-center">
              <div className="text-green-400 text-4xl mb-2">✓</div>
              <p className="font-medium text-green-300">
                {result?.change_type === "upgrade"
                  ? "Plan upgraded successfully!"
                  : "Plan change scheduled!"}
              </p>
              {result?.change_type === "downgrade" && result.effective_at && (
                <p className="text-sm text-zinc-400 mt-2">
                  Your plan will change on{" "}
                  {formatEffectiveDate(result.effective_at)}
                </p>
              )}
              {result?.amount_charged_cents && result.currency && (
                <p className="text-sm text-zinc-400 mt-2">
                  Charged:{" "}
                  {formatPrice(result.amount_charged_cents, result.currency)}
                </p>
              )}
            </div>

            <Button variant="primary" onClick={handleClose} className="w-full">
              Done
            </Button>
          </div>
        );

      case "error":
        return (
          <div className="space-y-4">
            <div className="p-4 bg-red-500/10 border border-red-500/30 rounded-lg">
              <div className="flex items-center gap-2 mb-2">
                <AlertTriangle className="w-4 h-4 text-red-400" />
                <span className="font-medium text-red-300">
                  Failed to change plan
                </span>
              </div>
              <p className="text-sm text-zinc-300">{error}</p>
            </div>

            <div className="flex gap-3">
              <Button variant="ghost" onClick={onClose} className="flex-1">
                Close
              </Button>
              <Button
                variant="primary"
                onClick={() => {
                  setState("loading");
                }}
                className="flex-1"
              >
                Try Again
              </Button>
            </div>
          </div>
        );
    }
  };

  const title = isUpgrade
    ? `Upgrade to ${newPlan.name}`
    : `Downgrade to ${newPlan.name}`;

  return (
    <Modal
      open={open}
      onClose={
        state === "processing" || state === "requires_action"
          ? () => {}
          : handleClose
      }
      title={title}
      size="md"
    >
      {renderContent()}
    </Modal>
  );
}
