// Re-export checkpoint metrics from ultradag-node
// This allows the network crate to use metrics without circular dependency

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Metrics for checkpoint production and synchronization
#[derive(Clone, Default)]
pub struct CheckpointMetrics {
    // Checkpoint Production
    checkpoints_produced_total: Arc<AtomicU64>,
    checkpoint_production_duration_ms: Arc<AtomicU64>,
    checkpoint_production_errors: Arc<AtomicU64>,
    checkpoint_size_bytes: Arc<AtomicU64>,
    
    // Checkpoint Co-signing
    checkpoints_cosigned_total: Arc<AtomicU64>,
    checkpoint_signatures_collected: Arc<AtomicU64>,
    checkpoint_quorum_reached_total: Arc<AtomicU64>,
    checkpoint_validation_failures: Arc<AtomicU64>,
    
    // Fast-Sync
    fast_sync_attempts_total: Arc<AtomicU64>,
    fast_sync_success_total: Arc<AtomicU64>,
    fast_sync_failures_total: Arc<AtomicU64>,
    fast_sync_duration_ms: Arc<AtomicU64>,
    fast_sync_bytes_downloaded: Arc<AtomicU64>,
    
    // Checkpoint Storage
    checkpoint_persist_success: Arc<AtomicU64>,
    checkpoint_persist_failures: Arc<AtomicU64>,
    checkpoint_load_success: Arc<AtomicU64>,
    checkpoint_load_failures: Arc<AtomicU64>,
    
    // Health Indicators
    last_checkpoint_round: Arc<AtomicU64>,
    last_checkpoint_timestamp: Arc<AtomicU64>,
    pending_checkpoints_count: Arc<AtomicU64>,
}

impl CheckpointMetrics {
    pub fn new() -> Self {
        Self::default()
    }
    
    // Checkpoint Production Metrics
    
    pub fn record_checkpoint_produced(&self, duration_ms: u64, size_bytes: u64, round: u64) {
        self.checkpoints_produced_total.fetch_add(1, Ordering::Relaxed);
        self.checkpoint_production_duration_ms.store(duration_ms, Ordering::Relaxed);
        self.checkpoint_size_bytes.store(size_bytes, Ordering::Relaxed);
        self.last_checkpoint_round.store(round, Ordering::Relaxed);
        self.last_checkpoint_timestamp.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            Ordering::Relaxed
        );
    }
    
    pub fn record_checkpoint_production_error(&self) {
        self.checkpoint_production_errors.fetch_add(1, Ordering::Relaxed);
    }
    
    // Checkpoint Co-signing Metrics
    
    pub fn record_checkpoint_cosigned(&self, signatures_count: u64) {
        self.checkpoints_cosigned_total.fetch_add(1, Ordering::Relaxed);
        self.checkpoint_signatures_collected.store(signatures_count, Ordering::Relaxed);
    }
    
    pub fn record_checkpoint_quorum_reached(&self) {
        self.checkpoint_quorum_reached_total.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_checkpoint_validation_failure(&self) {
        self.checkpoint_validation_failures.fetch_add(1, Ordering::Relaxed);
    }
    
    // Fast-Sync Metrics
    
    pub fn record_fast_sync_attempt(&self) {
        self.fast_sync_attempts_total.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_fast_sync_success(&self, duration_ms: u64, bytes_downloaded: u64) {
        self.fast_sync_success_total.fetch_add(1, Ordering::Relaxed);
        self.fast_sync_duration_ms.store(duration_ms, Ordering::Relaxed);
        self.fast_sync_bytes_downloaded.fetch_add(bytes_downloaded, Ordering::Relaxed);
    }
    
    pub fn record_fast_sync_failure(&self) {
        self.fast_sync_failures_total.fetch_add(1, Ordering::Relaxed);
    }
    
    // Storage Metrics
    
    pub fn record_checkpoint_persist_success(&self) {
        self.checkpoint_persist_success.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_checkpoint_persist_failure(&self) {
        self.checkpoint_persist_failures.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_checkpoint_load_success(&self) {
        self.checkpoint_load_success.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_checkpoint_load_failure(&self) {
        self.checkpoint_load_failures.fetch_add(1, Ordering::Relaxed);
    }
    
    // Health Indicators
    
    pub fn update_pending_checkpoints_count(&self, count: u64) {
        self.pending_checkpoints_count.store(count, Ordering::Relaxed);
    }
    
    pub fn last_checkpoint_round(&self) -> u64 {
        self.last_checkpoint_round.load(Ordering::Relaxed)
    }
    
    pub fn last_checkpoint_age_seconds(&self) -> u64 {
        let last_ts = self.last_checkpoint_timestamp.load(Ordering::Relaxed);
        if last_ts == 0 {
            return 0;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(last_ts)
    }
    
    // Prometheus-style metrics export
    
    pub fn export_prometheus(&self) -> String {
        format!(
            r#"# HELP checkpoint_produced_total Total number of checkpoints produced by this node
# TYPE checkpoint_produced_total counter
checkpoint_produced_total {}

# HELP checkpoint_production_duration_ms Duration of last checkpoint production in milliseconds
# TYPE checkpoint_production_duration_ms gauge
checkpoint_production_duration_ms {}

# HELP checkpoint_production_errors_total Total number of checkpoint production errors
# TYPE checkpoint_production_errors_total counter
checkpoint_production_errors_total {}

# HELP checkpoint_size_bytes Size of last checkpoint in bytes
# TYPE checkpoint_size_bytes gauge
checkpoint_size_bytes {}

# HELP checkpoint_cosigned_total Total number of checkpoints co-signed by this node
# TYPE checkpoint_cosigned_total counter
checkpoint_cosigned_total {}

# HELP checkpoint_signatures_collected Number of signatures in last checkpoint
# TYPE checkpoint_signatures_collected gauge
checkpoint_signatures_collected {}

# HELP checkpoint_quorum_reached_total Total number of checkpoints that reached quorum
# TYPE checkpoint_quorum_reached_total counter
checkpoint_quorum_reached_total {}

# HELP checkpoint_validation_failures_total Total number of checkpoint validation failures
# TYPE checkpoint_validation_failures_total counter
checkpoint_validation_failures_total {}

# HELP fast_sync_attempts_total Total number of fast-sync attempts
# TYPE fast_sync_attempts_total counter
fast_sync_attempts_total {}

# HELP fast_sync_success_total Total number of successful fast-syncs
# TYPE fast_sync_success_total counter
fast_sync_success_total {}

# HELP fast_sync_failures_total Total number of failed fast-syncs
# TYPE fast_sync_failures_total counter
fast_sync_failures_total {}

# HELP fast_sync_duration_ms Duration of last fast-sync in milliseconds
# TYPE fast_sync_duration_ms gauge
fast_sync_duration_ms {}

# HELP fast_sync_bytes_downloaded_total Total bytes downloaded during fast-syncs
# TYPE fast_sync_bytes_downloaded_total counter
fast_sync_bytes_downloaded_total {}

# HELP checkpoint_persist_success_total Total number of successful checkpoint persists
# TYPE checkpoint_persist_success_total counter
checkpoint_persist_success_total {}

# HELP checkpoint_persist_failures_total Total number of failed checkpoint persists
# TYPE checkpoint_persist_failures_total counter
checkpoint_persist_failures_total {}

# HELP checkpoint_load_success_total Total number of successful checkpoint loads
# TYPE checkpoint_load_success_total counter
checkpoint_load_success_total {}

# HELP checkpoint_load_failures_total Total number of failed checkpoint loads
# TYPE checkpoint_load_failures_total counter
checkpoint_load_failures_total {}

# HELP checkpoint_last_round Round number of last checkpoint
# TYPE checkpoint_last_round gauge
checkpoint_last_round {}

# HELP checkpoint_age_seconds Age of last checkpoint in seconds
# TYPE checkpoint_age_seconds gauge
checkpoint_age_seconds {}

# HELP checkpoint_pending_count Number of pending checkpoints awaiting quorum
# TYPE checkpoint_pending_count gauge
checkpoint_pending_count {}
"#,
            self.checkpoints_produced_total.load(Ordering::Relaxed),
            self.checkpoint_production_duration_ms.load(Ordering::Relaxed),
            self.checkpoint_production_errors.load(Ordering::Relaxed),
            self.checkpoint_size_bytes.load(Ordering::Relaxed),
            self.checkpoints_cosigned_total.load(Ordering::Relaxed),
            self.checkpoint_signatures_collected.load(Ordering::Relaxed),
            self.checkpoint_quorum_reached_total.load(Ordering::Relaxed),
            self.checkpoint_validation_failures.load(Ordering::Relaxed),
            self.fast_sync_attempts_total.load(Ordering::Relaxed),
            self.fast_sync_success_total.load(Ordering::Relaxed),
            self.fast_sync_failures_total.load(Ordering::Relaxed),
            self.fast_sync_duration_ms.load(Ordering::Relaxed),
            self.fast_sync_bytes_downloaded.load(Ordering::Relaxed),
            self.checkpoint_persist_success.load(Ordering::Relaxed),
            self.checkpoint_persist_failures.load(Ordering::Relaxed),
            self.checkpoint_load_success.load(Ordering::Relaxed),
            self.checkpoint_load_failures.load(Ordering::Relaxed),
            self.last_checkpoint_round.load(Ordering::Relaxed),
            self.last_checkpoint_age_seconds(),
            self.pending_checkpoints_count.load(Ordering::Relaxed),
        )
    }
    
    // JSON export for HTTP API
    
    pub fn export_json(&self) -> serde_json::Value {
        serde_json::json!({
            "checkpoint_production": {
                "total": self.checkpoints_produced_total.load(Ordering::Relaxed),
                "last_duration_ms": self.checkpoint_production_duration_ms.load(Ordering::Relaxed),
                "errors": self.checkpoint_production_errors.load(Ordering::Relaxed),
                "last_size_bytes": self.checkpoint_size_bytes.load(Ordering::Relaxed),
            },
            "checkpoint_cosigning": {
                "total": self.checkpoints_cosigned_total.load(Ordering::Relaxed),
                "last_signatures": self.checkpoint_signatures_collected.load(Ordering::Relaxed),
                "quorum_reached": self.checkpoint_quorum_reached_total.load(Ordering::Relaxed),
                "validation_failures": self.checkpoint_validation_failures.load(Ordering::Relaxed),
            },
            "fast_sync": {
                "attempts": self.fast_sync_attempts_total.load(Ordering::Relaxed),
                "successes": self.fast_sync_success_total.load(Ordering::Relaxed),
                "failures": self.fast_sync_failures_total.load(Ordering::Relaxed),
                "last_duration_ms": self.fast_sync_duration_ms.load(Ordering::Relaxed),
                "total_bytes_downloaded": self.fast_sync_bytes_downloaded.load(Ordering::Relaxed),
            },
            "storage": {
                "persist_success": self.checkpoint_persist_success.load(Ordering::Relaxed),
                "persist_failures": self.checkpoint_persist_failures.load(Ordering::Relaxed),
                "load_success": self.checkpoint_load_success.load(Ordering::Relaxed),
                "load_failures": self.checkpoint_load_failures.load(Ordering::Relaxed),
            },
            "health": {
                "last_checkpoint_round": self.last_checkpoint_round.load(Ordering::Relaxed),
                "last_checkpoint_age_seconds": self.last_checkpoint_age_seconds(),
                "pending_checkpoints": self.pending_checkpoints_count.load(Ordering::Relaxed),
            }
        })
    }
}
