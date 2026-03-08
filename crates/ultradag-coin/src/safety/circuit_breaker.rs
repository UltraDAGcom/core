use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{error, warn};

/// Emergency circuit breaker that halts the node if a rollback is detected.
/// This is the last line of defense against state corruption.
pub struct CircuitBreaker {
    /// Last finalized round seen
    last_finalized: AtomicU64,
    /// Whether circuit breaker is enabled
    enabled: bool,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(enabled: bool) -> Self {
        Self {
            last_finalized: AtomicU64::new(0),
            enabled,
        }
    }

    /// Check if round is moving forward
    /// HALTS THE PROCESS if rollback detected
    pub fn check_finality(&self, current_round: u64) {
        if !self.enabled {
            // When disabled, still track but don't enforce
            self.last_finalized.store(current_round, Ordering::SeqCst);
            return;
        }

        let last = self.last_finalized.load(Ordering::SeqCst);

        if current_round < last {
            // CRITICAL: ROLLBACK DETECTED
            error!("╔═══════════════════════════════════════════════════════╗");
            error!("║       🚨 EMERGENCY CIRCUIT BREAKER 🚨                ║");
            error!("║       ROLLBACK DETECTED - HALTING NODE               ║");
            error!("╚═══════════════════════════════════════════════════════╝");
            error!("");
            error!("Last finalized round: {}", last);
            error!("Current round: {}", current_round);
            error!("Rollback amount: {} rounds", last - current_round);
            error!("");
            error!("This indicates a critical consensus failure.");
            error!("The node is halting to prevent state corruption.");
            error!("");
            error!("MANUAL INTERVENTION REQUIRED:");
            error!("1. Check all validator logs");
            error!("2. Verify network state with other operators");
            error!("3. Determine root cause");
            error!("4. Coordinate recovery plan");
            error!("");
            error!("DO NOT RESTART without understanding the cause.");
            error!("");
            error!("Exit code 100 = circuit breaker triggered");

            // HALT THE PROCESS
            std::process::exit(100);
        }

        // Update last finalized
        self.last_finalized.store(current_round, Ordering::SeqCst);
    }

    /// Check if round is advancing too slowly (possible stall)
    pub fn check_liveness(&self, current_round: u64, max_lag: u64) {
        if !self.enabled {
            return;
        }

        let last = self.last_finalized.load(Ordering::SeqCst);

        if last > 0 && current_round == last {
            // No progress - this is checked elsewhere
            return;
        }

        // Check for large gaps (possible network partition)
        if current_round > last + max_lag {
            warn!("⚠️  Large finality gap detected: {} rounds", current_round - last);
            warn!("Possible network partition or synchronization issue");
        }
    }

    /// Get the last finalized round
    pub fn last_finalized(&self) -> u64 {
        self.last_finalized.load(Ordering::SeqCst)
    }

    /// Check if enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_allows_forward() {
        let cb = CircuitBreaker::new(true);
        
        cb.check_finality(1);
        cb.check_finality(2);
        cb.check_finality(3);
        
        assert_eq!(cb.last_finalized(), 3);
    }

    #[test]
    fn test_circuit_breaker_allows_same() {
        let cb = CircuitBreaker::new(true);
        
        cb.check_finality(5);
        cb.check_finality(5);
        
        assert_eq!(cb.last_finalized(), 5);
    }

    #[test]
    fn test_circuit_breaker_disabled_allows_backward() {
        // When disabled, circuit breaker allows backward movement
        let cb = CircuitBreaker::new(false);
        
        cb.check_finality(10);
        cb.check_finality(5); // Would halt if enabled, but disabled so OK
        
        assert_eq!(cb.last_finalized(), 5);
    }

    #[test]
    fn test_liveness_check() {
        let cb = CircuitBreaker::new(true);
        
        cb.check_finality(100);
        cb.check_liveness(1100, 100); // 1000 round gap - should warn but not halt
        
        // Test passes if no panic
    }
    
    #[test]
    fn test_is_enabled() {
        let cb_enabled = CircuitBreaker::new(true);
        let cb_disabled = CircuitBreaker::new(false);
        
        assert!(cb_enabled.is_enabled());
        assert!(!cb_disabled.is_enabled());
    }
    
    // Note: Cannot test actual rollback halt in unit tests as it calls std::process::exit(100)
    // This must be tested in integration tests or manually
}
