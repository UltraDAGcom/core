use std::sync::Arc;
use parking_lot::RwLock;
use std::time::{Duration, Instant};

/// Resource monitoring and auto-throttling
#[derive(Clone)]
pub struct ResourceMonitor {
    /// Current load level (0.0 = idle, 1.0 = max)
    load_level: Arc<RwLock<f32>>,
    /// Last update time
    last_update: Arc<RwLock<Instant>>,
    /// Request count in last window
    request_count: Arc<RwLock<u32>>,
}

/// Load thresholds for auto-throttling
pub mod thresholds {
    pub const NORMAL_LOAD: f32 = 0.5;      // Below this: normal operation
    pub const HIGH_LOAD: f32 = 0.75;       // Above this: start throttling
    pub const CRITICAL_LOAD: f32 = 0.9;    // Above this: aggressive throttling
    
    pub const REQUESTS_PER_SECOND_NORMAL: u32 = 100;
    pub const REQUESTS_PER_SECOND_HIGH: u32 = 50;
    pub const REQUESTS_PER_SECOND_CRITICAL: u32 = 10;
}

impl ResourceMonitor {
    pub fn new() -> Self {
        Self {
            load_level: Arc::new(RwLock::new(0.0)),
            last_update: Arc::new(RwLock::new(Instant::now())),
            request_count: Arc::new(RwLock::new(0)),
        }
    }

    /// Record a request and update load metrics
    pub fn record_request(&self) {
        let mut count = self.request_count.write();
        *count += 1;
        
        // Update load level every second
        let now = Instant::now();
        let mut last = self.last_update.write();
        
        if now.duration_since(*last) >= Duration::from_secs(1) {
            let requests_per_sec = *count;
            *count = 0;
            *last = now;
            
            // Calculate load level based on request rate
            let load = if requests_per_sec <= thresholds::REQUESTS_PER_SECOND_NORMAL {
                (requests_per_sec as f32) / (thresholds::REQUESTS_PER_SECOND_NORMAL as f32)
            } else if requests_per_sec <= thresholds::REQUESTS_PER_SECOND_HIGH {
                let delta = requests_per_sec.saturating_sub(thresholds::REQUESTS_PER_SECOND_NORMAL);
                let range = thresholds::REQUESTS_PER_SECOND_HIGH.saturating_sub(thresholds::REQUESTS_PER_SECOND_NORMAL);
                thresholds::NORMAL_LOAD + 
                    (delta as f32) / (range as f32) * 
                    (thresholds::HIGH_LOAD - thresholds::NORMAL_LOAD)
            } else {
                let delta = requests_per_sec.saturating_sub(thresholds::REQUESTS_PER_SECOND_HIGH);
                let range = thresholds::REQUESTS_PER_SECOND_CRITICAL.saturating_sub(thresholds::REQUESTS_PER_SECOND_HIGH);
                thresholds::HIGH_LOAD + 
                    (delta as f32) / (range as f32) * 
                    (thresholds::CRITICAL_LOAD - thresholds::HIGH_LOAD)
            };
            
            let mut load_level = self.load_level.write();
            *load_level = load.min(1.0);
        }
    }

    /// Get current load level
    pub fn get_load(&self) -> f32 {
        *self.load_level.read()
    }

    /// Check if system is under high load
    pub fn is_high_load(&self) -> bool {
        self.get_load() >= thresholds::HIGH_LOAD
    }

    /// Check if system is under critical load
    pub fn is_critical_load(&self) -> bool {
        self.get_load() >= thresholds::CRITICAL_LOAD
    }

    /// Get throttle delay based on current load
    pub fn get_throttle_delay(&self) -> Option<Duration> {
        let load = self.get_load();
        
        if load >= thresholds::CRITICAL_LOAD {
            Some(Duration::from_millis(100)) // 100ms delay under critical load
        } else if load >= thresholds::HIGH_LOAD {
            Some(Duration::from_millis(50))  // 50ms delay under high load
        } else {
            None // No throttling under normal load
        }
    }
}

impl Default for ResourceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_calculation() {
        let monitor = ResourceMonitor::new();
        
        // Simulate normal load
        for _ in 0..50 {
            monitor.record_request();
        }
        
        std::thread::sleep(Duration::from_millis(1100));
        monitor.record_request();
        
        let load = monitor.get_load();
        assert!(load < thresholds::NORMAL_LOAD);
    }

    #[test]
    fn test_throttle_delay() {
        let monitor = ResourceMonitor::new();
        
        // Normal load - no throttling
        assert!(monitor.get_throttle_delay().is_none());
        
        // Set high load
        *monitor.load_level.write() = thresholds::HIGH_LOAD;
        assert!(monitor.get_throttle_delay().is_some());
        
        // Set critical load
        *monitor.load_level.write() = thresholds::CRITICAL_LOAD;
        let delay = monitor.get_throttle_delay().unwrap();
        assert!(delay >= Duration::from_millis(100));
    }
}
