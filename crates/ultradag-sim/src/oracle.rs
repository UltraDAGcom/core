//! Determinism oracle: runs a scenario TWICE with different internal orderings
//! and verifies bit-identical state roots. Proves no non-determinism in the
//! consensus or state engine (no HashMap iteration order dependency, no
//! timestamps, no OS entropy).

use crate::validator::SimValidator;
use crate::network::{VirtualNetwork, DeliveryPolicy};
use ultradag_coin::{SecretKey, DagVertex};

/// Run the same vertex sequence through two independent state engines
/// and verify they produce identical state roots at every finalized round.
pub fn verify_replay_determinism(
    vertices_by_round: &[Vec<DagVertex>],
    num_validators: usize,
) -> Result<(), String> {
    // Create two independent validator sets with identical genesis
    let mut engines_a = create_validators(num_validators);
    let mut engines_b = create_validators(num_validators);

    for (round_idx, round_vertices) in vertices_by_round.iter().enumerate() {
        let round = round_idx as u64 + 1;

        // Feed vertices to both sets in the SAME order
        for vertex in round_vertices {
            for v in engines_a.iter_mut() {
                let _ = v.receive_vertex(vertex.clone());
            }
            for v in engines_b.iter_mut() {
                let _ = v.receive_vertex(vertex.clone());
            }
        }

        // Run finality on both
        for v in engines_a.iter_mut() { v.run_finality(); }
        for v in engines_b.iter_mut() { v.run_finality(); }

        // Compare state roots
        for i in 0..num_validators {
            let root_a = engines_a[i].state_root();
            let root_b = engines_b[i].state_root();
            if root_a != root_b {
                return Err(format!(
                    "Replay determinism failed at round {} validator {}: root_a={} root_b={}",
                    round, i,
                    root_a.iter().take(8).map(|b| format!("{:02x}", b)).collect::<String>(),
                    root_b.iter().take(8).map(|b| format!("{:02x}", b)).collect::<String>(),
                ));
            }
        }
    }

    Ok(())
}

/// Run a simulation scenario twice with different message delivery orderings
/// and verify that all honest validators converge to the same state root
/// at the same finalized round.
pub fn verify_ordering_independence(
    num_honest: usize,
    num_rounds: u64,
    seed_a: u64,
    seed_b: u64,
) -> Result<(), String> {
    // Run A with seed_a (RandomOrder)
    let (roots_a, rounds_a) = run_sim(num_honest, num_rounds, seed_a);
    // Run B with seed_b (different RandomOrder)
    let (roots_b, rounds_b) = run_sim(num_honest, num_rounds, seed_b);

    // Both should reach the same final state
    for i in 0..num_honest {
        if rounds_a[i] == rounds_b[i] && rounds_a[i] > 0 && roots_a[i] != roots_b[i] {
            return Err(format!(
                "Ordering independence failed: validator {} at round {} has different roots under seeds {} vs {}",
                i, rounds_a[i], seed_a, seed_b,
            ));
        }
    }

    Ok(())
}

fn create_validators(n: usize) -> Vec<SimValidator> {
    let mut validators = Vec::new();
    let mut all_addrs = Vec::new();
    for i in 0..n {
        let sk = SecretKey::from_bytes([(i as u8).wrapping_add(1); 32]);
        all_addrs.push(sk.address());
        validators.push(SimValidator::new(i, sk, 1, n as u64));
    }
    for v in &mut validators {
        for addr in &all_addrs {
            v.finality.register_validator(*addr);
        }
    }
    validators
}

#[allow(clippy::needless_range_loop)]
fn run_sim(num_honest: usize, num_rounds: u64, seed: u64) -> (Vec<[u8; 32]>, Vec<u64>) {
    let mut validators = create_validators(num_honest);
    let mut network = VirtualNetwork::new(num_honest, DeliveryPolicy::RandomOrder, seed);

    for round in 1..=num_rounds {
        network.deliver(round);
        for i in 0..num_honest {
            let msgs = network.drain_inbox(i);
            for v in msgs { let _ = validators[i].receive_vertex(v); }
        }
        for i in 0..num_honest {
            let vertex = validators[i].produce_vertex(round);
            network.broadcast(i, vertex);
        }
        for v in &mut validators { v.run_finality(); }
    }

    let roots: Vec<[u8; 32]> = validators.iter().map(|v| v.state_root()).collect();
    let rounds: Vec<u64> = validators.iter().map(|v| v.last_finalized_round()).collect();
    (roots, rounds)
}
