"use client";

import { useState } from "react";
import { Modal, Button, Badge, Select } from "@/components/ui";
import {
  PaymentScenario,
  DummyCheckoutResponse,
  formatPrice,
  getScenarioLabel,
  getScenarioDescription,
} from "@/types/billing";
import {
  AlertTriangle,
  Loader2,
  CheckCircle,
  CreditCard,
  TestTube,
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

interface DummyCheckoutModalProps {
  open: boolean;
  onClose: () => void;
  plan: Plan;
  apiDomain: string;
  onSuccess: () => void;
}

type ModalState =
  | "select_scenario"
  | "processing"
  | "requires_confirmation"
  | "success"
  | "error";

const SCENARIOS: PaymentScenario[] = [
  "success",
  "decline",
  "insufficient_funds",
  "three_d_secure",
  "expired_card",
  "processing_error",
];

export function DummyCheckoutModal({
  open,
  onClose,
  plan,
  apiDomain,
  onSuccess,
}: DummyCheckoutModalProps) {
  const [state, setState] = useState<ModalState>("select_scenario");
  const [scenario, setScenario] = useState<PaymentScenario>("success");
  const [error, setError] = useState<string | null>(null);
  const [confirmationToken, setConfirmationToken] = useState<string | null>(
    null,
  );
  const [subscriptionId, setSubscriptionId] = useState<string | null>(null);

  const handleReset = () => {
    setState("select_scenario");
    setScenario("success");
    setError(null);
    setConfirmationToken(null);
    setSubscriptionId(null);
  };

  const handleClose = () => {
    if (state === "success") {
      onSuccess();
    }
    handleReset();
    onClose();
  };

  const handleCheckout = async () => {
    setState("processing");
    setError(null);

    try {
      const res = await fetch(
        `/api/public/domain/${apiDomain}/billing/checkout/dummy`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            plan_code: plan.code,
            scenario,
          }),
          credentials: "include",
        },
      );

      const data: DummyCheckoutResponse = await res.json();

      if (!res.ok) {
        setError(
          (data as { message?: string }).message || "Failed to process payment",
        );
        setState("error");
        return;
      }

      if (data.success) {
        setSubscriptionId(data.subscription_id);
        setState("success");
      } else if (data.requires_confirmation) {
        setConfirmationToken(data.confirmation_token);
        setState("requires_confirmation");
      } else {
        setError(data.error_message || "Payment failed");
        setState("error");
      }
    } catch {
      setError("Network error");
      setState("error");
    }
  };

  const handleConfirm3DS = async () => {
    setState("processing");
    setError(null);

    try {
      const res = await fetch(
        `/api/public/domain/${apiDomain}/billing/dummy/confirm`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            confirmation_token: confirmationToken,
          }),
          credentials: "include",
        },
      );

      const data: DummyCheckoutResponse = await res.json();

      if (!res.ok) {
        setError(
          (data as { message?: string }).message || "Failed to confirm payment",
        );
        setState("error");
        return;
      }

      if (data.success) {
        setSubscriptionId(data.subscription_id);
        setState("success");
      } else {
        setError(data.error_message || "Confirmation failed");
        setState("error");
      }
    } catch {
      setError("Network error");
      setState("error");
    }
  };

  const renderContent = () => {
    switch (state) {
      case "select_scenario":
        return (
          <div className="space-y-4">
            {/* Test provider badge */}
            <div className="flex items-center gap-2 p-3 bg-zinc-800/50 rounded-lg border border-zinc-700">
              <TestTube className="w-5 h-5 text-zinc-400" />
              <div>
                <p className="text-sm font-medium text-white">
                  Test Payment Provider
                </p>
                <p className="text-xs text-zinc-400">
                  This is a simulated payment for testing purposes
                </p>
              </div>
            </div>

            {/* Plan summary */}
            <div className="p-4 bg-zinc-800/50 rounded-lg border border-zinc-700">
              <div className="flex items-center gap-2 mb-3">
                <CreditCard className="w-5 h-5 text-blue-400" />
                <span className="font-medium text-white">{plan.name}</span>
              </div>
              <p className="text-2xl font-bold text-white">
                {formatPrice(plan.price_cents, plan.currency)}
                <span className="text-sm font-normal text-zinc-400">
                  /
                  {plan.interval_count === 1
                    ? plan.interval.replace("ly", "")
                    : `${plan.interval_count} ${plan.interval}`}
                </span>
              </p>
            </div>

            {/* Scenario selection */}
            <div className="space-y-2">
              <label className="block text-sm font-medium text-zinc-300">
                Test Scenario
              </label>
              <select
                value={scenario}
                onChange={(e) => setScenario(e.target.value as PaymentScenario)}
                className="w-full px-3 py-2 bg-zinc-800 border border-zinc-700 rounded-lg text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
              >
                {SCENARIOS.map((s) => (
                  <option key={s} value={s}>
                    {getScenarioLabel(s)}
                  </option>
                ))}
              </select>
              <p className="text-sm text-zinc-400">
                {getScenarioDescription(scenario)}
              </p>
            </div>

            {/* Scenario-specific warnings */}
            {scenario !== "success" && (
              <div
                className={`p-3 rounded-lg border ${
                  scenario === "three_d_secure"
                    ? "bg-blue-500/10 border-blue-500/30"
                    : "bg-yellow-500/10 border-yellow-500/30"
                }`}
              >
                <div className="flex items-center gap-2">
                  <AlertTriangle
                    className={`w-4 h-4 ${
                      scenario === "three_d_secure"
                        ? "text-blue-400"
                        : "text-yellow-400"
                    }`}
                  />
                  <span
                    className={`text-sm font-medium ${
                      scenario === "three_d_secure"
                        ? "text-blue-300"
                        : "text-yellow-300"
                    }`}
                  >
                    {scenario === "three_d_secure"
                      ? "This scenario will require a confirmation step"
                      : "This scenario will result in a payment failure"}
                  </span>
                </div>
              </div>
            )}

            {/* Actions */}
            <div className="flex gap-3 pt-2">
              <Button variant="ghost" onClick={handleClose} className="flex-1">
                Cancel
              </Button>
              <Button
                variant="primary"
                onClick={handleCheckout}
                className="flex-1"
              >
                Complete Test Payment
              </Button>
            </div>
          </div>
        );

      case "processing":
        return (
          <div className="flex flex-col items-center py-8">
            <Loader2 className="w-8 h-8 text-blue-400 animate-spin" />
            <p className="mt-4 text-zinc-400">Processing test payment...</p>
          </div>
        );

      case "requires_confirmation":
        return (
          <div className="space-y-4">
            <div className="p-4 bg-blue-500/10 border border-blue-500/30 rounded-lg">
              <div className="flex items-center gap-2 mb-2">
                <AlertTriangle className="w-4 h-4 text-blue-400" />
                <span className="font-medium text-blue-300">
                  3D Secure Authentication Required
                </span>
              </div>
              <p className="text-sm text-zinc-300">
                This payment requires additional authentication. In a real
                scenario, the user would be redirected to their bank's
                authentication page.
              </p>
              <p className="text-xs text-zinc-400 mt-2">
                Token: {confirmationToken}
              </p>
            </div>

            <div className="flex gap-3">
              <Button variant="ghost" onClick={handleClose} className="flex-1">
                Cancel
              </Button>
              <Button
                variant="primary"
                onClick={handleConfirm3DS}
                className="flex-1"
              >
                Simulate 3DS Confirmation
              </Button>
            </div>
          </div>
        );

      case "success":
        return (
          <div className="space-y-4">
            <div className="p-4 bg-green-500/10 border border-green-500/30 rounded-lg text-center">
              <CheckCircle className="w-12 h-12 text-green-400 mx-auto mb-2" />
              <p className="font-medium text-green-300">
                Test payment successful!
              </p>
              <p className="text-sm text-zinc-400 mt-2">
                Subscription ID: {subscriptionId}
              </p>
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
                <span className="font-medium text-red-300">Payment failed</span>
              </div>
              <p className="text-sm text-zinc-300">{error}</p>
            </div>

            <div className="flex gap-3">
              <Button variant="ghost" onClick={handleClose} className="flex-1">
                Close
              </Button>
              <Button
                variant="primary"
                onClick={handleReset}
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
      onClose={state === "processing" ? () => {} : handleClose}
      title="Test Payment Checkout"
      size="md"
    >
      {renderContent()}
    </Modal>
  );
}
