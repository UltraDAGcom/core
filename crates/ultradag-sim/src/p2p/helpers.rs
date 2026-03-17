use std::sync::atomic::{AtomicU16, Ordering};

static NEXT_PORT: AtomicU16 = AtomicU16::new(21000);

/// Allocate a range of ports for a test (100 ports each).
pub fn allocate_ports() -> u16 {
    NEXT_PORT.fetch_add(100, Ordering::SeqCst)
}

/// Path to the node binary.
/// Searches from workspace root (2 levels up from crate dir).
pub fn node_binary() -> String {
    if let Ok(path) = std::env::var("ULTRADAG_NODE_BIN") {
        return path;
    }

    // Find workspace root by looking for Cargo.lock
    let mut dir = std::env::current_dir().unwrap();
    for _ in 0..5 {
        if dir.join("Cargo.lock").exists() {
            let release = dir.join("target/release/ultradag-node");
            if release.exists() {
                return release.to_string_lossy().to_string();
            }
            let debug = dir.join("target/debug/ultradag-node");
            if debug.exists() {
                return debug.to_string_lossy().to_string();
            }
        }
        dir = dir.parent().unwrap_or(&dir).to_path_buf();
    }

    // Fallback — let it fail with a clear message
    "ultradag-node".to_string()
}
