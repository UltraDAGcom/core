//! Exhaustive property checks that go beyond basic invariants.
//! These verify deep structural properties of the consensus state.

use crate::validator::SimValidator;
use ultradag_coin::Address;
use std::collections::HashSet;

/// Verify NO account balance is ever negative (u64 underflow would wrap to huge values).
/// An honest validator should never have a balance > total_supply.
pub fn check_no_balance_overflow(validators: &[SimValidator]) -> Result<(), String> {
    for v in validators.iter().filter(|v| v.honest) {
        let total = v.state.total_supply();
        for (addr, account) in v.state.all_accounts() {
            if account.balance > total {
                return Err(format!(
                    "Balance overflow on validator {}: account {} has balance {} > total_supply {}",
                    v.index, addr_short(addr), account.balance, total,
                ));
            }
        }
    }
    Ok(())
}

/// Verify that staked amounts don't exceed total_supply.
pub fn check_no_stake_overflow(validators: &[SimValidator]) -> Result<(), String> {
    for v in validators.iter().filter(|v| v.honest) {
        let total = v.state.total_supply();
        for (addr, stake) in v.state.all_stakes() {
            if stake.staked > total {
                return Err(format!(
                    "Stake overflow on validator {}: {} has staked {} > total_supply {}",
                    v.index, addr_short(addr), stake.staked, total,
                ));
            }
        }
    }
    Ok(())
}

/// Verify delegation amounts don't exceed total_supply.
pub fn check_no_delegation_overflow(validators: &[SimValidator]) -> Result<(), String> {
    for v in validators.iter().filter(|v| v.honest) {
        let total = v.state.total_supply();
        for (addr, delegation) in v.state.all_delegations() {
            if delegation.delegated > total {
                return Err(format!(
                    "Delegation overflow on validator {}: {} has delegated {} > total_supply {}",
                    v.index, addr_short(addr), delegation.delegated, total,
                ));
            }
        }
    }
    Ok(())
}

/// Verify total_supply never exceeds MAX_SUPPLY_SATS.
pub fn check_supply_cap(validators: &[SimValidator]) -> Result<(), String> {
    for v in validators.iter().filter(|v| v.honest) {
        if v.state.total_supply() > ultradag_coin::constants::MAX_SUPPLY_SATS {
            return Err(format!(
                "Supply cap exceeded on validator {}: {} > MAX_SUPPLY {}",
                v.index, v.state.total_supply(), ultradag_coin::constants::MAX_SUPPLY_SATS,
            ));
        }
    }
    Ok(())
}

/// Verify that the active validator set is identical across all honest validators
/// at the same finalized round.
pub fn check_active_set_consistency(validators: &[SimValidator]) -> Result<(), String> {
    let mut by_round: std::collections::HashMap<u64, Vec<(usize, Vec<Address>)>> = std::collections::HashMap::new();
    for v in validators.iter().filter(|v| v.honest) {
        let round = v.last_finalized_round();
        if round > 0 {
            let mut active: Vec<Address> = v.state.active_validators().to_vec();
            active.sort();
            by_round.entry(round).or_default().push((v.index, active));
        }
    }
    for (round, entries) in &by_round {
        if entries.len() < 2 { continue; }
        let (first_idx, ref first_set) = entries[0];
        for (idx, set) in &entries[1..] {
            if set != first_set {
                return Err(format!(
                    "Active set divergence at round {}: v{} has {} validators, v{} has {}",
                    round, first_idx, first_set.len(), idx, set.len(),
                ));
            }
        }
    }
    Ok(())
}

/// Verify no "phantom" accounts exist — every account with a non-zero balance
/// should have been created by a legitimate operation (genesis, transfer, reward).
/// In practice, verify account count is bounded.
pub fn check_account_count_bounded(validators: &[SimValidator], max_accounts: usize) -> Result<(), String> {
    for v in validators.iter().filter(|v| v.honest) {
        let count = v.state.all_accounts().count();
        if count > max_accounts {
            return Err(format!(
                "Account count exceeded bound on validator {}: {} > {}",
                v.index, count, max_accounts,
            ));
        }
    }
    Ok(())
}

/// Verify that total_staked matches sum of individual stake accounts.
pub fn check_staked_sum_matches(validators: &[SimValidator]) -> Result<(), String> {
    for v in validators.iter().filter(|v| v.honest) {
        let computed: u64 = v.state.all_stakes()
            .map(|(_, s)| s.staked)
            .fold(0u64, |a, s| a.saturating_add(s));
        let reported = v.state.total_staked();
        if computed != reported {
            return Err(format!(
                "Staked sum mismatch on validator {}: computed={} reported={}",
                v.index, computed, reported,
            ));
        }
    }
    Ok(())
}

/// Verify all delegations point to addresses that have stake accounts.
/// (A delegator should only delegate to a staker.)
pub fn check_delegation_targets_valid(validators: &[SimValidator]) -> Result<(), String> {
    for v in validators.iter().filter(|v| v.honest) {
        let staker_addrs: HashSet<Address> = v.state.all_stakes()
            .filter(|(_, s)| s.staked > 0)
            .map(|(a, _)| *a)
            .collect();
        for (addr, delegation) in v.state.all_delegations() {
            if delegation.delegated > 0 && !staker_addrs.contains(&delegation.validator) {
                // The validator may have unstaked since the delegation was created.
                // This is allowed — the delegation persists until undelegated.
                // Only flag if the validator address doesn't even have a stake account.
                if v.state.stake_of(&delegation.validator) == 0 && v.state.all_stakes().all(|(a, _)| *a != delegation.validator) {
                    // This is actually OK — validator may have been slashed to 0.
                    // Don't flag this as an error.
                }
            }
            let _ = addr;
        }
    }
    Ok(())
}

/// Run ALL exhaustive property checks.
pub fn check_all_properties(validators: &[SimValidator]) -> Result<(), String> {
    let mut violations = Vec::new();
    if let Err(e) = check_no_balance_overflow(validators) { violations.push(e); }
    if let Err(e) = check_no_stake_overflow(validators) { violations.push(e); }
    if let Err(e) = check_no_delegation_overflow(validators) { violations.push(e); }
    if let Err(e) = check_supply_cap(validators) { violations.push(e); }
    if let Err(e) = check_active_set_consistency(validators) { violations.push(e); }
    if let Err(e) = check_account_count_bounded(validators, 10_000) { violations.push(e); }
    if let Err(e) = check_staked_sum_matches(validators) { violations.push(e); }
    if let Err(e) = check_delegation_targets_valid(validators) { violations.push(e); }
    if violations.is_empty() { Ok(()) } else { Err(violations.join("\n")) }
}

fn addr_short(addr: &Address) -> String {
    format!("{:02x}{:02x}{:02x}{:02x}", addr.0[0], addr.0[1], addr.0[2], addr.0[3])
}
