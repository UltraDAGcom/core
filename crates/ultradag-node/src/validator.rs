use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tracing::{info, warn};

use ultradag_coin::{DagVertex, SecretKey, Signature, create_block};
use ultradag_network::{Message, NodeServer};

pub async fn validator_loop(
    server: Arc<NodeServer>,
    sk: SecretKey,
    round_duration: Duration,
    shutdown: Arc<AtomicBool>,
    data_dir: PathBuf,
) {
    let validator = sk.address();
    info!("Validator started for address {} (round={}ms)", validator, round_duration.as_millis());

    let mut interval = tokio::time::interval(round_duration);
    // First tick fires immediately — consume it so we wait a full round first
    interval.tick().await;

    let mut consecutive_skips = 0u32;
    const MAX_SKIPS_BEFORE_RECOVERY: u32 = 3;

    loop {
        // Wait for the round timer to fire
        interval.tick().await;

        if shutdown.load(Ordering::Relaxed) {
            info!("Validator shutdown");
            return;
        }

        // Determine the round we're producing for
        let dag_round = {
            let dag = server.dag.read().await;
            dag.current_round() + 1
        };

        let prev_round = dag_round.saturating_sub(1);

        // 2f+1 reference rule: check we have enough vertices from the previous round.
        // For round 1 (prev_round=0 with no DAG history), skip the check.
        // Stall recovery: after MAX_SKIPS consecutive skips, produce unconditionally
        // to break deadlocks caused by staggered startup.
        if dag_round > 1 && consecutive_skips < MAX_SKIPS_BEFORE_RECOVERY {
            let (prev_round_count, threshold) = {
                let dag = server.dag.read().await;
                let count = dag.distinct_validators_in_round(prev_round).len();
                let fin = server.finality.read().await;
                let thresh = fin.finality_threshold();
                (count, thresh)
            };

            if threshold != usize::MAX && prev_round_count < threshold {
                consecutive_skips += 1;
                warn!(
                    "Skipping round {} — only {}/{} validators seen in round {} (need quorum) [skip {}/{}]",
                    dag_round, prev_round_count, threshold, prev_round,
                    consecutive_skips, MAX_SKIPS_BEFORE_RECOVERY,
                );
                continue;
            }
        } else if consecutive_skips >= MAX_SKIPS_BEFORE_RECOVERY {
            warn!("Stall recovery: producing round {} unconditionally after {} skips", dag_round, consecutive_skips);
        }

        consecutive_skips = 0;

        // Equivocation check: don't produce a second vertex in the same round
        {
            let dag = server.dag.read().await;
            if dag.has_vertex_from_validator_in_round(&validator, dag_round) {
                warn!("Already produced a vertex in round {} — skipping", dag_round);
                continue;
            }
        }

        // Get DAG tips for parent references
        let dag_tips = {
            let dag = server.dag.read().await;
            dag.tips()
        };

        // Snapshot mempool
        let mempool_snap = server.mempool.read().await.clone();

        // Calculate height based on finalized rounds
        // Each finalized round = one "block" for reward purposes
        let height = {
            let state = server.state.read().await;
            state.last_finalized_round().unwrap_or(0) + 1
        };

        info!(
            "Producing vertex round={} height={} mempool={} parents={}",
            dag_round, height, mempool_snap.len(), dag_tips.len(),
        );

        // Create block with transactions from mempool
        // The "prev_hash" is just the first parent for merkle purposes
        let prev_hash = dag_tips.first().copied().unwrap_or([0u8; 32]);
        let block = create_block(
            prev_hash,
            height,
            &validator,
            &mempool_snap,
        );

        let block_hash = block.hash();

        // Build DAG vertex referencing all known tips
        let parent_hashes = if dag_tips.is_empty() {
            vec![[0u8; 32]] // Genesis vertex
        } else {
            dag_tips
        };

        // Sign the vertex with Ed25519
        let mut vertex = DagVertex::new(
            block.clone(),
            parent_hashes,
            dag_round,
            validator,
            sk.verifying_key().to_bytes(),
            Signature([0u8; 64]),
        );
        vertex.signature = sk.sign(&vertex.signable_bytes());

        // Insert into DAG
        {
            let mut dag = server.dag.write().await;
            dag.insert(vertex.clone());
        }

        // Check finality and apply to state (multi-pass for parent finality guarantee)
        {
            let mut fin = server.finality.write().await;
            fin.register_validator(validator);
            let dag_r = server.dag.read().await;

            let mut all_finalized = Vec::new();
            loop {
                let newly_finalized = fin.find_newly_finalized(&dag_r);
                if newly_finalized.is_empty() {
                    break;
                }
                all_finalized.extend(newly_finalized);
            }

            if !all_finalized.is_empty() {
                info!("DAG-BFT finalized {} vertices", all_finalized.len());

                let finalized_vertices: Vec<DagVertex> = all_finalized
                    .iter()
                    .filter_map(|h| dag_r.get(h).cloned())
                    .collect();

                drop(dag_r);

                let mut state_w = server.state.write().await;
                if let Err(e) = state_w.apply_finalized_vertices(&finalized_vertices) {
                    warn!("Failed to apply finalized vertices to state: {}", e);
                } else {
                    let mut mp = server.mempool.write().await;
                    for v in &finalized_vertices {
                        for tx in &v.block.transactions {
                            mp.remove(&tx.hash());
                        }
                    }
                }
            }
        }

        info!(
            "Produced vertex hash={} round={} height={}",
            hex_short(&block_hash),
            dag_round,
            height,
        );

        // Broadcast DAG vertex to peers (no separate block message)
        let _ = server.vertex_tx.send(vertex.clone());
        server
            .peers
            .broadcast(&Message::DagProposal(vertex), "")
            .await;

        // Periodic persistence: save state every 10 rounds
        if dag_round % 10 == 0 {
            let dag_path = data_dir.join("dag.json");
            let finality_path = data_dir.join("finality.json");
            let state_path = data_dir.join("state.json");
            let mempool_path = data_dir.join("mempool.json");

            let dag = server.dag.read().await;
            if let Err(e) = dag.save(&dag_path) { warn!("Persist DAG: {}", e); }
            drop(dag);
            let fin = server.finality.read().await;
            if let Err(e) = fin.save(&finality_path) { warn!("Persist finality: {}", e); }
            drop(fin);
            let st = server.state.read().await;
            if let Err(e) = st.save(&state_path) { warn!("Persist state: {}", e); }
            drop(st);
            let mp = server.mempool.read().await;
            if let Err(e) = mp.save(&mempool_path) { warn!("Persist mempool: {}", e); }
            drop(mp);
            info!("State persisted at round {}", dag_round);
        }
    }
}

fn hex_short(hash: &[u8; 32]) -> String {
    hash[..4].iter().map(|b| format!("{b:02x}")).collect()
}
