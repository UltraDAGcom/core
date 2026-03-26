use crate::address::Address;
use crate::consensus::finality::FinalityTracker;
use crate::state::engine::StateEngine;

/// Synchronize the FinalityTracker with the StateEngine's active validator set.
/// Called after applying finalized vertices when an epoch boundary is crossed.
///
/// When staking is active (active set is non-empty), the FinalityTracker is updated
/// to use only the epoch's active validators for quorum calculations.
/// When staking is not yet active, no changes are made (permissionless mode).
pub fn sync_epoch_validators(finality: &mut FinalityTracker, state: &StateEngine) {
    let active_set = state.active_validators();
    if active_set.is_empty() {
        // Pre-staking mode: keep permissionless validator registration
        return;
    }

    // Update FinalityTracker to use only the epoch's active validators
    let allowed: std::collections::HashSet<Address> =
        active_set.iter().copied().collect();
    let count = allowed.len();
    finality.set_allowed_validators(allowed);
    finality.set_configured_validators(count);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address::SecretKey;
    use crate::tx::stake::MIN_STAKE_SATS;

    #[test]
    fn sync_noop_when_no_stakers() {
        let mut ft = FinalityTracker::new(3);
        let state = StateEngine::new();

        // Register some validators in permissionless mode
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        let sk3 = SecretKey::generate();
        ft.register_validator(sk1.address());
        ft.register_validator(sk2.address());
        ft.register_validator(sk3.address());

        sync_epoch_validators(&mut ft, &state);

        // Should still have 3 validators (no change)
        assert_eq!(ft.validator_count(), 3);
    }

    #[test]
    fn sync_restricts_to_active_set() {
        let mut ft = FinalityTracker::new(2);
        let mut state = StateEngine::new_with_genesis();

        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        let sk3 = SecretKey::generate();

        // Register all 3 in FinalityTracker
        ft.register_validator(sk1.address());
        ft.register_validator(sk2.address());
        ft.register_validator(sk3.address());
        assert_eq!(ft.validator_count(), 3);

        // Fund and stake only sk1 and sk2
        let council_min = crate::tx::stake::MIN_STAKE_SATS;
        state.faucet_credit(&sk1.address(), council_min).unwrap();
        state.faucet_credit(&sk2.address(), council_min).unwrap();
        state.apply_stake_tx(&make_stake_tx(&sk1, council_min, 0)).unwrap();
        state.apply_stake_tx(&make_stake_tx(&sk2, council_min, 0)).unwrap();
        // Add sk1 and sk2 as council members (required by Council of 21)
        state.add_council_member(sk1.address(), crate::governance::CouncilSeatCategory::Engineering).unwrap();
        state.add_council_member(sk2.address(), crate::governance::CouncilSeatCategory::Engineering).unwrap();
        state.recalculate_active_set();

        // Now sync — should restrict FinalityTracker to only sk1 and sk2
        sync_epoch_validators(&mut ft, &state);

        assert_eq!(ft.validator_count(), 2);
        // sk3 should no longer be registered
        assert!(!ft.validator_set().contains(&sk3.address()));
    }

    #[test]
    fn sync_updates_configured_count() {
        let mut ft = FinalityTracker::new(2);
        let mut state = StateEngine::new_with_genesis();

        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();

        let council_min = crate::tx::stake::MIN_STAKE_SATS;
        state.faucet_credit(&sk1.address(), council_min).unwrap();
        state.faucet_credit(&sk2.address(), council_min).unwrap();
        state.apply_stake_tx(&make_stake_tx(&sk1, council_min, 0)).unwrap();
        state.apply_stake_tx(&make_stake_tx(&sk2, council_min, 0)).unwrap();
        state.add_council_member(sk1.address(), crate::governance::CouncilSeatCategory::Engineering).unwrap();
        state.add_council_member(sk2.address(), crate::governance::CouncilSeatCategory::Engineering).unwrap();
        state.recalculate_active_set();

        ft.register_validator(sk1.address());
        ft.register_validator(sk2.address());

        sync_epoch_validators(&mut ft, &state);

        // Configured count should be 2
        assert_eq!(ft.validator_set().configured_validators(), Some(2));
        // Quorum = ceil(4/3) = 2
        assert_eq!(ft.finality_threshold(), 2);
    }

    fn make_stake_tx(sk: &SecretKey, amount: u64, nonce: u64) -> crate::tx::stake::StakeTx {
        use crate::address::Signature;
        let mut tx = crate::tx::stake::StakeTx {
            from: sk.address(),
            amount,
            nonce,
            pub_key: sk.verifying_key().to_bytes(),
            signature: Signature([0u8; 64]),
        };
        tx.signature = sk.sign(&tx.signable_bytes());
        tx
    }
}
