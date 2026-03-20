use ultradag_coin::consensus::validator_set::ValidatorSet;
use ultradag_coin::{Address, SecretKey};

/// Helper: create a ValidatorSet with the given addresses pre-registered.
/// Uses min_validators=1 so small sets still produce finite quorum thresholds.
fn validator_set_from(addrs: &[Address]) -> ValidatorSet {
    let mut vset = ValidatorSet::new(1);
    for addr in addrs {
        vset.register(*addr);
    }
    vset
}

#[test]
fn test_validator_set_new() {
    let validators = vec![
        Address([1u8; 20]),
        Address([2u8; 20]),
        Address([3u8; 20]),
    ];

    let vset = validator_set_from(&validators);
    assert_eq!(vset.validators().len(), 3);
}

#[test]
fn test_validator_set_contains() {
    let addr1 = Address([1u8; 20]);
    let addr2 = Address([2u8; 20]);
    let addr3 = Address([99u8; 20]);

    let vset = validator_set_from(&[addr1, addr2]);

    assert!(vset.contains(&addr1));
    assert!(vset.contains(&addr2));
    assert!(!vset.contains(&addr3));
}

#[test]
fn test_validator_set_count() {
    let validators = vec![
        Address([1u8; 20]),
        Address([2u8; 20]),
        Address([3u8; 20]),
        Address([4u8; 20]),
    ];

    let vset = validator_set_from(&validators);
    assert_eq!(vset.len(), 4);
}

#[test]
fn test_validator_set_quorum() {
    let validators = vec![
        Address([1u8; 20]),
        Address([2u8; 20]),
        Address([3u8; 20]),
    ];

    let vset = validator_set_from(&validators);
    // ceil(2*3/3) = ceil(6/3) = 2  →  formula: (2*3+2)/3 = 8/3 = 2
    assert_eq!(vset.quorum_threshold(), 2);
}

#[test]
fn test_validator_set_quorum_four_validators() {
    let validators = vec![
        Address([1u8; 20]),
        Address([2u8; 20]),
        Address([3u8; 20]),
        Address([4u8; 20]),
    ];

    let vset = validator_set_from(&validators);
    // ceil(2*4/3) = ceil(8/3) = 3  →  formula: (2*4+2)/3 = 10/3 = 3
    assert_eq!(vset.quorum_threshold(), 3);
}

#[test]
fn test_validator_set_empty() {
    let vset = ValidatorSet::new(1);
    assert_eq!(vset.len(), 0);
    // With min_validators=1 and 0 registered, quorum_threshold returns usize::MAX
    assert_eq!(vset.quorum_threshold(), usize::MAX);
}

#[test]
fn test_validator_set_single() {
    let validators = vec![Address([1u8; 20])];
    let vset = validator_set_from(&validators);
    assert_eq!(vset.len(), 1);
    // ceil(2*1/3) = 1  →  formula: (2*1+2)/3 = 4/3 = 1
    assert_eq!(vset.quorum_threshold(), 1);
}

#[test]
fn test_validator_set_validators_list() {
    let addr1 = Address([1u8; 20]);
    let addr2 = Address([2u8; 20]);

    let vset = validator_set_from(&[addr1, addr2]);
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

    let vset = validator_set_from(&[sk1.address(), sk2.address(), sk3.address()]);

    assert_eq!(vset.len(), 3);
    assert!(vset.contains(&sk1.address()));
    assert!(vset.contains(&sk2.address()));
    assert!(vset.contains(&sk3.address()));
}

#[test]
fn test_validator_set_quorum_calculation() {
    // Verify ceil(2n/3) property for n=1..=10
    for n in 1usize..=10 {
        let validators: Vec<Address> = (0..n)
            .map(|i| {
                let mut addr = [0u8; 20];
                addr[0] = i as u8;
                Address(addr)
            })
            .collect();

        let vset = validator_set_from(&validators);
        let quorum = vset.quorum_threshold();

        // quorum should be ceil(2n/3): smallest q such that q*3 >= n*2
        // The implementation uses (2*n+2)/3 which equals ceil(2n/3) for integer n.
        assert!(quorum * 3 >= n * 2, "n={n}, quorum={quorum}: quorum*3 should be >= n*2");
        assert!((quorum - 1) * 3 < n * 2, "n={n}, quorum={quorum}: (quorum-1)*3 should be < n*2");
    }
}
