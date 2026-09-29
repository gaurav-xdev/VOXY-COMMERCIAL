//! Subscription Lifecycle State Machine.
//!
//! Models explicit subscription states, validates state transitions,
//! and prevents invalid lifecycle jumps.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Pending,
    Active,
    PastDue,
    Cancelled,
    Expired,
    Refunded,
    PaymentFailed,
    Suspended,
}

impl SubscriptionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::PastDue => "past_due",
            Self::Cancelled => "cancelled",
            Self::Expired => "expired",
            Self::Refunded => "refunded",
            Self::PaymentFailed => "payment_failed",
            Self::Suspended => "suspended",
        }
    }

    pub fn from_str_lossy(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "active" => Self::Active,
            "past_due" => Self::PastDue,
            "cancelled" | "canceled" => Self::Cancelled,
            "expired" => Self::Expired,
            "refunded" => Self::Refunded,
            "payment_failed" => Self::PaymentFailed,
            "suspended" => Self::Suspended,
            _ => Self::Pending,
        }
    }

    /// Indicates whether a subscription grants active commercial entitlements.
    pub fn grants_access(&self) -> bool {
        matches!(self, Self::Active | Self::PastDue) // PastDue provides grace period access
    }
}

pub struct SubscriptionStateMachine;

impl SubscriptionStateMachine {
    /// Validates whether a state transition from `current` to `target` is legally permitted.
    pub fn can_transition(current: SubscriptionStatus, target: SubscriptionStatus) -> bool {
        if current == target {
            return true;
        }

        match current {
            SubscriptionStatus::Pending => matches!(
                target,
                SubscriptionStatus::Active
                    | SubscriptionStatus::PaymentFailed
                    | SubscriptionStatus::Cancelled
            ),
            SubscriptionStatus::Active => matches!(
                target,
                SubscriptionStatus::PastDue
                    | SubscriptionStatus::Cancelled
                    | SubscriptionStatus::Expired
                    | SubscriptionStatus::Refunded
                    | SubscriptionStatus::Suspended
            ),
            SubscriptionStatus::PastDue => matches!(
                target,
                SubscriptionStatus::Active
                    | SubscriptionStatus::Expired
                    | SubscriptionStatus::Cancelled
                    | SubscriptionStatus::Suspended
            ),
            SubscriptionStatus::Cancelled => matches!(
                target,
                SubscriptionStatus::Expired | SubscriptionStatus::Refunded
            ),
            SubscriptionStatus::PaymentFailed => matches!(
                target,
                SubscriptionStatus::Active | SubscriptionStatus::Expired
            ),
            SubscriptionStatus::Suspended => matches!(
                target,
                SubscriptionStatus::Active
                    | SubscriptionStatus::Expired
                    | SubscriptionStatus::Cancelled
            ),
            SubscriptionStatus::Expired | SubscriptionStatus::Refunded => false, // Terminal states
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscription_state_transitions() {
        use SubscriptionStatus::*;

        // Valid transitions
        assert!(SubscriptionStateMachine::can_transition(Pending, Active));
        assert!(SubscriptionStateMachine::can_transition(Active, PastDue));
        assert!(SubscriptionStateMachine::can_transition(PastDue, Active));
        assert!(SubscriptionStateMachine::can_transition(Active, Cancelled));
        assert!(SubscriptionStateMachine::can_transition(Cancelled, Expired));

        // Invalid transitions
        assert!(!SubscriptionStateMachine::can_transition(Expired, Active));
        assert!(!SubscriptionStateMachine::can_transition(Refunded, Active));
        assert!(!SubscriptionStateMachine::can_transition(Pending, Expired));
    }

    #[test]
    fn test_entitlement_access_grant() {
        assert!(SubscriptionStatus::Active.grants_access());
        assert!(SubscriptionStatus::PastDue.grants_access()); // grace period
        assert!(!SubscriptionStatus::Cancelled.grants_access());
        assert!(!SubscriptionStatus::Expired.grants_access());
        assert!(!SubscriptionStatus::Pending.grants_access());
    }
}
