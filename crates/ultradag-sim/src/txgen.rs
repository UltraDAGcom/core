use ultradag_coin::{
    SecretKey, Address, Transaction, TransferTx, Signature,
    StakeTx, UnstakeTx, DelegateTx, UndelegateTx, SetCommissionTx,
    constants::MIN_FEE_SATS,
    tx::bridge::{BridgeDepositTx, BridgeReleaseTx},
};
use ultradag_coin::governance::{CreateProposalTx, VoteTx, ProposalType};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

/// Generate a random transfer transaction from a funded account.
pub fn generate_transfer(
    rng: &mut ChaCha8Rng,
    funded_accounts: &[(SecretKey, u64, u64)],
) -> Option<(Transaction, usize)> {
    if funded_accounts.is_empty() {
        return None;
    }

    let min_needed = MIN_FEE_SATS + 1;
    let eligible: Vec<usize> = funded_accounts.iter()
        .enumerate()
        .filter(|(_, (_, bal, _))| *bal >= min_needed)
        .map(|(i, _)| i)
        .collect();

    if eligible.is_empty() {
        return None;
    }

    let sender_idx = eligible[rng.gen_range(0..eligible.len())];
    let (ref sk, balance, nonce) = funded_accounts[sender_idx];

    let recipient_idx = loop {
        let idx = rng.gen_range(0..funded_accounts.len());
        if idx != sender_idx {
            break idx;
        }
        if funded_accounts.len() == 1 {
            break sender_idx;
        }
    };

    let to = if recipient_idx == sender_idx {
        let mut addr_bytes = [0u8; 20];
        rng.fill(&mut addr_bytes);
        Address(addr_bytes)
    } else {
        funded_accounts[recipient_idx].0.address()
    };

    let max_amount = balance.saturating_sub(MIN_FEE_SATS);
    if max_amount == 0 {
        return None;
    }
    let amount = rng.gen_range(1..=max_amount.min(balance / 2).max(1));

    let from = sk.address();
    let pub_key = sk.verifying_key().to_bytes();

    let mut tx = TransferTx {
        from, to, amount, fee: MIN_FEE_SATS, nonce, pub_key,
        signature: Signature([0u8; 64]), memo: None,
    };
    tx.signature = sk.sign(&tx.signable_bytes());

    Some((Transaction::Transfer(tx), sender_idx))
}

/// Generate up to `count` random transfers, updating account balances/nonces optimistically.
pub fn generate_round_transactions(
    rng: &mut ChaCha8Rng,
    accounts: &mut [(SecretKey, u64, u64)],
    count: usize,
) -> Vec<Transaction> {
    let mut txs = Vec::new();
    for _ in 0..count {
        if let Some((tx, sender_idx)) = generate_transfer(rng, accounts) {
            let cost = tx.fee().saturating_add(tx.amount());
            accounts[sender_idx].1 = accounts[sender_idx].1.saturating_sub(cost);
            accounts[sender_idx].2 += 1;
            txs.push(tx);
        }
    }
    txs
}

// === New transaction generators for staking, delegation, governance ===

pub fn generate_stake_tx(sk: &SecretKey, amount: u64, nonce: u64) -> Transaction {
    let from = sk.address();
    let pub_key = sk.verifying_key().to_bytes();
    let mut tx = StakeTx { from, amount, nonce, pub_key, signature: Signature([0u8; 64]) };
    tx.signature = sk.sign(&tx.signable_bytes());
    Transaction::Stake(tx)
}

pub fn generate_unstake_tx(sk: &SecretKey, nonce: u64) -> Transaction {
    let from = sk.address();
    let pub_key = sk.verifying_key().to_bytes();
    let mut tx = UnstakeTx { from, nonce, pub_key, signature: Signature([0u8; 64]) };
    tx.signature = sk.sign(&tx.signable_bytes());
    Transaction::Unstake(tx)
}

pub fn generate_delegate_tx(sk: &SecretKey, validator: Address, amount: u64, nonce: u64) -> Transaction {
    let from = sk.address();
    let pub_key = sk.verifying_key().to_bytes();
    let mut tx = DelegateTx { from, validator, amount, nonce, pub_key, signature: Signature([0u8; 64]) };
    tx.signature = sk.sign(&tx.signable_bytes());
    Transaction::Delegate(tx)
}

pub fn generate_undelegate_tx(sk: &SecretKey, nonce: u64) -> Transaction {
    let from = sk.address();
    let pub_key = sk.verifying_key().to_bytes();
    let mut tx = UndelegateTx { from, nonce, pub_key, signature: Signature([0u8; 64]) };
    tx.signature = sk.sign(&tx.signable_bytes());
    Transaction::Undelegate(tx)
}

pub fn generate_set_commission_tx(sk: &SecretKey, commission_percent: u8, nonce: u64) -> Transaction {
    let from = sk.address();
    let pub_key = sk.verifying_key().to_bytes();
    let mut tx = SetCommissionTx { from, commission_percent, nonce, pub_key, signature: Signature([0u8; 64]) };
    tx.signature = sk.sign(&tx.signable_bytes());
    Transaction::SetCommission(tx)
}

pub fn generate_create_proposal_tx(
    sk: &SecretKey,
    proposal_id: u64,
    proposal_type: ProposalType,
    fee: u64,
    nonce: u64,
) -> Transaction {
    let from = sk.address();
    let pub_key = sk.verifying_key().to_bytes();
    let mut tx = CreateProposalTx {
        from,
        proposal_id,
        title: format!("Proposal {}", proposal_id),
        description: "Simulation test proposal".to_string(),
        proposal_type,
        fee,
        nonce,
        pub_key,
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    Transaction::CreateProposal(tx)
}

pub fn generate_vote_tx(
    sk: &SecretKey,
    proposal_id: u64,
    vote: bool,
    fee: u64,
    nonce: u64,
) -> Transaction {
    let from = sk.address();
    let pub_key = sk.verifying_key().to_bytes();
    let mut tx = VoteTx {
        from, proposal_id, vote, fee, nonce, pub_key,
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    Transaction::Vote(tx)
}

pub fn generate_transfer_to(
    sk: &SecretKey,
    to: Address,
    amount: u64,
    nonce: u64,
) -> Option<Transaction> {
    if amount == 0 { return None; }
    let mut tx = TransferTx {
        from: sk.address(), to, amount, fee: MIN_FEE_SATS, nonce,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]), memo: None,
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    Some(Transaction::Transfer(tx))
}

// === Bridge transaction generators ===

/// Generate a BridgeDepositTx: lock UDAG on native chain for Arbitrum withdrawal.
pub fn generate_bridge_deposit_tx(
    sk: &SecretKey,
    recipient: [u8; 20],
    amount: u64,
    destination_chain_id: u64,
    nonce: u64,
) -> Transaction {
    let from = sk.address();
    let pub_key = sk.verifying_key().to_bytes();
    let mut tx = BridgeDepositTx {
        from, recipient, amount, destination_chain_id, fee: MIN_FEE_SATS, nonce, pub_key,
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    Transaction::BridgeDeposit(tx)
}

/// Generate a BridgeReleaseTx: release locked funds from bridge_reserve to native recipient.
/// Submitted by validators who observed an Arbitrum deposit.
pub fn generate_bridge_release_tx(
    sk: &SecretKey,
    recipient: Address,
    amount: u64,
    source_chain_id: u64,
    deposit_nonce: u64,
    nonce: u64,
) -> Transaction {
    let from = sk.address();
    let pub_key = sk.verifying_key().to_bytes();
    let mut tx = BridgeReleaseTx {
        from, recipient, amount, source_chain_id, deposit_nonce, nonce, pub_key,
        signature: Signature([0u8; 64]),
    };
    tx.signature = sk.sign(&tx.signable_bytes());
    Transaction::BridgeRelease(tx)
}
