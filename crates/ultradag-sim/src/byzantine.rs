use ultradag_coin::{
    DagVertex, Block, BlockHeader, CoinbaseTx, Signature, K_PARENTS,
};
use ultradag_coin::block::merkle_root;
use crate::validator::SimValidator;

/// A Byzantine strategy determines what a dishonest validator does each round.
#[derive(Clone)]
pub enum ByzantineStrategy {
    /// Produce two different vertices for the same round (equivocation).
    Equivocator,
    /// Produce a vertex but only send it to validators in `targets` (by index).
    Withholder { targets: Vec<usize> },
    /// Don't produce any vertex (offline/crashed).
    Crash,
    /// Produce vertices with far-future timestamps.
    TimestampManipulator { offset_secs: i64 },
}

/// Returns a list of (vertex, optional_target_subset).
/// None targets = broadcast to all. Some(targets) = send only to those indices.
pub fn produce_vertices(
    strategy: &ByzantineStrategy,
    validator: &mut SimValidator,
    round: u64,
) -> Vec<(DagVertex, Option<Vec<usize>>)> {
    match strategy {
        ByzantineStrategy::Equivocator => {
            // Produce first vertex normally
            let v1 = validator.produce_vertex(round);

            // Produce a second vertex with different timestamp so it has a different hash
            let parents = if round <= 1 {
                vec![[0u8; 32]]
            } else {
                let selected = validator.dag.select_parents(&validator.address, round - 1, K_PARENTS);
                if selected.is_empty() { vec![[0u8; 32]] } else { selected }
            };

            let coinbase = CoinbaseTx {
                to: validator.address,
                amount: 0,
                height: round,
            };
            let mr = merkle_root(&[coinbase.hash()]);
            let prev_hash = parents.first().copied().unwrap_or([0u8; 32]);
            let header = BlockHeader {
                version: 1,
                height: round,
                timestamp: 1_000_001 + round as i64, // Different timestamp!
                prev_hash,
                merkle_root: mr,
            };
            let block = Block {
                header,
                coinbase,
                transactions: vec![],
            };
            let pub_key = validator.sk.verifying_key().to_bytes();
            let mut v2 = DagVertex::new(
                block, parents, round, validator.address, pub_key,
                Signature([0u8; 64]),
            );
            v2.signature = validator.sk.sign(&v2.signable_bytes());

            // Only v1 is in our own DAG (already inserted by produce_vertex).
            // Return both for broadcast — receiving nodes will detect equivocation.
            vec![(v1, None), (v2, None)]
        }
        ByzantineStrategy::Withholder { targets } => {
            let v = validator.produce_vertex(round);
            vec![(v, Some(targets.clone()))]
        }
        ByzantineStrategy::Crash => {
            // Produce nothing
            vec![]
        }
        ByzantineStrategy::TimestampManipulator { offset_secs } => {
            // Produce a vertex with manipulated timestamp
            let parents = if round <= 1 {
                vec![[0u8; 32]]
            } else {
                let selected = validator.dag.select_parents(&validator.address, round - 1, K_PARENTS);
                if selected.is_empty() { vec![[0u8; 32]] } else { selected }
            };

            let txs = validator.mempool.best(100);
            let total_fees: u64 = txs.iter().map(|t| t.fee()).fold(0u64, |a, f| a.saturating_add(f));
            let coinbase = CoinbaseTx { to: validator.address, amount: total_fees, height: round };
            let mut leaves = vec![coinbase.hash()];
            for tx in &txs {
                leaves.push(tx.hash());
            }
            let mr = merkle_root(&leaves);
            let prev_hash = parents.first().copied().unwrap_or([0u8; 32]);
            let header = BlockHeader {
                version: 1,
                height: round,
                timestamp: 1_000_000 + round as i64 + offset_secs,
                prev_hash,
                merkle_root: mr,
            };
            let block = Block { header, coinbase, transactions: txs };
            let pub_key = validator.sk.verifying_key().to_bytes();
            let mut v = DagVertex::new(
                block, parents, round, validator.address, pub_key,
                Signature([0u8; 64]),
            );
            v.signature = validator.sk.sign(&v.signable_bytes());
            validator.dag.insert(v.clone());
            validator.finality.register_validator(validator.address);

            vec![(v, None)]
        }
    }
}
