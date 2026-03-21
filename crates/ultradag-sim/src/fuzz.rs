use ultradag_coin::{
    Address, SecretKey, Signature, Transaction, TransferTx,
    Block, BlockHeader, CoinbaseTx, DagVertex, K_PARENTS,
    MIN_STAKE_SATS, MIN_DELEGATION_SATS,
    constants::MIN_FEE_SATS,
};
use ultradag_coin::block::merkle_root;
use crate::validator::SimValidator;
use crate::network::{VirtualNetwork, DeliveryPolicy};
use crate::invariants;
use crate::properties;
use crate::txgen;

/// Get current timestamp for vertex creation.
/// Ensures vertices pass timestamp validation (within 5 min past, 1 min future).
fn current_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

// === Action types ===

#[derive(Debug, Clone)]
pub enum Action {
    ProduceNormal,
    Equivocate,
    IncludeStaleTxs { count: u8 },
    StallFinality,
    Skip,
    Stake,
    Unstake,
    Delegate { target_offset: u8 },
    Undelegate,
    SetCommission { percent: u8 },
    Transfer { amount_fraction: u8 },
    BadTimestamp { offset: i64 },
}

#[derive(Debug, Clone)]
pub struct RoundPlan {
    pub byzantine_actions: Vec<Action>,
    pub inject_txs: Vec<TxInjection>,
}

#[derive(Debug, Clone)]
pub enum TxInjection {
    Transfer { from_idx: u8, to_idx: u8, amount_fraction: u8 },
    Stake { from_idx: u8 },
    Unstake { from_idx: u8 },
    Delegate { from_idx: u8, to_idx: u8 },
    Undelegate { from_idx: u8 },
}

// === Execution engine ===

#[derive(Clone)]
pub struct FuzzConfig {
    pub num_honest: usize,
    pub num_byzantine: usize,
    pub delivery_policy: DeliveryPolicy,
}

#[allow(clippy::needless_range_loop)]
pub fn execute_fuzz(
    config: &FuzzConfig,
    plans: &[RoundPlan],
    seed: u64,
) -> Result<(), String> {
    let total = config.num_honest + config.num_byzantine;

    let mut validators: Vec<SimValidator> = (0..total)
        .map(|i| {
            let sk = SecretKey::from_bytes([(i as u8).wrapping_add(1); 32]);
            let mut v = SimValidator::new(i, sk, 1, total as u64);
            v.honest = i < config.num_honest;
            v
        })
        .collect();

    let all_addrs: Vec<Address> = validators.iter().map(|v| v.address).collect();
    for v in &mut validators {
        for addr in &all_addrs {
            v.finality.register_validator(*addr);
        }
    }

    let mut network = VirtualNetwork::new(total, config.delivery_policy.clone(), seed);
    let mut known_equivocators: Vec<Address> = Vec::new();

    for (round_idx, plan) in plans.iter().enumerate() {
        let round = (round_idx as u64) + 1;

        network.deliver(round);

        for i in 0..total {
            let messages = network.drain_inbox(i);
            for vertex in messages {
                if let Err(ultradag_coin::consensus::dag::DagInsertError::Equivocation { validator: v, .. }) = validators[i].receive_vertex(vertex) {
                    if !known_equivocators.contains(&v) {
                        known_equivocators.push(v);
                    }
                }
            }
        }

        for injection in &plan.inject_txs {
            if let Some(tx) = execute_injection(injection, &validators) {
                for v in validators.iter_mut().filter(|v| v.honest) {
                    v.add_transaction(tx.clone());
                }
            }
        }

        for i in 0..config.num_honest {
            let vertex = validators[i].produce_vertex(round);
            network.broadcast(i, vertex);
        }

        for (byz_idx, action) in plan.byzantine_actions.iter().enumerate() {
            let val_idx = config.num_honest + byz_idx;
            if val_idx < total {
                execute_byzantine_action(action, val_idx, round, &mut validators, &mut network, total);
            }
        }

        for v in &mut validators {
            v.run_finality();
        }

        if round % 10 == 0 || round == plans.len() as u64 {
            invariants::check_safety_invariants(&validators, &known_equivocators)?;
            properties::check_all_properties(&validators)?;
        }
    }

    Ok(())
}

fn execute_injection(injection: &TxInjection, validators: &[SimValidator]) -> Option<Transaction> {
    match injection {
        TxInjection::Transfer { from_idx, to_idx, amount_fraction } => {
            let from = *from_idx as usize % validators.len();
            let to = *to_idx as usize % validators.len();
            let v = &validators[from];
            let balance = v.state.balance(&v.address);
            let nonce = v.state.nonce(&v.address);
            let amount = (balance / 100).saturating_mul(*amount_fraction as u64);
            if amount == 0 || balance < amount + MIN_FEE_SATS { return None; }
            let to_addr = validators[to].address;
            txgen::generate_transfer_to(&v.sk, to_addr, amount, nonce)
        }
        TxInjection::Stake { from_idx } => {
            let from = *from_idx as usize % validators.len();
            let v = &validators[from];
            if v.state.balance(&v.address) < MIN_STAKE_SATS { return None; }
            let nonce = v.state.nonce(&v.address);
            Some(txgen::generate_stake_tx(&v.sk, MIN_STAKE_SATS, nonce))
        }
        TxInjection::Unstake { from_idx } => {
            let from = *from_idx as usize % validators.len();
            let v = &validators[from];
            if v.state.stake_of(&v.address) == 0 { return None; }
            let nonce = v.state.nonce(&v.address);
            Some(txgen::generate_unstake_tx(&v.sk, nonce))
        }
        TxInjection::Delegate { from_idx, to_idx } => {
            let from = *from_idx as usize % validators.len();
            let to = *to_idx as usize % validators.len();
            if from == to { return None; }
            let v = &validators[from];
            if v.state.balance(&v.address) < MIN_DELEGATION_SATS { return None; }
            let nonce = v.state.nonce(&v.address);
            let to_addr = validators[to].address;
            Some(txgen::generate_delegate_tx(&v.sk, to_addr, MIN_DELEGATION_SATS, nonce))
        }
        TxInjection::Undelegate { from_idx } => {
            let from = *from_idx as usize % validators.len();
            let v = &validators[from];
            v.state.delegation_account(&v.address)?;
            let nonce = v.state.nonce(&v.address);
            Some(txgen::generate_undelegate_tx(&v.sk, nonce))
        }
    }
}

fn execute_byzantine_action(
    action: &Action,
    val_idx: usize,
    round: u64,
    validators: &mut [SimValidator],
    network: &mut VirtualNetwork,
    total: usize,
) {
    match action {
        Action::ProduceNormal => {
            let vertex = validators[val_idx].produce_vertex(round);
            network.broadcast(val_idx, vertex);
        }
        Action::Equivocate => {
            let v1 = validators[val_idx].produce_vertex(round);
            network.broadcast(val_idx, v1);
            let parents = get_parents(&validators[val_idx], round);
            let current_ts = current_timestamp();
            let block = build_block(&validators[val_idx], round, &parents, vec![], current_ts);
            let v2 = build_and_sign_vertex(&validators[val_idx], block, parents, round);
            network.broadcast(val_idx, v2);
        }
        Action::IncludeStaleTxs { count } => {
            let parents = get_parents(&validators[val_idx], round);
            let v = &validators[val_idx];
            let pub_key = v.sk.verifying_key().to_bytes();
            let mut txs: Vec<Transaction> = v.mempool.best(50);
            for i in 0..*count as u64 {
                let mut stale = TransferTx {
                    from: v.address, to: Address([0xDD; 20]), amount: 1,
                    fee: MIN_FEE_SATS, nonce: i, pub_key,
                    signature: Signature([0u8; 64]), memo: None,
                };
                stale.signature = v.sk.sign(&stale.signable_bytes());
                txs.push(Transaction::Transfer(stale));
            }
            let current_ts = current_timestamp();
            let block = build_block(&validators[val_idx], round, &parents, txs, current_ts);
            let vertex = build_and_sign_vertex(&validators[val_idx], block, parents, round);
            validators[val_idx].dag.insert(vertex.clone());
            validators[val_idx].finality.register_validator(validators[val_idx].address);
            network.broadcast(val_idx, vertex);
        }
        Action::StallFinality => {
            let v = &validators[val_idx];
            let parents = if round <= 1 {
                vec![[0u8; 32]]
            } else {
                let own: Vec<[u8; 32]> = v.dag.vertices_in_round(round - 1).iter()
                    .filter(|vtx| vtx.validator == v.address)
                    .map(|vtx| vtx.hash())
                    .collect();
                if own.is_empty() { vec![[0u8; 32]] } else { own }
            };
            let current_ts = current_timestamp();
            let block = build_block(&validators[val_idx], round, &parents, vec![], current_ts);
            let vertex = build_and_sign_vertex(&validators[val_idx], block, parents, round);
            validators[val_idx].dag.insert(vertex.clone());
            validators[val_idx].finality.register_validator(validators[val_idx].address);
            network.broadcast(val_idx, vertex);
        }
        Action::Skip => {}
        Action::Stake => {
            let v = &validators[val_idx];
            if v.state.balance(&v.address) >= MIN_STAKE_SATS && v.state.stake_of(&v.address) == 0 {
                let nonce = v.state.nonce(&v.address);
                let tx = txgen::generate_stake_tx(&v.sk, MIN_STAKE_SATS, nonce);
                validators[val_idx].mempool.insert(tx);
            }
            let vertex = validators[val_idx].produce_vertex(round);
            network.broadcast(val_idx, vertex);
        }
        Action::Unstake => {
            let v = &validators[val_idx];
            if v.state.stake_of(&v.address) > 0 {
                let nonce = v.state.nonce(&v.address);
                let tx = txgen::generate_unstake_tx(&v.sk, nonce);
                validators[val_idx].mempool.insert(tx);
            }
            let vertex = validators[val_idx].produce_vertex(round);
            network.broadcast(val_idx, vertex);
        }
        Action::Delegate { target_offset } => {
            let target_idx = (*target_offset as usize) % total;
            let target_addr = validators[target_idx].address;
            let v = &validators[val_idx];
            if target_addr != v.address && v.state.balance(&v.address) >= MIN_DELEGATION_SATS && v.state.delegation_account(&v.address).is_none() {
                let nonce = v.state.nonce(&v.address);
                let tx = txgen::generate_delegate_tx(&v.sk, target_addr, MIN_DELEGATION_SATS, nonce);
                validators[val_idx].mempool.insert(tx);
            }
            let vertex = validators[val_idx].produce_vertex(round);
            network.broadcast(val_idx, vertex);
        }
        Action::Undelegate => {
            let v = &validators[val_idx];
            if v.state.delegation_account(&v.address).is_some() {
                let nonce = v.state.nonce(&v.address);
                let tx = txgen::generate_undelegate_tx(&v.sk, nonce);
                validators[val_idx].mempool.insert(tx);
            }
            let vertex = validators[val_idx].produce_vertex(round);
            network.broadcast(val_idx, vertex);
        }
        Action::SetCommission { percent } => {
            let pct = (*percent).min(100);
            let nonce = validators[val_idx].state.nonce(&validators[val_idx].address);
            let sk = validators[val_idx].sk.clone();
            let tx = txgen::generate_set_commission_tx(&sk, pct, nonce);
            validators[val_idx].mempool.insert(tx);
            let vertex = validators[val_idx].produce_vertex(round);
            network.broadcast(val_idx, vertex);
        }
        Action::Transfer { amount_fraction } => {
            let v = &validators[val_idx];
            let balance = v.state.balance(&v.address);
            let amount = (balance / 100).saturating_mul(*amount_fraction as u64);
            if amount > 0 && balance >= amount + MIN_FEE_SATS {
                let nonce = v.state.nonce(&v.address);
                let to = Address([((val_idx + 100) as u8); 20]);
                if let Some(tx) = txgen::generate_transfer_to(&v.sk, to, amount, nonce) {
                    validators[val_idx].mempool.insert(tx);
                }
            }
            let vertex = validators[val_idx].produce_vertex(round);
            network.broadcast(val_idx, vertex);
        }
        Action::BadTimestamp { offset } => {
            let parents = get_parents(&validators[val_idx], round);
            // Use current timestamp with small offset (keep within validation window)
            let current_ts = current_timestamp();
            let block = build_block(&validators[val_idx], round, &parents, vec![], current_ts + (*offset).min(30));
            let vertex = build_and_sign_vertex(&validators[val_idx], block, parents, round);
            validators[val_idx].dag.insert(vertex.clone());
            validators[val_idx].finality.register_validator(validators[val_idx].address);
            network.broadcast(val_idx, vertex);
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

fn build_and_sign_vertex(validator: &SimValidator, block: Block, parents: Vec<[u8; 32]>, round: u64) -> DagVertex {
    let pub_key = validator.sk.verifying_key().to_bytes();
    let mut v = DagVertex::new(block, parents, round, validator.address, pub_key, Signature([0u8; 64]));
    v.signature = validator.sk.sign(&v.signable_bytes());
    v
}
