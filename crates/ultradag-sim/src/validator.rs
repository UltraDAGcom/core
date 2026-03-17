use ultradag_coin::{
    Address, SecretKey, Signature,
    BlockDag, FinalityTracker, StateEngine, Mempool,
    DagVertex, Transaction, Block, BlockHeader, CoinbaseTx,
    K_PARENTS,
    consensus::dag::DagInsertError,
    consensus::compute_state_root,
    consensus::sync_epoch_validators,
};
use ultradag_coin::block::merkle_root;

pub struct SimValidator {
    pub index: usize,
    pub sk: SecretKey,
    pub address: Address,
    pub dag: BlockDag,
    pub finality: FinalityTracker,
    pub state: StateEngine,
    pub mempool: Mempool,
    pub honest: bool,
    /// History: (finalized_round, state_root).
    pub finality_history: Vec<(u64, [u8; 32])>,
}

impl SimValidator {
    pub fn new(
        index: usize,
        sk: SecretKey,
        min_validators: usize,
        configured_validator_count: u64,
    ) -> Self {
        let address = sk.address();
        let mut state = StateEngine::new_with_genesis();
        state.set_configured_validator_count(configured_validator_count);

        let mut finality = FinalityTracker::new(min_validators);
        finality.set_configured_validators(configured_validator_count as usize);
        finality.register_validator(address);

        Self {
            index,
            sk,
            address,
            dag: BlockDag::new(),
            finality,
            state,
            mempool: Mempool::new(),
            honest: true,
            finality_history: Vec::new(),
        }
    }

    /// Try to insert a vertex received from the network.
    pub fn receive_vertex(&mut self, vertex: DagVertex) -> Result<bool, DagInsertError> {
        let result = self.dag.try_insert(vertex.clone())?;
        if result {
            self.finality.register_validator(vertex.validator);
        }
        Ok(result)
    }

    /// Produce a vertex for the given round.
    pub fn produce_vertex(&mut self, round: u64) -> DagVertex {
        // 1. Select parents
        let parents = if round <= 1 {
            vec![[0u8; 32]]
        } else {
            let selected = self.dag.select_parents(&self.address, round - 1, K_PARENTS);
            if selected.is_empty() {
                vec![[0u8; 32]]
            } else {
                selected
            }
        };

        // 2. Get transactions from mempool
        let txs = self.mempool.best(100);

        // 3. Sort by (from, nonce) for valid execution order
        let mut sorted_txs = txs;
        sorted_txs.sort_by(|a, b| {
            a.from().0.cmp(&b.from().0)
                .then_with(|| a.nonce().cmp(&b.nonce()))
        });

        // 4. Compute total fees
        let total_fees: u64 = sorted_txs.iter()
            .map(|tx| tx.fee())
            .fold(0u64, |acc, f| acc.saturating_add(f));

        // 5. Coinbase = fees only (rewards distributed via distribute_round_rewards)
        let coinbase = CoinbaseTx {
            to: self.address,
            amount: total_fees,
            height: round,
        };

        // 6. Build merkle root
        let mut leaves: Vec<[u8; 32]> = vec![coinbase.hash()];
        for tx in &sorted_txs {
            leaves.push(tx.hash());
        }
        let mr = merkle_root(&leaves);

        // 7. Build block header with deterministic timestamp
        let prev_hash = parents.first().copied().unwrap_or([0u8; 32]);
        let header = BlockHeader {
            version: 1,
            height: round,
            timestamp: 1_000_000 + round as i64,
            prev_hash,
            merkle_root: mr,
        };

        let block = Block {
            header,
            coinbase,
            transactions: sorted_txs,
        };

        // 8. Create vertex and sign
        let pub_key = self.sk.verifying_key().to_bytes();
        let mut vertex = DagVertex::new(
            block,
            parents,
            round,
            self.address,
            pub_key,
            Signature([0u8; 64]),
        );
        vertex.signature = self.sk.sign(&vertex.signable_bytes());

        // 9. Insert into own DAG
        self.dag.insert(vertex.clone());
        self.finality.register_validator(self.address);

        vertex
    }

    /// Run finality and apply to state. Returns newly finalized vertices.
    pub fn run_finality(&mut self) -> Vec<DagVertex> {
        let prev_round = self.state.last_finalized_round();

        // Iteratively find all newly finalized vertices
        let mut all_finalized_hashes = Vec::new();
        loop {
            let newly = self.finality.find_newly_finalized(&self.dag);
            if newly.is_empty() {
                break;
            }
            all_finalized_hashes.extend(newly);
        }

        if all_finalized_hashes.is_empty() {
            return Vec::new();
        }

        // Collect actual vertices
        let finalized_vertices: Vec<DagVertex> = all_finalized_hashes.iter()
            .filter_map(|h| self.dag.get(h).cloned())
            .collect();

        if finalized_vertices.is_empty() {
            return Vec::new();
        }

        // Apply to state engine (it sorts internally by (round, hash))
        if let Err(e) = self.state.apply_finalized_vertices(&finalized_vertices) {
            eprintln!("Validator {} apply_finalized_vertices error: {}", self.index, e);
            return Vec::new();
        }

        // Handle epoch transitions
        if self.state.epoch_just_changed(prev_round) {
            sync_epoch_validators(&mut self.finality, &self.state);
        }

        // Remove finalized transactions from mempool
        for v in &finalized_vertices {
            for tx in &v.block.transactions {
                self.mempool.remove(&tx.hash());
            }
        }

        // Record state root for this finalized round
        let last_round = self.state.last_finalized_round().unwrap_or(0);
        let root = compute_state_root(&self.state.snapshot());
        self.finality_history.push((last_round, root));

        finalized_vertices
    }

    pub fn state_root(&self) -> [u8; 32] {
        compute_state_root(&self.state.snapshot())
    }

    pub fn last_finalized_round(&self) -> u64 {
        self.state.last_finalized_round().unwrap_or(0)
    }

    pub fn add_transaction(&mut self, tx: Transaction) {
        self.mempool.insert(tx);
    }

    /// Override a governance parameter directly (bypasses validation bounds).
    /// Must be called identically on ALL validators before any rounds run.
    pub fn override_governance_param_unchecked(&mut self, param: &str, value: u64) {
        let params = self.state.governance_params_mut();
        match param {
            "voting_period_rounds" => params.voting_period_rounds = value,
            "execution_delay_rounds" => params.execution_delay_rounds = value,
            "min_fee_sats" => params.min_fee_sats = value,
            "min_stake_to_propose" => params.min_stake_to_propose = value,
            "quorum_numerator" => params.quorum_numerator = value,
            "approval_numerator" => params.approval_numerator = value,
            "max_active_proposals" => params.max_active_proposals = value,
            "observer_reward_percent" => params.observer_reward_percent = value,
            "council_emission_percent" => params.council_emission_percent = value,
            "slash_percent" => params.slash_percent = value,
            _ => panic!("Unknown governance param: {}", param),
        }
    }

    /// Add a council member. Must be called identically on ALL validators.
    pub fn add_council_member(&mut self, address: Address, category: ultradag_coin::governance::CouncilSeatCategory) {
        let _ = self.state.add_council_member(address, category);
    }
}
