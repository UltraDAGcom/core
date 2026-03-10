use crate::address::{Address, Signature, SecretKey};
use crate::state::persistence::StateSnapshot;
use serde::{Deserialize, Serialize};

/// A checkpoint captures the full consensus and state at a specific finalized round.
/// Multiple validators sign the same checkpoint — a node accepts it when
/// it has signatures from >= quorum distinct validators.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// The finalized round this checkpoint covers.
    pub round: u64,
    /// Blake3 hash of the serialized StateSnapshot at this round.
    pub state_root: [u8; 32],
    /// Blake3 hash of the last finalized vertex at this round.
    pub dag_tip: [u8; 32],
    /// Total supply at this round (for quick sanity check).
    pub total_supply: u64,
    /// Blake3 hash of the previous checkpoint (links checkpoints into a chain).
    /// For genesis checkpoint (round 0), this is [0u8; 32].
    pub prev_checkpoint_hash: [u8; 32],
    /// Validator signatures over checkpoint_hash().
    pub signatures: Vec<CheckpointSignature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointSignature {
    pub validator: Address,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

impl Checkpoint {
    /// The bytes that validators sign over.
    /// Includes round, state_root, dag_tip, prev_checkpoint_hash — NOT the signatures field.
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"checkpoint");
        buf.extend_from_slice(&self.round.to_le_bytes());
        buf.extend_from_slice(&self.state_root);
        buf.extend_from_slice(&self.dag_tip);
        buf.extend_from_slice(&self.total_supply.to_le_bytes());
        buf.extend_from_slice(&self.prev_checkpoint_hash);
        buf
    }

    /// Hash of the signable bytes — this is what validators sign.
    pub fn checkpoint_hash(&self) -> [u8; 32] {
        *blake3::hash(&self.signable_bytes()).as_bytes()
    }

    /// Sign this checkpoint with a validator's key.
    pub fn sign(&mut self, sk: &SecretKey) {
        let sig = sk.sign(&self.signable_bytes());
        let pub_key = sk.verifying_key().to_bytes();
        self.signatures.push(CheckpointSignature {
            validator: sk.address(),
            pub_key,
            signature: sig,
        });
    }

    /// Verify all signatures and return the set of valid validator addresses.
    pub fn valid_signers(&self) -> Vec<Address> {
        self.signatures.iter()
            .filter(|s| self.verify_signature(s))
            .map(|s| s.validator)
            .collect()
    }

    /// Check if this checkpoint has >= quorum valid signatures
    /// from the provided active validator set.
    pub fn is_accepted(&self, active_validators: &[Address], quorum: usize) -> bool {
        let valid: std::collections::HashSet<Address> = self.valid_signers()
            .into_iter()
            .collect();
        let matching = active_validators.iter()
            .filter(|v| valid.contains(v))
            .count();
        matching >= quorum
    }

    fn verify_signature(&self, sig: &CheckpointSignature) -> bool {
        // Verify pub_key → address mapping
        let expected_addr = Address(*blake3::hash(&sig.pub_key).as_bytes());
        if expected_addr != sig.validator { return false; }
        // Verify Ed25519 signature
        let vk = ed25519_dalek::VerifyingKey::from_bytes(&sig.pub_key).ok();
        let s = ed25519_dalek::Signature::from_bytes(&sig.signature.0);
        vk.map(|k| k.verify_strict(&self.signable_bytes(), &s).is_ok())
          .unwrap_or(false)
    }
}

/// Compute the state root hash from a StateSnapshot.
pub fn compute_state_root(snapshot: &StateSnapshot) -> [u8; 32] {
    let bytes = serde_json::to_vec(snapshot)
        .expect("StateSnapshot serialization must not fail");
    *blake3::hash(&bytes).as_bytes()
}

/// Compute the hash of a checkpoint (for linking in the chain).
/// This is blake3(serialize(checkpoint)) excluding signatures.
pub fn compute_checkpoint_hash(checkpoint: &Checkpoint) -> [u8; 32] {
    // Serialize checkpoint without signatures for stable hash
    let mut cp_for_hash = checkpoint.clone();
    cp_for_hash.signatures.clear();
    
    let bytes = serde_json::to_vec(&cp_for_hash)
        .expect("Checkpoint serialization must not fail");
    *blake3::hash(&bytes).as_bytes()
}

/// Verify that a checkpoint chain links correctly back to genesis.
/// 
/// This function walks the checkpoint chain backwards via prev_checkpoint_hash
/// until it reaches a checkpoint with round 0 (genesis). It then verifies that
/// the genesis checkpoint's hash matches GENESIS_CHECKPOINT_HASH.
/// 
/// # Arguments
/// * `checkpoint` - The checkpoint to verify
/// * `get_checkpoint` - Function to retrieve a checkpoint by its hash
/// 
/// # Returns
/// * `Ok(())` if the chain is valid
/// * `Err(String)` with error description if invalid
pub fn verify_checkpoint_chain<F>(
    checkpoint: &Checkpoint,
    mut get_checkpoint: F,
) -> Result<(), String>
where
    F: FnMut([u8; 32]) -> Option<Checkpoint>,
{
    // If this is genesis, verify its hash matches the constant
    if checkpoint.round == 0 {
        let genesis_hash = compute_checkpoint_hash(checkpoint);
        if genesis_hash == crate::constants::GENESIS_CHECKPOINT_HASH {
            return Ok(());
        }
        // If GENESIS_CHECKPOINT_HASH is all zeros (placeholder), accept any genesis
        if crate::constants::GENESIS_CHECKPOINT_HASH == [0u8; 32] {
            return Ok(());
        }
        return Err(format!(
            "Genesis checkpoint hash mismatch: expected {:?}, got {:?}",
            crate::constants::GENESIS_CHECKPOINT_HASH,
            genesis_hash
        ));
    }
    
    // Walk backwards through the chain
    let mut current = checkpoint.clone();
    let mut visited = std::collections::HashSet::new();
    
    loop {
        // Detect cycles
        let current_hash = compute_checkpoint_hash(&current);
        if !visited.insert(current_hash) {
            return Err("Checkpoint chain contains a cycle".to_string());
        }
        
        // If we reached genesis, verify it
        if current.round == 0 {
            let genesis_hash = compute_checkpoint_hash(&current);
            if genesis_hash == crate::constants::GENESIS_CHECKPOINT_HASH {
                return Ok(());
            }
            // If GENESIS_CHECKPOINT_HASH is all zeros (placeholder), accept any genesis
            if crate::constants::GENESIS_CHECKPOINT_HASH == [0u8; 32] {
                return Ok(());
            }
            return Err(format!(
                "Genesis checkpoint hash mismatch: expected {:?}, got {:?}",
                crate::constants::GENESIS_CHECKPOINT_HASH,
                genesis_hash
            ));
        }
        
        // Get previous checkpoint
        let prev_hash = current.prev_checkpoint_hash;
        if prev_hash == [0u8; 32] && current.round != 0 {
            return Err(format!(
                "Non-genesis checkpoint (round {}) has zero prev_checkpoint_hash",
                current.round
            ));
        }
        
        match get_checkpoint(prev_hash) {
            Some(prev) => {
                // Verify the hash matches
                let computed_hash = compute_checkpoint_hash(&prev);
                if computed_hash != prev_hash {
                    return Err(format!(
                        "Checkpoint hash mismatch at round {}: expected {:?}, computed {:?}",
                        prev.round, prev_hash, computed_hash
                    ));
                }
                current = prev;
            }
            None => {
                return Err(format!(
                    "Missing checkpoint in chain: prev_hash {:?} not found",
                    prev_hash
                ));
            }
        }
        
        // Safety: limit chain length to prevent DoS
        if visited.len() > 10000 {
            return Err("Checkpoint chain too long (>10000 checkpoints)".to_string());
        }
    }
}
