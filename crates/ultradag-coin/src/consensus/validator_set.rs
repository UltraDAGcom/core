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
    /// Permissioned validator allowlist.
    /// When set, only addresses in this set can be registered as validators.
    /// Other nodes can still connect, sync, and submit transactions.
    allowed_validators: Option<HashSet<Address>>,
}

impl ValidatorSet {
    pub fn new(min_validators: usize) -> Self {
        Self {
            validators: HashSet::new(),
            min_validators: min_validators.max(1),
            configured_validators: None,
            allowed_validators: None,
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

    /// Set the permissioned validator allowlist.
    /// When set, only addresses in this set can register as validators.
    /// Also purges any already-registered validators not in the allowlist.
    pub fn set_allowed_validators(&mut self, addrs: HashSet<Address>) {
        self.validators.retain(|addr| addrs.contains(addr));
        self.allowed_validators = Some(addrs);
    }

    /// Check if an address is allowed to be a validator.
    /// Returns true if no allowlist is set (permissionless mode).
    pub fn is_allowed(&self, addr: &Address) -> bool {
        match &self.allowed_validators {
            Some(allowed) => allowed.contains(addr),
            None => true,
        }
    }

    pub fn register(&mut self, addr: Address) -> bool {
        if let Some(allowed) = &self.allowed_validators {
            if !allowed.contains(&addr) {
                return false;
            }
        }
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
    /// Returns usize::MAX if fewer than min_validators are known.
    /// When `configured_validators` is set, uses that count for the min check
    /// (the operator has declared the expected validator count).
    pub fn quorum_threshold(&self) -> usize {
        let effective_count = match self.configured_validators {
            Some(configured) => configured,
            None => self.validators.len(),
        };
        if effective_count < self.min_validators {
            return usize::MAX;
        }
        (2 * effective_count + 2) / 3
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

    #[test]
    fn allowlist_blocks_unregistered_validators() {
        let mut vs = ValidatorSet::new(1);
        let allowed_sk = SecretKey::generate();
        let blocked_sk = SecretKey::generate();

        let mut allowed = HashSet::new();
        allowed.insert(allowed_sk.address());
        vs.set_allowed_validators(allowed);

        assert!(vs.register(allowed_sk.address()));
        assert!(!vs.register(blocked_sk.address()));
        assert_eq!(vs.len(), 1);
        assert!(vs.is_allowed(&allowed_sk.address()));
        assert!(!vs.is_allowed(&blocked_sk.address()));
    }

    #[test]
    fn allowlist_purges_existing_validators() {
        let mut vs = ValidatorSet::new(1);
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        let sk3 = SecretKey::generate();

        // Register all three
        vs.register(sk1.address());
        vs.register(sk2.address());
        vs.register(sk3.address());
        assert_eq!(vs.len(), 3);

        // Set allowlist with only sk1 and sk2 — sk3 should be purged
        let mut allowed = HashSet::new();
        allowed.insert(sk1.address());
        allowed.insert(sk2.address());
        vs.set_allowed_validators(allowed);

        assert_eq!(vs.len(), 2);
        assert!(vs.contains(&sk1.address()));
        assert!(vs.contains(&sk2.address()));
        assert!(!vs.contains(&sk3.address()));

        // Future registrations of sk3 should still be blocked
        assert!(!vs.register(sk3.address()));
        assert_eq!(vs.len(), 2);
    }

    #[test]
    fn no_allowlist_permits_all() {
        let mut vs = ValidatorSet::new(1);
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();

        assert!(vs.register(sk1.address()));
        assert!(vs.register(sk2.address()));
        assert_eq!(vs.len(), 2);
        assert!(vs.is_allowed(&sk1.address()));
    }
}
