use serde::{Deserialize, Serialize};

/// Payment scenario for the dummy provider.
/// Simulates different payment outcomes for testing purposes.
/// Matches a subset of Stripe's test card behaviors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    pub fn as_str(&self) -> &'static str {
        match self {
            PaymentScenario::Success => "success",
            PaymentScenario::Decline => "decline",
            PaymentScenario::InsufficientFunds => "insufficient_funds",
            PaymentScenario::ThreeDSecure => "three_d_secure",
            PaymentScenario::ExpiredCard => "expired_card",
            PaymentScenario::ProcessingError => "processing_error",
        }
    }

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

impl std::fmt::Display for PaymentScenario {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for PaymentScenario {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "success" => Ok(PaymentScenario::Success),
            "decline" => Ok(PaymentScenario::Decline),
            "insufficient_funds" => Ok(PaymentScenario::InsufficientFunds),
            "three_d_secure" => Ok(PaymentScenario::ThreeDSecure),
            "expired_card" => Ok(PaymentScenario::ExpiredCard),
            "processing_error" => Ok(PaymentScenario::ProcessingError),
            _ => Err(format!("Invalid payment scenario: {}", s)),
        }
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
    fn test_as_str_all_variants() {
        assert_eq!(PaymentScenario::Success.as_str(), "success");
        assert_eq!(PaymentScenario::Decline.as_str(), "decline");
        assert_eq!(PaymentScenario::InsufficientFunds.as_str(), "insufficient_funds");
        assert_eq!(PaymentScenario::ThreeDSecure.as_str(), "three_d_secure");
        assert_eq!(PaymentScenario::ExpiredCard.as_str(), "expired_card");
        assert_eq!(PaymentScenario::ProcessingError.as_str(), "processing_error");
    }

    #[test]
    fn test_display_matches_as_str() {
        for variant in PaymentScenario::all() {
            assert_eq!(format!("{}", variant), variant.as_str());
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
