use serde::{Deserialize, Serialize};

/// Categories of Council of 21 seats.
/// Each category has a fixed allocation ensuring diverse expertise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CouncilSeatCategory {
    /// Protocol developers, security researchers, infrastructure engineers (7 seats)
    Technical,
    /// IoT CEOs, crypto founders, enterprise users (4 seats)
    Business,
    /// Lawyers, regulatory specialists (3 seats)
    Legal,
    /// Cryptographers, distributed systems researchers (3 seats)
    Academic,
    /// SDK developers, node operators, integration builders (2 seats)
    Community,
    /// Panama Foundation leadership (2 seats)
    Foundation,
}

impl CouncilSeatCategory {
    /// Maximum seats allocated to this category.
    pub fn max_seats(&self) -> usize {
        match self {
            CouncilSeatCategory::Technical => 7,
            CouncilSeatCategory::Business => 4,
            CouncilSeatCategory::Legal => 3,
            CouncilSeatCategory::Academic => 3,
            CouncilSeatCategory::Community => 2,
            CouncilSeatCategory::Foundation => 2,
        }
    }

    /// All category variants.
    pub fn all() -> &'static [CouncilSeatCategory] {
        &[
            CouncilSeatCategory::Technical,
            CouncilSeatCategory::Business,
            CouncilSeatCategory::Legal,
            CouncilSeatCategory::Academic,
            CouncilSeatCategory::Community,
            CouncilSeatCategory::Foundation,
        ]
    }

    /// Display name for the category.
    pub fn name(&self) -> &'static str {
        match self {
            CouncilSeatCategory::Technical => "Technical",
            CouncilSeatCategory::Business => "Business",
            CouncilSeatCategory::Legal => "Legal",
            CouncilSeatCategory::Academic => "Academic",
            CouncilSeatCategory::Community => "Community",
            CouncilSeatCategory::Foundation => "Foundation",
        }
    }

    /// Parse from string (for governance proposals).
    pub fn parse_name(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "technical" => Some(CouncilSeatCategory::Technical),
            "business" => Some(CouncilSeatCategory::Business),
            "legal" => Some(CouncilSeatCategory::Legal),
            "academic" => Some(CouncilSeatCategory::Academic),
            "community" => Some(CouncilSeatCategory::Community),
            "foundation" => Some(CouncilSeatCategory::Foundation),
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
