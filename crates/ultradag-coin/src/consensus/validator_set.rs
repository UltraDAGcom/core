use std::collections::{HashMap, HashSet};

use crate::address::Address;

/// Number of rounds a validator can be silent before being excluded from
/// the adaptive quorum calculation. At 2s per round this is ~17 minutes —
/// long enough to tolerate normal restarts/redeploys, short enough to heal
/// from real outages within a reasonable window.
pub const LIVENESS_WINDOW_ROUNDS: u64 = 500;

/// The set of known validators participating in DAG-BFT consensus.
/// Tracks active validators and computes BFT thresholds.
pub struct ValidatorSet {
    validators: HashSet<Address>,
    min_validators: usize,
    /// Fixed expected validator count (testnet mode).
    /// When set, quorum threshold uses this instead of the dynamic count,
    /// preventing phantom registrations from inflating the threshold.
    /// Must match `StateEngine::configured_validator_count` (which uses `u64`
    /// for reward math). Both are set together from --validators N in main.rs.
    configured_validators: Option<usize>,
    /// Permissioned validator allowlist.
    /// When set, only addresses in this set can be registered as validators.
    /// Other nodes can still connect, sync, and submit transactions.
    allowed_validators: Option<HashSet<Address>>,
    /// Tracks the most recent round each validator produced a vertex.
    /// Used by `adaptive_quorum_threshold` to exclude offline validators
    /// from the quorum, preserving liveness when some nodes go down.
    last_produced_round: HashMap<Address, u64>,
}

impl ValidatorSet {
    pub fn new(min_validators: usize) -> Self {
        Self {
            validators: HashSet::new(),
            min_validators: min_validators.max(1),
            configured_validators: None,
            allowed_validators: None,
            last_produced_round: HashMap::new(),
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

    /// Remove a validator (e.g., after equivocation/slashing).
    /// Prevents Byzantine validators from inflating the quorum threshold.
    pub fn remove(&mut self, addr: &Address) -> bool {
        self.validators.remove(addr)
    }

    pub fn contains(&self, addr: &Address) -> bool {
        self.validators.contains(addr)
    }

    pub fn len(&self) -> usize {
        self.validators.len()
    }

    /// Check if a validator allowlist has been configured.
    pub fn has_allowlist(&self) -> bool {
        self.allowed_validators.is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.validators.is_empty()
    }

    /// Returns true when the validator topology has been declared by the
    /// operator — either a fixed configured count or an explicit allowlist.
    ///
    /// SECURITY: Fully permissionless mode (neither set) is unsafe. In that
    /// mode an attacker can mint fresh keys, sign a single vertex per key,
    /// and inflate both `validators.len()` and the "active producer" count,
    /// manipulating the quorum threshold (GHSA-rprp-wjrh-hx7g). Without a
    /// declared topology there is no sybil-resistant way to count validators,
    /// so the thresholds fail closed (return `usize::MAX`).
    pub fn is_topology_configured(&self) -> bool {
        self.configured_validators.is_some() || self.allowed_validators.is_some()
    }

    /// BFT quorum threshold: ceil(2n/3).
    ///
    /// SECURITY: Fails closed (`usize::MAX`) when the validator topology is
    /// not declared via `configured_validators` or `allowed_validators`.
    /// Dynamic mode was exploitable: producer-backed phantom validators
    /// inflated the threshold and stalled finality (GHSA-rprp-wjrh-hx7g).
    /// Operators MUST set `--validators N` or `--validator-key <file>`.
    ///
    /// When `configured_validators` is set, uses that as `n`. Otherwise,
    /// when only an allowlist is set, uses the allowlist size as `n`
    /// (allowlisted addresses cannot be forged).
    ///
    /// Returns `usize::MAX` if fewer than `min_validators` are known.
    pub fn quorum_threshold(&self) -> usize {
        let effective_count = match (self.configured_validators, &self.allowed_validators) {
            (Some(configured), _) => configured,
            (None, Some(allowed)) => allowed.len(),
            (None, None) => {
                // Fail closed: permissionless mode cannot distinguish real
                // validators from phantoms. See `is_topology_configured`.
                return usize::MAX;
            }
        };

        if effective_count < self.min_validators {
            return usize::MAX;
        }

        (2 * effective_count).div_ceil(3)
    }

    /// Record that `addr` produced a vertex at `round`. Updates the
    /// liveness map used by `adaptive_quorum_threshold`.
    pub fn record_production(&mut self, addr: Address, round: u64) {
        let entry = self.last_produced_round.entry(addr).or_insert(0);
        if round > *entry {
            *entry = round;
        }
    }

    /// Count validators who produced a vertex within the last
    /// `LIVENESS_WINDOW_ROUNDS` rounds ending at `current_round`.
    ///
    /// This is the **proven-active** count: these validators cryptographically
    /// signed vertices recently, so they cannot be phantoms. Safe to use as
    /// the quorum base.
    pub fn active_validator_count(&self, current_round: u64) -> usize {
        let cutoff = current_round.saturating_sub(LIVENESS_WINDOW_ROUNDS);
        self.last_produced_round.values()
            .filter(|&&r| r >= cutoff)
            .count()
    }

    /// Adaptive BFT quorum threshold that shrinks when validators go offline,
    /// preserving liveness. Uses the smaller of:
    ///   - `configured_validators` (safety upper bound, prevents phantom inflation)
    ///   - `active_validator_count(current_round)` (liveness lower bound)
    /// floored at `min_validators`.
    ///
    /// Returns `usize::MAX` when finality cannot be decided (insufficient data
    /// or effective count below `min_validators`).
    ///
    /// SECURITY: Shrinking the quorum is safe because only **proven** validators
    /// (those who have signed vertices) count toward the active set. Phantom
    /// registrations cannot reduce the quorum.
    pub fn adaptive_quorum_threshold(&self, current_round: u64) -> usize {
        // SECURITY: Fail closed in permissionless mode. Without a declared
        // topology, an attacker can mint keys and produce signed vertices
        // to inflate `active_validator_count`, manipulating the threshold
        // (GHSA-rprp-wjrh-hx7g).
        if !self.is_topology_configured() {
            return usize::MAX;
        }

        // Upper bound derives ONLY from operator-declared topology, never
        // from the on-the-fly `validators.len()`. Producer-backed phantoms
        // cannot raise this ceiling.
        let upper_bound = match (self.configured_validators, &self.allowed_validators) {
            (Some(configured), _) => configured,
            (None, Some(allowed)) => allowed.len(),
            (None, None) => unreachable!("guarded by is_topology_configured above"),
        };
        let active = self.active_validator_count(current_round);

        // Compute the static threshold as the safety baseline.
        let static_threshold = self.quorum_threshold();

        // If we have insufficient liveness data (fewer producing validators
        // than min_validators), fall back to the static threshold. This preserves
        // the conservative behavior when the liveness map is incomplete or empty.
        if active < self.min_validators {
            return static_threshold;
        }

        // Effective count is min(configured, active) — can shrink but never
        // exceed the configured count.
        let effective = active.min(upper_bound);

        // Return whichever threshold is smaller (adaptive can only lower, never raise).
        let adaptive_threshold = (2 * effective).div_ceil(3);
        adaptive_threshold.min(static_threshold)
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

    /// Get a reference to the active validator address set.
    /// Used by FinalityTracker to filter descendant bitmaps to only active validators.
    pub fn active_addresses(&self) -> &HashSet<Address> {
        &self.validators
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
        // Permissionless mode fails closed.
        assert_eq!(vs.quorum_threshold(), usize::MAX);

        vs.set_configured_validators(4);
        // ceil(8/3) = 3
        assert_eq!(vs.quorum_threshold(), 3);
        assert!(vs.has_quorum(3));
        assert!(!vs.has_quorum(2));
    }

    #[test]
    fn threshold_with_3_validators() {
        let mut vs = ValidatorSet::new(3);
        vs.set_configured_validators(3);
        for _ in 0..3 {
            vs.register(SecretKey::generate().address());
        }
        // ceil(6/3) = 2
        assert_eq!(vs.quorum_threshold(), 2);
    }

    #[test]
    fn permissionless_mode_fails_closed() {
        // SECURITY: without configured count or allowlist, thresholds must
        // return usize::MAX so finality cannot progress. Registering and/or
        // "producing" phantom validators must not unstall the network.
        let mut vs = ValidatorSet::new(3);
        for _ in 0..10 {
            let addr = SecretKey::generate().address();
            vs.register(addr);
            vs.record_production(addr, 42);
        }
        assert_eq!(vs.quorum_threshold(), usize::MAX);
        assert_eq!(vs.adaptive_quorum_threshold(42), usize::MAX);
        assert!(!vs.has_quorum(100));
    }

    #[test]
    fn allowlist_alone_enables_threshold() {
        let mut vs = ValidatorSet::new(3);
        let sks: Vec<SecretKey> = (0..4).map(|_| SecretKey::generate()).collect();
        let allowed: HashSet<Address> = sks.iter().map(|s| s.address()).collect();
        vs.set_allowed_validators(allowed);
        for sk in &sks {
            vs.register(sk.address());
        }
        // Allowlist size 4 -> ceil(8/3) = 3
        assert_eq!(vs.quorum_threshold(), 3);
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
