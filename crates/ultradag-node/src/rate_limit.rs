use dashmap::DashMap;
use parking_lot::RwLock;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Rate limiter for protecting RPC endpoints from abuse
#[derive(Clone)]
pub struct RateLimiter {
    /// Per-IP request tracking: IP -> (request_count, window_start)
    ip_requests: Arc<DashMap<IpAddr, RwLock<(u32, Instant)>>>,
    /// Global connection counter
    active_connections: Arc<RwLock<u32>>,
    /// IP blacklist: IP -> (violation_count, ban_until)
    blacklist: Arc<DashMap<IpAddr, RwLock<(u32, Instant)>>>,
}

/// Rate limit configuration per endpoint
#[derive(Debug, Clone, Copy)]
pub struct RateLimit {
    pub requests_per_window: u32,
    pub window_duration: Duration,
}

impl RateLimit {
    pub const fn new(requests: u32, seconds: u64) -> Self {
        Self {
            requests_per_window: requests,
            window_duration: Duration::from_secs(seconds),
        }
    }
}

/// Predefined rate limits for different endpoints (AGGRESSIVE)
pub mod limits {
    use super::*;

    // Re-export RateLimit for external use
    pub use super::RateLimit;

    pub const TX: RateLimit = RateLimit::new(3, 60);            // 3 tx/min (tightened)
    pub const FAUCET: RateLimit = RateLimit::new(1, 600);       // 1 faucet/10min
    pub const STATUS: RateLimit = RateLimit::new(20, 60);       // 20 status/min (tightened)
    pub const STAKE: RateLimit = RateLimit::new(2, 60);         // 2 stake/min (tightened)
    pub const UNSTAKE: RateLimit = RateLimit::new(2, 60);       // 2 unstake/min (tightened)
    pub const GLOBAL: RateLimit = RateLimit::new(30, 60);       // 30 total/min (tightened)
    
    pub const MAX_CONCURRENT_CONNECTIONS: u32 = 500;             // Reduced from 1000
    pub const MAX_CONNECTIONS_PER_IP: u32 = 5;                   // Reduced from 10
    
    // Blacklist thresholds
    pub const BLACKLIST_THRESHOLD: u32 = 10;                     // Ban after 10 violations
    pub const BLACKLIST_DURATION_SECS: u64 = 3600;               // 1 hour ban
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            ip_requests: Arc::new(DashMap::new()),
            active_connections: Arc::new(RwLock::new(0)),
            blacklist: Arc::new(DashMap::new()),
        }
    }

    /// Check if IP is blacklisted
    pub fn is_blacklisted(&self, ip: IpAddr) -> bool {
        if let Some(entry) = self.blacklist.get(&ip) {
            let data = entry.read();
            let now = Instant::now();
            if now < data.1 {
                return true; // Still banned
            }
        }
        false
    }

    /// Record a rate limit violation and potentially blacklist the IP
    fn record_violation(&self, ip: IpAddr) {
        let now = Instant::now();
        let ban_until = now + Duration::from_secs(limits::BLACKLIST_DURATION_SECS);
        
        let entry = self.blacklist.entry(ip).or_insert_with(|| RwLock::new((0, now)));
        let mut data = entry.write();
        
        // Reset count if previous ban expired
        if now >= data.1 {
            data.0 = 0;
        }
        
        data.0 += 1;
        
        // Ban if threshold exceeded
        if data.0 >= limits::BLACKLIST_THRESHOLD {
            data.1 = ban_until;
        }
    }

    /// Check if request is allowed under rate limit
    pub fn check_rate_limit(&self, ip: IpAddr, limit: RateLimit) -> bool {
        // Check blacklist first
        if self.is_blacklisted(ip) {
            return false;
        }
        
        let now = Instant::now();
        
        // Get or create entry for this IP
        let entry = self.ip_requests.entry(ip).or_insert_with(|| RwLock::new((0, now)));
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
            // Record violation for potential blacklisting
            drop(data); // Release lock before recording violation
            self.record_violation(ip);
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

    /// Get current connection count
    pub fn connection_count(&self) -> u32 {
        *self.active_connections.read()
    }

    /// Count requests for a specific IP
    pub fn count_ip_requests(&self, ip: IpAddr) -> u32 {
        self.ip_requests
            .get(&ip)
            .map(|entry| entry.read().0)
            .unwrap_or(0)
    }

    /// Check if IP has too many connections
    pub fn check_ip_connection_limit(&self, ip: IpAddr) -> bool {
        self.count_ip_requests(ip) < limits::MAX_CONNECTIONS_PER_IP
    }

    /// Cleanup expired entries (call periodically)
    pub fn cleanup_expired(&self) {
        let now = Instant::now();
        self.ip_requests.retain(|_, data| {
            let guard = data.read();
            now.duration_since(guard.1) < Duration::from_secs(600) // Keep for 10 min
        });
        
        // Cleanup expired blacklist entries
        self.blacklist.retain(|_, data| {
            let guard = data.read();
            now < guard.1 || (now.duration_since(guard.1) < Duration::from_secs(3600))
        });
    }
    
    /// Get blacklist statistics
    pub fn blacklist_count(&self) -> usize {
        let now = Instant::now();
        self.blacklist.iter().filter(|entry| {
            let data = entry.value().read();
            now < data.1 // Currently banned
        }).count()
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
        let limit = RateLimit::new(5, 60);

        for _ in 0..5 {
            assert!(limiter.check_rate_limit(ip, limit));
        }
    }

    #[test]
    fn test_rate_limit_blocks_over_limit() {
        let limiter = RateLimiter::new();
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let limit = RateLimit::new(3, 60);

        for _ in 0..3 {
            assert!(limiter.check_rate_limit(ip, limit));
        }
        assert!(!limiter.check_rate_limit(ip, limit));
    }

    #[test]
    fn test_connection_limits() {
        let limiter = RateLimiter::new();
        
        assert!(limiter.add_connection().is_ok());
        assert_eq!(limiter.connection_count(), 1);
        
        limiter.remove_connection();
        assert_eq!(limiter.connection_count(), 0);
    }
}
