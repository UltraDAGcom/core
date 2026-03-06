use std::collections::HashSet;

use crate::address::Address;

/// The set of known validators participating in DAG-BFT consensus.
/// Tracks active validators and computes BFT thresholds.
pub struct ValidatorSet {
    validators: HashSet<Address>,
    min_validators: usize,
    /// Fixed expected validator count (testnet mode).
    /// When set, quorum threshold uses this instead of the dynamic count,
    /// preventing phantom registrations from inflating the threshold.
    configured_validators: Option<usize>,
}

impl ValidatorSet {
    pub fn new(min_validators: usize) -> Self {
        Self {
            validators: HashSet::new(),
            min_validators: min_validators.max(1),
            configured_validators: None,
        }
    }

    /// Set the expected validator count for quorum calculations.
    /// When set, the quorum threshold is based on this fixed count
    /// rather than the dynamically-growing registered count.
    pub fn set_configured_validators(&mut self, count: usize) {
        self.configured_validators = Some(count);
    }

    /// Get the configured validator count, if set.
    pub fn configured_validators(&self) -> Option<usize> {
        self.configured_validators
    }

    pub fn register(&mut self, addr: Address) -> bool {
        self.validators.insert(addr)
    }

    pub fn contains(&self, addr: &Address) -> bool {
        self.validators.contains(addr)
    }

    pub fn len(&self) -> usize {
        self.validators.len()
    }

    pub fn is_empty(&self) -> bool {
        self.validators.is_empty()
    }

    /// BFT quorum threshold: ceil(2n/3).
    /// When `configured_validators` is set, uses that as `n` to prevent
    /// phantom registrations from inflating the threshold.
    /// Returns usize::MAX if fewer than min_validators are registered.
    pub fn quorum_threshold(&self) -> usize {
        let registered = self.validators.len();
        if registered < self.min_validators {
            return usize::MAX;
        }
        let n = match self.configured_validators {
            Some(configured) => configured,
            None => registered,
        };
        (2 * n + 2) / 3
    }

    pub fn has_quorum(&self, count: usize) -> bool {
        count >= self.quorum_threshold()
    }

    /// Get all validator addresses (for persistence)
    pub fn validators(&self) -> Vec<Address> {
        self.validators.iter().copied().collect()
    }

    /// Get min_validators (for persistence)
    pub fn min_validators(&self) -> usize {
        self.min_validators
    }

    pub fn iter(&self) -> impl Iterator<Item = &Address> {
        self.validators.iter()
    }
}

impl Default for ValidatorSet {
    fn default() -> Self {
        Self::new(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::SecretKey;

    #[test]
    fn threshold_with_4_validators() {
        let mut vs = ValidatorSet::new(3);
        assert_eq!(vs.quorum_threshold(), usize::MAX);

        for _ in 0..4 {
            vs.register(SecretKey::generate().address());
        }
        // ceil(8/3) = 3
        assert_eq!(vs.quorum_threshold(), 3);
        assert!(vs.has_quorum(3));
        assert!(!vs.has_quorum(2));
    }

    #[test]
    fn threshold_with_3_validators() {
        let mut vs = ValidatorSet::new(3);
        for _ in 0..3 {
            vs.register(SecretKey::generate().address());
        }
        // ceil(6/3) = 2
        assert_eq!(vs.quorum_threshold(), 2);
    }

    #[test]
    fn register_is_idempotent() {
        let mut vs = ValidatorSet::new(1);
        let addr = SecretKey::generate().address();
        assert!(vs.register(addr));
        assert!(!vs.register(addr));
        assert_eq!(vs.len(), 1);
    }
}
