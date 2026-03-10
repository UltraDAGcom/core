use ultradag_node::rate_limit::{RateLimiter, limits};
use std::net::{IpAddr, Ipv4Addr};

#[test]
fn test_rate_limit_global_endpoint() {
    let limiter = RateLimiter::new();
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
    
    for _ in 0..limits::GLOBAL.requests_per_window {
        assert!(limiter.check_rate_limit(ip, limits::GLOBAL));
    }
    
    assert!(!limiter.check_rate_limit(ip, limits::GLOBAL));
}

#[test]
fn test_rate_limit_tx_endpoint() {
    let limiter = RateLimiter::new();
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 101));
    
    for _ in 0..limits::TX.requests_per_window {
        assert!(limiter.check_rate_limit(ip, limits::TX));
    }
    
    assert!(!limiter.check_rate_limit(ip, limits::TX));
}

#[test]
fn test_rate_limit_faucet_endpoint() {
    let limiter = RateLimiter::new();
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 102));
    
    assert!(limiter.check_rate_limit(ip, limits::FAUCET));
    assert!(!limiter.check_rate_limit(ip, limits::FAUCET));
}

#[test]
fn test_rate_limit_stake_endpoint() {
    let limiter = RateLimiter::new();
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 103));
    
    for _ in 0..limits::STAKE.requests_per_window {
        assert!(limiter.check_rate_limit(ip, limits::STAKE));
    }
    
    assert!(!limiter.check_rate_limit(ip, limits::STAKE));
}

#[test]
fn test_rate_limit_unstake_endpoint() {
    let limiter = RateLimiter::new();
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 104));
    
    for _ in 0..limits::UNSTAKE.requests_per_window {
        assert!(limiter.check_rate_limit(ip, limits::UNSTAKE));
    }
    
    assert!(!limiter.check_rate_limit(ip, limits::UNSTAKE));
}

#[test]
fn test_rate_limit_proposal_endpoint() {
    let limiter = RateLimiter::new();
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 105));
    
    for _ in 0..limits::PROPOSAL.requests_per_window {
        assert!(limiter.check_rate_limit(ip, limits::PROPOSAL));
    }
    
    assert!(!limiter.check_rate_limit(ip, limits::PROPOSAL));
}

#[test]
fn test_rate_limit_vote_endpoint() {
    let limiter = RateLimiter::new();
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 106));
    
    for _ in 0..limits::VOTE.requests_per_window {
        assert!(limiter.check_rate_limit(ip, limits::VOTE));
    }
    
    assert!(!limiter.check_rate_limit(ip, limits::VOTE));
}

#[test]
fn test_connection_limit_enforcement() {
    use ultradag_node::rate_limit::limits::MAX_CONCURRENT_CONNECTIONS;
    let limiter = RateLimiter::new();
    
    for _ in 0..MAX_CONCURRENT_CONNECTIONS {
        assert!(limiter.add_connection().is_ok());
    }
    
    assert!(limiter.add_connection().is_err());
}

#[test]
fn test_connection_removal_allows_new() {
    use ultradag_node::rate_limit::limits::MAX_CONCURRENT_CONNECTIONS;
    let limiter = RateLimiter::new();
    
    for _ in 0..MAX_CONCURRENT_CONNECTIONS {
        limiter.add_connection().unwrap();
    }
    
    assert!(limiter.add_connection().is_err());
    
    limiter.remove_connection();
    assert!(limiter.add_connection().is_ok());
}

#[test]
fn test_rate_limit_different_ips_independent() {
    let limiter = RateLimiter::new();
    let ip1 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
    let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2));
    
    for _ in 0..limits::TX.requests_per_window {
        assert!(limiter.check_rate_limit(ip1, limits::TX));
    }
    assert!(!limiter.check_rate_limit(ip1, limits::TX));
    
    assert!(limiter.check_rate_limit(ip2, limits::TX));
}

#[test]
fn test_rate_limit_cleanup_expired_entries() {
    let limiter = RateLimiter::new();
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 200));
    
    for _ in 0..5 {
        limiter.check_rate_limit(ip, limits::TX);
    }
    
    limiter.cleanup_expired();
    
    assert!(limiter.check_rate_limit(ip, limits::TX));
}

#[test]
fn test_concurrent_rate_limit_checks() {
    use std::sync::Arc;
    use std::thread;
    
    let limiter = Arc::new(RateLimiter::new());
    let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
    
    let mut handles = vec![];
    
    for _ in 0..5 {
        let l = Arc::clone(&limiter);
        let handle = thread::spawn(move || {
            let mut count = 0;
            for _ in 0..20 {
                if l.check_rate_limit(ip, limits::TX) {
                    count += 1;
                }
            }
            count
        });
        handles.push(handle);
    }
    
    let total: usize = handles.into_iter()
        .map(|h| h.join().unwrap())
        .sum();
    
    assert_eq!(total, limits::TX.requests_per_window as usize);
}
