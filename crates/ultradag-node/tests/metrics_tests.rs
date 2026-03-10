use ultradag_node::CheckpointMetrics;

#[test]
fn test_checkpoint_production_metrics() {
    let metrics = CheckpointMetrics::new();
    
    metrics.record_checkpoint_produced(150, 5000, 1000);
    
    let prometheus = metrics.export_prometheus();
    assert!(prometheus.contains("checkpoint_produced_total 1"));
    assert!(prometheus.contains("checkpoint_production_duration_ms 150"));
    assert!(prometheus.contains("checkpoint_size_bytes 5000"));
    assert!(prometheus.contains("checkpoint_last_round 1000"));
}

#[test]
fn test_checkpoint_age() {
    let metrics = CheckpointMetrics::new();
    
    metrics.record_checkpoint_produced(100, 1000, 500);
    
    let json = metrics.export_json();
    let age = json["health"]["last_checkpoint_age_seconds"].as_u64().unwrap();
    
    assert!(age < 5);
}

#[test]
fn test_fast_sync_metrics() {
    let metrics = CheckpointMetrics::new();
    
    metrics.record_fast_sync_attempt();
    metrics.record_fast_sync_success(2000, 100000);
    
    let json = metrics.export_json();
    assert_eq!(json["fast_sync"]["attempts"], 1);
    assert_eq!(json["fast_sync"]["successes"], 1);
    assert_eq!(json["fast_sync"]["last_duration_ms"], 2000);
    assert_eq!(json["fast_sync"]["total_bytes_downloaded"], 100000);
}

#[test]
fn test_checkpoint_cosigning_metrics() {
    let metrics = CheckpointMetrics::new();
    
    metrics.record_checkpoint_cosigned(3);
    metrics.record_checkpoint_quorum_reached();
    
    let json = metrics.export_json();
    assert_eq!(json["checkpoint_cosigning"]["total"], 1);
    assert_eq!(json["checkpoint_cosigning"]["last_signatures"], 3);
    assert_eq!(json["checkpoint_cosigning"]["quorum_reached"], 1);
}

#[test]
fn test_checkpoint_validation_failures() {
    let metrics = CheckpointMetrics::new();
    
    metrics.record_checkpoint_validation_failure();
    metrics.record_checkpoint_validation_failure();
    
    let json = metrics.export_json();
    assert_eq!(json["checkpoint_cosigning"]["validation_failures"], 2);
}

#[test]
fn test_checkpoint_persistence_metrics() {
    let metrics = CheckpointMetrics::new();
    
    metrics.record_checkpoint_persist_success();
    metrics.record_checkpoint_persist_success();
    metrics.record_checkpoint_persist_failure();
    
    let json = metrics.export_json();
    assert_eq!(json["storage"]["persist_success"], 2);
    assert_eq!(json["storage"]["persist_failures"], 1);
}

#[test]
fn test_prometheus_export() {
    let metrics = CheckpointMetrics::new();
    
    metrics.record_checkpoint_produced(100, 2000, 500);
    metrics.record_fast_sync_success(1500, 50000);
    
    let prometheus = metrics.export_prometheus();
    
    assert!(prometheus.contains("# HELP checkpoint_produced_total"));
    assert!(prometheus.contains("# TYPE checkpoint_produced_total counter"));
    assert!(prometheus.contains("checkpoint_produced_total 1"));
    
    assert!(prometheus.contains("# HELP fast_sync_success_total"));
    assert!(prometheus.contains("fast_sync_success_total 1"));
}

#[test]
fn test_json_export() {
    let metrics = CheckpointMetrics::new();
    
    metrics.record_checkpoint_produced(200, 3000, 1000);
    
    let json = metrics.export_json();
    
    assert!(json.is_object());
    assert!(json.get("checkpoint_production").is_some());
    assert!(json["checkpoint_production"].get("total").is_some());
    assert!(json["checkpoint_production"].get("last_duration_ms").is_some());
    assert!(json["health"].get("last_checkpoint_round").is_some());
}

#[test]
fn test_pending_checkpoints_count() {
    let metrics = CheckpointMetrics::new();
    
    metrics.update_pending_checkpoints_count(3);
    
    let json = metrics.export_json();
    assert_eq!(json["health"]["pending_checkpoints"], 3);
    
    metrics.update_pending_checkpoints_count(0);
    let json = metrics.export_json();
    assert_eq!(json["health"]["pending_checkpoints"], 0);
}

#[test]
fn test_metrics_are_cumulative() {
    let metrics = CheckpointMetrics::new();
    
    metrics.record_checkpoint_produced(100, 1000, 1);
    metrics.record_checkpoint_produced(200, 2000, 2);
    metrics.record_checkpoint_produced(300, 3000, 3);
    
    let json = metrics.export_json();
    assert_eq!(json["checkpoint_production"]["total"], 3);
    assert_eq!(json["health"]["last_checkpoint_round"], 3);
}

#[test]
fn test_concurrent_metric_updates() {
    use std::sync::Arc;
    use std::thread;
    
    let metrics = Arc::new(CheckpointMetrics::new());
    let mut handles = vec![];
    
    for _ in 0..10 {
        let m = Arc::clone(&metrics);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                m.record_checkpoint_produced(100, 1000, 1);
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let json = metrics.export_json();
    assert_eq!(json["checkpoint_production"]["total"], 1000);
}
