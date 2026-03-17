use ultradag_coin::{
    SecretKey, Address, Transaction, TransferTx, Signature,
    constants::MIN_FEE_SATS,
};
use rand::Rng;
use rand_chacha::ChaCha8Rng;

/// Generate a random transfer transaction from a funded account.
pub fn generate_transfer(
    rng: &mut ChaCha8Rng,
    funded_accounts: &[(SecretKey, u64, u64)], // (sk, balance, nonce)
) -> Option<(Transaction, usize)> {
    if funded_accounts.is_empty() {
        return None;
    }

    // Find accounts with sufficient balance
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

    // Pick a random recipient (different from sender)
    let recipient_idx = loop {
        let idx = rng.gen_range(0..funded_accounts.len());
        if idx != sender_idx {
            break idx;
        }
        if funded_accounts.len() == 1 {
            // Single account — send to a random address
            break sender_idx; // Will use a generated address below
        }
    };

    let to = if recipient_idx == sender_idx {
        // Generate a random address
        let mut addr_bytes = [0u8; 32];
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
        from,
        to,
        amount,
        fee: MIN_FEE_SATS,
        nonce,
        pub_key,
        signature: Signature([0u8; 64]),
        memo: None,
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
            // Update sender balance and nonce optimistically
            let cost = tx.fee().saturating_add(tx.amount());
            accounts[sender_idx].1 = accounts[sender_idx].1.saturating_sub(cost);
            accounts[sender_idx].2 += 1;
            txs.push(tx);
        }
    }
    txs
}
