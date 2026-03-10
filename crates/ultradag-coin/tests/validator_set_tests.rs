use ultradag_coin::consensus::validator_set::ValidatorSet;
use ultradag_coin::{Address, SecretKey};

#[test]
fn test_validator_set_new() {
    let validators = vec![
        Address([1u8; 32]),
        Address([2u8; 32]),
        Address([3u8; 32]),
    ];
    
    let vset = ValidatorSet::new(validators.clone());
    assert_eq!(vset.validators().len(), 3);
}

#[test]
fn test_validator_set_contains() {
    let addr1 = Address([1u8; 32]);
    let addr2 = Address([2u8; 32]);
    let addr3 = Address([99u8; 32]);
    
    let validators = vec![addr1, addr2];
    let vset = ValidatorSet::new(validators);
    
    assert!(vset.contains(&addr1));
    assert!(vset.contains(&addr2));
    assert!(!vset.contains(&addr3));
}

#[test]
fn test_validator_set_count() {
    let validators = vec![
        Address([1u8; 32]),
        Address([2u8; 32]),
        Address([3u8; 32]),
        Address([4u8; 32]),
    ];
    
    let vset = ValidatorSet::new(validators);
    assert_eq!(vset.count(), 4);
}

#[test]
fn test_validator_set_quorum() {
    let validators = vec![
        Address([1u8; 32]),
        Address([2u8; 32]),
        Address([3u8; 32]),
    ];
    
    let vset = ValidatorSet::new(validators);
    assert_eq!(vset.quorum(), 2);
}

#[test]
fn test_validator_set_quorum_four_validators() {
    let validators = vec![
        Address([1u8; 32]),
        Address([2u8; 32]),
        Address([3u8; 32]),
        Address([4u8; 32]),
    ];
    
    let vset = ValidatorSet::new(validators);
    assert_eq!(vset.quorum(), 3);
}

#[test]
fn test_validator_set_empty() {
    let vset = ValidatorSet::new(vec![]);
    assert_eq!(vset.count(), 0);
    assert_eq!(vset.quorum(), 0);
}

#[test]
fn test_validator_set_single() {
    let validators = vec![Address([1u8; 32])];
    let vset = ValidatorSet::new(validators);
    assert_eq!(vset.count(), 1);
    assert_eq!(vset.quorum(), 1);
}

#[test]
fn test_validator_set_validators_list() {
    let addr1 = Address([1u8; 32]);
    let addr2 = Address([2u8; 32]);
    let validators = vec![addr1, addr2];
    
    let vset = ValidatorSet::new(validators.clone());
    let list = vset.validators();
    
    assert_eq!(list.len(), 2);
    assert!(list.contains(&addr1));
    assert!(list.contains(&addr2));
}

#[test]
fn test_validator_set_from_addresses() {
    let sk1 = SecretKey::generate();
    let sk2 = SecretKey::generate();
    let sk3 = SecretKey::generate();
    
    let validators = vec![sk1.address(), sk2.address(), sk3.address()];
    let vset = ValidatorSet::new(validators);
    
    assert_eq!(vset.count(), 3);
    assert!(vset.contains(&sk1.address()));
    assert!(vset.contains(&sk2.address()));
    assert!(vset.contains(&sk3.address()));
}

#[test]
fn test_validator_set_quorum_calculation() {
    for n in 1..=10 {
        let validators: Vec<Address> = (0..n)
            .map(|i| {
                let mut addr = [0u8; 32];
                addr[0] = i as u8;
                Address(addr)
            })
            .collect();
        
        let vset = ValidatorSet::new(validators);
        let quorum = vset.quorum();
        
        assert!(quorum * 3 > n * 2);
        assert!((quorum - 1) * 3 <= n * 2);
    }
}
