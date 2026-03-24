use crate::address::{Address, Signature, SecretKey};
use crate::state::persistence::StateSnapshot;
use serde::{Deserialize, Serialize};

/// A checkpoint captures the full consensus and state at a specific finalized round.
/// Multiple validators sign the same checkpoint — a node accepts it when
/// it has signatures from >= quorum distinct validators.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// The finalized round this checkpoint covers.
    pub round: u64,
    /// Blake3 hash of the serialized StateSnapshot at this round.
    pub state_root: [u8; 32],
    /// Blake3 hash of the last finalized vertex at this round.
    pub dag_tip: [u8; 32],
    /// Total supply at this round (for quick sanity check).
    pub total_supply: u64,
    /// Blake3 hash of the previous checkpoint (links checkpoints into a chain).
    /// For genesis checkpoint (round 0), this is [0u8; 32].
    pub prev_checkpoint_hash: [u8; 32],
    /// Validator signatures over checkpoint_hash().
    pub signatures: Vec<CheckpointSignature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointSignature {
    pub validator: Address,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

impl Checkpoint {
    /// The bytes that validators sign over.
    /// Includes round, state_root, dag_tip, prev_checkpoint_hash — NOT the signatures field.
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"checkpoint");
        buf.extend_from_slice(&self.round.to_le_bytes());
        buf.extend_from_slice(&self.state_root);
        buf.extend_from_slice(&self.dag_tip);
        buf.extend_from_slice(&self.total_supply.to_le_bytes());
        buf.extend_from_slice(&self.prev_checkpoint_hash);
        buf
    }

    /// Hash of the signable bytes — this is what validators sign.
    pub fn checkpoint_hash(&self) -> [u8; 32] {
        *blake3::hash(&self.signable_bytes()).as_bytes()
    }

    /// Sign this checkpoint with a validator's key.
    pub fn sign(&mut self, sk: &SecretKey) {
        let sig = sk.sign(&self.signable_bytes());
        let pub_key = sk.verifying_key().to_bytes();
        self.signatures.push(CheckpointSignature {
            validator: sk.address(),
            pub_key,
            signature: sig,
        });
    }

    /// Verify all signatures and return the set of valid validator addresses.
    pub fn valid_signers(&self) -> Vec<Address> {
        self.signatures.iter()
            .filter(|s| self.verify_signature(s))
            .map(|s| s.validator)
            .collect()
    }

    /// Check if this checkpoint has >= quorum valid signatures
    /// from the provided active validator set.
    pub fn is_accepted(&self, active_validators: &[Address], quorum: usize) -> bool {
        let valid: std::collections::HashSet<Address> = self.valid_signers()
            .into_iter()
            .collect();
        let matching = active_validators.iter()
            .filter(|v| valid.contains(v))
            .count();
        matching >= quorum
    }

    pub fn verify_signature(&self, sig: &CheckpointSignature) -> bool {
        // Verify pub_key → address mapping
        let expected_addr = Address::from_pubkey(&sig.pub_key);
        if expected_addr != sig.validator { return false; }
        // Verify Ed25519 signature
        let vk = ed25519_dalek::VerifyingKey::from_bytes(&sig.pub_key).ok();
        let s = ed25519_dalek::Signature::from_bytes(&sig.signature.0);
        vk.map(|k| k.verify_strict(&self.signable_bytes(), &s).is_ok())
          .unwrap_or(false)
    }
}

/// Compute the state root hash from a StateSnapshot.
///
/// Uses a hand-rolled canonical byte representation instead of postcard serialization.
/// This ensures the state root is stable across serialization library upgrades —
/// a postcard version change cannot silently break checkpoint verification.
///
/// The canonical format hashes all fields in a fixed order using little-endian
/// integers and raw byte arrays. String fields (proposal titles, descriptions)
/// are length-prefixed. Depends on all collections being sorted before calling
/// (done in `StateEngine::snapshot()`).
///
/// VERSION PREFIX: The hash is prefixed with "ultradag-state-root-v1" so any
/// future format changes can be explicitly versioned.
pub fn compute_state_root(snapshot: &StateSnapshot) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();

    // Version prefix — changing the format requires changing this version string,
    // which makes the incompatibility explicit rather than silent.
    hasher.update(b"ultradag-state-root-v1");

    // Core financial state (fixed-size fields)
    hasher.update(&snapshot.total_supply.to_le_bytes());
    hasher.update(&snapshot.treasury_balance.to_le_bytes());
    hasher.update(&snapshot.bridge_reserve.to_le_bytes());
    hasher.update(&snapshot.current_epoch.to_le_bytes());
    hasher.update(&snapshot.next_proposal_id.to_le_bytes());

    // last_finalized_round: 0xFF sentinel for None, else round bytes
    match snapshot.last_finalized_round {
        Some(r) => {
            hasher.update(&[1u8]);
            hasher.update(&r.to_le_bytes());
        }
        None => { hasher.update(&[0u8]); }
    }

    // Accounts (sorted by address in snapshot())
    hasher.update(&(snapshot.accounts.len() as u64).to_le_bytes());
    for (addr, acct) in &snapshot.accounts {
        hasher.update(&addr.0);
        hasher.update(&acct.balance.to_le_bytes());
        hasher.update(&acct.nonce.to_le_bytes());
    }

    // Stake accounts (sorted by address in snapshot())
    hasher.update(&(snapshot.stake_accounts.len() as u64).to_le_bytes());
    for (addr, stake) in &snapshot.stake_accounts {
        hasher.update(&addr.0);
        hasher.update(&stake.staked.to_le_bytes());
        match stake.unlock_at_round {
            Some(r) => {
                hasher.update(&[1u8]);
                hasher.update(&r.to_le_bytes());
            }
            None => { hasher.update(&[0u8]); }
        }
        hasher.update(&[stake.commission_percent]);
        match stake.commission_last_changed {
            Some(r) => {
                hasher.update(&[1u8]);
                hasher.update(&r.to_le_bytes());
            }
            None => { hasher.update(&[0u8]); }
        }
    }

    // Delegation accounts (sorted by address in snapshot())
    hasher.update(&(snapshot.delegation_accounts.len() as u64).to_le_bytes());
    for (addr, deleg) in &snapshot.delegation_accounts {
        hasher.update(&addr.0);
        hasher.update(&deleg.delegated.to_le_bytes());
        hasher.update(&deleg.validator.0);
        match deleg.unlock_at_round {
            Some(r) => {
                hasher.update(&[1u8]);
                hasher.update(&r.to_le_bytes());
            }
            None => { hasher.update(&[0u8]); }
        }
    }

    // Active validator set
    hasher.update(&(snapshot.active_validator_set.len() as u64).to_le_bytes());
    for addr in &snapshot.active_validator_set {
        hasher.update(&addr.0);
    }

    // Council members (sorted by address in snapshot())
    hasher.update(&(snapshot.council_members.len() as u64).to_le_bytes());
    for (addr, category) in &snapshot.council_members {
        hasher.update(&addr.0);
        hasher.update(&[council_category_byte(category)]);
    }

    // Governance params (all u64).
    // IMPORTANT: If you add a field to GovernanceParams, you MUST add it here too.
    // Otherwise the state root will silently ignore the new field, and two nodes
    // with different values for it will compute the same hash (consensus split).
    // After adding a new field here, update the regression test in
    // state_root_regression.rs (the known-fixture hash will change).
    // Current fields (12): min_fee_sats, min_stake_to_propose, quorum_numerator,
    // approval_numerator, voting_period_rounds, execution_delay_rounds,
    // max_active_proposals, observer_reward_percent, council_emission_percent,
    // slash_percent, treasury_emission_percent, founder_emission_percent.
    hasher.update(&snapshot.governance_params.min_fee_sats.to_le_bytes());
    hasher.update(&snapshot.governance_params.min_stake_to_propose.to_le_bytes());
    hasher.update(&snapshot.governance_params.quorum_numerator.to_le_bytes());
    hasher.update(&snapshot.governance_params.approval_numerator.to_le_bytes());
    hasher.update(&snapshot.governance_params.voting_period_rounds.to_le_bytes());
    hasher.update(&snapshot.governance_params.execution_delay_rounds.to_le_bytes());
    hasher.update(&snapshot.governance_params.max_active_proposals.to_le_bytes());
    hasher.update(&snapshot.governance_params.observer_reward_percent.to_le_bytes());
    hasher.update(&snapshot.governance_params.council_emission_percent.to_le_bytes());
    hasher.update(&snapshot.governance_params.slash_percent.to_le_bytes());
    hasher.update(&snapshot.governance_params.treasury_emission_percent.to_le_bytes());
    hasher.update(&snapshot.governance_params.founder_emission_percent.to_le_bytes());

    // Proposals (sorted by ID in snapshot())
    hasher.update(&(snapshot.proposals.len() as u64).to_le_bytes());
    for (id, proposal) in &snapshot.proposals {
        hasher.update(&id.to_le_bytes());
        hasher.update(&proposal.proposer.0);
        // Length-prefixed strings for title and description
        hasher.update(&(proposal.title.len() as u32).to_le_bytes());
        hasher.update(proposal.title.as_bytes());
        hasher.update(&(proposal.description.len() as u32).to_le_bytes());
        hasher.update(proposal.description.as_bytes());
        hasher.update(&proposal.voting_starts.to_le_bytes());
        hasher.update(&proposal.voting_ends.to_le_bytes());
        hasher.update(&proposal.votes_for.to_le_bytes());
        hasher.update(&proposal.votes_against.to_le_bytes());
        hasher.update(&proposal.snapshot_total_stake.to_le_bytes());
        // Proposal type discriminant + fields
        match &proposal.proposal_type {
            crate::governance::ProposalType::TextProposal => { hasher.update(&[0u8]); }
            crate::governance::ProposalType::ParameterChange { param, new_value } => {
                hasher.update(&[1u8]);
                hasher.update(&(param.len() as u32).to_le_bytes());
                hasher.update(param.as_bytes());
                hasher.update(&(new_value.len() as u32).to_le_bytes());
                hasher.update(new_value.as_bytes());
            }
            crate::governance::ProposalType::CouncilMembership { action, address, category } => {
                hasher.update(&[2u8]);
                hasher.update(&[council_action_byte(action)]);
                hasher.update(&address.0);
                hasher.update(&[council_category_byte(category)]);
            }
            crate::governance::ProposalType::TreasurySpend { recipient, amount } => {
                hasher.update(&[3u8]);
                hasher.update(&recipient.0);
                hasher.update(&amount.to_le_bytes());
            }
            crate::governance::ProposalType::BridgeRefund { nonce } => {
                hasher.update(&[4u8]);
                hasher.update(&nonce.to_le_bytes());
            }
        }
        // Proposal status discriminant
        match &proposal.status {
            crate::governance::ProposalStatus::Active => { hasher.update(&[0u8]); }
            crate::governance::ProposalStatus::PassedPending { execute_at_round } => {
                hasher.update(&[1u8]);
                hasher.update(&execute_at_round.to_le_bytes());
            }
            crate::governance::ProposalStatus::Executed => { hasher.update(&[2u8]); }
            crate::governance::ProposalStatus::Failed { reason } => {
                hasher.update(&[3u8]);
                hasher.update(&(reason.len() as u32).to_le_bytes());
                hasher.update(reason.as_bytes());
            }
            crate::governance::ProposalStatus::Rejected => { hasher.update(&[4u8]); }
            crate::governance::ProposalStatus::Cancelled => { hasher.update(&[5u8]); }
        }
    }

    // Votes (sorted by (proposal_id, address) in snapshot())
    hasher.update(&(snapshot.votes.len() as u64).to_le_bytes());
    for ((proposal_id, voter), approve) in &snapshot.votes {
        hasher.update(&proposal_id.to_le_bytes());
        hasher.update(&voter.0);
        hasher.update(&[*approve as u8]);
    }

    // Configured validator count: affects pre-staking reward distribution (divisor).
    // Two nodes with different --validators N values would compute different rewards
    // but identical state roots without this field, allowing checkpoint co-signing
    // to succeed despite divergent financial state.
    match snapshot.configured_validator_count {
        Some(n) => {
            hasher.update(&[1u8]);
            hasher.update(&n.to_le_bytes());
        }
        None => { hasher.update(&[0u8]); }
    }

    // Bridge state: nonce and attestations are deterministic (created during consensus).
    // Bridge signatures are NOT included — they are generated locally by each validator
    // in their validator loop, so different nodes have different signature sets. Including
    // them here would cause state root divergence across nodes.
    // Bridge signatures are still persisted (redb) and included in snapshots for fast-sync,
    // but they must NOT affect the state root hash.
    hasher.update(&snapshot.bridge_nonce.to_le_bytes());

    // Bridge attestations (sorted by nonce in snapshot())
    hasher.update(&(snapshot.bridge_attestations.len() as u64).to_le_bytes());
    for (nonce, att) in &snapshot.bridge_attestations {
        hasher.update(&nonce.to_le_bytes());
        hasher.update(&att.sender.0);
        hasher.update(&att.recipient);
        hasher.update(&att.amount.to_le_bytes());
        hasher.update(&att.nonce.to_le_bytes());
        hasher.update(&att.destination_chain_id.to_le_bytes());
        hasher.update(&att.bridge_contract_address);
        hasher.update(&att.creation_round.to_le_bytes());
    }

    // Bridge contract address (C3 fix: included in state root)
    hasher.update(&snapshot.bridge_contract_address);

    // NOTE: bridge_signatures intentionally excluded from state root (C4 fix).
    // They are non-deterministic across nodes (each validator signs locally).

    // Used release nonces (sorted for determinism)
    let mut release_nonces: Vec<_> = snapshot.used_release_nonces.iter().copied().collect();
    release_nonces.sort();
    hasher.update(&(release_nonces.len() as u64).to_le_bytes());
    for (chain_id, nonce) in &release_nonces {
        hasher.update(&chain_id.to_le_bytes());
        hasher.update(&nonce.to_le_bytes());
    }

    // Bridge release votes (sorted for determinism)
    let mut release_votes: Vec<_> = snapshot.bridge_release_votes.iter().collect();
    release_votes.sort_by(|a, b| a.0.cmp(&b.0));
    hasher.update(&(release_votes.len() as u64).to_le_bytes());
    for ((chain_id, deposit_nonce), voters) in &release_votes {
        hasher.update(&chain_id.to_le_bytes());
        hasher.update(&deposit_nonce.to_le_bytes());
        hasher.update(&(voters.len() as u64).to_le_bytes());
        for voter in voters.iter() {
            hasher.update(&voter.0);
        }
    }

    *hasher.finalize().as_bytes()
}

/// Map CouncilSeatCategory to a stable byte value for canonical hashing.
fn council_category_byte(cat: &crate::governance::CouncilSeatCategory) -> u8 {
    use crate::governance::CouncilSeatCategory::*;
    match cat {
        Technical => 0,
        Business => 1,
        Legal => 2,
        Academic => 3,
        Community => 4,
        Foundation => 5,
    }
}

/// Map CouncilAction to a stable byte value for canonical hashing.
fn council_action_byte(action: &crate::governance::council::CouncilAction) -> u8 {
    use crate::governance::council::CouncilAction::*;
    match action {
        Add => 0,
        Remove => 1,
    }
}

/// Compute the hash of a checkpoint (for linking in the chain).
/// Uses deterministic raw byte construction instead of JSON serialization
/// to guarantee identical hashes across serde versions.
pub fn compute_checkpoint_hash(checkpoint: &Checkpoint) -> [u8; 32] {
    let mut buf = Vec::new();
    buf.extend_from_slice(&checkpoint.round.to_le_bytes());
    buf.extend_from_slice(&checkpoint.state_root);
    buf.extend_from_slice(&checkpoint.dag_tip);
    buf.extend_from_slice(&checkpoint.total_supply.to_le_bytes());
    buf.extend_from_slice(&checkpoint.prev_checkpoint_hash);
    *blake3::hash(&buf).as_bytes()
}

/// Verify that a checkpoint chain links correctly back to genesis.
/// 
/// This function walks the checkpoint chain backwards via prev_checkpoint_hash
/// until it reaches a checkpoint with round 0 (genesis). It then verifies that
/// the genesis checkpoint's hash matches GENESIS_CHECKPOINT_HASH.
/// 
/// # Arguments
/// * `checkpoint` - The checkpoint to verify
/// * `get_checkpoint` - Function to retrieve a checkpoint by its hash
/// 
/// # Returns
/// * `Ok(())` if the chain is valid
/// * `Err(String)` with error description if invalid
pub fn verify_checkpoint_chain<F>(
    checkpoint: &Checkpoint,
    mut get_checkpoint: F,
) -> Result<(), String>
where
    F: FnMut([u8; 32]) -> Option<Checkpoint>,
{
    // If this is genesis, verify its hash matches the constant
    // Skip verification when GENESIS_CHECKPOINT_HASH is [0u8; 32] (testnet with configurable dev key)
    let skip_genesis_check = crate::constants::GENESIS_CHECKPOINT_HASH == [0u8; 32];
    if checkpoint.round == 0 {
        if skip_genesis_check {
            return Ok(());
        }
        let genesis_hash = compute_checkpoint_hash(checkpoint);
        if genesis_hash == crate::constants::GENESIS_CHECKPOINT_HASH {
            return Ok(());
        }
        return Err(format!(
            "Genesis checkpoint hash mismatch: expected {:?}, got {:?}",
            crate::constants::GENESIS_CHECKPOINT_HASH,
            genesis_hash
        ));
    }
    
    // Walk backwards through the chain
    let mut current = checkpoint.clone();
    let mut visited = std::collections::HashSet::new();
    
    loop {
        // Detect cycles
        let current_hash = compute_checkpoint_hash(&current);
        if !visited.insert(current_hash) {
            return Err("Checkpoint chain contains a cycle".to_string());
        }
        
        // If we reached genesis, verify it
        if current.round == 0 {
            if skip_genesis_check {
                return Ok(());
            }
            let genesis_hash = compute_checkpoint_hash(&current);
            if genesis_hash == crate::constants::GENESIS_CHECKPOINT_HASH {
                return Ok(());
            }
            return Err(format!(
                "Genesis checkpoint hash mismatch: expected {:?}, got {:?}",
                crate::constants::GENESIS_CHECKPOINT_HASH,
                genesis_hash
            ));
        }
        
        // Get previous checkpoint
        let prev_hash = current.prev_checkpoint_hash;
        if prev_hash == [0u8; 32] && current.round != 0 {
            return Err(format!(
                "Non-genesis checkpoint (round {}) has zero prev_checkpoint_hash",
                current.round
            ));
        }
        
        match get_checkpoint(prev_hash) {
            Some(prev) => {
                // Verify the hash matches
                let computed_hash = compute_checkpoint_hash(&prev);
                if computed_hash != prev_hash {
                    return Err(format!(
                        "Checkpoint hash mismatch at round {}: expected {:?}, computed {:?}",
                        prev.round, prev_hash, computed_hash
                    ));
                }
                current = prev;
            }
            None => {
                return Err(format!(
                    "Missing checkpoint in chain: prev_hash {:?} not found",
                    prev_hash
                ));
            }
        }
        
        // Safety: limit chain length to prevent DoS
        if visited.len() > 10000 {
            return Err("Checkpoint chain too long (>10000 checkpoints)".to_string());
        }
    }
}
