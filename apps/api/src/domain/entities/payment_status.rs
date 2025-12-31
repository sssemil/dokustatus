use serde::{Deserialize, Serialize};

/// Payment status for billing payments
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "payment_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    Pending,
    Paid,
    Failed,
    Refunded,
    PartialRefund,
    Uncollectible,
    Void,
}

impl PaymentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PaymentStatus::Pending => "pending",
            PaymentStatus::Paid => "paid",
            PaymentStatus::Failed => "failed",
            PaymentStatus::Refunded => "refunded",
            PaymentStatus::PartialRefund => "partial_refund",
            PaymentStatus::Uncollectible => "uncollectible",
            PaymentStatus::Void => "void",
        }
    }

    /// Convert from Stripe invoice status string
    pub fn from_stripe_invoice_status(s: &str) -> Self {
        match s {
            "paid" => PaymentStatus::Paid,
            "open" | "draft" => PaymentStatus::Pending,
            "uncollectible" => PaymentStatus::Uncollectible,
            "void" => PaymentStatus::Void,
            _ => PaymentStatus::Pending,
        }
    }

    /// Check if this status represents a successful payment
    pub fn is_successful(&self) -> bool {
        matches!(self, PaymentStatus::Paid)
    }

    /// Check if this status represents a failed or problematic payment
    pub fn is_failed(&self) -> bool {
        matches!(
            self,
            PaymentStatus::Failed | PaymentStatus::Uncollectible | PaymentStatus::Void
        )
    }

    /// Check if this status represents a refund (full or partial)
    pub fn is_refunded(&self) -> bool {
        matches!(self, PaymentStatus::Refunded | PaymentStatus::PartialRefund)
    }

    /// Check if this status is terminal (should not be overwritten by webhooks)
    /// Terminal states: paid, refunded, partial_refund, void
    /// Non-terminal states: pending, failed, uncollectible (can still transition)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            PaymentStatus::Paid
                | PaymentStatus::Refunded
                | PaymentStatus::PartialRefund
                | PaymentStatus::Void
        )
    }
}

impl Default for PaymentStatus {
    fn default() -> Self {
        PaymentStatus::Pending
    }
}

impl std::fmt::Display for PaymentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for PaymentStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(PaymentStatus::Pending),
            "paid" => Ok(PaymentStatus::Paid),
            "failed" => Ok(PaymentStatus::Failed),
            "refunded" => Ok(PaymentStatus::Refunded),
            "partial_refund" => Ok(PaymentStatus::PartialRefund),
            "uncollectible" => Ok(PaymentStatus::Uncollectible),
            "void" => Ok(PaymentStatus::Void),
            _ => Err(format!("Invalid payment status: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_stripe_invoice_status() {
        assert_eq!(
            PaymentStatus::from_stripe_invoice_status("paid"),
            PaymentStatus::Paid
        );
        assert_eq!(
            PaymentStatus::from_stripe_invoice_status("open"),
            PaymentStatus::Pending
        );
        assert_eq!(
            PaymentStatus::from_stripe_invoice_status("draft"),
            PaymentStatus::Pending
        );
        assert_eq!(
            PaymentStatus::from_stripe_invoice_status("void"),
            PaymentStatus::Void
        );
        assert_eq!(
            PaymentStatus::from_stripe_invoice_status("uncollectible"),
            PaymentStatus::Uncollectible
        );
        assert_eq!(
            PaymentStatus::from_stripe_invoice_status("unknown"),
            PaymentStatus::Pending
        );
    }

    #[test]
    fn test_status_checks() {
        assert!(PaymentStatus::Paid.is_successful());
        assert!(!PaymentStatus::Failed.is_successful());

        assert!(PaymentStatus::Failed.is_failed());
        assert!(PaymentStatus::Void.is_failed());
        assert!(!PaymentStatus::Paid.is_failed());

        assert!(PaymentStatus::Refunded.is_refunded());
        assert!(PaymentStatus::PartialRefund.is_refunded());
        assert!(!PaymentStatus::Paid.is_refunded());
    }

    #[test]
    fn test_is_terminal() {
        // Terminal states - should not be overwritten
        assert!(PaymentStatus::Paid.is_terminal());
        assert!(PaymentStatus::Refunded.is_terminal());
        assert!(PaymentStatus::PartialRefund.is_terminal());
        assert!(PaymentStatus::Void.is_terminal());

        // Non-terminal states - can still transition
        assert!(!PaymentStatus::Pending.is_terminal());
        assert!(!PaymentStatus::Failed.is_terminal());
        assert!(!PaymentStatus::Uncollectible.is_terminal());
    }
}
