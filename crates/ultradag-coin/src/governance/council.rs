use serde::{Deserialize, Serialize};

/// Categories of Council of 21 seats.
/// Each category has a fixed allocation ensuring diverse expertise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CouncilSeatCategory {
    /// Protocol developers, core engineers, infrastructure (5 seats)
    Engineering,
    /// Partnerships, adoption, ecosystem development (3 seats)
    Growth,
    /// Legal counsel, regulatory compliance (2 seats)
    Legal,
    /// Cryptography, distributed systems, economics research (2 seats)
    Research,
    /// Community advocates, content creators, ambassadors (4 seats)
    Community,
    /// Treasury management, infrastructure, DevOps (3 seats)
    Operations,
    /// Security auditors, penetration testers, incident response (2 seats)
    Security,
}

impl CouncilSeatCategory {
    /// Maximum seats allocated to this category.
    pub fn max_seats(&self) -> usize {
        match self {
            CouncilSeatCategory::Engineering => 5,
            CouncilSeatCategory::Growth => 3,
            CouncilSeatCategory::Legal => 2,
            CouncilSeatCategory::Research => 2,
            CouncilSeatCategory::Community => 4,
            CouncilSeatCategory::Operations => 3,
            CouncilSeatCategory::Security => 2,
        }
    }

    /// All category variants.
    pub fn all() -> &'static [CouncilSeatCategory] {
        &[
            CouncilSeatCategory::Engineering,
            CouncilSeatCategory::Growth,
            CouncilSeatCategory::Legal,
            CouncilSeatCategory::Research,
            CouncilSeatCategory::Community,
            CouncilSeatCategory::Operations,
            CouncilSeatCategory::Security,
        ]
    }

    /// Display name for the category.
    pub fn name(&self) -> &'static str {
        match self {
            CouncilSeatCategory::Engineering => "Engineering",
            CouncilSeatCategory::Growth => "Growth",
            CouncilSeatCategory::Legal => "Legal",
            CouncilSeatCategory::Research => "Research",
            CouncilSeatCategory::Community => "Community",
            CouncilSeatCategory::Operations => "Operations",
            CouncilSeatCategory::Security => "Security",
        }
    }

    /// Parse from string (for governance proposals).
    pub fn parse_name(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "engineering" => Some(CouncilSeatCategory::Engineering),
            "growth" => Some(CouncilSeatCategory::Growth),
            "legal" => Some(CouncilSeatCategory::Legal),
            "research" => Some(CouncilSeatCategory::Research),
            "community" => Some(CouncilSeatCategory::Community),
            "operations" => Some(CouncilSeatCategory::Operations),
            "security" => Some(CouncilSeatCategory::Security),
            _ => None,
        }
    }
}

/// Action for council membership governance proposals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CouncilAction {
    Add,
    Remove,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seat_allocation_sums_to_21() {
        let total: usize = CouncilSeatCategory::all().iter().map(|c| c.max_seats()).sum();
        assert_eq!(total, 21);
    }

    #[test]
    fn parse_name_roundtrip() {
        for cat in CouncilSeatCategory::all() {
            assert_eq!(CouncilSeatCategory::parse_name(cat.name()), Some(*cat));
        }
        assert_eq!(CouncilSeatCategory::parse_name("unknown"), None);
    }
}
