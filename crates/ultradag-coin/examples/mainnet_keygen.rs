//! Offline mainnet keypair generator.
//!
//! Generates a fresh Ed25519 keypair using the operating system's secure
//! random source, derives the exact UltraDAG address the node would compute
//! from the same public key, and prints everything a founder / treasury
//! holder / multisig member needs to safely hold the key.
//!
//! # Why this exists
//!
//! - `SecretKey::generate()` is gated behind `#[cfg(not(feature = "mainnet"))]`
//!   so it cannot be used for real mainnet key material (this is intentional —
//!   we don't want test-style random key generation baked into production
//!   binaries).
//! - The Rust SDK (`sdk/rust/src/crypto.rs`) has a bug where its
//!   `derive_address()` returns the full 32-byte blake3 hash instead of the
//!   truncated 20-byte address the protocol actually uses. Keys generated
//!   via the SDK cannot be spent on the real chain.
//! - The testnet `/keygen` RPC endpoint requires the node to see your secret
//!   key, which is the opposite of what you want for production.
//!
//! This example uses `ed25519_dalek::SigningKey::generate()` with `OsRng`
//! directly (the same path `SecretKey::generate` uses internally), then
//! imports the resulting bytes through `SecretKey::from_bytes()` — which
//! IS available on mainnet — and uses the real protocol `Address::from_pubkey`
//! for derivation, so the result is guaranteed byte-identical to what a
//! mainnet node would compute.
//!
//! # Usage
//!
//!     cargo run --release --features mainnet -p ultradag-coin --example mainnet_keygen
//!
//! On a mainnet build the printed bech32m prefix is `udag1…`. On a testnet
//! build (no `--features mainnet`) it's `tudg1…` but the underlying 20-byte
//! address identity is identical.
//!
//! # Security
//!
//! - **Run on an air-gapped machine** if at all possible. Disable networking,
//!   run this binary, write the secret hex onto paper or a hardware wallet,
//!   then wipe the machine.
//! - **Never paste the secret into a chat window, email, cloud note, browser
//!   form, or any file that syncs to a cloud backup.** Once it leaves your
//!   local machine the address is compromised.
//! - **Back it up in at least two physical locations** (paper in a safe,
//!   steel plate, hardware wallet seed). Loss of the secret = permanent loss
//!   of every UDAG credited to the address by genesis or emission.

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use ultradag_coin::address::{Address, SecretKey};

fn main() {
    // Generate a fresh Ed25519 secret via OsRng (the OS's CSPRNG).
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let secret_bytes: [u8; 32] = signing_key.to_bytes();

    // Import through the production SecretKey API to guarantee the same
    // pubkey derivation a real node would use.
    let sk = SecretKey::from_bytes(secret_bytes);
    let pubkey_bytes: [u8; 32] = sk.verifying_key().to_bytes();

    // Derive the address using the EXACT same function the node uses.
    // Address::from_pubkey is blake3(pubkey)[..20]. This is NOT the same as
    // the buggy 32-byte form in sdk/rust/src/crypto.rs::derive_address.
    let address: Address = Address::from_pubkey(&pubkey_bytes);

    let secret_hex: String = secret_bytes.iter().map(|b| format!("{:02x}", b)).collect();
    let pubkey_hex: String = pubkey_bytes.iter().map(|b| format!("{:02x}", b)).collect();
    let address_hex: String = address.to_hex();
    let address_bech32: String = address.to_bech32();

    // 20-byte array literal for pasting into constants.rs.
    // Format: two lines of 10 bytes each, with trailing commas on every
    // byte so the result is copy-paste-valid Rust syntax.
    let mut bytes_literal = String::new();
    for (i, b) in address.0.iter().enumerate() {
        if i == 0 {
            bytes_literal.push_str("    ");
        } else if i == 10 {
            bytes_literal.push_str("\n    ");
        } else {
            bytes_literal.push(' ');
        }
        bytes_literal.push_str(&format!("0x{:02x},", b));
    }

    println!();
    println!("===========================================================");
    println!("  UltraDAG offline keygen — fresh mainnet-safe keypair");
    println!("===========================================================");
    println!();
    println!("  SECRET KEY (hex, 32 bytes):");
    println!("    {}", secret_hex);
    println!();
    println!("  PUBLIC KEY (hex, 32 bytes):");
    println!("    {}", pubkey_hex);
    println!();
    println!("  ADDRESS (hex, 20 bytes):");
    println!("    {}", address_hex);
    println!();
    println!("  ADDRESS (bech32m, for display):");
    println!("    {}", address_bech32);
    println!();
    println!("  CONSTANTS.RS PASTE (for DEV_ADDRESS_BYTES / etc.):");
    println!();
    println!("  pub const DEV_ADDRESS_BYTES: [u8; 20] = [");
    println!("{}", bytes_literal);
    println!("  ];");
    println!();
    println!("===========================================================");
    println!("  SECURITY WARNINGS:");
    println!("===========================================================");
    println!();
    println!("  1. The SECRET KEY above unlocks every UDAG credited to this");
    println!("     address by genesis or by ongoing emission. There is NO");
    println!("     recovery path if you lose it.");
    println!();
    println!("  2. DO NOT paste the secret into chat, email, screenshots,");
    println!("     cloud notes, version control, or any file that syncs off");
    println!("     this machine.");
    println!();
    println!("  3. Write the secret onto paper or a steel backup plate, OR");
    println!("     load it into a hardware wallet. Keep at least two copies");
    println!("     in physically separate locations.");
    println!();
    println!("  4. After you have backed it up and verified the backup can");
    println!("     restore the same secret, CLEAR YOUR TERMINAL SCROLLBACK:");
    println!("       clear && printf '\\e[3J'");
    println!("     and consider rebooting the machine if it's not air-gapped.");
    println!();
    println!("  5. Run this binary ONCE per key. Every invocation produces a");
    println!("     brand-new random keypair with no relationship to any");
    println!("     previous run.");
    println!();
    println!("===========================================================");
}
