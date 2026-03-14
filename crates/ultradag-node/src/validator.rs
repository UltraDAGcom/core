use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tracing::{error, info, warn};

use ultradag_coin::{DagVertex, SecretKey, Signature, create_block, sync_epoch_validators};
use ultradag_coin::safety::circuit_breaker::CircuitBreaker;
use ultradag_network::{Message, NodeServer, hex_short};


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
    // Prevent burst catch-up: if a tick is missed (lock contention, slow processing),
    // skip it instead of firing rapidly to catch up. Without this, after a long lock
    // hold, the interval fires N times immediately, causing rapid-fire production.
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // First tick fires immediately — consume it so we wait a full round first
    interval.tick().await;

    let mut consecutive_skips = 0u32;
    let mut in_recovery = false;
    let last_fin = server.finality.read().await.last_finalized_round();
    let mut last_checkpoint_round: u64 = (last_fin / ultradag_coin::CHECKPOINT_INTERVAL) * ultradag_coin::CHECKPOINT_INTERVAL;
    let circuit_breaker = CircuitBreaker::new(true);
    if last_fin > 0 {
        circuit_breaker.check_finality(last_fin);
    }
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
                // Enforce minimum cooldown even on timer ticks (safety net for Delay behavior)
                let elapsed = last_production.elapsed();
                if elapsed < min_production_interval / 2 {
                    continue;
                }
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

        // Sync gate: don't produce vertices until initial sync is complete.
        // A new node at round 0 producing vertices while peers are at round 1500+
        // creates orphans, triggers allowlist bans, and wastes bandwidth.
        if !server.sync_complete.load(Ordering::Relaxed) {
            if timer_fired && consecutive_skips.is_multiple_of(6) {
                let our_round = server.dag.read().await.current_round();
                info!("Waiting for initial sync to complete (current round: {})", our_round);
            }
            if timer_fired {
                consecutive_skips += 1;
            }
            continue;
        }

        // Peer-count gate: if fewer than 2 peers connected, pause production.
        // A lone node (or near-lone) cannot achieve BFT finality and will race
        // ahead alone, creating a divergent chain that requires CLEAN_STATE to fix.
        {
            let peer_count = server.peers.connected_count().await;
            if peer_count < 2 {
                if timer_fired {
                    consecutive_skips += 1;
                    if consecutive_skips.is_multiple_of(6) {
                        warn!("Waiting for peers ({} connected, need ≥2) — skip #{}", peer_count, consecutive_skips);
                    }
                }
                continue;
            }
        }

        // Defensive: if our own validator is marked Byzantine in our local DAG,
        // that's a bug (the equivocation check should prevent self-equivocation).
        // Clear the flag and log a critical error so we can diagnose.
        {
            let mut dag = server.dag.write().await;
            if dag.is_byzantine(&validator) {
                warn!("CRITICAL: our own validator {} is marked Byzantine in local DAG — clearing flag (this is a bug)", validator);
                dag.clear_byzantine(&validator);
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
                let quorum = (2 * configured).div_ceil(3);
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
            let quorum = (2 * configured).div_ceil(3);
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

        // Get parent references: use K_PARENTS partial selection from previous round.
        // This enables unlimited validator scaling by keeping parent count bounded at K
        // regardless of the number of validators N. Follows Narwhal's approach.
        // The DAG stays well-connected through deterministic cross-references, and finality
        // math still works because descendants propagate through the partial parent graph.
        // Peers may not have all prev-round vertices yet — orphan resolution via
        // GetParents handles this automatically.
        let dag_tips = {
            let dag = server.dag.read().await;
            let prev_round = dag_round.saturating_sub(1);
            let mut parents: Vec<[u8; 32]> = dag.vertices_in_round(prev_round)
                .iter()
                .map(|v| v.hash())
                .collect();
            
            // Use partial parent selection if we have more than K_PARENTS vertices
            if parents.len() > ultradag_coin::consensus::dag::K_PARENTS {
                // Deterministic selection: blake3(validator || parent_hash) for uniform scoring.
                // All nodes must use identical algorithm for consensus.
                let mut scored: Vec<([u8; 32], [u8; 32])> = parents
                    .iter()
                    .map(|parent| {
                        let mut h = blake3::Hasher::new();
                        h.update(&validator.0);
                        h.update(parent);
                        (*parent, *h.finalize().as_bytes())
                    })
                    .collect();
                scored.sort_by_key(|(_, s)| *s);
                scored.truncate(ultradag_coin::consensus::dag::K_PARENTS);
                parents = scored.into_iter().map(|(p, _)| p).collect();
            }
            
            if parents.is_empty() {
                vec![[0u8; 32]] // Genesis
            } else {
                parents
            }
        };

        // Snapshot mempool
        let mempool_snap = server.mempool.read().await.clone();

        // Use dag_round as height for block_reward — matches engine.rs which uses vertex.round.
        // This eliminates TOCTOU: both producer and engine use the same immutable round number.
        let height = dag_round;

        info!(
            "Producing vertex round={} height={} mempool={} parents={}",
            dag_round, height, mempool_snap.len(), dag_tips.len(),
        );

        // Compute per-validator reward for this round (capped at supply limit)
        let total_round_reward = ultradag_coin::block_reward(height);
        let validator_reward = {
            let state = server.state.read().await;
            let total_stake = state.total_staked();
            let own_stake = state.stake_of(&validator);
            let base_reward = if total_stake > 0 && own_stake > 0 {
                // Proportional to stake
                let proportional = ((total_round_reward as u128)
                    .saturating_mul(own_stake as u128)
                    / total_stake as u128) as u64;
                // Observer penalty: staked but not in the active validator set
                let active_set = state.active_validators();
                if !active_set.is_empty() && !active_set.contains(&validator) {
                    proportional * ultradag_coin::constants::OBSERVER_REWARD_PERCENT / 100
                } else {
                    proportional
                }
            } else {
                // Pre-staking fallback: split block_reward equally among validators.
                // Use configured_validators (--validators N) for deterministic agreement
                // between all nodes.
                let configured = server.finality.read().await
                    .validator_set().configured_validators().unwrap_or(1) as u64;
                let n = configured.max(1);
                total_round_reward / n
            };
            // Cap at supply limit (must match StateEngine validation)
            let max_supply = ultradag_coin::constants::MAX_SUPPLY_SATS;
            let total_supply = state.total_supply();
            if total_supply.saturating_add(base_reward) > max_supply {
                max_supply.saturating_sub(total_supply)
            } else {
                base_reward
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

        // Insert into DAG using try_insert to catch equivocation races.
        // Between the equivocation check (line 151) and here, a P2P vertex from
        // another node could have been inserted for the same validator+round.
        {
            let mut dag = server.dag.write().await;
            match dag.try_insert(vertex.clone()) {
                Ok(true) => {} // Inserted successfully
                Ok(false) => {
                    let is_byz = dag.is_byzantine(&validator);
                    let has_existing = dag.has_vertex_from_validator_in_round(&validator, dag_round);
                    warn!(
                        "Vertex rejected: is_byzantine={} has_existing_in_round={} round={} current_round={} — skipping",
                        is_byz, has_existing, dag_round, dag.current_round()
                    );
                    continue;
                }
                Err(e) => {
                    warn!("Vertex insertion failed: {:?} — skipping broadcast", e);
                    continue;
                }
            }
        }

        // Check finality and apply to state (multi-pass for parent finality guarantee)
        // Lock ordering: finality+dag → drop → state → drop → finality (for epoch sync)
        let (all_finalized, finalized_vertices) = {
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

            // Diagnostic: log why finality might be stuck (every 20 rounds)
            if all_finalized.is_empty() && dag_round % 20 == 0 {
                let genesis: [u8; 32] = [0u8; 32];
                let threshold = fin.finality_threshold();
                let last_fin = fin.last_finalized_round();
                let scan_round = last_fin; // first round to scan
                let hashes = dag_r.hashes_in_round(scan_round);
                let total = hashes.len();
                let mut unfinalized = 0;
                let mut sample_info = String::new();
                for hash in hashes {
                    if fin.is_finalized(hash) { continue; }
                    unfinalized += 1;
                    if unfinalized <= 2 {
                        if let Some(v) = dag_r.get(hash) {
                            let parents_ok = v.parent_hashes.is_empty()
                                || v.parent_hashes.iter()
                                    .all(|p| *p == genesis || fin.is_finalized(p));
                            let desc_count = dag_r.descendant_validator_count(hash);
                            let missing_parents: Vec<String> = v.parent_hashes.iter()
                                .filter(|p| **p != genesis && !fin.is_finalized(p))
                                .map(hex_short)
                                .collect();
                            sample_info.push_str(&format!(
                                " [v={} desc={}/{} parents_ok={} missing={}]",
                                hex_short(hash), desc_count, threshold, parents_ok,
                                if missing_parents.is_empty() { "none".to_string() } else { missing_parents.join(",") }
                            ));
                        }
                    }
                }
                warn!("Finality stuck: last_fin={} scan_round={} total={} unfinalized={} threshold={}{}",
                    last_fin, scan_round, total, unfinalized, threshold, sample_info);
            }

            let finalized_vertices: Vec<DagVertex> = all_finalized
                .iter()
                .filter_map(|h| dag_r.get(h).cloned())
                .collect();

            (all_finalized, finalized_vertices)
        }; // finality + dag locks dropped here

        if !all_finalized.is_empty() {
            info!("DAG-BFT finalized {} vertices", all_finalized.len());

            let epoch_changed = {
                let mut state_w = server.state.write().await;
                let prev_round = state_w.last_finalized_round();
                if let Err(e) = state_w.apply_finalized_vertices(&finalized_vertices) {
                    warn!("Failed to apply finalized vertices to state: {}", e);
                    false
                } else {
                    let changed = state_w.epoch_just_changed(prev_round);
                    let mut mp = server.mempool.write().await;
                    for v in &finalized_vertices {
                        for tx in &v.block.transactions {
                            mp.remove(&tx.hash());
                        }
                    }
                    changed
                }
            }; // state_w dropped here

            // Epoch transition: acquire finality AFTER dropping state to prevent deadlock
            if epoch_changed {
                let mut fin = server.finality.write().await;
                let state_r = server.state.read().await;
                sync_epoch_validators(&mut fin, &state_r);
                info!("Epoch transition to epoch {} — active set: {} validators",
                    state_r.current_epoch(), state_r.active_validators().len());
            }
        }

        // Circuit breaker: halt if finality rolls back (critical safety check)
        let current_fin = server.finality.read().await.last_finalized_round();
        circuit_breaker.check_finality(current_fin);

        // Checkpoint generation: runs independently of finality block above.
        // P2P handler may finalize vertices before validator loop, making all_finalized empty.
        // We check last_finalized_round directly from the finality tracker.
        // Iterate all crossed multiples of CHECKPOINT_INTERVAL between last_checkpoint_round
        // and current_finalized, so we don't miss boundaries when finality jumps (e.g., 198→201).
        let current_finalized = server.finality.read().await.last_finalized_round();
        let interval = ultradag_coin::CHECKPOINT_INTERVAL;
        let first_crossed = ((last_checkpoint_round / interval) + 1) * interval;
        let mut cp_round = first_crossed;
        while cp_round <= current_finalized {
            let checkpoint_round = cp_round;
            cp_round += interval;

            let checkpoint_start = tokio::time::Instant::now();

            let state_r = server.state.read().await;
            let state_snapshot = state_r.snapshot();
            let state_root = ultradag_coin::consensus::compute_state_root(&state_snapshot);
            let total_supply = state_r.total_supply();
            drop(state_r);

            let dag_r = server.dag.read().await;
            let dag_tip = dag_r.tips().first().copied().unwrap_or([0u8; 32]);
            drop(dag_r);

            // Get previous checkpoint hash for chain linking
            let prev_checkpoint_hash = if checkpoint_round == 0 {
                [0u8; 32] // Genesis has no predecessor
            } else {
                // Load the specific previous checkpoint by round (not "latest")
                let prev_round = checkpoint_round.saturating_sub(ultradag_coin::CHECKPOINT_INTERVAL);
                match ultradag_coin::persistence::load_checkpoint_by_round(&data_dir, prev_round) {
                    Some(prev_cp) if prev_cp.round == prev_round => {
                        ultradag_coin::consensus::compute_checkpoint_hash(&prev_cp)
                    }
                    _ => {
                        // Missing previous checkpoint would break chain verification.
                        // Skip this checkpoint rather than producing one with [0u8; 32]
                        // that permanently breaks fast-sync for new nodes.
                        error!("Previous checkpoint at round {} not found — skipping checkpoint production. Verify disk integrity.", prev_round);
                        continue;
                    }
                }
            };

            let mut checkpoint = ultradag_coin::Checkpoint {
                round: checkpoint_round,
                state_root,
                dag_tip,
                total_supply,
                prev_checkpoint_hash,
                signatures: vec![],
            };

            let sig = ultradag_coin::consensus::CheckpointSignature {
                validator,
                pub_key: sk.verifying_key().to_bytes(),
                signature: sk.sign(&checkpoint.signable_bytes()),
            };
            checkpoint.signatures.push(sig);

            // Store in pending_checkpoints so co-signatures can accumulate
            server.pending_checkpoints.write().await.insert(checkpoint_round, checkpoint.clone());

            // Persist checkpoint and its state snapshot to disk
            match ultradag_coin::persistence::save_checkpoint(&data_dir, &checkpoint) {
                Ok(_) => {
                    server.checkpoint_metrics.record_checkpoint_persist_success();
                    // Save the state snapshot at checkpoint time so GetCheckpoint
                    // can serve the correct state (not the advanced current state)
                    if let Err(e) = ultradag_coin::persistence::save_checkpoint_state(&data_dir, checkpoint_round, &state_snapshot) {
                        warn!("Failed to save checkpoint state for round {}: {}", checkpoint_round, e);
                    }
                }
                Err(e) => {
                    warn!("Failed to save checkpoint: {}", e);
                    server.checkpoint_metrics.record_checkpoint_persist_failure();
                }
            }

            server.peers.broadcast(&Message::CheckpointProposal(checkpoint.clone()), "").await;
            last_checkpoint_round = checkpoint_round;

            // Record metrics
            let duration_ms = checkpoint_start.elapsed().as_millis() as u64;
            let size_bytes = serde_json::to_vec(&checkpoint).map(|v| v.len()).unwrap_or(0) as u64;
            server.checkpoint_metrics.record_checkpoint_produced(duration_ms, size_bytes, checkpoint_round);

            info!("Produced checkpoint at round {} ({}ms, {} bytes)", checkpoint_round, duration_ms, size_bytes);

            // Prune old checkpoints to limit disk usage (keep last 10 checkpoints)
            match ultradag_coin::persistence::prune_old_checkpoints(&data_dir, 10) {
                Ok(deleted) => {
                    if deleted > 0 {
                        server.checkpoint_metrics.record_checkpoints_pruned(deleted as u64);
                        info!("Pruned {} old checkpoints from disk", deleted);
                    }
                    // Update disk count metric
                    let disk_count = ultradag_coin::persistence::list_checkpoints(&data_dir).len() as u64;
                    server.checkpoint_metrics.update_checkpoint_disk_count(disk_count);
                }
                Err(e) => {
                    warn!("Failed to prune old checkpoints: {}", e);
                }
            }
        }

        // Prune old rounds to bound memory (every 50 rounds to amortize cost)
        // pruning_depth=0 means archive mode (no pruning)
        if dag_round % 50 == 0 && server.pruning_depth > 0 {
            let last_fin = server.finality.read().await.last_finalized_round();
            if last_fin > 0 {
                let pruned = {
                    let mut dag_w = server.dag.write().await;
                    dag_w.prune_old_rounds_with_depth(last_fin, server.pruning_depth)
                };
                if pruned > 0 {
                    info!("Pruned {} old vertices (floor={})", last_fin, last_fin);
                    // Also prune finalized hash set to prevent unbounded memory growth.
                    // Acquire dag read (not write) after dropping dag write above.
                    let mut fin = server.finality.write().await;
                    let dag_r = server.dag.read().await;
                    fin.prune_finalized(&dag_r);
                }
            }
        }

        last_production = tokio::time::Instant::now();
        consecutive_skips = 0;
        // Don't reset in_recovery here — only exit recovery when quorum
        // actually resumes (line 138-141). Resetting here causes a tight
        // loop: produce → reset → 3 skips → recovery → produce → reset...

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

        // Vertex hash gossip reconciliation: exchange compact hash summaries with peers
        // to detect and recover missing vertices. Much more efficient than requesting
        // full vertex batches — hashes are 32 bytes vs ~2-5 KB per vertex.
        //
        // Every 10 rounds (~50s): scan last 200 rounds for missing vertices.
        // Every 100 rounds (~500s): full-range scan of entire retained DAG (safety net).
        if dag_round % 10 == 5 {
            let dag_r = server.dag.read().await;
            let floor = dag_r.pruning_floor();
            let current = dag_r.current_round();
            drop(dag_r);

            let (from_round, to_round) = if dag_round % 100 == 75 {
                // Full-range scan every 100 rounds
                (floor, current.saturating_sub(2))
            } else {
                // Recent 200-round window every 10 rounds
                (current.saturating_sub(200).max(floor), current.saturating_sub(2))
            };

            if from_round < to_round {
                server
                    .peers
                    .broadcast(
                        &Message::GetRoundHashes { from_round, to_round },
                        "",
                    )
                    .await;
            }
        }

        // Periodic persistence: save state every 10 rounds
        if dag_round % 10 == 0 {
            let dag_path = data_dir.join("dag.json");
            let finality_path = data_dir.join("finality.json");
            let state_path = data_dir.join("state.redb");
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
