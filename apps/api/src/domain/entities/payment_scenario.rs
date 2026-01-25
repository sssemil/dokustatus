use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumString};

/// Payment scenario for the dummy provider.
/// Simulates different payment outcomes for testing purposes.
/// Matches a subset of Stripe's test card behaviors.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, AsRefStr, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[derive(Default)]
pub enum PaymentScenario {
    /// Payment succeeds immediately (Stripe test card: 4242424242424242)
    #[default]
    Success,
    /// Card is declined (Stripe test card: 4000000000000002)
    Decline,
    /// Insufficient funds (Stripe test card: 4000000000009995)
    InsufficientFunds,
    /// Requires 3D Secure confirmation (Stripe test card: 4000000000003220)
    ThreeDSecure,
    /// Card is expired (Stripe test card: 4000000000000069)
    ExpiredCard,
    /// Processing error (Stripe test card: 4000000000000119)
    ProcessingError,
}

impl PaymentScenario {
    /// Human-readable description of the scenario
    pub fn description(&self) -> &'static str {
        match self {
            PaymentScenario::Success => "Payment succeeds immediately",
            PaymentScenario::Decline => "Card is declined",
            PaymentScenario::InsufficientFunds => "Card has insufficient funds",
            PaymentScenario::ThreeDSecure => "Requires 3D Secure confirmation",
            PaymentScenario::ExpiredCard => "Card is expired",
            PaymentScenario::ProcessingError => "Payment processing error",
        }
    }

    /// Stripe test card number that triggers this scenario
    pub fn test_card_number(&self) -> &'static str {
        match self {
            PaymentScenario::Success => "4242424242424242",
            PaymentScenario::Decline => "4000000000000002",
            PaymentScenario::InsufficientFunds => "4000000000009995",
            PaymentScenario::ThreeDSecure => "4000000000003220",
            PaymentScenario::ExpiredCard => "4000000000000069",
            PaymentScenario::ProcessingError => "4000000000000119",
        }
    }

    /// Detect scenario from a card number (for compatibility with Stripe test cards)
    pub fn from_card_number(card: &str) -> Self {
        // Remove spaces and dashes
        let card = card.replace([' ', '-'], "");

        match card.as_str() {
            "4242424242424242" => PaymentScenario::Success,
            "4000000000000002" => PaymentScenario::Decline,
            "4000000000009995" => PaymentScenario::InsufficientFunds,
            "4000000000003220" => PaymentScenario::ThreeDSecure,
            "4000000000000069" => PaymentScenario::ExpiredCard,
            "4000000000000119" => PaymentScenario::ProcessingError,
            // Default to success for any other card starting with 4242
            s if s.starts_with("4242") => PaymentScenario::Success,
            // Default to decline for any card starting with 4000
            s if s.starts_with("4000") => PaymentScenario::Decline,
            // Default to success for any other valid-looking card
            _ => PaymentScenario::Success,
        }
    }

    /// Whether this scenario requires additional user confirmation
    pub fn requires_confirmation(&self) -> bool {
        matches!(self, PaymentScenario::ThreeDSecure)
    }

    /// Whether this scenario results in a successful payment
    pub fn is_success(&self) -> bool {
        matches!(self, PaymentScenario::Success)
    }

    /// Whether this scenario results in a failed payment
    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            PaymentScenario::Decline
                | PaymentScenario::InsufficientFunds
                | PaymentScenario::ExpiredCard
                | PaymentScenario::ProcessingError
        )
    }

    /// Get the error message for failed scenarios
    pub fn error_message(&self) -> Option<&'static str> {
        match self {
            PaymentScenario::Success | PaymentScenario::ThreeDSecure => None,
            PaymentScenario::Decline => Some("Your card was declined."),
            PaymentScenario::InsufficientFunds => Some("Your card has insufficient funds."),
            PaymentScenario::ExpiredCard => Some("Your card has expired."),
            PaymentScenario::ProcessingError => {
                Some("An error occurred while processing your card.")
            }
        }
    }

    /// All available scenarios
    pub fn all() -> &'static [PaymentScenario] {
        &[
            PaymentScenario::Success,
            PaymentScenario::Decline,
            PaymentScenario::InsufficientFunds,
            PaymentScenario::ThreeDSecure,
            PaymentScenario::ExpiredCard,
            PaymentScenario::ProcessingError,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_card_number() {
        assert_eq!(
            PaymentScenario::from_card_number("4242424242424242"),
            PaymentScenario::Success
        );
        assert_eq!(
            PaymentScenario::from_card_number("4242 4242 4242 4242"),
            PaymentScenario::Success
        );
        assert_eq!(
            PaymentScenario::from_card_number("4000000000000002"),
            PaymentScenario::Decline
        );
        assert_eq!(
            PaymentScenario::from_card_number("4000000000003220"),
            PaymentScenario::ThreeDSecure
        );
    }

    #[test]
    fn test_scenario_properties() {
        assert!(PaymentScenario::Success.is_success());
        assert!(!PaymentScenario::Success.is_failure());
        assert!(!PaymentScenario::Success.requires_confirmation());

        assert!(!PaymentScenario::Decline.is_success());
        assert!(PaymentScenario::Decline.is_failure());

        assert!(PaymentScenario::ThreeDSecure.requires_confirmation());
        assert!(!PaymentScenario::ThreeDSecure.is_failure());
    }

    #[test]
    fn test_error_messages() {
        assert!(PaymentScenario::Success.error_message().is_none());
        assert!(PaymentScenario::Decline.error_message().is_some());
        assert!(PaymentScenario::InsufficientFunds.error_message().is_some());
    }

    #[test]
    fn test_from_str() {
        assert_eq!(
            "success".parse::<PaymentScenario>().unwrap(),
            PaymentScenario::Success
        );
        assert_eq!(
            "decline".parse::<PaymentScenario>().unwrap(),
            PaymentScenario::Decline
        );
        assert_eq!(
            "three_d_secure".parse::<PaymentScenario>().unwrap(),
            PaymentScenario::ThreeDSecure
        );
        assert!("3ds".parse::<PaymentScenario>().is_err());
        assert!("invalid".parse::<PaymentScenario>().is_err());
    }

    #[test]
    fn test_as_ref_all_variants() {
        assert_eq!(PaymentScenario::Success.as_ref(), "success");
        assert_eq!(PaymentScenario::Decline.as_ref(), "decline");
        assert_eq!(
            PaymentScenario::InsufficientFunds.as_ref(),
            "insufficient_funds"
        );
        assert_eq!(PaymentScenario::ThreeDSecure.as_ref(), "three_d_secure");
        assert_eq!(PaymentScenario::ExpiredCard.as_ref(), "expired_card");
        assert_eq!(
            PaymentScenario::ProcessingError.as_ref(),
            "processing_error"
        );
    }

    #[test]
    fn test_display_matches_as_ref() {
        for variant in PaymentScenario::all() {
            assert_eq!(format!("{}", variant), variant.as_ref());
        }
    }

    #[test]
    fn test_from_str_case_insensitive() {
        assert_eq!(
            "SUCCESS".parse::<PaymentScenario>().unwrap(),
            PaymentScenario::Success
        );
        assert_eq!(
            "Decline".parse::<PaymentScenario>().unwrap(),
            PaymentScenario::Decline
        );
        assert_eq!(
            "THREE_D_SECURE".parse::<PaymentScenario>().unwrap(),
            PaymentScenario::ThreeDSecure
        );
    }
}
