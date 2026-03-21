use ultradag_coin::{
    Address, SecretKey, DagVertex, Block, BlockHeader, CoinbaseTx, Signature,
    Transaction, TransferTx, K_PARENTS, MIN_STAKE_SATS, MIN_DELEGATION_SATS,
};
use ultradag_coin::block::merkle_root;
use ultradag_coin::constants::MIN_FEE_SATS;
use ultradag_coin::governance::ProposalType;
use crate::validator::SimValidator;
use crate::txgen;

/// A Byzantine strategy determines what a dishonest validator does each round.
#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
pub enum ByzantineStrategy {
    /// Produce two different vertices for the same round (equivocation).
    Equivocator,
    /// Produce a vertex but only send it to validators in `targets` (by index).
    Withholder { targets: Vec<usize> },
    /// Don't produce any vertex (offline/crashed).
    Crash,
    /// Produce vertices with far-future timestamps.
    TimestampManipulator { offset_secs: i64 },
    /// Adaptive reward manipulation using stake/delegation timing.
    RewardGambler { puppet_sk: SecretKey, puppet_address: Address },
    /// Malicious council member who floods extreme parameter change proposals.
    GovernanceTakeover,
    /// Include transactions with stale nonces to stress fee clawback.
    DuplicateTxFlooder,
    /// Adaptive finality stalling — produce vertices that avoid contributing
    /// to honest vertices' descendant counts.
    FinalityStaller,
    /// Equivocate with different transactions in each vertex, sent to different halves.
    SelectiveEquivocator,
}

/// Returns a list of (vertex, optional_target_subset).
pub fn produce_vertices(
    strategy: &ByzantineStrategy,
    validator: &mut SimValidator,
    round: u64,
    num_validators: usize,
) -> Vec<(DagVertex, Option<Vec<usize>>)> {
    match strategy {
        ByzantineStrategy::Equivocator => produce_equivocation(validator, round),
        ByzantineStrategy::Withholder { targets } => {
            let v = validator.produce_vertex(round);
            vec![(v, Some(targets.clone()))]
        }
        ByzantineStrategy::Crash => vec![],
        ByzantineStrategy::TimestampManipulator { offset_secs } => {
            produce_timestamp_manipulated(validator, round, *offset_secs)
        }
        ByzantineStrategy::RewardGambler { puppet_sk, puppet_address } => {
            produce_reward_gambler(validator, round, puppet_sk, puppet_address)
        }
        ByzantineStrategy::GovernanceTakeover => {
            produce_governance_takeover(validator, round)
        }
        ByzantineStrategy::DuplicateTxFlooder => {
            produce_duplicate_tx_flood(validator, round)
        }
        ByzantineStrategy::FinalityStaller => {
            produce_finality_staller(validator, round)
        }
        ByzantineStrategy::SelectiveEquivocator => {
            produce_selective_equivocation(validator, round, num_validators)
        }
    }
}

fn get_parents(validator: &SimValidator, round: u64) -> Vec<[u8; 32]> {
    if round <= 1 {
        vec![[0u8; 32]]
    } else {
        let selected = validator.dag.select_parents(&validator.address, round - 1, K_PARENTS);
        if selected.is_empty() { vec![[0u8; 32]] } else { selected }
    }
}

fn build_and_sign_vertex(
    validator: &SimValidator,
    block: Block,
    parents: Vec<[u8; 32]>,
    round: u64,
) -> DagVertex {
    let pub_key = validator.sk.verifying_key().to_bytes();
    let mut v = DagVertex::new(block, parents, round, validator.address, pub_key, Signature([0u8; 64]));
    v.signature = validator.sk.sign(&v.signable_bytes());
    v
}

fn build_block(validator: &SimValidator, round: u64, parents: &[[u8; 32]], txs: Vec<Transaction>, timestamp: i64) -> Block {
    let coinbase = CoinbaseTx { to: validator.address, amount: 0, height: round };
    let mut leaves = vec![coinbase.hash()];
    for tx in &txs { leaves.push(tx.hash()); }
    let mr = merkle_root(&leaves);
    let prev_hash = parents.first().copied().unwrap_or([0u8; 32]);
    Block {
        header: BlockHeader { version: 1, height: round, timestamp, prev_hash, merkle_root: mr },
        coinbase,
        transactions: txs,
    }
}

// === Existing strategies ===

fn produce_equivocation(validator: &mut SimValidator, round: u64) -> Vec<(DagVertex, Option<Vec<usize>>)> {
    let v1 = validator.produce_vertex(round);
    let parents = get_parents(validator, round);
    // Use current timestamp for validation (within acceptable window)
    let current_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let block = build_block(validator, round, &parents, vec![], current_timestamp);
    let v2 = build_and_sign_vertex(validator, block, parents, round);
    vec![(v1, None), (v2, None)]
}

fn produce_timestamp_manipulated(validator: &mut SimValidator, round: u64, offset: i64) -> Vec<(DagVertex, Option<Vec<usize>>)> {
    let parents = get_parents(validator, round);
    let txs = validator.mempool.best(100);
    // Use current timestamp with offset (keep within validation window)
    let current_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let block = build_block(validator, round, &parents, txs, current_timestamp + offset.min(30));
    let v = build_and_sign_vertex(validator, block, parents.clone(), round);
    validator.dag.insert(v.clone());
    validator.finality.register_validator(validator.address);
    vec![(v, None)]
}

// === New strategies ===

fn produce_reward_gambler(
    validator: &mut SimValidator,
    round: u64,
    puppet_sk: &SecretKey,
    puppet_address: &Address,
) -> Vec<(DagVertex, Option<Vec<usize>>)> {
    // The attacker follows the protocol honestly but includes staking txs
    let mut extra_txs = Vec::new();
    let addr = validator.address;
    let balance = validator.state.balance(&addr);
    let staked = validator.state.stake_of(&addr);
    let nonce = validator.state.nonce(&addr);

    if staked == 0 && balance >= MIN_STAKE_SATS {
        // Stake to start earning proportional rewards
        extra_txs.push(txgen::generate_stake_tx(&validator.sk, MIN_STAKE_SATS, nonce));
    } else if staked > 0 {
        // Try to delegate puppet funds to self for more effective stake
        let puppet_bal = validator.state.balance(puppet_address);
        let puppet_nonce = validator.state.nonce(puppet_address);
        let puppet_delegation = validator.state.delegation_account(puppet_address);
        if puppet_delegation.is_none() && puppet_bal >= MIN_DELEGATION_SATS {
            extra_txs.push(txgen::generate_delegate_tx(puppet_sk, addr, MIN_DELEGATION_SATS, puppet_nonce));
        }
    }

    // Add extra txs to mempool so they get included in the vertex
    for tx in &extra_txs {
        validator.mempool.insert(tx.clone());
    }

    let v = validator.produce_vertex(round);
    vec![(v, None)]
}

fn produce_governance_takeover(
    validator: &mut SimValidator,
    round: u64,
) -> Vec<(DagVertex, Option<Vec<usize>>)> {
    let addr = validator.address;
    let balance = validator.state.balance(&addr);
    let nonce = validator.state.nonce(&addr);

    // Only create proposals if we're a council member and have balance for fee
    if validator.state.is_council_member(&addr) && balance >= MIN_FEE_SATS {
        let proposal_id = validator.state.next_proposal_id();
        let param_change = match round % 4 {
            0 => ProposalType::ParameterChange { param: "slash_percent".into(), new_value: "10".into() },
            1 => ProposalType::ParameterChange { param: "min_fee_sats".into(), new_value: "100000000".into() },
            2 => ProposalType::ParameterChange { param: "observer_reward_percent".into(), new_value: "100".into() },
            _ => ProposalType::ParameterChange { param: "council_emission_percent".into(), new_value: "30".into() },
        };

        let proposal_tx = txgen::generate_create_proposal_tx(
            &validator.sk, proposal_id, param_change, MIN_FEE_SATS, nonce,
        );
        validator.mempool.insert(proposal_tx);

        // Also vote YES on any active proposal we haven't voted on
        if proposal_id > 0 {
            let vote_tx = txgen::generate_vote_tx(
                &validator.sk, proposal_id - 1, true, MIN_FEE_SATS, nonce + 1,
            );
            validator.mempool.insert(vote_tx);
        }
    }

    let v = validator.produce_vertex(round);
    vec![(v, None)]
}

fn produce_duplicate_tx_flood(
    validator: &mut SimValidator,
    round: u64,
) -> Vec<(DagVertex, Option<Vec<usize>>)> {
    // Produce a vertex with stale-nonce transactions to stress fee clawback
    let parents = get_parents(validator, round);

    // Get normal mempool txs
    let mut txs = validator.mempool.best(50);

    // Add 5 stale-nonce transfers (nonce=0, always stale after round 1)
    let addr = validator.address;
    let pub_key = validator.sk.verifying_key().to_bytes();
    for i in 0..5u64 {
        let mut stale_tx = TransferTx {
            from: addr,
            to: Address([0xDD; 20]),
            amount: 1,
            fee: MIN_FEE_SATS,
            nonce: i, // Will be stale after first few rounds
            pub_key,
            signature: Signature([0u8; 64]),
            memo: None,
        };
        stale_tx.signature = validator.sk.sign(&stale_tx.signable_bytes());
        txs.push(Transaction::Transfer(stale_tx));
    }

    let block = build_block(validator, round, &parents, txs, 1_000_000 + round as i64);
    let v = build_and_sign_vertex(validator, block, parents.clone(), round);
    validator.dag.insert(v.clone());
    validator.finality.register_validator(validator.address);
    vec![(v, None)]
}

fn produce_finality_staller(
    validator: &mut SimValidator,
    round: u64,
) -> Vec<(DagVertex, Option<Vec<usize>>)> {
    // Produce vertex with parents that DON'T help honest vertices finalize.
    // Only reference own previous vertices or the genesis sentinel.
    let parents = if round <= 1 {
        vec![[0u8; 32]]
    } else {
        // Try to find our own vertex from the previous round
        let prev_vertices = validator.dag.vertices_in_round(round - 1);
        let own_parents: Vec<[u8; 32]> = prev_vertices.iter()
            .filter(|v| v.validator == validator.address)
            .map(|v| v.hash())
            .collect();
        if own_parents.is_empty() {
            // Fall back to genesis if we have no previous vertices
            vec![[0u8; 32]]
        } else {
            own_parents
        }
    };

    let block = build_block(validator, round, &parents, vec![], 1_000_000 + round as i64);
    let v = build_and_sign_vertex(validator, block, parents, round);
    validator.dag.insert(v.clone());
    validator.finality.register_validator(validator.address);
    vec![(v, None)]
}

fn produce_selective_equivocation(
    validator: &mut SimValidator,
    round: u64,
    _num_validators: usize,
) -> Vec<(DagVertex, Option<Vec<usize>>)> {
    let parents = get_parents(validator, round);
    let addr = validator.address;
    let pub_key = validator.sk.verifying_key().to_bytes();

    // Build vertex A: transfer to address 0xAA
    let mut tx_a = TransferTx {
        from: addr, to: Address([0xAA; 20]), amount: 1,
        fee: MIN_FEE_SATS, nonce: round, // Use round as nonce (will be wrong but that's fine)
        pub_key, signature: Signature([0u8; 64]), memo: None,
    };
    tx_a.signature = validator.sk.sign(&tx_a.signable_bytes());

    let block_a = build_block(validator, round, &parents, vec![Transaction::Transfer(tx_a)], 1_000_000 + round as i64);
    let v_a = build_and_sign_vertex(validator, block_a, parents.clone(), round);

    // Build vertex B: transfer to address 0xBB (different tx, different hash)
    let mut tx_b = TransferTx {
        from: addr, to: Address([0xBB; 20]), amount: 1,
        fee: MIN_FEE_SATS, nonce: round,
        pub_key, signature: Signature([0u8; 64]), memo: None,
    };
    tx_b.signature = validator.sk.sign(&tx_b.signable_bytes());

    let block_b = build_block(validator, round, &parents, vec![Transaction::Transfer(tx_b)], 1_000_002 + round as i64);
    let v_b = build_and_sign_vertex(validator, block_b, parents, round);

    // Insert v_a into own DAG
    validator.dag.insert(v_a.clone());
    validator.finality.register_validator(validator.address);

    // Send both to all — each validator's try_insert will catch the equivocation
    // when the second one arrives. The first to arrive gets inserted, the second rejected.
    vec![(v_a, None), (v_b, None)]
}
