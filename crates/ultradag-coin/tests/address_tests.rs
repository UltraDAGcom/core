use ultradag_coin::{Address, SecretKey};

#[test]
fn test_address_from_secret_key() {
    let sk = SecretKey::generate();
    let addr = sk.address();
    
    assert_ne!(addr.0, [0u8; 20]);
}

#[test]
fn test_address_deterministic() {
    let sk = SecretKey::generate();
    let addr1 = sk.address();
    let addr2 = sk.address();
    
    assert_eq!(addr1, addr2);
}

#[test]
fn test_address_from_hex_valid() {
    let hex = "0000000000000000000000000000000000000001";
    let addr = Address::from_hex(hex);

    assert!(addr.is_some());
    assert_eq!(addr.unwrap().0[19], 1);
}

#[test]
fn test_address_from_hex_invalid() {
    let hex = "invalid";
    let addr = Address::from_hex(hex);
    
    assert!(addr.is_none());
}

#[test]
fn test_address_to_hex() {
    let mut bytes = [0u8; 20];
    bytes[19] = 1;
    let addr = Address(bytes);

    let hex = addr.to_hex();
    assert_eq!(hex.len(), 40);
    assert!(hex.ends_with("01"));
}

#[test]
fn test_address_equality() {
    let sk = SecretKey::generate();
    let addr1 = sk.address();
    let addr2 = sk.address();
    
    assert_eq!(addr1, addr2);
}

#[test]
fn test_address_inequality() {
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    
    let addr1 = sk1.address();
    let addr2 = sk2.address();
    
    assert_ne!(addr1, addr2);
}

#[test]
fn test_address_clone() {
    let sk = SecretKey::generate();
    let addr1 = sk.address();
    let addr2 = addr1;
    
    assert_eq!(addr1, addr2);
}

#[test]
fn test_address_debug() {
    let addr = Address([1u8; 20]);
    let debug_str = format!("{:?}", addr);
    
    assert!(debug_str.contains("Address"));
}

#[test]
fn test_secret_key_generate() {
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    
    assert_ne!(sk1.address(), sk2.address());
}

#[test]
fn test_secret_key_sign() {
    let sk = SecretKey::generate();
    let message = b"test message";
    
    let signature = sk.sign(message);
    assert_ne!(signature.0, [0u8; 64]);
}

#[test]
fn test_secret_key_verifying_key() {
    let sk = SecretKey::generate();
    let vk = sk.verifying_key();
    
    assert_ne!(vk.to_bytes(), [0u8; 32]);
}

#[test]
fn test_signature_verification() {
    let sk = SecretKey::generate();
    let message = b"test message";
    
    let signature = sk.sign(message);
    let vk = sk.verifying_key();
    
    assert!(signature.verify(&vk, message));
}

#[test]
fn test_signature_verification_wrong_message() {
    let sk = SecretKey::generate();
    let message1 = b"test message";
    let message2 = b"different message";

    let signature = sk.sign(message1);
    let vk = sk.verifying_key();

    assert!(!signature.verify(&vk, message2));
}

#[test]
fn test_address_from_different_keys() {
    let addresses: Vec<Address> = (0..10)
        .map(|_| SecretKey::generate().address())
        .collect();
    
    for i in 0..addresses.len() {
        for j in (i + 1)..addresses.len() {
            assert_ne!(addresses[i], addresses[j]);
        }
    }
}

#[test]
fn test_address_hex_roundtrip() {
    let sk = SecretKey::generate();
    let addr1 = sk.address();
    
    let hex = addr1.to_hex();
    let addr2 = Address::from_hex(&hex).unwrap();
    
    assert_eq!(addr1, addr2);
}
