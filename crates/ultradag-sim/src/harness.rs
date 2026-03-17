use crate::network::{VirtualNetwork, DeliveryPolicy};
use crate::validator::SimValidator;
use crate::byzantine::{ByzantineStrategy, produce_vertices};
use crate::invariants;
use crate::txgen;
use ultradag_coin::{SecretKey, Address};
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;

/// Configuration for a simulation run.
pub struct SimConfig {
    pub num_honest: usize,
    pub byzantine: Vec<ByzantineStrategy>,
    pub num_rounds: u64,
    pub delivery_policy: DeliveryPolicy,
    pub seed: u64,
    /// Number of random transactions to generate per round.
    pub txs_per_round: usize,
    /// Whether to check invariants every round (slower but catches issues earlier).
    pub check_every_round: bool,
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
    /// Known equivocator addresses for invariant checking.
    known_equivocators: Vec<Address>,
    /// Funded accounts for tx generation: (sk, balance, nonce).
    funded_accounts: Vec<(SecretKey, u64, u64)>,
    /// Total equivocations detected.
    equivocations_detected: usize,
}

impl SimHarness {
    pub fn new(config: &SimConfig) -> Self {
        let total = config.num_honest + config.byzantine.len();
        let min_validators = if total == 1 { 1 } else { 3.min(total) };

        // Create validators with deterministic keys
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

        // Register ALL validators in every validator's FinalityTracker
        for v in &mut validators {
            for addr in &all_addresses {
                v.finality.register_validator(*addr);
            }
        }

        // Build Byzantine strategies vector
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

        // Build funded accounts (validators will earn rewards as they produce)
        let funded_accounts: Vec<(SecretKey, u64, u64)> = (0..total)
            .map(|i| {
                let seed_byte = (i as u8).wrapping_add(1);
                let sk = SecretKey::from_bytes([seed_byte; 32]);
                (sk, 0u64, 0u64) // Start with 0 — will accumulate rewards
            })
            .collect();

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
                        Err(_) => {} // Missing parents, future round, etc. — OK
                    }
                }
            }

            // 3. Produce phase
            for i in 0..self.validators.len() {
                match &self.byzantine_strategies[i] {
                    None => {
                        // Honest validator
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

            // 5. Transaction injection (after some rounds to allow balance accumulation)
            if config.txs_per_round > 0 && round > 10 {
                // Update funded_accounts from the first honest validator's state
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

            // 6. Invariant checking
            if config.check_every_round || round == config.num_rounds {
                if let Err(e) = invariants::check_all(&self.validators, &self.known_equivocators) {
                    violations.push(format!("Round {}: {}", round, e));
                    if config.check_every_round {
                        // Print debug info
                        eprintln!("INVARIANT VIOLATION at round {} (seed: 0x{:X}):", round, self.seed);
                        eprintln!("{}", e);
                        for v in self.validators.iter().filter(|v| v.honest) {
                            eprintln!("  Validator {}: finalized_round={}, state_root={}",
                                v.index, v.last_finalized_round(),
                                hex_short(&v.state_root()));
                        }
                        break;
                    }
                }
            }

            // 7. Pruning every 100 rounds
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

        // Build result
        let final_state_roots: Vec<(usize, [u8; 32])> = self.validators.iter()
            .map(|v| (v.index, v.state_root()))
            .collect();
        let final_finalized_rounds: Vec<(usize, u64)> = self.validators.iter()
            .map(|v| (v.index, v.last_finalized_round()))
            .collect();

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

    /// Update funded_accounts from the first honest validator's state.
    fn update_funded_accounts(&mut self) {
        if let Some(v) = self.validators.iter().find(|v| v.honest) {
            for (i, (sk, bal, nonce)) in self.funded_accounts.iter_mut().enumerate() {
                let addr = sk.address();
                *bal = v.state.balance(&addr);
                *nonce = v.state.nonce(&addr);
                // If this is beyond original validators, leave as-is
                let _ = i;
            }
        }
    }
}

fn hex_short(bytes: &[u8; 32]) -> String {
    bytes.iter().take(8).map(|b| format!("{:02x}", b)).collect()
}
