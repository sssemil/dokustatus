use serde::{Deserialize, Serialize};

/// Billing state for tracking provider switching operations.
/// Used as a state machine to handle partial failures during provider switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "billing_state", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum BillingState {
    /// Normal active state - subscription is functioning normally
    #[default]
    Active,
    /// Provider switch in progress - old subscription canceled, new one being created
    PendingSwitch,
    /// Provider switch failed - needs manual intervention or retry
    SwitchFailed,
}

impl BillingState {
    pub fn as_str(&self) -> &'static str {
        match self {
            BillingState::Active => "active",
            BillingState::PendingSwitch => "pending_switch",
            BillingState::SwitchFailed => "switch_failed",
        }
    }

    /// Human-readable description of the state
    pub fn description(&self) -> &'static str {
        match self {
            BillingState::Active => "Subscription is active",
            BillingState::PendingSwitch => "Provider switch in progress",
            BillingState::SwitchFailed => "Provider switch failed",
        }
    }

    /// Whether the subscription is in a healthy state
    pub fn is_healthy(&self) -> bool {
        matches!(self, BillingState::Active)
    }

    /// Whether the subscription is in a transitional state
    pub fn is_transitional(&self) -> bool {
        matches!(self, BillingState::PendingSwitch)
    }

    /// Whether the subscription needs attention
    pub fn needs_attention(&self) -> bool {
        matches!(self, BillingState::SwitchFailed)
    }

    /// Valid transitions from this state
    pub fn valid_transitions(&self) -> &'static [BillingState] {
        match self {
            BillingState::Active => &[BillingState::PendingSwitch],
            BillingState::PendingSwitch => &[BillingState::Active, BillingState::SwitchFailed],
            BillingState::SwitchFailed => &[BillingState::Active, BillingState::PendingSwitch],
        }
    }

    /// Check if transition to the given state is valid
    pub fn can_transition_to(&self, new_state: BillingState) -> bool {
        self.valid_transitions().contains(&new_state)
    }
}

impl std::fmt::Display for BillingState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for BillingState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(BillingState::Active),
            "pending_switch" => Ok(BillingState::PendingSwitch),
            "switch_failed" => Ok(BillingState::SwitchFailed),
            _ => Err(format!("Invalid billing state: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_properties() {
        assert!(BillingState::Active.is_healthy());
        assert!(!BillingState::Active.is_transitional());
        assert!(!BillingState::Active.needs_attention());

        assert!(!BillingState::PendingSwitch.is_healthy());
        assert!(BillingState::PendingSwitch.is_transitional());

        assert!(BillingState::SwitchFailed.needs_attention());
    }

    #[test]
    fn test_valid_transitions() {
        // From Active, can only go to PendingSwitch
        assert!(BillingState::Active.can_transition_to(BillingState::PendingSwitch));
        assert!(!BillingState::Active.can_transition_to(BillingState::SwitchFailed));

        // From PendingSwitch, can go to Active or SwitchFailed
        assert!(BillingState::PendingSwitch.can_transition_to(BillingState::Active));
        assert!(BillingState::PendingSwitch.can_transition_to(BillingState::SwitchFailed));

        // From SwitchFailed, can retry (PendingSwitch) or recover (Active)
        assert!(BillingState::SwitchFailed.can_transition_to(BillingState::Active));
        assert!(BillingState::SwitchFailed.can_transition_to(BillingState::PendingSwitch));
    }

    #[test]
    fn test_from_str() {
        assert_eq!(
            "active".parse::<BillingState>().unwrap(),
            BillingState::Active
        );
        assert_eq!(
            "pending_switch".parse::<BillingState>().unwrap(),
            BillingState::PendingSwitch
        );
        assert_eq!(
            "switch_failed".parse::<BillingState>().unwrap(),
            BillingState::SwitchFailed
        );
        assert!("invalid".parse::<BillingState>().is_err());
    }

    #[test]
    fn test_as_str_all_variants() {
        assert_eq!(BillingState::Active.as_str(), "active");
        assert_eq!(BillingState::PendingSwitch.as_str(), "pending_switch");
        assert_eq!(BillingState::SwitchFailed.as_str(), "switch_failed");
    }

    #[test]
    fn test_display_matches_as_str() {
        for variant in [
            BillingState::Active,
            BillingState::PendingSwitch,
            BillingState::SwitchFailed,
        ] {
            assert_eq!(format!("{}", variant), variant.as_str());
        }
    }

    #[test]
    fn test_from_str_case_insensitive() {
        assert_eq!(
            "ACTIVE".parse::<BillingState>().unwrap(),
            BillingState::Active
        );
        assert_eq!(
            "Active".parse::<BillingState>().unwrap(),
            BillingState::Active
        );
        assert_eq!(
            "PENDING_SWITCH".parse::<BillingState>().unwrap(),
            BillingState::PendingSwitch
        );
        assert_eq!(
            "SWITCH_FAILED".parse::<BillingState>().unwrap(),
            BillingState::SwitchFailed
        );
    }
}
