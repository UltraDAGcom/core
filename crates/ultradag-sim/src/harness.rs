use crate::network::{VirtualNetwork, DeliveryPolicy};
use crate::validator::SimValidator;
use crate::byzantine::{ByzantineStrategy, produce_vertices};
use crate::invariants;
use crate::txgen;
use ultradag_coin::{SecretKey, Address, MIN_STAKE_SATS, MIN_DELEGATION_SATS};
use ultradag_coin::constants::MIN_FEE_SATS;
use ultradag_coin::governance::{CouncilSeatCategory, ProposalType};
use ultradag_coin::consensus::sync_epoch_validators;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;

/// Pre-scripted transaction injection scenarios.
#[derive(Clone)]
pub enum Scenario {
    /// Staking lifecycle: validators stake, earn rewards, unstake, cooldown.
    StakingLifecycle,
    /// Delegation: accounts delegate to validators, earn rewards minus commission.
    DelegationRewards,
    /// Governance: council creates proposal, votes, proposal executes parameter change.
    GovernanceParameterChange,
    /// Full cross-feature: stake + delegate + governance + equivocation simultaneously.
    CrossFeature,
    /// Epoch boundary: force epoch transition with stakers.
    EpochTransition,
}

/// Configuration for a simulation run.
pub struct SimConfig {
    pub num_honest: usize,
    pub byzantine: Vec<ByzantineStrategy>,
    pub num_rounds: u64,
    pub delivery_policy: DeliveryPolicy,
    pub seed: u64,
    pub txs_per_round: usize,
    pub check_every_round: bool,
    /// Optional scenario for scripted transaction injection.
    pub scenario: Option<Scenario>,
}

/// Result of a simulation run.
pub struct SimResult {
    pub passed: bool,
    pub rounds_completed: u64,
    pub seed: u64,
    pub violations: Vec<String>,
    pub final_state_roots: Vec<(usize, [u8; 32])>,
    pub final_finalized_rounds: Vec<(usize, u64)>,
    pub total_messages_sent: u64,
    pub total_messages_dropped: u64,
    pub equivocations_detected: usize,
    pub total_txs_applied: u64,
}

pub struct SimHarness {
    pub validators: Vec<SimValidator>,
    pub network: VirtualNetwork,
    pub byzantine_strategies: Vec<Option<ByzantineStrategy>>,
    pub current_round: u64,
    pub rng: ChaCha8Rng,
    pub trace: Vec<String>,
    pub seed: u64,
    known_equivocators: Vec<Address>,
    funded_accounts: Vec<(SecretKey, u64, u64)>,
    equivocations_detected: usize,
}

impl SimHarness {
    pub fn new(config: &SimConfig) -> Self {
        let total = config.num_honest + config.byzantine.len();
        let min_validators = if total == 1 { 1 } else { 3.min(total) };

        let mut validators = Vec::with_capacity(total);
        let mut all_addresses = Vec::with_capacity(total);

        for i in 0..total {
            let seed_byte = (i as u8).wrapping_add(1);
            let sk = SecretKey::from_bytes([seed_byte; 32]);
            all_addresses.push(sk.address());
            let mut v = SimValidator::new(i, sk, min_validators, total as u64);
            v.honest = i < config.num_honest;
            validators.push(v);
        }

        for v in &mut validators {
            for addr in &all_addresses {
                v.finality.register_validator(*addr);
            }
        }

        let mut byzantine_strategies: Vec<Option<ByzantineStrategy>> = Vec::with_capacity(total);
        for _ in 0..config.num_honest {
            byzantine_strategies.push(None);
        }
        let mut known_equivocators = Vec::new();
        for (i, strategy) in config.byzantine.iter().enumerate() {
            if matches!(strategy, ByzantineStrategy::Equivocator) {
                known_equivocators.push(all_addresses[config.num_honest + i]);
            }
            byzantine_strategies.push(Some(strategy.clone()));
        }

        let funded_accounts: Vec<(SecretKey, u64, u64)> = (0..total)
            .map(|i| {
                let seed_byte = (i as u8).wrapping_add(1);
                let sk = SecretKey::from_bytes([seed_byte; 32]);
                (sk, 0u64, 0u64)
            })
            .collect();

        // Scenario-specific setup (identical on ALL validators)
        if matches!(config.scenario, Some(Scenario::GovernanceParameterChange) | Some(Scenario::CrossFeature)) {
            let council_addrs: Vec<Address> = validators.iter()
                .take(3.min(validators.len()))
                .map(|v| v.address)
                .collect();
            for v in &mut validators {
                for addr in &council_addrs {
                    v.add_council_member(*addr, CouncilSeatCategory::Technical);
                }
                v.override_governance_param_unchecked("voting_period_rounds", 20);
                v.override_governance_param_unchecked("execution_delay_rounds", 10);
            }
        }

        let network = VirtualNetwork::new(total, config.delivery_policy.clone(), config.seed);

        Self {
            validators,
            network,
            byzantine_strategies,
            current_round: 0,
            rng: ChaCha8Rng::seed_from_u64(config.seed.wrapping_add(1000)),
            trace: Vec::new(),
            seed: config.seed,
            known_equivocators,
            funded_accounts,
            equivocations_detected: 0,
        }
    }

    pub fn run(&mut self, config: &SimConfig) -> SimResult {
        let mut violations = Vec::new();
        let mut total_txs: u64 = 0;

        for round in 1..=config.num_rounds {
            self.current_round = round;

            // 1. Deliver pending messages
            self.network.deliver(round);

            // 2. Receive phase
            for i in 0..self.validators.len() {
                let messages = self.network.drain_inbox(i);
                for vertex in messages {
                    match self.validators[i].receive_vertex(vertex) {
                        Ok(_) => {}
                        Err(ultradag_coin::consensus::dag::DagInsertError::Equivocation { .. }) => {
                            self.equivocations_detected += 1;
                        }
                        Err(_) => {}
                    }
                }
            }

            // 3. Produce phase
            for i in 0..self.validators.len() {
                match &self.byzantine_strategies[i] {
                    None => {
                        let vertex = self.validators[i].produce_vertex(round);
                        self.network.broadcast(i, vertex);
                    }
                    Some(strategy) => {
                        let strategy = strategy.clone();
                        let results = produce_vertices(&strategy, &mut self.validators[i], round);
                        for (vertex, targets) in results {
                            match targets {
                                None => self.network.broadcast(i, vertex),
                                Some(ref t) => self.network.send_to(i, vertex, t),
                            }
                        }
                    }
                }
            }

            // 4. Finality phase
            for v in &mut self.validators {
                let finalized = v.run_finality();
                total_txs += finalized.iter()
                    .map(|fv| fv.block.transactions.len() as u64)
                    .sum::<u64>();
            }

            // 5. Transaction injection
            if config.txs_per_round > 0 && round > 10 {
                self.update_funded_accounts();
                let txs = txgen::generate_round_transactions(
                    &mut self.rng,
                    &mut self.funded_accounts,
                    config.txs_per_round,
                );
                for tx in &txs {
                    for v in &mut self.validators {
                        if v.honest {
                            v.add_transaction(tx.clone());
                        }
                    }
                }
            }

            // 6. Scenario-specific transaction injection
            if let Some(ref scenario) = config.scenario {
                let txs = inject_scenario_transactions(scenario, round, &self.validators);
                for tx in txs {
                    for v in &mut self.validators {
                        if v.honest {
                            v.add_transaction(tx.clone());
                        }
                    }
                }
            }

            // 7. Scenario-specific hooks (e.g., forced epoch transitions)
            if let Some(ref scenario) = config.scenario {
                if matches!(scenario, Scenario::EpochTransition) && round == 100 {
                    self.force_epoch_transition();
                }
            }

            // 8. Invariant checking
            if config.check_every_round || round == config.num_rounds {
                if let Err(e) = invariants::check_all(&self.validators, &self.known_equivocators) {
                    violations.push(format!("Round {}: {}", round, e));
                    if config.check_every_round {
                        eprintln!("INVARIANT VIOLATION at round {} (seed: 0x{:X}):", round, self.seed);
                        eprintln!("{}", e);
                        for v in self.validators.iter().filter(|v| v.honest) {
                            eprintln!("  Validator {}: finalized_round={}, state_root={}",
                                v.index, v.last_finalized_round(), hex_short(&v.state_root()));
                        }
                        break;
                    }
                }
            }

            // 9. Pruning every 100 rounds
            if round % 100 == 0 {
                for v in &mut self.validators {
                    let last_fin = v.last_finalized_round();
                    if last_fin > 100 {
                        v.dag.prune_old_rounds(last_fin);
                        v.finality.prune_finalized(&v.dag);
                    }
                }
            }
        }

        let final_state_roots: Vec<(usize, [u8; 32])> = self.validators.iter()
            .map(|v| (v.index, v.state_root())).collect();
        let final_finalized_rounds: Vec<(usize, u64)> = self.validators.iter()
            .map(|v| (v.index, v.last_finalized_round())).collect();

        SimResult {
            passed: violations.is_empty(),
            rounds_completed: self.current_round,
            seed: self.seed,
            violations,
            final_state_roots,
            final_finalized_rounds,
            total_messages_sent: self.network.messages_sent,
            total_messages_dropped: self.network.messages_dropped,
            equivocations_detected: self.equivocations_detected,
            total_txs_applied: total_txs,
        }
    }

    fn update_funded_accounts(&mut self) {
        if let Some(v) = self.validators.iter().find(|v| v.honest) {
            for (sk, bal, nonce) in self.funded_accounts.iter_mut() {
                let addr = sk.address();
                *bal = v.state.balance(&addr);
                *nonce = v.state.nonce(&addr);
            }
        }
    }

    /// Force epoch transition on all validators (for testing active set recalculation).
    pub fn force_epoch_transition(&mut self) {
        for v in &mut self.validators {
            v.state.recalculate_active_set();
            sync_epoch_validators(&mut v.finality, &v.state);
        }
    }
}

/// Inject scenario-specific transactions at specific rounds.
fn inject_scenario_transactions(
    scenario: &Scenario,
    round: u64,
    validators: &[SimValidator],
) -> Vec<ultradag_coin::Transaction> {
    match scenario {
        Scenario::StakingLifecycle => inject_staking_lifecycle(round, validators),
        Scenario::DelegationRewards => inject_delegation_rewards(round, validators),
        Scenario::GovernanceParameterChange => inject_governance(round, validators),
        Scenario::CrossFeature => inject_cross_feature(round, validators),
        Scenario::EpochTransition => inject_epoch_transition(round, validators),
    }
}

fn inject_staking_lifecycle(round: u64, validators: &[SimValidator]) -> Vec<ultradag_coin::Transaction> {
    let mut txs = Vec::new();
    match round {
        10 => {
            // Validators 0 and 1 stake MIN_STAKE_SATS
            for v in validators.iter().take(2) {
                if v.state.balance(&v.address) >= MIN_STAKE_SATS {
                    let nonce = v.state.nonce(&v.address);
                    txs.push(txgen::generate_stake_tx(&v.sk, MIN_STAKE_SATS, nonce));
                }
            }
        }
        11 => {
            // Validator 2 stakes 2x MIN_STAKE_SATS
            if validators.len() > 2 {
                let v = &validators[2];
                if v.state.balance(&v.address) >= MIN_STAKE_SATS * 2 {
                    let nonce = v.state.nonce(&v.address);
                    txs.push(txgen::generate_stake_tx(&v.sk, MIN_STAKE_SATS * 2, nonce));
                }
            }
        }
        20 => {
            // Validator 0 sets commission to 20%
            let v = &validators[0];
            let nonce = v.state.nonce(&v.address);
            txs.push(txgen::generate_set_commission_tx(&v.sk, 20, nonce));
        }
        250 => {
            // Validator 1 unstakes
            if validators.len() > 1 {
                let v = &validators[1];
                if v.state.stake_of(&v.address) > 0 {
                    let nonce = v.state.nonce(&v.address);
                    txs.push(txgen::generate_unstake_tx(&v.sk, nonce));
                }
            }
        }
        _ => {}
    }
    txs
}

fn inject_delegation_rewards(round: u64, validators: &[SimValidator]) -> Vec<ultradag_coin::Transaction> {
    let mut txs = Vec::new();

    // Reuse staking setup from rounds 10-11
    match round {
        10 => {
            for v in validators.iter().take(2) {
                if v.state.balance(&v.address) >= MIN_STAKE_SATS {
                    let nonce = v.state.nonce(&v.address);
                    txs.push(txgen::generate_stake_tx(&v.sk, MIN_STAKE_SATS, nonce));
                }
            }
        }
        15 => {
            // Fund two delegator accounts from validator 0
            let v = &validators[0];
            let balance = v.state.balance(&v.address);
            let fund_amount = MIN_DELEGATION_SATS * 2;
            if balance >= fund_amount * 2 + MIN_FEE_SATS * 2 {
                let nonce = v.state.nonce(&v.address);
                let del1 = SecretKey::from_bytes([101u8; 32]);
                let del2 = SecretKey::from_bytes([102u8; 32]);
                // Transfer to delegator 1
                let mut tx1 = ultradag_coin::TransferTx {
                    from: v.address, to: del1.address(), amount: fund_amount,
                    fee: MIN_FEE_SATS, nonce, pub_key: v.sk.verifying_key().to_bytes(),
                    signature: ultradag_coin::Signature([0u8; 64]), memo: None,
                };
                tx1.signature = v.sk.sign(&tx1.signable_bytes());
                txs.push(ultradag_coin::Transaction::Transfer(tx1));
                // Transfer to delegator 2
                let mut tx2 = ultradag_coin::TransferTx {
                    from: v.address, to: del2.address(), amount: fund_amount,
                    fee: MIN_FEE_SATS, nonce: nonce + 1, pub_key: v.sk.verifying_key().to_bytes(),
                    signature: ultradag_coin::Signature([0u8; 64]), memo: None,
                };
                tx2.signature = v.sk.sign(&tx2.signable_bytes());
                txs.push(ultradag_coin::Transaction::Transfer(tx2));
            }
        }
        50 => {
            // Delegator 101 delegates to validator 0
            let del1 = SecretKey::from_bytes([101u8; 32]);
            let v0_addr = validators[0].address;
            let v = &validators[0]; // Read nonce from v0's state
            let del1_nonce = v.state.nonce(&del1.address());
            let del1_bal = v.state.balance(&del1.address());
            if del1_bal >= MIN_DELEGATION_SATS {
                txs.push(txgen::generate_delegate_tx(&del1, v0_addr, MIN_DELEGATION_SATS, del1_nonce));
            }
            // Delegator 102 delegates to validator 1
            if validators.len() > 1 {
                let del2 = SecretKey::from_bytes([102u8; 32]);
                let v1_addr = validators[1].address;
                let del2_nonce = v.state.nonce(&del2.address());
                let del2_bal = v.state.balance(&del2.address());
                if del2_bal >= MIN_DELEGATION_SATS {
                    txs.push(txgen::generate_delegate_tx(&del2, v1_addr, MIN_DELEGATION_SATS, del2_nonce));
                }
            }
        }
        100 => {
            // Validator 0 changes commission to 25%
            let v = &validators[0];
            let nonce = v.state.nonce(&v.address);
            txs.push(txgen::generate_set_commission_tx(&v.sk, 25, nonce));
        }
        200 => {
            // Delegator 101 undelegates
            let del1 = SecretKey::from_bytes([101u8; 32]);
            let v = &validators[0];
            let del1_nonce = v.state.nonce(&del1.address());
            if v.state.delegation_account(&del1.address()).is_some() {
                txs.push(txgen::generate_undelegate_tx(&del1, del1_nonce));
            }
        }
        _ => {}
    }
    txs
}

fn inject_governance(round: u64, validators: &[SimValidator]) -> Vec<ultradag_coin::Transaction> {
    let mut txs = Vec::new();
    match round {
        20 => {
            // Validator 0 creates a ParameterChange proposal
            let v = &validators[0];
            let nonce = v.state.nonce(&v.address);
            let proposal_id = v.state.next_proposal_id();
            let proposal_type = ProposalType::ParameterChange {
                param: "min_fee_sats".to_string(),
                new_value: "20000".to_string(),
            };
            txs.push(txgen::generate_create_proposal_tx(
                &v.sk, proposal_id, proposal_type, MIN_FEE_SATS, nonce,
            ));
        }
        25 => {
            // Validators 0, 1, 2 vote YES
            for v in validators.iter().take(3) {
                let nonce = v.state.nonce(&v.address);
                txs.push(txgen::generate_vote_tx(&v.sk, 0, true, MIN_FEE_SATS, nonce));
            }
        }
        _ => {}
    }
    txs
}

fn inject_cross_feature(round: u64, validators: &[SimValidator]) -> Vec<ultradag_coin::Transaction> {
    let mut txs = Vec::new();
    match round {
        10 => {
            // Validators 0, 1, 2 stake
            for v in validators.iter().take(3) {
                if v.state.balance(&v.address) >= MIN_STAKE_SATS {
                    let nonce = v.state.nonce(&v.address);
                    txs.push(txgen::generate_stake_tx(&v.sk, MIN_STAKE_SATS, nonce));
                }
            }
        }
        15 => {
            // Fund delegator from validator 0
            let v = &validators[0];
            let fund_amount = MIN_DELEGATION_SATS * 2;
            if v.state.balance(&v.address) >= fund_amount + MIN_FEE_SATS {
                let nonce = v.state.nonce(&v.address);
                let del1 = SecretKey::from_bytes([101u8; 32]);
                let mut tx = ultradag_coin::TransferTx {
                    from: v.address, to: del1.address(), amount: fund_amount,
                    fee: MIN_FEE_SATS, nonce, pub_key: v.sk.verifying_key().to_bytes(),
                    signature: ultradag_coin::Signature([0u8; 64]), memo: None,
                };
                tx.signature = v.sk.sign(&tx.signable_bytes());
                txs.push(ultradag_coin::Transaction::Transfer(tx));
            }
        }
        50 => {
            // Delegator delegates to validator 0
            let del1 = SecretKey::from_bytes([101u8; 32]);
            let v0_addr = validators[0].address;
            let v = &validators[0];
            let del1_nonce = v.state.nonce(&del1.address());
            let del1_bal = v.state.balance(&del1.address());
            if del1_bal >= MIN_DELEGATION_SATS {
                txs.push(txgen::generate_delegate_tx(&del1, v0_addr, MIN_DELEGATION_SATS, del1_nonce));
            }
        }
        60 => {
            // Validator 0 creates a TextProposal
            let v = &validators[0];
            let nonce = v.state.nonce(&v.address);
            let proposal_id = v.state.next_proposal_id();
            txs.push(txgen::generate_create_proposal_tx(
                &v.sk, proposal_id, ProposalType::TextProposal, MIN_FEE_SATS, nonce,
            ));
        }
        65 => {
            // Validators 0, 1, 2 vote YES
            for v in validators.iter().take(3) {
                let nonce = v.state.nonce(&v.address);
                txs.push(txgen::generate_vote_tx(&v.sk, 0, true, MIN_FEE_SATS, nonce));
            }
        }
        100 => {
            // Validator 2 sets commission to 15%
            if validators.len() > 2 {
                let v = &validators[2];
                let nonce = v.state.nonce(&v.address);
                txs.push(txgen::generate_set_commission_tx(&v.sk, 15, nonce));
            }
        }
        _ => {}
    }
    txs
}

fn inject_epoch_transition(round: u64, validators: &[SimValidator]) -> Vec<ultradag_coin::Transaction> {
    let mut txs = Vec::new();
    // Stake validators 0-3 early so the epoch transition has stakers to recalculate
    if round == 10 {
        for (i, v) in validators.iter().take(4).enumerate() {
            if v.state.balance(&v.address) >= MIN_STAKE_SATS {
                let nonce = v.state.nonce(&v.address);
                txs.push(txgen::generate_stake_tx(&v.sk, MIN_STAKE_SATS * (i as u64 + 1), nonce));
            }
        }
    }
    txs
}

fn hex_short(bytes: &[u8; 32]) -> String {
    bytes.iter().take(8).map(|b| format!("{:02x}", b)).collect()
}
