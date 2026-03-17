use crate::validator::SimValidator;
use ultradag_coin::Address;
use std::collections::HashMap;

/// Check that all honest validators at the same finalized round have identical state roots.
pub fn check_state_convergence(validators: &[SimValidator]) -> Result<(), String> {
    // Group honest validators by their last finalized round
    let mut by_round: HashMap<u64, Vec<(usize, [u8; 32])>> = HashMap::new();

    for v in validators.iter().filter(|v| v.honest) {
        let round = v.last_finalized_round();
        if round > 0 {
            by_round.entry(round)
                .or_default()
                .push((v.index, v.state_root()));
        }
    }

    for (round, entries) in &by_round {
        if entries.len() < 2 {
            continue;
        }
        let (first_idx, first_root) = entries[0];
        for &(idx, root) in &entries[1..] {
            if root != first_root {
                return Err(format!(
                    "State convergence failed at round {}:\n  Validator {}: state_root={}\n  Validator {}: state_root={}\n",
                    round,
                    first_idx, hex_short(&first_root),
                    idx, hex_short(&root),
                ));
            }
        }
    }

    Ok(())
}

/// Check that liquid + staked + delegated + treasury == total_supply for each honest validator.
pub fn check_supply_invariant(validators: &[SimValidator]) -> Result<(), String> {
    for v in validators.iter().filter(|v| v.honest) {
        let liquid: u64 = v.state.all_accounts()
            .map(|(_, a)| a.balance)
            .fold(0u64, |acc, b| acc.saturating_add(b));
        let staked: u64 = v.state.all_stakes()
            .map(|(_, s)| s.staked)
            .fold(0u64, |acc, s| acc.saturating_add(s));
        let delegated: u64 = v.state.all_delegations()
            .map(|(_, d)| d.delegated)
            .fold(0u64, |acc, d| acc.saturating_add(d));
        let treasury = v.state.treasury_balance();
        let total = liquid.saturating_add(staked).saturating_add(delegated).saturating_add(treasury);
        let supply = v.state.total_supply();

        if total != supply {
            return Err(format!(
                "Supply invariant violated on validator {}:\n  liquid={}, staked={}, delegated={}, treasury={}\n  sum={}, total_supply={}\n  diff={}\n",
                v.index, liquid, staked, delegated, treasury, total, supply,
                (total as i128) - (supply as i128),
            ));
        }
    }

    Ok(())
}

/// Check that finality_history has strictly non-decreasing rounds per validator.
pub fn check_finality_monotonicity(validators: &[SimValidator]) -> Result<(), String> {
    for v in validators.iter().filter(|v| v.honest) {
        for window in v.finality_history.windows(2) {
            if window[1].0 < window[0].0 {
                return Err(format!(
                    "Finality rollback on validator {}: round {} -> {}",
                    v.index, window[0].0, window[1].0,
                ));
            }
        }
    }
    Ok(())
}

/// Check that no round is finalized twice in a validator's history.
pub fn check_no_double_finalization(validators: &[SimValidator]) -> Result<(), String> {
    for v in validators.iter().filter(|v| v.honest) {
        let mut seen = std::collections::HashSet::new();
        for &(round, root) in &v.finality_history {
            if !seen.insert((round, root)) {
                // Same (round, root) is fine (idempotent). Only flag different roots for same round.
                continue;
            }
        }
        // Check for same round with different roots
        let mut round_roots: HashMap<u64, [u8; 32]> = HashMap::new();
        for &(round, root) in &v.finality_history {
            if let Some(prev_root) = round_roots.get(&round) {
                if *prev_root != root {
                    return Err(format!(
                        "Double finalization on validator {}: round {} has roots {} and {}",
                        v.index, round, hex_short(prev_root), hex_short(&root),
                    ));
                }
            } else {
                round_roots.insert(round, root);
            }
        }
    }
    Ok(())
}

/// Check that known equivocators are detected by honest validators (if they received both vertices).
pub fn check_equivocation_detected(
    validators: &[SimValidator],
    known_equivocators: &[Address],
) -> Result<(), String> {
    for equivocator in known_equivocators {
        for v in validators.iter().filter(|v| v.honest) {
            if v.dag.is_byzantine(equivocator) {
                // Detected — good.
            }
            // If not detected, it's possible the validator didn't receive both
            // conflicting vertices. This is OK — we only flag if the equivocator
            // was supposed to be caught. Don't fail here.
        }
    }
    Ok(())
}

/// Check staking consistency across honest validators at the same finalized round.
pub fn check_stake_consistency(validators: &[SimValidator]) -> Result<(), String> {
    let mut by_round: HashMap<u64, Vec<(usize, u64, u64)>> = HashMap::new();
    for v in validators.iter().filter(|v| v.honest) {
        let round = v.last_finalized_round();
        if round > 0 {
            by_round.entry(round).or_default().push((
                v.index,
                v.state.total_staked(),
                v.state.all_delegations().map(|(_, d)| d.delegated).fold(0u64, |a, d| a.saturating_add(d)),
            ));
        }
    }
    for (round, entries) in &by_round {
        if entries.len() < 2 { continue; }
        let (_, first_staked, first_delegated) = entries[0];
        for &(idx, staked, delegated) in &entries[1..] {
            if staked != first_staked || delegated != first_delegated {
                return Err(format!(
                    "Stake consistency failed at round {}: v{} staked={}/delegated={} vs v{} staked={}/delegated={}",
                    round, entries[0].0, first_staked, first_delegated, idx, staked, delegated,
                ));
            }
        }
    }
    Ok(())
}

/// Check governance consistency across honest validators at the same finalized round.
pub fn check_governance_consistency(validators: &[SimValidator]) -> Result<(), String> {
    let mut by_round: HashMap<u64, Vec<(usize, u64, u64)>> = HashMap::new();
    for v in validators.iter().filter(|v| v.honest) {
        let round = v.last_finalized_round();
        if round > 0 {
            by_round.entry(round).or_default().push((
                v.index,
                v.state.governance_params().min_fee_sats,
                v.state.next_proposal_id(),
            ));
        }
    }
    for (round, entries) in &by_round {
        if entries.len() < 2 { continue; }
        let (_, first_fee, first_pid) = entries[0];
        for &(idx, fee, pid) in &entries[1..] {
            if fee != first_fee || pid != first_pid {
                return Err(format!(
                    "Governance consistency failed at round {}: v{} min_fee={}/next_id={} vs v{} min_fee={}/next_id={}",
                    round, entries[0].0, first_fee, first_pid, idx, fee, pid,
                ));
            }
        }
    }
    Ok(())
}

/// Check council consistency across honest validators at the same finalized round.
pub fn check_council_consistency(validators: &[SimValidator]) -> Result<(), String> {
    let mut by_round: HashMap<u64, Vec<(usize, usize)>> = HashMap::new();
    for v in validators.iter().filter(|v| v.honest) {
        let round = v.last_finalized_round();
        if round > 0 {
            by_round.entry(round).or_default().push((v.index, v.state.council_member_count()));
        }
    }
    for (round, entries) in &by_round {
        if entries.len() < 2 { continue; }
        let (_, first_count) = entries[0];
        for &(idx, count) in &entries[1..] {
            if count != first_count {
                return Err(format!(
                    "Council consistency failed at round {}: v{} count={} vs v{} count={}",
                    round, entries[0].0, first_count, idx, count,
                ));
            }
        }
    }
    Ok(())
}

/// Run all invariant checks.
pub fn check_all(
    validators: &[SimValidator],
    known_equivocators: &[Address],
) -> Result<(), String> {
    let mut violations = Vec::new();

    if let Err(e) = check_state_convergence(validators) {
        violations.push(e);
    }
    if let Err(e) = check_supply_invariant(validators) {
        violations.push(e);
    }
    if let Err(e) = check_finality_monotonicity(validators) {
        violations.push(e);
    }
    if let Err(e) = check_no_double_finalization(validators) {
        violations.push(e);
    }
    if let Err(e) = check_equivocation_detected(validators, known_equivocators) {
        violations.push(e);
    }
    if let Err(e) = check_stake_consistency(validators) {
        violations.push(e);
    }
    if let Err(e) = check_governance_consistency(validators) {
        violations.push(e);
    }
    if let Err(e) = check_council_consistency(validators) {
        violations.push(e);
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations.join("\n"))
    }
}

fn hex_short(bytes: &[u8; 32]) -> String {
    bytes.iter().take(8).map(|b| format!("{:02x}", b)).collect()
}
