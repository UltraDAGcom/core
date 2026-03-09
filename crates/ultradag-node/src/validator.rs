use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tracing::{info, warn};

use ultradag_coin::{DagVertex, SecretKey, Signature, create_block, sync_epoch_validators};
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
    let mut in_recovery = false;
    const MAX_SKIPS_BEFORE_RECOVERY: u32 = 3;
    // Minimum time between vertex productions to prevent runaway optimistic loops.
    // Must equal round_duration: if set lower, fast validators race ahead of peers
    // and lose quorum, causing cascading network splits after ~80 rounds.
    // Optimistic responsiveness still helps: instead of waiting for the next timer
    // tick (up to round_duration away), validators produce immediately when quorum
    // is seen — but never faster than once per round.
    let min_production_interval = round_duration;
    let mut last_production = tokio::time::Instant::now() - round_duration;

    loop {
        // Optimistic responsiveness: produce early when notified (peer vertex arrived),
        // or fall back to timer. Notification only fires from DagProposal (single vertex),
        // never from bulk sync handlers, preventing runaway loops.
        let timer_fired;
        tokio::select! {
            _ = interval.tick() => {
                timer_fired = true;
            }
            _ = server.round_notify.notified() => {
                timer_fired = false;
                // Enforce minimum cooldown between productions
                let elapsed = last_production.elapsed();
                if elapsed < min_production_interval {
                    continue;
                }
            }
        }

        if shutdown.load(Ordering::Relaxed) {
            info!("Validator shutdown");
            return;
        }

        // Peer-count gate: if fewer than 2 peers connected, pause production.
        // A lone node (or near-lone) cannot achieve BFT finality and will race
        // ahead alone, creating a divergent chain that requires CLEAN_STATE to fix.
        {
            let peer_count = server.peers.connected_count().await;
            if peer_count < 2 {
                if timer_fired {
                    consecutive_skips += 1;
                    if consecutive_skips % 6 == 0 {
                        warn!("Waiting for peers ({} connected, need ≥2) — skip #{}", peer_count, consecutive_skips);
                    }
                }
                continue;
            }
        }

        // Determine the round we're producing for.
        // Only advance to current_round + 1 when we see quorum in current_round.
        // This prevents validators from racing ahead independently.
        let dag_round = {
            let dag = server.dag.read().await;
            let current = dag.current_round();

            if !dag.has_vertex_from_validator_in_round(&validator, current) {
                // Haven't produced in current round — produce there
                current.max(1)
            } else if in_recovery {
                // In stall recovery — advance unconditionally
                current + 1
            } else {
                // Already produced. Check if quorum validators produced in current
                // round OR the previous round. The prev-round fallback breaks deadlocks
                // where current-round vertices are still propagating but the network
                // was healthy one round ago.
                let validators_in_current = dag.distinct_validators_in_round(current).len();
                let prev = current.saturating_sub(1);
                let validators_in_prev = dag.distinct_validators_in_round(prev).len();
                let configured = server.finality.read().await.validator_set().configured_validators().unwrap_or(4);
                let quorum = (2 * configured + 2) / 3;
                if validators_in_current >= quorum || (prev > 0 && validators_in_prev >= quorum) {
                    current + 1
                } else {
                    // Not enough validators in either round yet.
                    if timer_fired {
                        consecutive_skips += 1;
                        if consecutive_skips >= MAX_SKIPS_BEFORE_RECOVERY {
                            warn!("Entering stall recovery after {} skips at round {}", consecutive_skips, current);
                            in_recovery = true;
                            current + 1
                        } else {
                            continue; // Wait for more validators
                        }
                    } else {
                        continue; // Notification — just re-check
                    }
                }
            }
        };

        // Check if quorum has resumed so we can exit recovery
        if in_recovery {
            if !timer_fired {
                continue; // In recovery, only produce on timer ticks
            }
            let dag = server.dag.read().await;
            let prev = dag_round.saturating_sub(1);
            let validators_in_prev = dag.distinct_validators_in_round(prev).len();
            let configured = server.finality.read().await.validator_set().configured_validators().unwrap_or(4);
            let quorum = (2 * configured + 2) / 3;
            if validators_in_prev >= quorum {
                info!("Quorum restored — exiting stall recovery mode");
                in_recovery = false;
            }
        }

        // Active set check: when staking is active, only active validators produce
        {
            let state = server.state.read().await;
            let active = state.active_validators();
            if !active.is_empty() && !active.contains(&validator) {
                // Staking is active but we're not in the active set — observe only
                continue;
            }
        }

        // Equivocation check: don't produce a second vertex in the same round
        {
            let dag = server.dag.read().await;
            if dag.has_vertex_from_validator_in_round(&validator, dag_round) {
                warn!("Already produced a vertex in round {} — skipping", dag_round);
                continue;
            }
        }

        // Get parent references: use ALL vertices from the previous round.
        // This creates dense cross-links between validators, enabling fast finality
        // (descendant validator sets grow quickly when vertices reference all peers).
        // Using tips() instead would collapse to 1 parent per validator since each
        // validator's chain tip is its own last vertex.
        // Peers may not have all prev-round vertices yet — orphan resolution via
        // GetParents handles this automatically.
        let dag_tips = {
            let dag = server.dag.read().await;
            let prev_round = dag_round.saturating_sub(1);
            let parents: Vec<[u8; 32]> = dag.vertices_in_round(prev_round)
                .iter()
                .map(|v| v.hash())
                .collect();
            if parents.is_empty() {
                vec![[0u8; 32]] // Genesis
            } else {
                parents
            }
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

        // Compute per-validator reward for this round
        let total_round_reward = ultradag_coin::block_reward(height);
        let validator_reward = {
            let state = server.state.read().await;
            let total_stake = state.total_staked();
            let own_stake = state.stake_of(&validator);
            if total_stake > 0 && own_stake > 0 {
                // Proportional to stake
                ((total_round_reward as u128)
                    .saturating_mul(own_stake as u128)
                    / total_stake as u128) as u64
            } else {
                // Pre-staking fallback: each vertex gets full block_reward.
                // Matches StateEngine::apply_finalized_vertices pre-staking mode (count=1).
                total_round_reward
            }
        };

        // Create block with transactions from mempool
        // The "prev_hash" is just the first parent for merkle purposes
        let prev_hash = dag_tips.first().copied().unwrap_or([0u8; 32]);
        let block = create_block(
            prev_hash,
            height,
            &validator,
            &mempool_snap,
            validator_reward,
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
                let prev_round = state_w.last_finalized_round();
                if let Err(e) = state_w.apply_finalized_vertices(&finalized_vertices) {
                    warn!("Failed to apply finalized vertices to state: {}", e);
                } else {
                    // Update high-water mark after successful finalization
                    let last_finalized_round = state_w.last_finalized_round().unwrap_or(0);
                    if last_finalized_round > 0 {
                        use ultradag_coin::persistence::monotonicity::HighWaterMark;
                        let hwm_path = HighWaterMark::path_in_dir(&data_dir);
                        
                        match HighWaterMark::load_or_create(&hwm_path) {
                            Ok(mut hwm) => {
                                let state_snapshot = state_w.snapshot();
                                let state_hash = ultradag_coin::consensus::compute_state_root(&state_snapshot);
                                hwm.update(last_finalized_round, state_hash);
                                
                                if let Err(e) = hwm.save(&hwm_path) {
                                    warn!("Failed to save high-water mark: {}", e);
                                }
                            }
                            Err(e) => {
                                warn!("Failed to load high-water mark for update: {}", e);
                            }
                        }
                    }
                    
                    // Epoch transition: sync active validator set to FinalityTracker
                    if state_w.epoch_just_changed(prev_round) {
                        sync_epoch_validators(&mut fin, &state_w);
                        info!("Epoch transition to epoch {} — active set: {} validators",
                            state_w.current_epoch(), state_w.active_validators().len());
                    }
                    let mut mp = server.mempool.write().await;
                    for v in &finalized_vertices {
                        for tx in &v.block.transactions {
                            mp.remove(&tx.hash());
                        }
                    }
                    
                    // Checkpoint generation: produce checkpoint at CHECKPOINT_INTERVAL
                    if last_finalized_round > 0 && last_finalized_round % ultradag_coin::CHECKPOINT_INTERVAL == 0 {
                        let state_snapshot = state_w.snapshot();
                        let state_root = ultradag_coin::consensus::compute_state_root(&state_snapshot);
                        let dag_r = server.dag.read().await;
                        let dag_tip = dag_r.tips().first().copied().unwrap_or([0u8; 32]);
                        drop(dag_r);
                        
                        let mut checkpoint = ultradag_coin::Checkpoint {
                            round: last_finalized_round,
                            state_root,
                            dag_tip,
                            total_supply: state_w.total_supply(),
                            signatures: vec![],
                        };
                        
                        // Sign with our validator key
                        let sig = ultradag_coin::consensus::CheckpointSignature {
                            validator,
                            pub_key: sk.verifying_key().to_bytes(),
                            signature: sk.sign(&checkpoint.signable_bytes()),
                        };
                        checkpoint.signatures.push(sig);
                        
                        // Persist locally
                        if let Err(e) = ultradag_coin::persistence::save_checkpoint(&data_dir, &checkpoint) {
                            warn!("Failed to save checkpoint: {}", e);
                        }
                        
                        // Broadcast to peers for co-signing
                        server.peers.broadcast(&Message::CheckpointProposal(checkpoint), "").await;
                        
                        info!("Produced checkpoint at round {}", last_finalized_round);
                    }
                }
            }
        }

        // Prune old rounds to bound memory (every 50 rounds to amortize cost)
        if dag_round % 50 == 0 {
            let last_fin = server.finality.read().await.last_finalized_round();
            if last_fin > 0 {
                let mut dag_w = server.dag.write().await;
                let pruned = dag_w.prune_old_rounds(last_fin);
                if pruned > 0 {
                    info!("Pruned {} old vertices (floor={})", pruned, dag_w.pruning_floor());
                }
            }
        }

        last_production = tokio::time::Instant::now();
        consecutive_skips = 0;
        in_recovery = false;

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

        // Stall-recovery sync: if finality lag > 10 rounds, request vertices
        // from peers around the last finalized round to trigger parent resolution
        let last_fin = {
            let st = server.state.read().await;
            st.last_finalized_round().unwrap_or(0)
        };
        if dag_round.saturating_sub(last_fin) > 10 {
            let sync_from = last_fin.saturating_sub(5);
            server
                .peers
                .broadcast(&Message::GetDagVertices { from_round: sync_from, max_count: 100 }, "")
                .await;
        }

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
