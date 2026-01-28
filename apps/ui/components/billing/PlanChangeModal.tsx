"use client";

import { useState, useEffect } from "react";
import { loadStripe } from "@stripe/stripe-js";
import { Modal, Button } from "@/components/ui";
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
  ArrowLeftRight,
  Loader2,
  CreditCard,
  Calendar,
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

  const localIsUpgrade = newPlan.price_cents > currentPlan.price_cents;

  const isUpgrade = preview
    ? preview.change_type === "upgrade"
    : localIsUpgrade;
  const isLateral = preview?.change_type === "lateral";
  const isScheduled = preview
    ? preview.effective_at === preview.period_end
    : false;
  const isImmediate = preview ? !isScheduled : false;

  useEffect(() => {
    if (!open) {
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

      if (data.schedule_id) {
        setState("success");
      } else if (data.payment_intent_status === "succeeded") {
        setState("success");
      } else if (
        data.payment_intent_status === "requires_action" &&
        data.client_secret
      ) {
        if (!stripePublishableKey) {
          if (data.hosted_invoice_url) {
            window.location.href = data.hosted_invoice_url;
          } else {
            setError("Payment requires authentication. Please try again.");
            setState("error");
          }
          return;
        }

        setState("requires_action");

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
        setError(
          "Your payment method was declined. Please update your payment method and try again.",
        );
        setState("error");
      } else {
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

  const getChangeIcon = () => {
    if (isUpgrade) return <ArrowUp className="w-4 h-4 text-green-400" />;
    if (isLateral)
      return <ArrowLeftRight className="w-4 h-4 text-blue-400" />;
    return <ArrowDown className="w-4 h-4 text-yellow-400" />;
  };

  const getConfirmLabel = () => {
    if (isUpgrade) return "Upgrade Now";
    if (isImmediate) return "Confirm & End Trial";
    if (isLateral) return "Confirm Change";
    return "Confirm Downgrade";
  };

  const getTitle = () => {
    if (isUpgrade) return `Upgrade to ${newPlan.name}`;
    if (isLateral) return `Change to ${newPlan.name}`;
    return `Downgrade to ${newPlan.name}`;
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
            {/* From → To summary */}
            <div className="p-4 bg-zinc-800/50 rounded-lg border border-zinc-700">
              <div className="space-y-2 text-sm">
                <div className="flex justify-between text-zinc-400">
                  <span className="flex items-center gap-2">
                    {getChangeIcon()}
                    From
                  </span>
                  <span>
                    {currentPlan.name} (
                    {formatPrice(currentPlan.price_cents, currentPlan.currency)}
                    )
                  </span>
                </div>
                <div className="flex justify-between text-zinc-400">
                  <span className="ml-6">To</span>
                  <span>
                    {newPlan.name} (
                    {formatPrice(newPlan.price_cents, newPlan.currency)})
                  </span>
                </div>
              </div>
            </div>

            {/* Warnings — only when backend sends them */}
            {preview && preview.warnings.length > 0 && (
              <div className="p-4 bg-yellow-500/10 border border-yellow-500/30 rounded-lg">
                <div className="flex items-center gap-2 mb-2">
                  <AlertTriangle className="w-4 h-4 text-yellow-400" />
                  <span className="font-medium text-yellow-300">
                    Important
                  </span>
                </div>
                <ul className="text-sm text-zinc-300 space-y-1 ml-6 list-disc">
                  {preview.warnings.map((warning, i) => (
                    <li key={i}>{warning}</li>
                  ))}
                </ul>
              </div>
            )}

            {/* Immediate change — amount due today */}
            {preview && isImmediate && (
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
                {preview.prorated_amount_cents > 0 && (
                  <p className="text-sm text-zinc-400 mt-1">
                    Prorated for remaining billing period
                  </p>
                )}
                {preview.period_end > 0 && (
                  <p className="text-sm text-zinc-400 mt-1">
                    Next bill on {formatEffectiveDate(preview.period_end)} at{" "}
                    {formatPrice(
                      preview.new_plan_price_cents,
                      preview.currency,
                    )}
                  </p>
                )}
              </div>
            )}

            {/* Scheduled change — no charge now */}
            {preview && isScheduled && (
              <div className="p-4 bg-zinc-800/50 rounded-lg border border-zinc-600">
                <div className="flex items-center gap-2 mb-2">
                  <Calendar className="w-4 h-4 text-zinc-400" />
                  <span className="font-medium text-zinc-300">
                    Effective {formatEffectiveDate(preview.effective_at)}
                  </span>
                </div>
                <div className="text-sm text-zinc-400 space-y-1">
                  <p>
                    Your {currentPlan.name} access continues until then.
                  </p>
                  <p>
                    No charge today — starting{" "}
                    {formatEffectiveDate(preview.effective_at)}, you'll pay{" "}
                    {formatPrice(newPlan.price_cents, newPlan.currency)} for{" "}
                    {newPlan.name}.
                  </p>
                </div>
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
                {getConfirmLabel()}
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
                {result?.schedule_id
                  ? "Plan change scheduled!"
                  : "Plan changed successfully!"}
              </p>
              {result?.schedule_id && result.effective_at && (
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

  return (
    <Modal
      open={open}
      onClose={
        state === "processing" || state === "requires_action"
          ? () => {}
          : handleClose
      }
      title={getTitle()}
      size="md"
    >
      {renderContent()}
    </Modal>
  );
}
