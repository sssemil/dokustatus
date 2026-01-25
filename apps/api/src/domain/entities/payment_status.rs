use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumString};

/// Payment status for billing payments
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, AsRefStr, Display,
    EnumString,
)]
#[sqlx(type_name = "payment_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[derive(Default)]
pub enum PaymentStatus {
    #[default]
    Pending,
    Paid,
    Failed,
    Refunded,
    PartialRefund,
    Uncollectible,
    Void,
}

impl PaymentStatus {
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

    #[test]
    fn test_as_ref_all_variants() {
        assert_eq!(PaymentStatus::Pending.as_ref(), "pending");
        assert_eq!(PaymentStatus::Paid.as_ref(), "paid");
        assert_eq!(PaymentStatus::Failed.as_ref(), "failed");
        assert_eq!(PaymentStatus::Refunded.as_ref(), "refunded");
        assert_eq!(PaymentStatus::PartialRefund.as_ref(), "partial_refund");
        assert_eq!(PaymentStatus::Uncollectible.as_ref(), "uncollectible");
        assert_eq!(PaymentStatus::Void.as_ref(), "void");
    }

    #[test]
    fn test_display_matches_as_ref() {
        for variant in [
            PaymentStatus::Pending,
            PaymentStatus::Paid,
            PaymentStatus::Failed,
            PaymentStatus::Refunded,
            PaymentStatus::PartialRefund,
            PaymentStatus::Uncollectible,
            PaymentStatus::Void,
        ] {
            assert_eq!(format!("{}", variant), variant.as_ref());
        }
    }

    #[test]
    fn test_from_str() {
        assert_eq!(
            "pending".parse::<PaymentStatus>().unwrap(),
            PaymentStatus::Pending
        );
        assert_eq!(
            "paid".parse::<PaymentStatus>().unwrap(),
            PaymentStatus::Paid
        );
        assert_eq!(
            "failed".parse::<PaymentStatus>().unwrap(),
            PaymentStatus::Failed
        );
        assert_eq!(
            "refunded".parse::<PaymentStatus>().unwrap(),
            PaymentStatus::Refunded
        );
        assert_eq!(
            "partial_refund".parse::<PaymentStatus>().unwrap(),
            PaymentStatus::PartialRefund
        );
        assert_eq!(
            "uncollectible".parse::<PaymentStatus>().unwrap(),
            PaymentStatus::Uncollectible
        );
        assert_eq!(
            "void".parse::<PaymentStatus>().unwrap(),
            PaymentStatus::Void
        );
        assert!("invalid".parse::<PaymentStatus>().is_err());
    }

    #[test]
    fn test_from_str_case_insensitive() {
        assert_eq!(
            "PENDING".parse::<PaymentStatus>().unwrap(),
            PaymentStatus::Pending
        );
        assert_eq!(
            "Paid".parse::<PaymentStatus>().unwrap(),
            PaymentStatus::Paid
        );
        assert_eq!(
            "PARTIAL_REFUND".parse::<PaymentStatus>().unwrap(),
            PaymentStatus::PartialRefund
        );
    }
}
