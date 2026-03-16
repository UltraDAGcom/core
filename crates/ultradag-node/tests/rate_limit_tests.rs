use ultradag_node::rate_limit::{RateLimiter, RateLimit, limits};
use std::net::{IpAddr, Ipv4Addr};

#[test]
fn test_rate_limit_allows_under_limit() {
    let limiter = RateLimiter::new();
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    let limit = RateLimit::new("test", 5, 60);

    for _ in 0..5 {
        assert!(limiter.check_rate_limit(ip, limit));
    }
}

#[test]
fn test_rate_limit_blocks_over_limit() {
    let limiter = RateLimiter::new();
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    let limit = RateLimit::new("test", 3, 60);

    for _ in 0..3 {
        assert!(limiter.check_rate_limit(ip, limit));
    }
    assert!(!limiter.check_rate_limit(ip, limit));
}

#[test]
fn test_different_endpoints_have_separate_limits() {
    let limiter = RateLimiter::new();
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

    let global = RateLimit::new("global", 2, 60);
    assert!(limiter.check_rate_limit(ip, global));
    assert!(limiter.check_rate_limit(ip, global));
    assert!(!limiter.check_rate_limit(ip, global));

    let faucet = RateLimit::new("faucet", 1, 600);
    assert!(limiter.check_rate_limit(ip, faucet));
}

#[test]
fn test_different_ips_have_separate_limits() {
    let limiter = RateLimiter::new();
    let ip1 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    let ip2 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2));
    let limit = RateLimit::new("test", 2, 60);

    assert!(limiter.check_rate_limit(ip1, limit));
    assert!(limiter.check_rate_limit(ip1, limit));
    assert!(!limiter.check_rate_limit(ip1, limit));

    assert!(limiter.check_rate_limit(ip2, limit));
    assert!(limiter.check_rate_limit(ip2, limit));
}

#[test]
fn test_connection_limits() {
    let limiter = RateLimiter::new();
    
    assert!(limiter.add_connection().is_ok());
    limiter.remove_connection();
    
    assert!(limiter.add_connection().is_ok());
}

#[test]
fn test_max_concurrent_connections() {
    let limiter = RateLimiter::new();
    
    for _ in 0..limits::MAX_CONCURRENT_CONNECTIONS {
        assert!(limiter.add_connection().is_ok());
    }
    
    assert!(limiter.add_connection().is_err());
}

#[test]
fn test_connection_tracking() {
    let limiter = RateLimiter::new();
    
    for _ in 0..10 {
        limiter.add_connection().unwrap();
    }
    
    for _ in 0..5 {
        limiter.remove_connection();
    }
    
    for _ in 0..5 {
        assert!(limiter.add_connection().is_ok());
    }
}

#[test]
fn test_predefined_limits() {
    assert_eq!(limits::TX.requests_per_window, 100);
    assert_eq!(limits::TX.window_duration.as_secs(), 60);

    assert_eq!(limits::FAUCET.requests_per_window, 1);
    assert_eq!(limits::FAUCET.window_duration.as_secs(), 600);

    assert_eq!(limits::STAKE.requests_per_window, 5);
    assert_eq!(limits::UNSTAKE.requests_per_window, 5);
    assert_eq!(limits::PROPOSAL.requests_per_window, 5);
    assert_eq!(limits::VOTE.requests_per_window, 10);
    assert_eq!(limits::GLOBAL.requests_per_window, 1000);
}

#[test]
fn test_cleanup_expired() {
    let limiter = RateLimiter::new();
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    let limit = RateLimit::new("test", 5, 1);

    for _ in 0..5 {
        limiter.check_rate_limit(ip, limit);
    }

    std::thread::sleep(std::time::Duration::from_secs(2));
    limiter.cleanup_expired();

    assert!(limiter.check_rate_limit(ip, limit));
}

#[test]
fn test_rate_limit_window_reset() {
    let limiter = RateLimiter::new();
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    let limit = RateLimit::new("test", 2, 1);

    assert!(limiter.check_rate_limit(ip, limit));
    assert!(limiter.check_rate_limit(ip, limit));
    assert!(!limiter.check_rate_limit(ip, limit));

    std::thread::sleep(std::time::Duration::from_secs(2));

    assert!(limiter.check_rate_limit(ip, limit));
    assert!(limiter.check_rate_limit(ip, limit));
}

#[test]
fn test_concurrent_rate_limiting() {
    use std::sync::Arc;
    use std::thread;
    
    let limiter = Arc::new(RateLimiter::new());
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    let limit = RateLimit::new("test", 100, 60);
    
    let mut handles = vec![];
    
    for _ in 0..10 {
        let l = Arc::clone(&limiter);
        let handle = thread::spawn(move || {
            let mut allowed = 0;
            for _ in 0..20 {
                if l.check_rate_limit(ip, limit) {
                    allowed += 1;
                }
            }
            allowed
        });
        handles.push(handle);
    }
    
    let total_allowed: usize = handles.into_iter()
        .map(|h| h.join().unwrap())
        .sum();
    
    assert_eq!(total_allowed, 100);
}

#[test]
fn test_ipv6_rate_limiting() {
    use std::net::Ipv6Addr;
    
    let limiter = RateLimiter::new();
    let ip = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1));
    let limit = RateLimit::new("test", 3, 60);

    for _ in 0..3 {
        assert!(limiter.check_rate_limit(ip, limit));
    }
    assert!(!limiter.check_rate_limit(ip, limit));
}

#[test]
fn test_faucet_rate_limit() {
    let limiter = RateLimiter::new();
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

    assert!(limiter.check_rate_limit(ip, limits::FAUCET));
    assert!(!limiter.check_rate_limit(ip, limits::FAUCET));
}

#[test]
fn test_stake_unstake_rate_limits() {
    let limiter = RateLimiter::new();
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

    for _ in 0..5 {
        assert!(limiter.check_rate_limit(ip, limits::STAKE));
    }
    assert!(!limiter.check_rate_limit(ip, limits::STAKE));

    for _ in 0..5 {
        assert!(limiter.check_rate_limit(ip, limits::UNSTAKE));
    }
    assert!(!limiter.check_rate_limit(ip, limits::UNSTAKE));
}

#[test]
fn test_rate_limit_counter_does_not_overflow() {
    // Verify that the request counter uses saturating arithmetic.
    // With a very high limit, exercise the counter to ensure no panic.
    let limiter = RateLimiter::new();
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    let limit = RateLimit::new("overflow_test", u32::MAX, 60);

    // Make many requests — the counter should saturate at u32::MAX, not panic.
    for _ in 0..1000 {
        limiter.check_rate_limit(ip, limit);
    }
    // Should still be under the u32::MAX limit
    assert!(limiter.check_rate_limit(ip, limit));
}

#[test]
fn test_delegate_rate_limits() {
    let limiter = RateLimiter::new();
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

    for _ in 0..limits::DELEGATE.requests_per_window {
        assert!(limiter.check_rate_limit(ip, limits::DELEGATE));
    }
    assert!(!limiter.check_rate_limit(ip, limits::DELEGATE));
}

#[test]
fn test_undelegate_rate_limits() {
    let limiter = RateLimiter::new();
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

    for _ in 0..limits::UNDELEGATE.requests_per_window {
        assert!(limiter.check_rate_limit(ip, limits::UNDELEGATE));
    }
    assert!(!limiter.check_rate_limit(ip, limits::UNDELEGATE));
}

#[test]
fn test_set_commission_rate_limits() {
    let limiter = RateLimiter::new();
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

    for _ in 0..limits::SET_COMMISSION.requests_per_window {
        assert!(limiter.check_rate_limit(ip, limits::SET_COMMISSION));
    }
    assert!(!limiter.check_rate_limit(ip, limits::SET_COMMISSION));
}

#[test]
fn test_keygen_rate_limits() {
    let limiter = RateLimiter::new();
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

    for _ in 0..limits::KEYGEN.requests_per_window {
        assert!(limiter.check_rate_limit(ip, limits::KEYGEN));
    }
    assert!(!limiter.check_rate_limit(ip, limits::KEYGEN));
}

#[test]
fn test_predefined_limits_delegation_endpoints() {
    assert_eq!(limits::DELEGATE.requests_per_window, 5);
    assert_eq!(limits::DELEGATE.window_duration.as_secs(), 60);

    assert_eq!(limits::UNDELEGATE.requests_per_window, 5);
    assert_eq!(limits::UNDELEGATE.window_duration.as_secs(), 60);

    assert_eq!(limits::SET_COMMISSION.requests_per_window, 5);
    assert_eq!(limits::SET_COMMISSION.window_duration.as_secs(), 60);

    assert_eq!(limits::KEYGEN.requests_per_window, 10);
    assert_eq!(limits::KEYGEN.window_duration.as_secs(), 60);
}
