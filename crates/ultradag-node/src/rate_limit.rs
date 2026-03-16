use dashmap::DashMap;
use parking_lot::RwLock;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Rate limiter for protecting RPC endpoints from abuse
#[derive(Clone)]
pub struct RateLimiter {
    /// Per-(IP, endpoint) request tracking: (IP, endpoint) -> (request_count, window_start)
    ip_requests: Arc<DashMap<(IpAddr, &'static str), RwLock<(u32, Instant)>>>,
    /// Global connection counter
    active_connections: Arc<RwLock<u32>>,
}

/// Rate limit configuration per endpoint
#[derive(Debug, Clone, Copy)]
pub struct RateLimit {
    pub name: &'static str,
    pub requests_per_window: u32,
    pub window_duration: Duration,
}

impl RateLimit {
    pub const fn new(name: &'static str, requests: u32, seconds: u64) -> Self {
        Self {
            name,
            requests_per_window: requests,
            window_duration: Duration::from_secs(seconds),
        }
    }
}

/// Predefined rate limits for different endpoints
pub mod limits {
    use super::*;

    pub const TX: RateLimit = RateLimit::new("tx", 100, 60);           // 100 tx/min (testnet)
    pub const FAUCET: RateLimit = RateLimit::new("faucet", 1, 600);   // 1 request/10min
    pub const STAKE: RateLimit = RateLimit::new("stake", 5, 60);      // 5 stake/min
    pub const UNSTAKE: RateLimit = RateLimit::new("unstake", 5, 60);  // 5 unstake/min
    pub const PROPOSAL: RateLimit = RateLimit::new("proposal", 5, 60); // 5 proposal/min
    pub const VOTE: RateLimit = RateLimit::new("vote", 10, 60);       // 10 vote/min
    pub const DELEGATE: RateLimit = RateLimit::new("delegate", 5, 60); // 5 delegate/min
    pub const UNDELEGATE: RateLimit = RateLimit::new("undelegate", 5, 60); // 5 undelegate/min
    pub const SET_COMMISSION: RateLimit = RateLimit::new("set_commission", 5, 60); // 5 set-commission/min
    pub const KEYGEN: RateLimit = RateLimit::new("keygen", 10, 60);   // 10 keygen/min
    pub const GLOBAL: RateLimit = RateLimit::new("global", 1000, 60);  // 1000 total/min (testnet)

    pub const MAX_CONCURRENT_CONNECTIONS: u32 = 1000;
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            ip_requests: Arc::new(DashMap::new()),
            active_connections: Arc::new(RwLock::new(0)),
        }
    }

    /// Check if request is allowed under rate limit
    pub fn check_rate_limit(&self, ip: IpAddr, limit: RateLimit) -> bool {
        let now = Instant::now();
        let key = (ip, limit.name);

        // Get or create entry for this (IP, endpoint) pair
        let entry = self.ip_requests.entry(key).or_insert_with(|| RwLock::new((0, now)));
        let mut data = entry.write();
        
        // Reset window if expired
        if now.duration_since(data.1) > limit.window_duration {
            data.0 = 0;
            data.1 = now;
        }
        
        // Check if under limit
        if data.0 < limit.requests_per_window {
            data.0 += 1;
            true
        } else {
            false
        }
    }

    /// Increment active connection count
    pub fn add_connection(&self) -> Result<(), &'static str> {
        let mut count = self.active_connections.write();
        if *count >= limits::MAX_CONCURRENT_CONNECTIONS {
            return Err("max concurrent connections reached");
        }
        *count += 1;
        Ok(())
    }

    /// Decrement active connection count
    pub fn remove_connection(&self) {
        let mut count = self.active_connections.write();
        if *count > 0 {
            *count -= 1;
        }
    }

    /// Cleanup expired entries (call periodically)
    pub fn cleanup_expired(&self) {
        let now = Instant::now();
        self.ip_requests.retain(|_key, data| {
            let guard = data.read();
            now.duration_since(guard.1) < Duration::from_secs(600) // Keep for 10 min
        });
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

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

        // Use up the "global" limit
        let global = RateLimit::new("global", 2, 60);
        assert!(limiter.check_rate_limit(ip, global));
        assert!(limiter.check_rate_limit(ip, global));
        assert!(!limiter.check_rate_limit(ip, global));

        // "faucet" should still be allowed (separate bucket)
        let faucet = RateLimit::new("faucet", 1, 5);
        assert!(limiter.check_rate_limit(ip, faucet));
    }

    #[test]
    fn test_connection_limits() {
        let limiter = RateLimiter::new();
        
        assert!(limiter.add_connection().is_ok());
        limiter.remove_connection();
        
        // Connection tracking works (verified by no panic)
        assert!(limiter.add_connection().is_ok());
    }
}
